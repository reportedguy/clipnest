import { forwardRef, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { DropdownMenu } from "radix-ui";
import {
  AlertCircle,
  CalendarClock,
  ChevronDown,
  ChevronRight,
  Copy,
  ExternalLink,
  Tags,
  ImageIcon,
  MoreHorizontal,
  Pencil,
  RotateCw,
  StickyNote,
  Trash2,
} from "lucide-react";
import { ipc, mediaUrl, type Card } from "../lib/ipc";
import { dueLabel, shortUrl, timeAgo } from "../lib/format";
import { menu } from "./ui";

export type CardAction =
  | { kind: "enlarge" }
  | { kind: "editCategories" }
  | { kind: "editText" }
  | { kind: "editNote" }
  | { kind: "deadline" }
  | { kind: "copy" }
  | { kind: "open" }
  | { kind: "retry" }
  | { kind: "delete" };

interface Props {
  card: Card;
  /** Whether the image model is installed (changes what a waiting Image card says). */
  visionReady?: boolean;
  /** Show the summary and tags without a click. */
  showSummary?: boolean;
  onAction: (card: Card, action: CardAction) => void;
  onKeyDown?: (e: KeyboardEvent<HTMLElement>) => void;
}

/**
 * One saved screenshot: image, categories and title; clicking the title reveals the
 * summary and tags (or they always show, with the "Show summary on cards" setting). A
 * collapsed "Original text" section holds the user's note and any links. The title
 * shimmers until the AI has filed it.
 */
export const ScreenshotCard = forwardRef<HTMLElement, Props>(function ScreenshotCard(
  { card, visionReady = true, showSummary = false, onAction, onKeyDown },
  ref,
) {
  const [open, setOpen] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const pending = card.status === "queued" || card.status === "processing";
  const failed = card.status === "failed";
  // Collapsed: screenshot, categories and title. Expanded (a click, or the setting):
  // also the summary, tags and the "Original text" toggle.
  const details = showSummary || expanded;
  const canToggle = !showSummary && !pending && !failed;
  const act = (a: CardAction) => onAction(card, a);

  // An expanded card folds back when you click anywhere outside it.
  const self = useRef<HTMLElement | null>(null);
  useEffect(() => {
    if (!expanded) return;
    const onDown = (e: PointerEvent) => {
      if (!self.current?.contains(e.target as Node)) {
        setExpanded(false);
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", onDown);
    return () => document.removeEventListener("pointerdown", onDown);
  }, [expanded]);

  return (
    <article
      ref={(el) => {
        self.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
      }}
      tabIndex={0}
      onKeyDown={onKeyDown}
      aria-label={card.title ?? card.userNote ?? "Screenshot"}
      className="group flex flex-col overflow-hidden rounded-[var(--radius-card)] border border-line bg-surface-raised"
    >
      <div className="relative">
        <button
          type="button"
          onClick={() => act({ kind: "enlarge" })}
          className="flex aspect-[16/10] w-full items-center justify-center overflow-hidden bg-surface p-2"
          aria-label="Enlarge screenshot"
          tabIndex={-1}
        >
          <img
            src={mediaUrl(card.thumbPath)}
            alt=""
            loading="lazy"
            draggable={false}
            className="max-h-full max-w-full rounded-[4px] object-contain"
          />
        </button>
        <div className="absolute top-3 right-3 opacity-0 transition-opacity duration-100 group-focus-within:opacity-100 group-hover:opacity-100">
          <CardMenu card={card} onAction={act} />
        </div>
        {card.mode === "image" && (
          <span
            className="absolute bottom-3 left-3 inline-flex items-center gap-1 rounded-[var(--radius-input)] bg-[#181d26]/85 px-2 py-1 text-xs font-medium text-white"
            title={card.autoUpgraded ? "Little text was found, so the picture itself was read" : "Read as a picture"}
          >
            <ImageIcon size={12} /> {card.autoUpgraded ? "Auto-upgraded to Image" : "Image"}
          </span>
        )}
      </div>

      <div
        className={`flex flex-1 flex-col gap-2 p-4 ${canToggle ? "cursor-pointer" : ""}`}
        onClick={(e) => {
          // A click on the card's text area expands it; its own buttons and links act alone.
          if (canToggle && !(e.target as HTMLElement).closest("button, a")) {
            setExpanded((v) => !v);
            setOpen(false);
          }
        }}
      >
        <CategoryChips card={card} onEdit={() => act({ kind: "editCategories" })} />

        {pending ? (
          <div aria-label="Processing" className="flex flex-col gap-2 py-1">
            <div className="h-4 w-3/4 animate-pulse rounded-[var(--radius-input)] bg-surface-strong" />
            <div className="h-3 w-full animate-pulse rounded-[var(--radius-input)] bg-surface" />
            <div className="h-3 w-2/3 animate-pulse rounded-[var(--radius-input)] bg-surface" />
            <span className="text-[13px] text-ink-muted">{pendingLabel(card, visionReady)}</span>
          </div>
        ) : failed ? (
          <div className="flex items-start gap-2 rounded-[var(--radius-input)] bg-signature-coral/8 p-3 text-[13px] text-ink">
            <AlertCircle size={14} className="mt-0.5 shrink-0" />
            <span className="flex-1">{card.error ?? "Couldn't file this screenshot."}</span>
            <button type="button" onClick={() => act({ kind: "retry" })} className="font-medium text-link">
              Retry
            </button>
          </div>
        ) : (
          <>
            <h3 className="text-base leading-[1.4] font-medium text-ink">{card.title}</h3>
            {details && card.summary && <p className="text-sm leading-[1.45] text-body">{card.summary}</p>}
          </>
        )}

        {card.deadlineAt !== null && card.deadlineImportant && !card.deadlineDone && <DueChip card={card} />}

        {details && !pending && card.tags.length > 0 && (
          <ul className="flex flex-wrap gap-1 pt-0.5" aria-label="Tags">
            {card.tags.map((t) => (
              <li key={t} className="rounded-[var(--radius-input)] bg-surface px-2 py-0.5 text-xs text-ink-muted">
                {t}
              </li>
            ))}
          </ul>
        )}

        <div className="mt-auto pt-1">
          {details ? (
            <>
              <button
                type="button"
                aria-expanded={open}
                onClick={() => setOpen((v) => !v)}
                className="flex w-full items-center gap-1 text-[13px] whitespace-nowrap text-ink-muted"
              >
                <ChevronRight size={14} className={`transition-transform duration-150 ${open ? "rotate-90" : ""}`} />
                Original text
                <span className="ml-auto pl-2">{timeAgo(card.createdAt)}</span>
              </button>
              {open && <OriginalText card={card} />}
            </>
          ) : (
            <button
              type="button"
              aria-expanded={false}
              disabled={!canToggle}
              onClick={() => setExpanded(true)}
              className="flex w-full items-center gap-1 text-[13px] whitespace-nowrap text-ink-muted disabled:cursor-default"
            >
              {canToggle && (
                <>
                  <ChevronDown size={14} /> Summary
                </>
              )}
              <span className="ml-auto pl-2">{timeAgo(card.createdAt)}</span>
            </button>
          )}
        </div>
      </div>
    </article>
  );
});

function OriginalText({ card }: { card: Card }) {
  return (
    <div className="mt-2 flex flex-col gap-2.5 border-t border-line pt-2.5 text-sm">
      <section>
        <h4 className="text-xs font-medium text-ink-muted">Your note</h4>
        <p className="mt-0.5 whitespace-pre-wrap text-ink">{card.userNote || <span className="text-ink-muted italic">No note</span>}</p>
      </section>
      {card.urls.length > 0 && (
        <section>
          <h4 className="text-xs font-medium text-ink-muted">Links</h4>
          <ul className="mt-0.5 flex flex-col gap-0.5">
            {card.urls.map((u) => (
              <li key={u} className="truncate text-xs">
                <a
                  href={withScheme(u)}
                  onClick={(e) => {
                    e.preventDefault();
                    void ipc.openUrl(withScheme(u));
                  }}
                  className="text-ink-muted underline decoration-line underline-offset-2"
                  title={u}
                >
                  {shortUrl(u)}
                </a>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

/** The card's categories as small colored chips; clicking them edits the categories. */
function CategoryChips({ card, onEdit }: { card: Card; onEdit: () => void }) {
  if (card.categories.length === 0) return null;
  return (
    <button type="button" onClick={onEdit} title="Edit categories" className="flex flex-wrap gap-1.5 text-left">
      {card.categories.map((c) => (
        <span
          key={c.id}
          className="inline-flex items-center gap-1.5 rounded-full border border-line px-2.5 py-0.5 text-xs font-medium text-ink"
        >
          <span className="color-dot h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: c.color }} aria-hidden />
          {c.name}
        </span>
      ))}
    </button>
  );
}

function DueChip({ card }: { card: Card }) {
  const due = dueLabel(card.deadlineAt!);
  return (
    <span
      className={`inline-flex w-fit items-center gap-1.5 rounded-[var(--radius-input)] px-2 py-1 text-xs font-medium ${
        due.urgent ? "bg-signature-coral text-white" : "bg-signature-cream text-[#181d26]"
      }`}
    >
      <CalendarClock size={12} aria-hidden />
      Due {due.text}
      {card.deadlineLabel ? ` · ${card.deadlineLabel}` : ""}
    </span>
  );
}

function pendingLabel(card: Card, visionReady: boolean): string {
  if (card.status === "processing") return card.ocrText === null ? "Reading the text…" : "Filing…";
  if (card.ocrText === null) return "Waiting to be read…";
  if (card.mode === "image" && !visionReady) return "Waiting for the image model…";
  return "Waiting to be filed…";
}

function withScheme(url: string) {
  return /^[a-z]+:\/\//i.test(url) ? url : `https://${url}`;
}

function CardMenu({ card, onAction }: { card: Card; onAction: (a: CardAction) => void }) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger
        aria-label="Screenshot actions"
        className="inline-flex h-9 w-9 items-center justify-center rounded-full border border-line bg-surface-raised text-ink active:bg-surface"
      >
        <MoreHorizontal size={16} />
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content align="end" sideOffset={4} className={menu.content}>
          <DropdownMenu.Item className={menu.item} onSelect={() => onAction({ kind: "editCategories" })}>
            <Tags size={14} /> Edit categories…
          </DropdownMenu.Item>
          <DropdownMenu.Item className={menu.item} onSelect={() => onAction({ kind: "editText" })}>
            <Pencil size={14} /> Edit title & summary
          </DropdownMenu.Item>
          <DropdownMenu.Item className={menu.item} onSelect={() => onAction({ kind: "editNote" })}>
            <StickyNote size={14} /> Edit note
          </DropdownMenu.Item>
          <DropdownMenu.Item className={menu.item} onSelect={() => onAction({ kind: "deadline" })}>
            <CalendarClock size={14} /> {card.deadlineAt !== null && card.deadlineImportant ? "Change deadline…" : "Set deadline…"}
          </DropdownMenu.Item>
          <DropdownMenu.Separator className={menu.separator} />
          <DropdownMenu.Item className={menu.item} onSelect={() => onAction({ kind: "copy" })}>
            <Copy size={14} /> Copy image
          </DropdownMenu.Item>
          <DropdownMenu.Item className={menu.item} onSelect={() => onAction({ kind: "open" })}>
            <ExternalLink size={14} /> Open in image viewer
          </DropdownMenu.Item>
          <DropdownMenu.Item className={menu.item} onSelect={() => onAction({ kind: "retry" })}>
            <RotateCw size={14} /> Retry AI
          </DropdownMenu.Item>
          <DropdownMenu.Separator className={menu.separator} />
          <DropdownMenu.Item
            className={`${menu.item} text-signature-coral dark:text-signature-peach`}
            onSelect={() => onAction({ kind: "delete" })}
          >
            <Trash2 size={14} /> Delete
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
