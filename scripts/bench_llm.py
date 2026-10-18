"""Spikes 4-5: benchmark candidate models under llama-server (CPU) with JSON-schema output.

    python -I scripts/bench_llm.py --server <llama-server.exe> --model <m.gguf> [--mmproj <p.gguf>]
           [--threads 4] [--images a.png b.png]

For each model: load time, per-case latency and tokens/s, JSON/schema validity, category
and deadline accuracy against scripts/bench_cases.json, and peak RAM of the server process.
Standard library only.
"""

import argparse
import base64
import ctypes
import ctypes.wintypes as wt
import json
import pathlib
import subprocess
import sys
import time
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
PORT = 18765


class PROCESS_MEMORY_COUNTERS_EX(ctypes.Structure):
    _fields_ = [
        ("cb", wt.DWORD), ("PageFaultCount", wt.DWORD),
        ("PeakWorkingSetSize", ctypes.c_size_t), ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t), ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t), ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t), ("PeakPagefileUsage", ctypes.c_size_t),
        ("PrivateUsage", ctypes.c_size_t),
    ]


def peak_memory_mb(pid):
    h = ctypes.windll.kernel32.OpenProcess(0x0410, False, pid)  # QUERY_INFORMATION | VM_READ
    c = PROCESS_MEMORY_COUNTERS_EX()
    c.cb = ctypes.sizeof(c)
    ctypes.windll.psapi.GetProcessMemoryInfo(h, ctypes.byref(c), c.cb)
    ctypes.windll.kernel32.CloseHandle(h)
    return c.PeakWorkingSetSize / 2**20, c.PeakPagefileUsage / 2**20


