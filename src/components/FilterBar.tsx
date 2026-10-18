import { useEffect, useLayoutEffect, useMemo, useState, type ReactNode } from "react";
import { Popover } from "radix-ui";
import { Check, ListFilter, Plus, X } from "lucide-react";
import type { Category, FeedQuery, TagCount } from "../lib/ipc";

/** Category pills wrap onto at most this many rows; the rest are in the category search. */
const CATEGORY_ROWS = 2;
/** Tags get a single row; the rest are in the tag search. */
const TAG_ROWS = 1;
/** Rows a search popover renders; typing narrows the rest. */
const SEARCH_ROWS = 50;
/** Tags measured for the tag row; one row never holds more than this. */
const TAG_CANDIDATES = 60;

const pill =
  "inline-flex shrink-0 items-center gap-1.5 rounded-full border px-3.5 py-1.5 text-[13px] leading-[1.2] font-medium transition-colors duration-100";
const pillOff = "border-line bg-surface-raised text-ink active:bg-surface";
const pillOn = "border-primary bg-primary text-on-primary";
const pillQuiet = "border-line bg-transparent text-ink-muted active:bg-surface";

type Filters = Pick<FeedQuery, "categoryIds" | "tags">;

/**
 * Filters for the feed (every selected category and tag must match): a block of category
 * pills, then one row of tags drawn from the selected categories. Each ends with a search
 * for whatever didn't fit.
 */
