import { useCallback, useEffect, useState } from "react";
import { CalendarClock, Check } from "lucide-react";
import { ipc, mediaUrl, type Card } from "../lib/ipc";
import { LIBRARY_CHANGED, useTauriEvent } from "../lib/events";
import { dueLabel } from "../lib/format";
import { useToast } from "./Toast";

/** Pending deadlines, soonest first; refreshes whenever the library changes. */
export function useDeadlines(): Card[] {
  const [items, setItems] = useState<Card[]>([]);
  const load = useCallback(() => void ipc.upcomingDeadlines().then(setItems, () => {}), []);
  useEffect(load, [load]);
  useTauriEvent(LIBRARY_CHANGED, load);
  return items;
}

/** "Upcoming deadlines": soonest first, click to open, the check to dismiss (spec 3.6). */
export function DeadlineStrip({ onOpen }: { onOpen: (card: Card) => void }) {
  const items = useDeadlines();
  const toast = useToast();

  if (items.length === 0) return null;
  return (
    <section aria-label="Upcoming deadlines" className="flex flex-col gap-3 rounded-[var(--radius-card)] border border-line bg-surface p-4">
      <h2 className="flex items-center gap-2 px-1 text-[15px] font-medium text-ink">
        <span className="inline-flex h-7 w-7 items-center justify-center rounded-full bg-signature-coral/12 text-signature-coral dark:bg-signature-peach/15 dark:text-signature-peach">
          <CalendarClock size={15} aria-hidden />
        </span>
        Upcoming deadlines
        <span className="rounded-full bg-surface-strong px-2 py-0.5 text-xs font-medium text-ink-muted tabular-nums">{items.length}</span>
      </h2>
      <ul className="flex gap-3 overflow-x-auto pb-1">
        {items.map((c) => {
          const due = dueLabel(c.deadlineAt!);
          return (
            <li
              key={c.id}
              className="flex w-80 shrink-0 items-center gap-3 rounded-[var(--radius-card)] border border-line bg-surface-raised p-2.5 pr-3"
            >
              <button type="button" onClick={() => onOpen(c)} className="flex min-w-0 flex-1 items-center gap-3 text-left">
                <img
                  src={mediaUrl(c.thumbPath)}
                  alt=""
                  className="h-12 w-16 shrink-0 rounded-[var(--radius-input)] border border-line bg-canvas object-cover object-top"
                  draggable={false}
                />
                <span className="flex min-w-0 flex-col gap-1">
                  <span className="truncate text-sm font-medium text-ink">{c.title ?? c.userNote ?? "Screenshot"}</span>
                  <span className="flex min-w-0 items-center gap-1.5 text-xs">
                    <span
                      className={`shrink-0 rounded-full px-2 py-0.5 font-medium ${
                        due.urgent ? "bg-signature-coral text-white" : "bg-surface-strong text-ink"
                      }`}
                    >
                      {due.text.charAt(0).toUpperCase() + due.text.slice(1)}
                    </span>
                    {c.deadlineLabel && <span className="truncate text-ink-muted">{c.deadlineLabel}</span>}
                  </span>
                </span>
              </button>
              <button
                type="button"
                title="Done: remove from this list"
                aria-label={`Mark “${c.title ?? "deadline"}” done`}
                onClick={() => void ipc.markDeadlineDone(c.id).catch((e) => toast({ message: String(e), tone: "error" }))}
                className="inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-line text-ink active:bg-surface"
              >
                <Check size={15} />
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
