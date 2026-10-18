import { useEffect, useState } from "react";
import { DropdownMenu, Popover } from "radix-ui";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Bell, CalendarClock, Check, ChevronDown, Copy, Minus, Settings as SettingsIcon, Square, X } from "lucide-react";
import { ipc, mediaUrl, type Card, type ThemePref } from "../lib/ipc";
import { dueLabel } from "../lib/format";
import { menu } from "./ui";
import { QueueBadge } from "./QueueBadge";
import { SearchBar } from "./SearchBar";
import { useDeadlines } from "./DeadlineStrip";
import { useToast } from "./Toast";

interface Props {
  theme: ThemePref;
  query: string;
  onQuery: (q: string) => void;
  focusSearch: boolean;
  onHome: () => void;
  onSettings: () => void;
  onOpenCard: (card: Card) => void;
  /** First-run setup: just the brand and the window buttons. */
  onboarding?: boolean;
}

/**
 * The window's own title bar (the OS frame is off): brand and the settings menu on the
 * left, then notifications, search and the window buttons on the right. Empty space drags
 * the window; double-clicking it maximizes.
 */
export function TitleBar({ theme, query, onQuery, focusSearch, onHome, onSettings, onOpenCard, onboarding = false }: Props) {
  return (
    <header data-tauri-drag-region className="relative z-30 flex h-14 shrink-0 items-center gap-3 bg-canvas pl-5 select-none">
      <Brand onHome={onHome} />
      {!onboarding && <SettingsMenu theme={theme} onSettings={onSettings} />}
      <div data-tauri-drag-region className="h-full min-w-4 flex-1" />
      {!onboarding && (
        <>
          <QueueBadge />
          <Notifications onOpenCard={onOpenCard} />
          <div className="w-[min(320px,34vw)]">
            <SearchBar value={query} onChange={onQuery} autoFocus={focusSearch} />
          </div>
        </>
      )}
      <WindowButtons />
    </header>
  );
}

function Brand({ onHome }: { onHome: () => void }) {
  return (
    <button type="button" onClick={onHome} className="flex shrink-0 items-center gap-2.5" aria-label="ClipNest, go to your library">
      <img src="/logo-light.svg" alt="" className="h-7 w-7 dark:hidden" draggable={false} />
      <img src="/logo-dark.svg" alt="" className="hidden h-7 w-7 dark:block" draggable={false} />
      <span className="type-brand text-[19px] text-ink">ClipNest</span>
    </button>
  );
}

