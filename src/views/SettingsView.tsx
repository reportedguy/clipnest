import { useEffect, useState, type ReactNode } from "react";
import { formatBytes } from "../lib/format";
import { ipc, type AppInfo, type Category, type Settings, type StorageInfo, type ThemePref } from "../lib/ipc";
import { CategoryManager } from "../components/CategoryManager";
import { AiSetup } from "../components/AiSetup";
import { useOfferRecheck } from "../components/NewCategoryDialog";
import { AddFromProfessionDialog } from "../components/ProfessionSetup";
import { HotkeyRecorder } from "../components/HotkeyRecorder";
import { Button, Switch, inputClass } from "../components/ui";
import { useToast } from "../components/Toast";

interface Props {
  settings: Settings;
  info: AppInfo | null;
  categories: Category[];
  onInfoStale: () => void;
}

const LEAD_OPTIONS = [
  { hours: 1, label: "1 hour before" },
  { hours: 3, label: "3 hours before" },
  { hours: 12, label: "12 hours before" },
  { hours: 24, label: "1 day before" },
  { hours: 48, label: "2 days before" },
  { hours: 168, label: "1 week before" },
];

export function SettingsView({ settings, info, categories, onInfoStale }: Props) {
  const toast = useToast();
  const [storage, setStorage] = useState<StorageInfo | null>(null);
  const offerRecheck = useOfferRecheck();
  const [fromProfession, setFromProfession] = useState(false);

  // Refresh the storage numbers whenever the library changes (categories reload then).
  useEffect(() => {
    ipc.storageInfo().then(setStorage, () => {});
  }, [categories]);

  const update = (patch: Partial<Settings>) => void ipc.updateSettings(patch).catch((e) => toast({ message: String(e), tone: "error" }));

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-6">
      <Section title="Categories" description="A screenshot can be in up to three categories. Hints help the AI choose.">
        <CategoryManager categories={categories} onCreated={offerRecheck} />
        <div className="mt-3">
          <Button size="sm" onClick={() => setFromProfession(true)}>
            Add categories from a profession…
          </Button>
        </div>
        <AddFromProfessionDialog open={fromProfession} onClose={() => setFromProfession(false)} categories={categories} />
      </Section>

      <Section title="Capture">
        <HotkeyRecorder current={settings.hotkey} status={info?.hotkey} onChanged={onInfoStale} />
        <Switch label="Start with Windows" checked={settings.startWithWindows} onChange={(v) => update({ startWithWindows: v })} />
        <Switch
          label="Keep running in the tray when closed"
          description="So the shortcut keeps working after you close this window."
          checked={settings.closeToTray}
          onChange={(v) => update({ closeToTray: v })}
        />
      </Section>

      <Section title="Deadline reminders" description="Only for screenshots whose note says you want to act (register, apply, remind me…).">
        <Row label="Remind me">
          <select
            className={`${inputClass} w-auto`}
            value={settings.deadlineLeadHours}
            onChange={(e) => update({ deadlineLeadHours: Number(e.target.value) })}
          >
            {LEAD_OPTIONS.map((o) => (
              <option key={o.hours} value={o.hours}>
                {o.label}
              </option>
            ))}
          </select>
        </Row>
        <Row label="Around">
          <select
            className={`${inputClass} w-auto`}
            value={settings.notifyHour}
            onChange={(e) => update({ notifyHour: Number(e.target.value) })}
          >
            {Array.from({ length: 24 }, (_, h) => (
              <option key={h} value={h}>
                {new Date(2000, 0, 1, h).toLocaleTimeString("en-IN", { hour: "numeric", minute: "2-digit" })}
              </option>
            ))}
          </select>
        </Row>
      </Section>

      <Section title="AI" description="Runs on this computer. Nothing you capture is ever uploaded.">
        <AiSetup showManage />
        <Row label="Improve existing screenshots">
          <Button
            size="sm"
            onClick={() =>
              void ipc.refileAll().then(
                (n) => toast({ message: n ? `Re-filing ${n} screenshot${n === 1 ? "" : "s"} in the background` : "Nothing to re-file" }),
                (e) => toast({ message: String(e), tone: "error" }),
              )
            }
          >
            Re-file all with the latest AI
          </Button>
        </Row>
        <Row label="Model">
          <select className={`${inputClass} w-auto`} value={settings.aiProfile} onChange={(e) => update({ aiProfile: e.target.value })}>
            <option value="standard">Standard: fast, fits 8 GB laptops</option>
            <option value="accurate">More accurate: slower, for 16 GB+ RAM</option>
          </select>
        </Row>
        <Switch
          label="Pause AI processing"
          description="New screenshots are still saved; they're filed when you resume."
          checked={settings.aiPaused}
          onChange={(v) => update({ aiPaused: v })}
        />
        <Switch
          label="Pause while battery saver is on"
          description="Filing waits until you're plugged in or battery saver is off."
          checked={settings.pauseOnBatterySaver}
          onChange={(v) => update({ pauseOnBatterySaver: v })}
        />
      </Section>

      <Section title="Appearance">
        <div role="radiogroup" aria-label="Theme" className="flex gap-2 py-2">
          {(["system", "light", "dark"] as ThemePref[]).map((t) => (
            <Button
              key={t}
              role="radio"
              aria-checked={settings.theme === t}
              variant={settings.theme === t ? "primary" : "secondary"}
              onClick={() => update({ theme: t })}
              className="capitalize"
            >
              {t}
            </Button>
          ))}
        </div>
        <Switch
          label="Show summary on cards"
          description="Off: cards show the screenshot, categories and title; click a title to see its summary and tags."
          checked={settings.showSummary}
          onChange={(v) => update({ showSummary: v })}
        />
      </Section>

      <Section title="Storage">
        <Row label="Location">
          <code className="truncate text-xs text-ink-muted" title={storage?.dataDir}>
            {storage?.dataDir ?? "…"}
          </code>
        </Row>
        <Row label="Export">
          <Button
            onClick={() =>
              void ipc.exportAll().then(
                (path) => toast({ message: `Exported to ${path.split(/[\\/]/).pop()}` }),
                (e) => toast({ message: String(e), tone: "error" }),
              )
            }
          >
            Export all data (.zip)
          </Button>
        </Row>
        <Row label="Used">
          <span className="text-sm text-ink-muted">
            {storage ? `${formatBytes(storage.bytes)} · ${storage.screenshots} screenshot${storage.screenshots === 1 ? "" : "s"}` : "…"}
          </span>
        </Row>
      </Section>

      <Section title="About">
        <p className="text-sm">ClipNest {info && <span className="text-ink-muted">v{info.version}</span>}</p>
        <p className="mt-1 text-sm text-ink-muted">
          Free, private and local. Your screenshots never leave this computer. The only network use is the one-time download of the AI
          engine (from llama.cpp's official GitHub releases) and the AI model (from Hugging Face). No account, no analytics.
        </p>
        {info && info.ocrLanguages.length > 0 && (
          <p className="mt-1 text-xs text-ink-muted">Text recognition languages: {info.ocrLanguages.join(", ")}</p>
        )}
      </Section>

      {import.meta.env.DEV && (
        <Section title="Developer">
          <Button
            onClick={() =>
              void ipc.devFakeAi().then(
                (n) => toast({ message: `Filled ${n} queued screenshot${n === 1 ? "" : "s"} with sample results` }),
                (e) => toast({ message: String(e), tone: "error" }),
              )
            }
          >
            Fill queued screenshots with sample AI results
          </Button>
        </Section>
      )}
    </div>
  );
}

function Section({ title, description, children }: { title: string; description?: string; children: ReactNode }) {
  return (
    <section className="rounded-[var(--radius-card)] border border-line bg-surface-raised p-8">
      <h2 className="text-xl leading-[1.5] text-ink">{title}</h2>
      {description && <p className="mt-1 text-sm leading-[1.4] text-ink-muted">{description}</p>}
      <div className="mt-4 divide-y divide-line">{children}</div>
    </section>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 py-3">
      <span className="shrink-0 text-sm text-ink">{label}</span>
      {children}
    </div>
  );
}
