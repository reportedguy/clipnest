import { useEffect, useState, type ComponentType } from "react";
import {
  Briefcase,
  Check,
  Clapperboard,
  Code,
  GraduationCap,
  Kanban,
  Megaphone,
  Palette,
  PenLine,
  Plus,
  Rocket,
  Shapes,
  SlidersHorizontal,
  X,
} from "lucide-react";
import { ipc, type Category, type CategoryInput, type Preset } from "../lib/ipc";
import { nextColor } from "../lib/palette";
import { Button, IconButton, Modal } from "./ui";
import { useToast } from "./Toast";

const ICONS: Record<string, ComponentType<{ size?: number }>> = {
  kanban: Kanban,
  palette: Palette,
  clapperboard: Clapperboard,
  rocket: Rocket,
  "graduation-cap": GraduationCap,
  code: Code,
  megaphone: Megaphone,
  "pen-line": PenLine,
  shapes: Shapes,
};

/** Past this many, filing a screenshot with a lot of text can overflow the AI's input (measured). */
const COMFORTABLE_MAX = 80;

/** One row in the review list. */
export interface Draft {
  key: string;
  name: string;
  hint: string;
  keep: boolean;
  /** From a profession: unticked rather than removed. Rows added by hand can be removed. */
  fromPreset: boolean;
}

let nextKey = 0;
const draft = (name: string, hint: string, fromPreset: boolean): Draft => ({
  key: `d${nextKey++}`,
  name,
  hint,
  keep: true,
  fromPreset,
});

export function usePresets(): Preset[] {
  const [presets, setPresets] = useState<Preset[]>([]);
  useEffect(() => {
    void ipc.categoryPresets().then(setPresets);
  }, []);
  return presets;
}

/** The chosen professions' categories in the order they were picked, without repeats or `skip`. */
export function mergePresets(presets: Preset[], ids: string[], skip: string[] = []): Draft[] {
  const seen = new Set(skip.map((n) => n.toLowerCase()));
  const out: Draft[] = [];
  for (const id of ids) {
    for (const c of presets.find((p) => p.id === id)?.categories ?? []) {
      const k = c.name.toLowerCase();
      if (!seen.has(k)) {
        seen.add(k);
        out.push(draft(c.name, c.hint, true));
      }
    }
  }
  return out;
}

/** The kept rows, each given an unused palette color. */
export function toInputs(drafts: Draft[], usedColors: string[] = []): CategoryInput[] {
  const used = [...usedColors];
  return drafts
    .filter((d) => d.keep && d.name.trim())
    .map((d) => {
      const color = nextColor(used);
      used.push(color);
      return { name: d.name.trim(), hint: d.hint.trim(), color };
    });
}

/** Profession cards (pick any number) plus "I'll set up my own". */
export function ProfessionPicker({
  presets,
  selected,
  custom,
  onChange,
  wide = false,
}: {
  presets: Preset[];
  selected: string[];
  /** Whether "I'll set up my own" is chosen; leave out to hide that card. */
  custom?: boolean;
  onChange: (selected: string[], custom: boolean) => void;
  /** Three columns (setup) instead of two (the Settings dialog). */
  wide?: boolean;
}) {
  return (
    <div className={`grid gap-2 sm:grid-cols-2 ${wide ? "lg:grid-cols-3" : ""}`}>
      {presets.map((p) => {
        const on = selected.includes(p.id);
        const Icon = ICONS[p.icon] ?? Briefcase;
        return (
          <ChoiceCard
            key={p.id}
            on={on}
            icon={<Icon size={18} />}
            title={p.name}
            detail={p.description}
            onClick={() => onChange(on ? selected.filter((s) => s !== p.id) : [...selected, p.id], false)}
          />
        );
      })}
      {custom !== undefined && (
        <ChoiceCard
          on={custom}
          icon={<SlidersHorizontal size={18} />}
          title="I'll set up my own"
          detail="Start empty and add your own categories"
          onClick={() => onChange([], !custom)}
        />
      )}
    </div>
  );
}

