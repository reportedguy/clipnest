import { useRef, useState, type KeyboardEvent } from "react";
import { Dialog } from "radix-ui";
import { X } from "lucide-react";
import { ipc, mediaUrl, type Card, type Category } from "../lib/ipc";
import { ScreenshotCard, type CardAction } from "./ScreenshotCard";
import { Button, IconButton, Modal, inputClass } from "./ui";
import { useToast } from "./Toast";
import { CategoryPicker } from "./CategoryPicker";
import { useOfferRecheck } from "./NewCategoryDialog";
import { nextColor } from "../lib/palette";
import { useAiStatus } from "./AiSetup";

interface Props {
  cards: Card[];
  /** All categories, for the "Edit categories" picker. */
  categories: Category[];
  /** Show summaries without a click (the "Show summary on cards" setting). */
  showSummary?: boolean;
}

type Editing = { card: Card; what: "text" | "note" | "deadline" | "categories" } | null;

/**
 * Responsive grid of cards plus everything they need: the action handlers, the
 * enlarge view, edit dialogs, and keyboard navigation (arrows move, Enter enlarges,
 * Delete removes with undo).
 */
export function CardGrid({ cards, categories, showSummary = false }: Props) {
  const toast = useToast();
  const ai = useAiStatus();
  const [enlarged, setEnlarged] = useState<Card | null>(null);
  const [editing, setEditing] = useState<Editing>(null);
  const refs = useRef<(HTMLElement | null)[]>([]);
  const grid = useRef<HTMLDivElement>(null);

  const fail = (e: unknown) => toast({ message: String(e), tone: "error" });

  async function run(card: Card, action: CardAction) {
    try {
      switch (action.kind) {
        case "enlarge":
          return setEnlarged(card);
        case "editText":
          return setEditing({ card, what: "text" });
        case "editNote":
          return setEditing({ card, what: "note" });
        case "deadline":
          return setEditing({ card, what: "deadline" });
        case "editCategories":
          return setEditing({ card, what: "categories" });
        case "copy":
          await ipc.copyCardImage(card.id);
          return toast({ message: "Image copied" });
        case "open":
          return await ipc.openCardImage(card.id);
        case "retry":
          await ipc.retryCard(card.id);
          return toast({ message: "Queued for another try" });
        case "delete":
          await ipc.deleteCard(card.id);
          return toast({
            message: "Screenshot deleted",
            action: { label: "Undo", run: () => void ipc.restoreCard(card.id).catch(fail) },
          });
      }
    } catch (e) {
      fail(e);
    }
  }

  function onKeyDown(index: number, e: KeyboardEvent<HTMLElement>) {
    if (e.target !== e.currentTarget) return; // keys inside the card's own controls
    const columns = columnCount(grid.current);
    const move = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: columns, ArrowUp: -columns }[e.key];
    if (move !== undefined) {
      e.preventDefault();
      refs.current[Math.min(Math.max(index + move, 0), cards.length - 1)]?.focus();
    } else if (e.key === "Delete") {
      e.preventDefault();
      // Keep focus in the grid after the card disappears.
      (refs.current[index + 1] ?? refs.current[index - 1])?.focus();
      void run(cards[index], { kind: "delete" });
    } else if (e.key === "Enter") {
      e.preventDefault();
      setEnlarged(cards[index]);
    }
  }

  return (
    <>
      <div ref={grid} className="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-5">
        {cards.map((card, i) => (
          <ScreenshotCard
            key={card.id}
            ref={(el) => {
              refs.current[i] = el;
            }}
            card={card}
            visionReady={ai?.visionInstalled ?? false}
            showSummary={showSummary}
            onAction={run}
            onKeyDown={(e) => onKeyDown(i, e)}
          />
        ))}
      </div>

      <Lightbox card={enlarged} onClose={() => setEnlarged(null)} />
      {editing && (editing.what === "text" || editing.what === "note") && (
        <EditDialog editing={{ card: editing.card, what: editing.what }} onClose={() => setEditing(null)} onError={fail} />
      )}
      {editing?.what === "categories" && (
        <EditCategoriesDialog card={editing.card} categories={categories} onClose={() => setEditing(null)} onError={fail} />
      )}
      {editing?.what === "deadline" && <DeadlineDialog card={editing.card} onClose={() => setEditing(null)} onError={fail} />}
    </>
  );
}

/** Number of columns currently rendered, read from the computed grid template. */
function columnCount(el: HTMLElement | null): number {
  if (!el) return 1;
  return getComputedStyle(el).gridTemplateColumns.split(" ").filter(Boolean).length || 1;
}

