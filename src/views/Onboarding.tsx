import { useEffect, useState } from "react";
import { Check, CloudOff, Cpu, Download, Gift, Lock, Pause } from "lucide-react";
import { formatHotkey, ipc, type AiProfiles, type Settings } from "../lib/ipc";
import { LIBRARY_CHANGED, useTauriEvent } from "../lib/events";
import { formatBytes } from "../lib/format";
import { Button } from "../components/ui";
import { useAiStatus } from "../components/AiSetup";
import { Kbd } from "../components/Kbd";
import { useToast } from "../components/Toast";
import { CategoryReview, ProfessionPicker, mergePresets, toInputs, usePresets, type Draft } from "../components/ProfessionSetup";

const WELCOME = 0;
const PROFESSION = 1;
const REVIEW = 2;
const AI = 3;
const TRY = 4;
const STEPS = 5;

/** First run, a few short steps. Capturing works from the very first moment. */
export function Onboarding({ hotkey, settings, onDone }: { hotkey: string | null; settings: Settings; onDone: () => void }) {
  const [step, setStep] = useState(WELCOME);
  const [captured, setCaptured] = useState(false);
  useTauriEvent(LIBRARY_CHANGED, () => step === TRY && setCaptured(true));

  const presets = usePresets();
  const [selected, setSelected] = useState<string[]>([]);
  const [custom, setCustom] = useState(false);
  // The review list, and the choice it was built from (edits survive going back unchanged).
  const [drafts, setDrafts] = useState<Draft[]>([]);
  const [draftsFrom, setDraftsFrom] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const key = formatHotkey(hotkey ?? settings.hotkey);

  function review() {
    const from = custom ? "custom" : selected.join(",");
    if (from !== draftsFrom) {
      setDrafts(custom ? [] : mergePresets(presets, selected));
      setDraftsFrom(from);
    }
    setError(null);
    setStep(REVIEW);
  }

  async function saveCategories() {
    setSaving(true);
    setError(null);
    try {
      await ipc.applyCategorySetup(toInputs(drafts), true);
      setStep(AI);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex min-h-full justify-center bg-canvas px-8 pt-10 pb-12">
      <div className={`flex w-full flex-col gap-8 ${step === PROFESSION ? "max-w-3xl" : "max-w-xl"}`}>
        <ol className="flex gap-1.5" aria-label={`Step ${step + 1} of ${STEPS}`}>
          {Array.from({ length: STEPS }, (_, i) => i).map((i) => (
            <li key={i} className={`h-1 flex-1 rounded-full ${i <= step ? "bg-primary" : "bg-line"}`} />
          ))}
        </ol>

        {step === WELCOME ? (
          <>
            <div>
              <h1 className="type-display text-[40px] leading-[1.2]">Welcome to ClipNest</h1>
              <p className="mt-3 text-base leading-[1.5] text-body">
                Keep anything you see on screen and find it again in seconds. ClipNest saves your screenshots, sorts them for you and
                reminds you about deadlines.
              </p>
            </div>
            <ol className="flex flex-col gap-5 rounded-[var(--radius-action)] border border-line bg-surface p-6">
              <HowStep
                n={1}
                title={
                  <>
                    Press <Kbd>{key}</Kbd> anywhere
                  </>
                }
              >
                The snipping tool opens over whatever you're looking at: a web page, a chat, a PDF, a video.
              </HowStep>
              <HowStep n={2} title="Drag around what you want to keep">
                Type a short note, like “apply by Friday” or “font for the website”, then press Enter.
              </HowStep>
              <HowStep n={3} title="ClipNest files it for you">
                A small AI on your computer reads it, gives it a title and puts it in the right categories.
              </HowStep>
              <HowStep n={4} title="Find it when you need it">
                Filter by category or tag, or search any word that was on screen. Dates in your notes become reminders.
              </HowStep>
            </ol>
            <ul className="grid grid-cols-1 gap-2 text-[13px] text-body sm:grid-cols-3">
              <Perk icon={<Lock size={14} />} text="Private: nothing leaves your computer" />
              <Perk icon={<Gift size={14} />} text="Free, no account needed" />
              <Perk icon={<CloudOff size={14} />} text="Works offline after a one-time AI download" />
            </ul>
            <Button variant="primary" onClick={() => setStep(PROFESSION)}>
              Get started
            </Button>
          </>
        ) : step === PROFESSION ? (
          <>
            <div>
              <h1 className="type-display text-[40px] leading-[1.2]">What do you do?</h1>
              <p className="mt-3 text-base leading-[1.5] text-body">
                We'll start you with categories that fit your work. Pick as many as apply; you can change everything later.
              </p>
            </div>
            <ProfessionPicker
              wide
              presets={presets}
              selected={selected}
              custom={custom}
              onChange={(s, c) => {
                setSelected(s);
                setCustom(c);
              }}
            />
            <div className="flex justify-between">
              <Button variant="ghost" onClick={() => setStep(WELCOME)}>
                Back
              </Button>
              <Button variant="primary" disabled={!selected.length && !custom} onClick={review}>
                Next
              </Button>
            </div>
          </>
        ) : step === REVIEW ? (
          <>
            <div>
              <h1 className="type-display text-[40px] leading-[1.2]">Your categories</h1>
              <p className="mt-3 text-base leading-[1.5] text-body">
                {custom
                  ? "Add the categories you want. The AI files each screenshot into up to three of them."
                  : "Untick any you don't need, rename them, or add your own. The AI files each screenshot into up to three of them."}
              </p>
            </div>
            <CategoryReview drafts={drafts} onChange={setDrafts} />
            {error && (
              <p role="alert" className="text-sm text-signature-coral dark:text-signature-peach">
                {error}
              </p>
            )}
            <div className="flex justify-between">
              <Button variant="ghost" onClick={() => setStep(PROFESSION)}>
                Back
              </Button>
              <Button variant="primary" disabled={saving} onClick={() => void saveCategories()}>
                {toInputs(drafts).length ? "Save and continue" : "Continue with Miscellaneous only"}
              </Button>
            </div>
          </>
        ) : step === AI ? (
          <AiStep settings={settings} onBack={() => setStep(REVIEW)} onNext={() => setStep(TRY)} />
        ) : (
          <>
            <div>
              <h1 className="type-display text-[40px] leading-[1.2]">Try it now</h1>
              {hotkey ? (
                <p className="mt-3 text-base leading-[1.5] text-body">
                  Press <Kbd>{formatHotkey(hotkey)}</Kbd>, drag around anything on screen, type a note, and press Enter.
                </p>
              ) : (
                <p className="mt-3 text-base leading-[1.5] text-body">
                  The capture shortcut couldn't be set up. Right-click the tray icon and choose “Capture now”, or pick a different shortcut
                  in Settings.
                </p>
              )}
            </div>
            {captured ? (
              <p className="flex items-center gap-2 rounded-[var(--radius-card)] bg-signature-mint p-4 text-sm text-[#181d26]">
                <Check size={16} /> Saved! ClipNest is filing it now.
              </p>
            ) : (
              <p className="rounded-[var(--radius-card)] bg-surface p-4 text-center text-sm text-ink-muted">
                Waiting for your first capture…
              </p>
            )}
            <div className="flex justify-between">
              <Button variant="ghost" onClick={() => setStep(AI)}>
                Back
              </Button>
              <Button variant="primary" onClick={onDone}>
                Start using ClipNest
              </Button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function HowStep({ n, title, children }: { n: number; title: React.ReactNode; children: React.ReactNode }) {
  return (
    <li className="flex gap-4">
      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-primary text-[13px] font-medium text-on-primary">
        {n}
      </span>
      <span className="flex flex-col gap-1">
        <span className="text-[15px] font-medium text-ink">{title}</span>
        <span className="text-sm leading-[1.45] text-body">{children}</span>
      </span>
    </li>
  );
}

function Perk({ icon, text }: { icon: React.ReactNode; text: string }) {
  return (
    <li className="flex items-center gap-2 rounded-[var(--radius-input)] px-1 py-1">
      <span className="text-ink">{icon}</span>
      {text}
    </li>
  );
}

const PROFILE_COPY: Record<string, { title: string; text: string }> = {
  standard: { title: "Standard", text: "Fast, and light on memory. Good titles for everyday screenshots." },
  accurate: { title: "More accurate", text: "Better titles and filing, a little slower. Best with 16 GB of memory or more." },
};

/**
 * Pick a model (recommended from this computer's memory) and download it. The next step
 * unlocks only once the model is ready.
 */
function AiStep({ settings, onBack, onNext }: { settings: Settings; onBack: () => void; onNext: () => void }) {
  const toast = useToast();
  const status = useAiStatus();
  const [profiles, setProfiles] = useState<AiProfiles | null>(null);
  const [busy, setBusy] = useState(false);
  const fail = (e: unknown) => toast({ message: String(e), tone: "error" });

  useEffect(() => {
    ipc.aiProfiles().then((p) => {
      setProfiles(p);
      // Start on the recommendation, unless a download was already chosen.
      if (!settings.aiDownloadConsent && settings.aiProfile !== p.recommended) {
        void ipc.updateSettings({ aiProfile: p.recommended }).catch(() => {});
      }
    }, fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const state = status?.state ?? "notInstalled";
  const locked = state === "downloading" || state === "ready";
  const ready = state === "ready";
  const pct = status && status.totalBytes > 0 ? Math.floor((status.downloadedBytes / status.totalBytes) * 100) : 0;
  const ram = profiles?.totalRamGb ?? 0;

  async function choose(id: string) {
    if (locked || id === settings.aiProfile) return;
    await ipc.updateSettings({ aiProfile: id }).catch(fail);
  }

  async function download() {
    setBusy(true);
    try {
      await ipc.aiStartSetup();
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <div>
        <h1 className="type-display text-[40px] leading-[1.2]">Set up the AI</h1>
        <p className="mt-3 text-base leading-[1.5] text-body">
          ClipNest's AI runs on your computer, so your screenshots never leave it. Pick a model and download it once; this is the only time
          ClipNest uses the internet.
        </p>
      </div>

      <p className="flex items-center gap-2.5 rounded-[var(--radius-card)] bg-surface px-4 py-3 text-sm text-ink">
        <Cpu size={16} aria-hidden />
        {ram > 0 ? (
          <span>
            This computer has <span className="font-medium">{ram} GB</span> of memory (RAM).
          </span>
        ) : (
          <span>ClipNest couldn't read how much memory this computer has. Standard works on most laptops.</span>
        )}
      </p>

      <div role="radiogroup" aria-label="AI model" className="flex flex-col gap-3">
        {profiles?.profiles.map((p) => {
          const copy = PROFILE_COPY[p.id] ?? { title: p.modelName, text: "" };
          const on = settings.aiProfile === p.id;
          const tight = ram > 0 && ram < p.minRamGb;
          return (
            <button
              key={p.id}
              type="button"
              role="radio"
              aria-checked={on}
              disabled={locked && !on}
              onClick={() => void choose(p.id)}
              className={`flex items-start gap-3 rounded-[var(--radius-card)] border p-4 text-left transition-colors duration-100 disabled:opacity-45 ${
                on ? "border-primary bg-surface-raised ring-1 ring-primary" : "border-line bg-surface-raised"
              }`}
            >
              <span
                className={`mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border ${on ? "border-primary" : "border-line-strong"}`}
                aria-hidden
              >
                {on && <span className="h-2 w-2 rounded-full bg-primary" />}
              </span>
              <span className="flex min-w-0 flex-1 flex-col gap-1">
                <span className="flex flex-wrap items-center gap-2">
                  <span className="text-[15px] font-medium text-ink">{copy.title}</span>
                  {profiles.recommended === p.id && (
                    <span className="rounded-full bg-signature-mint px-2 py-0.5 text-xs font-medium text-[#181d26]">
                      Recommended for your computer
                    </span>
                  )}
                </span>
                <span className="text-sm leading-[1.45] text-body">{copy.text}</span>
                <span className="text-xs text-ink-muted">
                  {p.modelName} · {formatBytes(p.downloadBytes)} download · for {p.minRamGb} GB RAM or more
                </span>
                {tight && (
                  <span className="text-xs text-signature-coral dark:text-signature-peach">
                    This computer has less memory than this model wants, so it may be slow.
                  </span>
                )}
              </span>
            </button>
          );
        })}
      </div>

      {status && (
        <div className="flex flex-col gap-3">
          {(state === "downloading" || state === "paused" || (state === "error" && status.downloadedBytes > 0)) && (
            <div className="flex flex-col gap-1.5">
              <div className="flex justify-between text-[13px] text-body">
                <span>{state === "paused" ? "Download paused" : `Downloading ${status.modelName}…`}</span>
                <span className="tabular-nums">
                  {pct}% · {formatBytes(Math.max(status.totalBytes - status.downloadedBytes, 0))} left
                </span>
              </div>
              <div
                className="h-1.5 overflow-hidden rounded-full bg-line"
                role="progressbar"
                aria-valuenow={pct}
                aria-valuemin={0}
                aria-valuemax={100}
              >
                <div className="h-full rounded-full bg-primary transition-[width] duration-300" style={{ width: `${pct}%` }} />
              </div>
            </div>
          )}
          {ready && (
            <p className="flex items-center gap-2 rounded-[var(--radius-card)] bg-signature-mint p-4 text-sm text-[#181d26]">
              <Check size={16} /> {status.modelName} is ready. It runs on this computer.
            </p>
          )}
          {status.error && (
            <p role="alert" className="text-[13px] text-signature-coral dark:text-signature-peach">
              {status.error}
            </p>
          )}
        </div>
      )}

      <div className="flex items-center justify-between gap-3">
        <Button variant="ghost" onClick={onBack} disabled={state === "downloading"}>
          Back
        </Button>
        <div className="flex items-center gap-2">
          {state === "downloading" ? (
            <Button onClick={() => void ipc.aiPauseDownload().catch(fail)}>
              <Pause size={14} /> Pause
            </Button>
          ) : !ready ? (
            <Button variant="primary" disabled={busy} onClick={() => void download()}>
              <Download size={14} />
              {state === "notInstalled" ? `Download (${formatBytes(status?.totalBytes ?? 0)})` : "Resume download"}
            </Button>
          ) : null}
          <Button
            variant={ready ? "primary" : "secondary"}
            disabled={!ready}
            onClick={onNext}
            title={ready ? undefined : "Available once the download finishes"}
          >
            Continue
          </Button>
        </div>
      </div>
      {!ready && (
        <p className="-mt-4 text-right text-xs text-ink-muted">
          Continue unlocks when the download finishes. You can keep using your computer meanwhile.
        </p>
      )}
    </>
  );
}