function ChoiceCard({
  on,
  icon,
  title,
  detail,
  onClick,
}: {
  on: boolean;
  icon: React.ReactNode;
  title: string;
  detail: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-pressed={on}
      onClick={onClick}
      className={`flex items-start gap-3 rounded-[var(--radius-card)] border p-4 text-left transition-colors duration-100 active:bg-surface ${
        on ? "border-primary bg-surface" : "border-line bg-surface-raised"
      }`}
    >
      <span className={`mt-0.5 ${on ? "text-ink" : "text-ink-muted"}`}>{icon}</span>
      <span className="min-w-0 flex-1">
        <span className="block text-[15px] font-medium text-ink">{title}</span>
        <span className="mt-0.5 block text-[13px] leading-[1.4] text-ink-muted">{detail}</span>
      </span>
      <span
        className={`mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full border ${
          on ? "border-primary bg-primary text-on-primary" : "border-line-strong"
        }`}
      >
        {on && <Check size={13} />}
      </span>
    </button>
  );
}

/** Tick, rename, describe and add categories before they're saved. */
export function CategoryReview({
  drafts,
  onChange,
  alreadyHave = 0,
}: {
  drafts: Draft[];
  onChange: (drafts: Draft[]) => void;
  /** Categories that exist already and stay (for the running total). */
  alreadyHave?: number;
}) {
  const [newName, setNewName] = useState("");
  const kept = drafts.filter((d) => d.keep && d.name.trim()).length;
  const total = kept + alreadyHave;
  const set = (key: string, patch: Partial<Draft>) => onChange(drafts.map((d) => (d.key === key ? { ...d, ...patch } : d)));

  function add() {
    const name = newName.trim();
    if (!name) return;
    onChange([...drafts, draft(name, "", false)]);
    setNewName("");
  }

  return (
    <div className="flex flex-col gap-3">
      {drafts.length > 0 && (
        <ul className="flex max-h-[min(440px,50vh)] flex-col gap-1 overflow-y-auto pr-1">
          {drafts.map((d) => (
            <li
              key={d.key}
              className={`flex items-start gap-3 rounded-[var(--radius-card)] border border-line bg-surface-raised px-3 py-1.5 ${
                d.keep ? "" : "opacity-50"
              }`}
            >
              <input
                type="checkbox"
                checked={d.keep}
                aria-label={`Keep ${d.name || "this category"}`}
                onChange={(e) => set(d.key, { keep: e.target.checked })}
                className="mt-1 h-4 w-4 shrink-0 accent-[var(--color-primary)]"
              />
              <div className="min-w-0 flex-1">
                <input
                  value={d.name}
                  maxLength={40}
                  aria-label="Category name"
                  onChange={(e) => set(d.key, { name: e.target.value })}
                  className="w-full rounded-[var(--radius-input)] border border-transparent bg-transparent px-1 py-0 text-sm font-medium text-ink focus:border-focus focus:outline-none"
                />
                <input
                  value={d.hint}
                  maxLength={160}
                  aria-label="What belongs here"
                  placeholder="What belongs here? (optional, helps the AI)"
                  onChange={(e) => set(d.key, { hint: e.target.value })}
                  className="w-full rounded-[var(--radius-input)] border border-transparent bg-transparent px-1 py-0 text-[13px] text-ink-muted placeholder:text-ink-muted/70 focus:border-focus focus:outline-none"
                />
              </div>
              {!d.fromPreset && (
                <IconButton label={`Remove ${d.name}`} onClick={() => onChange(drafts.filter((x) => x.key !== d.key))}>
                  <X size={15} />
                </IconButton>
              )}
            </li>
          ))}
        </ul>
      )}
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          add();
        }}
      >
        <input
          value={newName}
          maxLength={40}
          placeholder={drafts.length ? "Add another category" : "Name your first category, e.g. Client work"}
          onChange={(e) => setNewName(e.target.value)}
          className="w-full rounded-[var(--radius-input)] border border-line bg-canvas px-3 py-2 text-sm text-ink placeholder:text-ink-muted focus:border-focus focus:outline-none"
        />
        <Button type="submit" size="sm" disabled={!newName.trim()}>
          <Plus size={15} /> Add
        </Button>
      </form>
      <p className="text-[13px] text-ink-muted">
        {total} {total === 1 ? "category" : "categories"} + Miscellaneous for anything that doesn't fit.
        {total > COMFORTABLE_MAX && " That's a lot: with more than 80, screenshots with a lot of text may fail to file."}
      </p>
    </div>
  );
}