export function Lightbox({ card, onClose }: { card: Card | null; onClose: () => void }) {
  return (
    <Dialog.Root open={card !== null} onOpenChange={(o) => !o && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-[#181d26]/90" />
        <Dialog.Content
          className="fixed inset-0 z-50 flex items-center justify-center p-8 outline-none"
          onClick={onClose}
          aria-describedby={undefined}
        >
          <Dialog.Title className="sr-only">{card?.title ?? "Screenshot"}</Dialog.Title>
          {card && (
            <img
              src={mediaUrl(card.imagePath)}
              alt={card.title ?? "Screenshot"}
              className="max-h-full max-w-full rounded-[var(--radius-input)] object-contain shadow-2xl"
              onClick={(e) => e.stopPropagation()}
            />
          )}
          <Dialog.Close asChild>
            <IconButton shape="circle" label="Close" className="absolute top-5 right-5">
              <X size={18} />
            </IconButton>
          </Dialog.Close>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function EditDialog({
  editing,
  onClose,
  onError,
}: {
  editing: { card: Card; what: "text" | "note" };
  onClose: () => void;
  onError: (e: unknown) => void;
}) {
  const { card, what } = editing;
  const [title, setTitle] = useState(card.title ?? "");
  const [summary, setSummary] = useState(card.summary ?? "");
  const [note, setNote] = useState(card.userNote);
  const [saving, setSaving] = useState(false);

  async function save() {
    setSaving(true);
    try {
      if (what === "text") await ipc.updateCardText(card.id, title, summary);
      else await ipc.updateCardNote(card.id, note);
      onClose();
    } catch (e) {
      onError(e);
      setSaving(false);
    }
  }

  return (
    <Modal open onOpenChange={(o) => !o && onClose()} title={what === "text" ? "Edit title & summary" : "Edit note"}>
      <form
        className="flex flex-col gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        {what === "text" ? (
          <>
            <label className="flex flex-col gap-1 text-sm">
              Title
              <input className={inputClass} value={title} onChange={(e) => setTitle(e.target.value)} maxLength={90} autoFocus />
            </label>
            <label className="flex flex-col gap-1 text-sm">
              Summary
              <textarea
                className={`${inputClass} resize-none`}
                rows={3}
                value={summary}
                onChange={(e) => setSummary(e.target.value)}
                maxLength={320}
              />
            </label>
          </>
        ) : (
          <label className="flex flex-col gap-1 text-sm">
            Your note
            <textarea className={`${inputClass} resize-none`} rows={4} value={note} onChange={(e) => setNote(e.target.value)} autoFocus />
          </label>
        )}
        <div className="flex justify-end gap-2 pt-1">
          <Button onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={saving || (what === "text" && !title.trim())}>
            Save
          </Button>
        </div>
      </form>
    </Modal>
  );
}

function toInputDate(ms: number) {
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** Set, change or clear a deadline by hand. Hand-set deadlines always show and remind. */
function DeadlineDialog({ card, onClose, onError }: { card: Card; onClose: () => void; onError: (e: unknown) => void }) {
  const has = card.deadlineAt !== null && card.deadlineImportant;
  const [date, setDate] = useState(has ? toInputDate(card.deadlineAt!) : "");
  const [label, setLabel] = useState(card.deadlineLabel ?? "");

  const save = (d: string | null) => void ipc.setDeadline(card.id, d, label).then(onClose, onError);

  return (
    <Modal open onOpenChange={(o) => !o && onClose()} title={has ? "Change deadline" : "Set a deadline"}>
      <form
        className="flex flex-col gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          if (date) save(date);
        }}
      >
        <label className="flex flex-col gap-1 text-sm">
          Date
          <input type="date" className={inputClass} value={date} onChange={(e) => setDate(e.target.value)} autoFocus required />
        </label>
        <label className="flex flex-col gap-1 text-sm">
          What it's for <span className="text-xs text-ink-muted">(optional)</span>
          <input
            className={inputClass}
            value={label}
            maxLength={80}
            placeholder="e.g. registration closes"
            onChange={(e) => setLabel(e.target.value)}
          />
        </label>
        <p className="text-xs text-ink-muted">It'll appear under Upcoming deadlines, with a reminder before it's due.</p>
        <div className="flex justify-between gap-2 pt-1">
          {has ? (
            <Button variant="ghost" onClick={() => save(null)}>
              Remove deadline
            </Button>
          ) : (
            <span />
          )}
          <div className="flex gap-2">
            <Button onClick={onClose}>Cancel</Button>
            <Button type="submit" variant="primary" disabled={!date}>
              Save
            </Button>
          </div>
        </div>
      </form>
    </Modal>
  );
}

/** Pick any number of categories for a card (or create one). Hand-picked categories are final. */
function EditCategoriesDialog({
  card,
  categories,
  onClose,
  onError,
}: {
  card: Card;
  categories: Category[];
  onClose: () => void;
  onError: (e: unknown) => void;
}) {
  const [selected, setSelected] = useState(card.categories.map((c) => c.id));
  const [list, setList] = useState(categories);
  const offerRecheck = useOfferRecheck();

  async function create(name: string) {
    const id = await ipc.createCategory({ name, color: nextColor(list.map((c) => c.color)) });
    const fresh = await ipc.listCategories();
    setList(fresh);
    offerRecheck(name);
    return id;
  }

  return (
    <Modal open onOpenChange={(o) => !o && onClose()} title="Categories">
      <div className="flex flex-col gap-4">
        <CategoryPicker categories={list} selected={selected} onChange={setSelected} onCreate={create} />
        <p className="text-[13px] text-ink-muted">
          A screenshot can be in several categories. Your choice is kept; the AI won't change it.
        </p>
        <div className="flex justify-end gap-2">
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={() => void ipc.setCardCategories(card.id, selected).then(onClose, onError)}>
            Save
          </Button>
        </div>
      </div>
    </Modal>
  );
}