export function FilterBar({
  categories,
  tags,
  filters,
  onChange,
  onNewCategory,
}: {
  categories: Category[];
  /** Tags on the screenshots in the selected categories, most used first. */
  tags: TagCount[];
  filters: Filters;
  onChange: (f: Filters) => void;
  onNewCategory: () => void;
}) {
  const none = filters.categoryIds.length === 0 && filters.tags.length === 0;
  const toggleCategory = (id: number) =>
    onChange({
      ...filters,
      categoryIds: filters.categoryIds.includes(id) ? filters.categoryIds.filter((x) => x !== id) : [...filters.categoryIds, id],
    });
  const toggleTag = (tag: string) =>
    onChange({ ...filters, tags: filters.tags.includes(tag) ? filters.tags.filter((x) => x !== tag) : [...filters.tags, tag] });

  // Categories: as many as fit in CATEGORY_ROWS, in the user's order, plus any selected.
  const [catSearch, setCatSearch] = useState(false);
  const { bindRow: catRow, budget: catBudget } = useRowFit((i) => filters.categoryIds.includes(categories[i]?.id), CATEGORY_ROWS, [
    categories,
    filters.categoryIds,
  ]);
  const shownCats = catBudget === null ? categories : categories.filter((c, i) => i < catBudget! || filters.categoryIds.includes(c.id));
  const hiddenCats = categories.length - shownCats.length;

  // Tags: one row, selected first so active filters always show.
  const [tagSearch, setTagSearch] = useState(false);
  const tagList = useMemo(() => {
    const names = tags.map((t) => t.tag);
    return [...filters.tags, ...names.filter((t) => !filters.tags.includes(t))];
  }, [tags, filters.tags]);
  const tagCandidates = useMemo(() => tagList.slice(0, Math.max(TAG_CANDIDATES, filters.tags.length)), [tagList, filters.tags.length]);
  const { bindRow: tagRow, budget: tagBudget } = useRowFit((i) => filters.tags.includes(tagCandidates[i]), TAG_ROWS, [tagCandidates]);
  const shownTags = tagBudget === null ? tagCandidates : tagCandidates.filter((t, i) => i < tagBudget! || filters.tags.includes(t));
  const hiddenTags = tagList.length - shownTags.length;

  return (
    <div className="flex flex-col gap-4">
      <div ref={catRow} className="flex flex-wrap items-center gap-2" role="toolbar" aria-label="Categories">
        <button
          type="button"
          data-fit="head"
          aria-pressed={none}
          onClick={() => onChange({ categoryIds: [], tags: [] })}
          className={`${pill} ${none ? pillOn : pillOff}`}
        >
          All
        </button>
        {shownCats.map((c) => {
          const on = filters.categoryIds.includes(c.id);
          return (
            <button
              key={c.id}
              type="button"
              data-fit="item"
              aria-pressed={on}
              onClick={() => toggleCategory(c.id)}
              className={`${pill} ${on ? pillOn : pillOff}`}
            >
              <span className="color-dot h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: c.color }} aria-hidden />
              <span className="max-w-48 truncate">{c.name}</span>
              <span className={on ? "opacity-70" : "text-ink-muted"}>{c.itemCount}</span>
            </button>
          );
        })}
        <button
          type="button"
          data-fit="tail"
          onClick={onNewCategory}
          className={`${pill} border-dashed border-line-strong bg-transparent text-ink active:bg-surface`}
        >
          <Plus size={13} aria-hidden /> New category
        </button>
        <SearchPopover
          open={catSearch}
          onOpenChange={setCatSearch}
          label="Filter categories"
          placeholder="Filter categories"
          trigger={
            <>
              <ListFilter size={13} aria-hidden /> Filter
              {(catBudget === null || hiddenCats > 0) && <span className="text-ink-muted">+{catBudget === null ? 999 : hiddenCats}</span>}
            </>
          }
          items={categories.map((c) => ({
            key: String(c.id),
            text: c.name,
            on: filters.categoryIds.includes(c.id),
            count: c.itemCount,
            icon: <span className="color-dot h-2.5 w-2.5 shrink-0 rounded-full" style={{ backgroundColor: c.color }} aria-hidden />,
            toggle: () => toggleCategory(c.id),
          }))}
          noun="categories"
          onClear={filters.categoryIds.length ? () => onChange({ ...filters, categoryIds: [] }) : undefined}
        />
      </div>

      {tagList.length > 0 && (
        <div ref={tagRow} className="flex flex-wrap items-center gap-2" role="toolbar" aria-label="Tags">
          {shownTags.map((t) => {
            const on = filters.tags.includes(t);
            return (
              <button
                key={t}
                type="button"
                data-fit="item"
                aria-pressed={on}
                onClick={() => toggleTag(t)}
                className={`${pill} ${on ? pillOn : pillQuiet}`}
              >
                <span className={on ? "opacity-70" : ""}>#</span>
                <span className="max-w-48 truncate">{t}</span>
              </button>
            );
          })}
          <SearchPopover
            open={tagSearch}
            onOpenChange={setTagSearch}
            label="Filter tags"
            placeholder="Filter tags"
            trigger={
              <>
                <ListFilter size={13} aria-hidden /> Filter
                {(tagBudget === null || hiddenTags > 0) && (
                  <span className="text-ink-muted">+{tagBudget === null ? "9,999" : hiddenTags.toLocaleString()}</span>
                )}
              </>
            }
            items={tagList.map((t) => ({
              key: t,
              text: t,
              on: filters.tags.includes(t),
              count: tags.find((x) => x.tag === t)?.count ?? 0,
              icon: <span className="text-ink-muted">#</span>,
              toggle: () => toggleTag(t),
            }))}
            noun="tags"
            match={(text, q) => text.includes(q.replace(/^#/, ""))}
            onClear={filters.tags.length ? () => onChange({ ...filters, tags: [] }) : undefined}
          />
        </div>
      )}
    </div>
  );
}

/**
 * How many optional pills (`data-fit="item"`, in order) a wrapping toolbar can show within
 * `maxRows`, next to its fixed pills (`head` before, `tail` after). While the
 * budget is null the toolbar renders every candidate so they can be measured; a layout
 * effect then simulates the wrap once and settles, before anything paints. Pinned items
 * (selected filters) always show. Re-measures when the width or `deps` change.
 */
function useRowFit(pinned: (i: number) => boolean, maxRows: number, deps: unknown[]) {
  // A callback ref: the tag row mounts only once there are tags.
  const [el, bindRow] = useState<HTMLDivElement | null>(null);
  const [width, setWidth] = useState(0);
  const [budget, setBudget] = useState<number | null>(null);
  useEffect(() => {
    if (!el) return;
    const ro = new ResizeObserver(([e]) => setWidth(Math.round(e.contentRect.width)));
    ro.observe(el);
    return () => ro.disconnect();
  }, [el]);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useLayoutEffect(() => setBudget(null), [el, width, ...deps]);
  // Runs after every render but only acts while measuring (budget null), then settles.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useLayoutEffect(() => {
    if (budget !== null || !el) return;
    const kids = Array.from(el.children) as HTMLElement[];
    const widths = (kind: string) => kids.filter((k) => k.dataset.fit === kind).map((k) => k.offsetWidth);
    const items = widths("item");
    const [head, tail] = [widths("head"), widths("tail")];
    const rows = (ws: number[]) => {
      let n = 1;
      let x = 0;
      for (const w of ws) {
        if (x > 0 && x + GAP + w > el.clientWidth) {
          n++;
          x = w;
        } else x += (x > 0 ? GAP : 0) + w;
      }
      return n;
    };
    let k = items.length;
    for (; k > 0; k--) {
      const shown = items.filter((_, i) => i < k || pinned(i));
      if (rows([...head, ...shown, ...tail]) <= maxRows) break;
    }
    setBudget(k);
  });
  return { bindRow, budget };
}

/** The toolbars' gap (`gap-2`). */
const GAP = 8;

interface SearchItem {
  key: string;
  text: string;
  on: boolean;
  count: number;
  icon: ReactNode;
  toggle: () => void;
}

/** A pill that opens a searchable checklist (renders at most SEARCH_ROWS rows). */
function SearchPopover({
  open,
  onOpenChange,
  label,
  placeholder,
  trigger,
  items,
  noun,
  match = (text, q) => text.toLowerCase().includes(q),
  onClear,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  label: string;
  placeholder: string;
  trigger: ReactNode;
  items: SearchItem[];
  noun: string;
  match?: (text: string, q: string) => boolean;
  onClear?: () => void;
}) {
  const [q, setQ] = useState("");
  const query = q.trim().toLowerCase();
  const matches = useMemo(() => (query ? items.filter((i) => match(i.text, query)) : items), [items, query, match]);
  const shown = matches.slice(0, SEARCH_ROWS);

  return (
    <Popover.Root
      open={open}
      onOpenChange={(o) => {
        onOpenChange(o);
        if (!o) setQ("");
      }}
    >
      <Popover.Trigger data-fit="tail" className={`${pill} ${pillQuiet} ml-auto`} aria-label={label}>
        {trigger}
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          align="start"
          sideOffset={8}
          className="z-50 flex w-80 flex-col gap-2 rounded-[var(--radius-card)] border border-line bg-surface-raised p-3 text-sm text-ink shadow-[0_8px_24px_rgba(24,29,38,0.12)]"
        >
          <input
            autoFocus
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder={placeholder}
            aria-label={placeholder}
            className="w-full rounded-[var(--radius-input)] border border-line bg-canvas px-3 py-2 text-sm text-ink placeholder:text-ink-muted focus:border-focus focus:outline-none"
          />
          <div className="max-h-80 overflow-y-auto">
            {shown.map((i) => (
              <Row key={i.key} on={i.on} onClick={i.toggle} count={i.count}>
                {i.icon}
                <span className="truncate">{i.text}</span>
              </Row>
            ))}
            {matches.length > shown.length && (
              <p className="px-2 py-1.5 text-xs text-ink-muted">
                {(matches.length - shown.length).toLocaleString()} more {noun}, type to narrow
              </p>
            )}
            {matches.length === 0 && (
              <p className="px-2 py-3 text-ink-muted">{items.length ? `Nothing matches “${q.trim()}”.` : `No ${noun} yet.`}</p>
            )}
          </div>
          {onClear && (
            <button type="button" onClick={onClear} className="flex items-center gap-1.5 self-start px-2 py-1 text-[13px] text-link">
              <X size={13} aria-hidden /> Clear {noun}
            </button>
          )}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

function Row({ on, onClick, count, children }: { on: boolean; onClick: () => void; count: number; children: ReactNode }) {
  return (
    <button
      type="button"
      role="menuitemcheckbox"
      aria-checked={on}
      onClick={onClick}
      className="flex w-full items-center gap-2.5 rounded-[var(--radius-input)] px-2 py-1.5 text-left active:bg-surface"
    >
      <span
        className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-[4px] border ${on ? "border-primary bg-primary text-on-primary" : "border-line-strong"}`}
        aria-hidden
      >
        {on && <Check size={11} strokeWidth={3} />}
      </span>
      <span className="flex min-w-0 flex-1 items-center gap-1.5 truncate">{children}</span>
      <span className="text-xs text-ink-muted tabular-nums">{count}</span>
    </button>
  );
}