const THEMES: { value: ThemePref; label: string }[] = [
  { value: "system", label: "Match Windows" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

function SettingsMenu({ theme, onSettings }: { theme: ThemePref; onSettings: () => void }) {
  const toast = useToast();
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger
        aria-label="Menu"
        className="-ml-1 inline-flex h-7 w-7 items-center justify-center rounded-full text-ink-muted data-[state=open]:bg-surface data-[state=open]:text-ink"
      >
        <ChevronDown size={15} />
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content align="start" sideOffset={6} className={menu.content}>
          <DropdownMenu.Item className={menu.item} onSelect={onSettings}>
            <SettingsIcon size={14} /> Settings
          </DropdownMenu.Item>
          <DropdownMenu.Separator className={menu.separator} />
          <DropdownMenu.Label className="px-2.5 pt-1 pb-1.5 text-xs text-ink-muted">Theme</DropdownMenu.Label>
          {THEMES.map((t) => (
            <DropdownMenu.Item
              key={t.value}
              className={menu.item}
              onSelect={() => void ipc.updateSettings({ theme: t.value }).catch((e) => toast({ message: String(e), tone: "error" }))}
            >
              <span className="w-3.5">{theme === t.value && <Check size={14} />}</span>
              {t.label}
            </DropdownMenu.Item>
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

/** Bell with every pending reminder; a count when some are due today, tomorrow or overdue. */
function Notifications({ onOpenCard }: { onOpenCard: (card: Card) => void }) {
  const items = useDeadlines();
  const toast = useToast();
  const urgent = items.filter((c) => dueLabel(c.deadlineAt!).urgent).length;
  const label = items.length ? `Reminders: ${items.length}${urgent ? `, ${urgent} due soon` : ""}` : "Reminders";

  return (
    <Popover.Root>
      <Popover.Trigger
        aria-label={label}
        title={label}
        className="relative inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-full text-ink active:bg-surface data-[state=open]:bg-surface"
      >
        <Bell size={18} />
        {urgent > 0 && (
          <span className="absolute top-1 right-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-signature-coral px-1 text-[10px] leading-none font-semibold text-white ring-2 ring-canvas">
            {urgent}
          </span>
        )}
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content align="end" sideOffset={6} className={`${menu.content} w-96 p-0`}>
          <div className="flex items-center justify-between border-b border-line px-4 py-3">
            <p className="font-medium">Reminders</p>
            <span className="text-xs text-ink-muted">{items.length ? `${items.length} upcoming` : ""}</span>
          </div>
          {items.length === 0 ? (
            <p className="px-4 py-6 text-center text-[13px] leading-[1.5] text-ink-muted">
              No reminders yet. When a note says things like “apply by Friday”, the deadline shows up here.
            </p>
          ) : (
            <ul className="max-h-[60vh] overflow-y-auto p-1.5">
              {items.map((c) => {
                const due = dueLabel(c.deadlineAt!);
                return (
                  <li key={c.id} className="flex items-center gap-3 rounded-[var(--radius-input)] p-2">
                    <Popover.Close asChild>
                      <button type="button" onClick={() => onOpenCard(c)} className="flex min-w-0 flex-1 items-center gap-3 text-left">
                        <img
                          src={mediaUrl(c.thumbPath)}
                          alt=""
                          className="h-10 w-12 shrink-0 rounded-[4px] border border-line object-cover object-top"
                          draggable={false}
                        />
                        <span className="min-w-0">
                          <span className="block truncate text-[13px] font-medium text-ink">{c.title ?? c.userNote ?? "Screenshot"}</span>
                          <span className="mt-0.5 flex items-center gap-1 truncate text-xs text-ink-muted">
                            <CalendarClock size={11} aria-hidden />
                            <span className={due.urgent ? "font-medium text-signature-coral dark:text-signature-peach" : ""}>
                              Due {due.text}
                            </span>
                            {c.deadlineLabel ? ` · ${c.deadlineLabel}` : ""}
                          </span>
                        </span>
                      </button>
                    </Popover.Close>
                    <button
                      type="button"
                      title="Done: remove this reminder"
                      aria-label={`Mark “${c.title ?? "deadline"}” done`}
                      onClick={() => void ipc.markDeadlineDone(c.id).catch((e) => toast({ message: String(e), tone: "error" }))}
                      className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-line text-ink active:bg-surface"
                    >
                      <Check size={14} />
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

/** Minimize, maximize/restore and close, drawn like Windows' own. */
function WindowButtons() {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    const w = getCurrentWindow();
    const sync = () => void w.isMaximized().then(setMaximized, () => {});
    sync();
    const off = w.onResized(sync);
    return () => void off.then((f) => f());
  }, []);
  const w = () => getCurrentWindow();
  const base = "inline-flex h-14 w-12 items-center justify-center text-ink-muted transition-colors duration-100";
  return (
    <div className="ml-1 flex shrink-0 self-start">
      <button type="button" aria-label="Minimize" onClick={() => void w().minimize()} className={`${base} hover:bg-surface hover:text-ink`}>
        <Minus size={16} />
      </button>
      <button
        type="button"
        aria-label={maximized ? "Restore" : "Maximize"}
        onClick={() => void w().toggleMaximize()}
        className={`${base} hover:bg-surface hover:text-ink`}
      >
        {maximized ? <Copy size={13} className="-scale-x-100" /> : <Square size={12} />}
      </button>
      <button type="button" aria-label="Close" onClick={() => void w().close()} className={`${base} hover:bg-[#c42b1c] hover:text-white`}>
        <X size={17} />
      </button>
    </div>
  );
}
