import { useCallback, useEffect, useRef, useState } from "react";
import { formatHotkey, ipc, type Card, type FeedQuery, type FilterOptions, type TagCount } from "../lib/ipc";
import { LIBRARY_CHANGED, useTauriEvent } from "../lib/events";
import { nextColor } from "../lib/palette";
import { CardGrid } from "../components/CardGrid";
import { DeadlineStrip } from "../components/DeadlineStrip";
import { FilterBar } from "../components/FilterBar";
import { NewCategoryDialog, useOfferRecheck } from "../components/NewCategoryDialog";
import { Kbd } from "../components/Kbd";

const PAGE = 60;

type Filters = Pick<FeedQuery, "categoryIds" | "tags">;

/**
 * The home screen: every screenshot, newest first, filtered by category/tag pills (all
 * selected must match) and the header's text search. Loads more as you scroll.
 */
export function Feed({
  text,
  activeHotkey,
  showSummary,
  onOpenCard,
}: {
  text: string;
  activeHotkey: string | null;
  showSummary: boolean;
  onOpenCard: (card: Card) => void;
}) {
  const [filters, setFilters] = useState<Filters>({ categoryIds: [], tags: [] });
  const [options, setOptions] = useState<FilterOptions>({ categories: [], tags: [] });
  const [cards, setCards] = useState<Card[] | null>(null);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [tags, setTags] = useState<TagCount[]>([]);
  const [creating, setCreating] = useState(false);
  const shown = useRef(PAGE);
  const sentinel = useRef<HTMLDivElement>(null);
  const offerRecheck = useOfferRecheck();

  const query: FeedQuery = { ...filters, text };
  const queryKey = JSON.stringify(query);

  const load = useCallback(() => {
    ipc.feed(JSON.parse(queryKey) as FeedQuery, 0, shown.current).then(
      (page) => {
        setCards(page.cards);
        setTotal(page.total);
        setError(null);
      },
      (e) => setError(String(e)),
    );
  }, [queryKey]);
  const loadOptions = useCallback(() => void ipc.filterOptions().then(setOptions, () => {}), []);
  // The tag row follows the selected categories.
  const catKey = JSON.stringify(filters.categoryIds);
  const loadTags = useCallback(() => void ipc.tagOptions(JSON.parse(catKey) as number[]).then(setTags, () => {}), [catKey]);

  // New filters start from the top.
  useEffect(() => {
    shown.current = PAGE;
    load();
  }, [load]);
  useEffect(loadOptions, [loadOptions]);
  useEffect(loadTags, [loadTags]);
  useTauriEvent(LIBRARY_CHANGED, () => {
    load();
    loadOptions();
    loadTags();
  });

  // Drop filters for categories/tags that no longer exist (e.g. a deleted category).
  // The options arrive asynchronously from Rust, so this can't be derived during render.
  useEffect(() => {
    const cats = new Set(options.categories.map((c) => c.id));
    if (filters.categoryIds.some((id) => !cats.has(id))) {
      setFilters((f) => ({ ...f, categoryIds: f.categoryIds.filter((id) => cats.has(id)) }));
    }
  }, [options.categories, filters.categoryIds]);

  // Load the next page when the bottom comes into view.
  const hasMore = cards !== null && cards.length < total;
  useEffect(() => {
    const el = sentinel.current;
    if (!el || !hasMore) return;
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        shown.current += PAGE;
        load();
      }
    });
    io.observe(el);
    return () => io.disconnect();
  }, [hasMore, load]);

  const filtered = filters.categoryIds.length + filters.tags.length > 0 || text.trim() !== "";
  const libraryEmpty = !filtered && cards?.length === 0;

  return (
    <div className="flex flex-col gap-6">
      {/* Reminders sit above the filters (and stay put while pills are toggled, so the
          pills don't jump); a text search hides them. */}
      {text.trim() === "" && <DeadlineStrip onOpen={onOpenCard} />}

      <div className="sticky top-0 z-20 -mx-8 border-b border-line bg-canvas/95 px-8 pt-2 pb-3 backdrop-blur">
        <FilterBar
          categories={options.categories}
          tags={tags}
          filters={filters}
          onChange={setFilters}
          onNewCategory={() => setCreating(true)}
        />
      </div>

      {error && (
        <p role="alert" className="text-sm text-signature-coral">
          {error}
        </p>
      )}

      {libraryEmpty && activeHotkey && (
        <div className="rounded-[var(--radius-card)] bg-signature-cream p-6 text-[#181d26]">
          <p className="text-xl leading-[1.5]">Capture your first screenshot</p>
          <p className="mt-1 text-sm">
            Press <Kbd>{formatHotkey(activeHotkey)}</Kbd>, drag around anything on screen, add a note and press Enter.
          </p>
        </div>
      )}

      {filtered && cards && (
        <p className="text-sm text-ink-muted">
          {total === 0 ? "Nothing matches these filters." : `${total} screenshot${total === 1 ? "" : "s"} match`}
        </p>
      )}

      {cards && cards.length > 0 && <CardGrid cards={cards} categories={options.categories} showSummary={showSummary} />}
      {hasMore && <div ref={sentinel} className="h-10" aria-hidden />}

      {creating && (
        <NewCategoryDialog
          open
          defaultColor={nextColor(options.categories.map((c) => c.color))}
          onClose={() => setCreating(false)}
          onCreated={(_, name) => offerRecheck(name)}
        />
      )}
    </div>
  );
}