def post(path, body, timeout=900):
    req = urllib.request.Request(f"http://127.0.0.1:{PORT}{path}", json.dumps(body).encode(),
                                 {"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.load(r)


def wait_healthy(proc, timeout=300):
    start = time.time()
    while time.time() - start < timeout:
        if proc.poll() is not None:
            raise RuntimeError("llama-server exited during load")
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{PORT}/health", timeout=2) as r:
                if r.status == 200:
                    return time.time() - start
        except Exception:
            pass
        time.sleep(0.25)
    raise TimeoutError("server did not become healthy")


def build_prompt(cfg):
    system = (ROOT / "prompts/categorize.v1.md").read_text(encoding="utf-8")
    system = (system.replace("{{locale}}", cfg["locale"]).replace("{{today}}", cfg["today"])
              .replace("{{timezone}}", cfg["timezone"]))
    schema_txt = (ROOT / "prompts/categorize.v1.schema.json").read_text(encoding="utf-8")
    names = [c["name"] for c in cfg["categories"]]
    schema = json.loads(schema_txt.replace('["{{categories}}"]', json.dumps(names)))
    cat_lines = "\n".join(f"- {c['name']}" + (f": {c['hint']}" if c["hint"] else "") for c in cfg["categories"])
    return system, schema, cat_lines


def user_message(case, cat_lines):
    return (f"Categories:\n{cat_lines}\n\nNote: {case['note'] or '(none)'}\n\n"
            f"Screenshot text:\n\"\"\"\n{case['ocr'] or '(no text found)'}\n\"\"\"")


def chat(system, schema, content):
    body = {
        "messages": [{"role": "system", "content": system}, {"role": "user", "content": content}],
        "temperature": 0.2,
        "max_tokens": 400,
        "response_format": {"type": "json_schema", "json_schema": {"name": "screenshot", "schema": schema}},
        # Qwen3.x and similar default to a thinking phase; we want the answer directly.
        "chat_template_kwargs": {"enable_thinking": False},
    }
    t = time.time()
    r = post("/v1/chat/completions", body)
    return r, time.time() - t


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--server", required=True)
    ap.add_argument("--model", required=True)
    ap.add_argument("--mmproj")
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--ctx", type=int, default=4096)
    ap.add_argument("--images", nargs="*", default=[])
    ap.add_argument("--skip-text", action="store_true")
    args = ap.parse_args()

    cfg = json.loads((ROOT / "scripts/bench_cases.json").read_text(encoding="utf-8"))
    system, schema, cat_lines = build_prompt(cfg)

    cmd = [args.server, "-m", args.model, "--port", str(PORT), "--host", "127.0.0.1",
           "-t", str(args.threads), "-c", str(args.ctx), "-ngl", "0", "--no-webui", "-np", "1"]
    if args.mmproj:
        cmd += ["--mmproj", args.mmproj]
    # BELOW_NORMAL_PRIORITY_CLASS, as the app will run it.
    proc = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, creationflags=0x4000)
    name = pathlib.Path(args.model).name
    report = {"model": name, "threads": args.threads, "cases": []}
    try:
        report["load_s"] = round(wait_healthy(proc), 1)
        print(f"\n### {name}  (threads={args.threads})  load {report['load_s']} s", flush=True)

        if not args.skip_text:
            for case in cfg["cases"]:
                r, secs = chat(system, schema, user_message(case, cat_lines))
                raw = r["choices"][0]["message"]["content"]
                tm = r.get("timings", {})
                row = {"id": case["id"], "secs": round(secs, 1),
                       "prompt_tps": round(tm.get("prompt_per_second", 0), 1),
                       "gen_tps": round(tm.get("predicted_per_second", 0), 1)}
                try:
                    out = json.loads(raw)
                    exp = case["expect"]
                    dl = out.get("deadline") or {}
                    row.update(valid=True, cat_ok=out["category"] == exp["category"],
                               intent_ok=out["note_has_action_intent"] == exp["intent"],
                               deadline_ok=dl.get("iso") == exp["deadline_iso"],
                               category=out["category"], deadline=dl.get("iso"), title=out["title"],
                               summary=out["summary"])
                except Exception as e:  # noqa: BLE001 - report any parse failure
                    row.update(valid=False, error=str(e), raw=raw[:200])
                report["cases"].append(row)
                flag = lambda k: "ok" if row.get(k) else "XX"  # noqa: E731
                print(f"  {case['id']:<22} {row['secs']:>5}s  gen {row['gen_tps']:>5} t/s  "
                      f"json {flag('valid')} cat {flag('cat_ok')} intent {flag('intent_ok')} "
                      f"deadline {flag('deadline_ok')}  | {row.get('category')} | {row.get('deadline')} | "
                      f"{row.get('title')}", flush=True)

        for img in args.images:
            b64 = base64.b64encode(pathlib.Path(img).read_bytes()).decode()
            content = [
                {"type": "text", "text": user_message({"note": "save this for my design moodboard", "ocr": ""}, cat_lines)
                 + "\n\nThe screenshot is attached; describe what it shows."},
                {"type": "image_url", "image_url": {"url": f"data:image/png;base64,{b64}"}},
            ]
            r, secs = chat(system, schema, content)
            out = r["choices"][0]["message"]["content"]
            tm = r.get("timings", {})
            report.setdefault("images", []).append({"image": img, "secs": round(secs, 1), "out": out})
            print(f"  IMAGE {pathlib.Path(img).name}: {secs:.1f}s (prompt {tm.get('prompt_n')} tok @ "
                  f"{tm.get('prompt_per_second', 0):.1f} t/s)\n    {out}", flush=True)

        ws, private = peak_memory_mb(proc.pid)
        report["peak_ws_mb"], report["peak_private_mb"] = round(ws), round(private)
        rows = [c for c in report["cases"]]
        if rows:
            n = len(rows)
            summary = {k: sum(1 for c in rows if c.get(k)) for k in ("valid", "cat_ok", "intent_ok", "deadline_ok")}
            report["summary"] = summary
            report["median_secs"] = sorted(c["secs"] for c in rows)[n // 2]
            print(f"  => json {summary['valid']}/{n}  category {summary['cat_ok']}/{n}  intent {summary['intent_ok']}/{n}"
                  f"  deadline {summary['deadline_ok']}/{n}  median {report['median_secs']}s", flush=True)
        print(f"  => peak RAM: working set {report['peak_ws_mb']} MB, private {report['peak_private_mb']} MB", flush=True)
    finally:
        proc.kill()
        proc.wait()
    out_dir = ROOT / ".spikes"
    out_dir.mkdir(exist_ok=True)
    (out_dir / f"bench-{name}-t{args.threads}.json").write_text(json.dumps(report, indent=2), encoding="utf-8")


if __name__ == "__main__":
    sys.exit(main())
