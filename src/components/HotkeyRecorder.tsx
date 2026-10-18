import { useEffect, useState } from "react";
import { formatHotkey, ipc, type HotkeyStatus } from "../lib/ipc";
import { Button } from "./ui";

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
  "OSLeft",
  "OSRight",
]);

/** Turns a key press into a Tauri accelerator ("Ctrl+Shift+J"), or null if incomplete. */
export function toAccelerator(e: Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">): string | null {
  if (MODIFIER_CODES.has(e.code)) return null;
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean);
  // Shift alone would steal ordinary typing (Shift+J is just "J").
  if (mods.length === 0 || (mods.length === 1 && e.shiftKey)) return null;
  const key = e.code.startsWith("Key") ? e.code.slice(3) : e.code.startsWith("Digit") ? e.code.slice(5) : e.code;
  return [...mods, key].join("+");
}

/** Shows the capture shortcut and lets the user record a new one. */
export function HotkeyRecorder({
  current,
  status,
  onChanged,
}: {
  current: string;
  status: HotkeyStatus | undefined;
  onChanged: () => void;
}) {
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.code === "Escape") return setRecording(false);
      const acc = toAccelerator(e);
      if (!acc) return;
      setRecording(false);
      ipc
        .updateSettings({ hotkey: acc })
        .then(() => {
          setError(null);
          onChanged();
        })
        .catch((err) => setError(String(err)));
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording, onChanged]);

  return (
    <div className="flex flex-col gap-1.5 py-2">
      <div className="flex items-center justify-between gap-4">
        <span className="text-sm">Capture shortcut</span>
        <div className="flex items-center gap-2">
          <kbd className="rounded-[var(--radius-input)] border border-line px-2 py-1 font-mono text-xs">
            {recording ? "Press keys…" : formatHotkey(current)}
          </kbd>
          <Button onClick={() => setRecording((r) => !r)}>{recording ? "Cancel" : "Change"}</Button>
        </div>
      </div>
      {recording && <p className="text-xs text-ink-muted">Press the new combination, including Ctrl, Alt or Win. Esc cancels.</p>}
      {error && (
        <p role="alert" className="text-xs text-signature-coral dark:text-signature-peach">
          {error}
        </p>
      )}
      {!error && status?.kind === "usingFallback" && (
        <p className="text-xs text-body">
          {formatHotkey(status.wanted)} is taken ({status.reason}). {formatHotkey(status.accelerator)} works instead for now.{" "}
          <button
            type="button"
            className="underline"
            onClick={() => void ipc.updateSettings({ hotkey: status.accelerator }).then(onChanged, (e) => setError(String(e)))}
          >
            Use {formatHotkey(status.accelerator)} permanently
          </button>
        </p>
      )}
      {!error && status?.kind === "failed" && (
        <p className="text-xs text-signature-coral dark:text-signature-peach">
          {formatHotkey(status.wanted)} couldn't be set up ({status.reason}). Pick another, or capture from the tray icon.
        </p>
      )}
    </div>
  );
}