/** Settings: add a profession's categories to the existing ones (nothing is removed). */
export function AddFromProfessionDialog({ open, onClose, categories }: { open: boolean; onClose: () => void; categories: Category[] }) {
  const toast = useToast();
  const presets = usePresets();
  const [selected, setSelected] = useState<string[]>([]);
  const [drafts, setDrafts] = useState<Draft[] | null>(null);
  const [saving, setSaving] = useState(false);

  // Each opening starts fresh; the dialog stays mounted so it can animate.
  useEffect(() => {
    if (open) {
      setSelected([]);
      setDrafts(null);
      setSaving(false);
    }
  }, [open]);

  const existing = categories.map((c) => c.name);
  const userCount = categories.filter((c) => !c.isSystem).length;

  async function save() {
    if (!drafts) return;
    setSaving(true);
    try {
      const r = await ipc.applyCategorySetup(
        toInputs(
          drafts,
          categories.map((c) => c.color),
        ),
        false,
      );
      onClose();
      toast({
        message: r.added
          ? `Added ${r.added} ${r.added === 1 ? "category" : "categories"}. Check your existing screenshots for them?`
          : "You already have all of those",
        action: r.added
          ? {
              label: "Check",
              run: () =>
                void ipc.recheckCategories().then(
                  (n) =>
                    toast({
                      message: n ? `Checking ${n} screenshot${n === 1 ? "" : "s"} in the background` : "No screenshots to check yet",
                    }),
                  (e) => toast({ message: String(e), tone: "error" }),
                ),
            }
          : undefined,
      });
    } catch (e) {
      toast({ message: String(e), tone: "error" });
      setSaving(false);
    }
  }

  return (
    <Modal open={open} onOpenChange={(o) => !o && onClose()} title="Add categories from a profession" wide>
      {drafts === null ? (
        <div className="flex flex-col gap-4">
          <p className="text-sm text-body">Pick one or more. You'll see the list before anything is added.</p>
          <div className="max-h-[min(460px,55vh)] overflow-y-auto pr-1">
            <ProfessionPicker presets={presets} selected={selected} onChange={(s) => setSelected(s)} />
          </div>
          <div className="flex justify-end gap-2">
            <Button onClick={onClose}>Cancel</Button>
            <Button variant="primary" disabled={!selected.length} onClick={() => setDrafts(mergePresets(presets, selected, existing))}>
              Next
            </Button>
          </div>
        </div>
      ) : (
        <div className="flex flex-col gap-4">
          <p className="text-sm text-body">
            {drafts.length
              ? "These are new to you. Untick any you don't want; categories you already have stay as they are."
              : "You already have every category from these professions."}
          </p>
          <CategoryReview drafts={drafts} onChange={setDrafts} alreadyHave={userCount} />
          <div className="flex justify-between gap-2">
            <Button variant="ghost" onClick={() => setDrafts(null)}>
              Back
            </Button>
            <Button variant="primary" disabled={saving || !toInputs(drafts).length} onClick={() => void save()}>
              Add {toInputs(drafts).length || ""} {toInputs(drafts).length === 1 ? "category" : "categories"}
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
}
