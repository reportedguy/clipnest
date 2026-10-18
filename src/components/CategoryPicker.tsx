import { useMemo, useState } from "react";
import { Check, Plus } from "lucide-react";
import type { Category } from "../lib/ipc";

/**
 * Multi-select list of categories with a find-or-create box. Used to edit a card's
 * categories and in the capture popup. Typing a name that doesn't exist offers
 * "Create “name”".
 */
export function CategoryPicker({
  categories,
  selected,
  onChange,
  onCreate,
  compact = false,
}: {
  categories: Category[];
  selected: number[];
  onChange: (ids: number[]) => void;
  /** Creates a category and returns its id (it's then selected). */
  onCreate: (name: string) => Promise<number>;
  compact?: boolean;
}) {
  const [query, setQuery] = useState("");
  const [creating, setCreating] = useState(false);
  const q = query.trim().toLowerCase();
  const shown = useMemo(() => categories.filter((c) => c.name.toLowerCase().includes(q)), [categories, q]);
  const exact = categories.some((c) => c.name.toLowerCase() === q);

  const toggle = (id: number) => onChange(selected.includes(id) ? selected.filter((x) => x !== id) : [...selected, id]);

  async function create() {
    if (!q || exact || creating) return;
    setCreating(true);
    try {
      const id = await onCreate(query.trim());
      onChange([...selected, id]);
      setQuery("");
    } finally {
      setCreating(false);
    }
  }

  return (
    <div className="flex min-h-0 flex-col gap-2">
      <input
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            e.stopPropagation();
            if (shown.length === 1 && exact) toggle(shown[0].id);
            else void create();
          }
        }}
        placeholder="Find or create a category"
        aria-label="Find or create a category"
        className={`w-full rounded-[var(--radius-input)] border border-line bg-canvas px-3 text-ink placeholder:text-ink-muted focus:border-focus focus:outline-none ${
          compact ? "py-1.5 text-[13px]" : "py-2 text-sm"
        }`}
      />
      <ul role="listbox" aria-multiselectable className={`overflow-y-auto ${compact ? "max-h-28" : "max-h-72"}`}>
        {shown.map((c) => {
          const on = selected.includes(c.id);
          return (
            <li key={c.id}>
              <button
                type="button"
                role="option"
                aria-selected={on}
                onClick={() => toggle(c.id)}
                className={`flex w-full items-center gap-2.5 rounded-[var(--radius-input)] px-2 text-left text-ink active:bg-surface ${
                  compact ? "py-1 text-[13px]" : "py-1.5 text-sm"
                }`}
              >
                <span
                  className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-[4px] border ${on ? "border-primary bg-primary text-on-primary" : "border-line-strong"}`}
                  aria-hidden
                >
                  {on && <Check size={11} strokeWidth={3} />}
                </span>
                <span className="color-dot h-2.5 w-2.5 shrink-0 rounded-full" style={{ backgroundColor: c.color }} aria-hidden />
                <span className="truncate">{c.name}</span>
              </button>
            </li>
          );
        })}
        {q && !exact && (
          <li>
            <button
              type="button"
              onClick={() => void create()}
              disabled={creating}
              className={`flex w-full items-center gap-2.5 rounded-[var(--radius-input)] px-2 text-left text-link active:bg-surface ${
                compact ? "py-1 text-[13px]" : "py-1.5 text-sm"
              }`}
            >
              <Plus size={14} aria-hidden /> Create “{query.trim()}”
            </button>
          </li>
        )}
      </ul>
    </div>
  );
}
