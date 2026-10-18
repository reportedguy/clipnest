import { useEffect, useRef } from "react";
import { Search, X } from "lucide-react";

/** Global search box. Ctrl+K focuses it from anywhere; Esc clears it. */
export function SearchBar({ value, onChange, autoFocus }: { value: string; onChange: (v: string) => void; autoFocus?: boolean }) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        input.current?.focus();
        input.current?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="relative w-full max-w-lg">
      <Search size={15} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-ink-muted" aria-hidden />
      <input
        ref={input}
        autoFocus={autoFocus}
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => e.key === "Escape" && onChange("")}
        placeholder="Search everything (Ctrl+K)"
        aria-label="Search screenshots"
        className="h-10 w-full rounded-[var(--radius-input)] border border-line bg-surface pr-8 pl-9 text-sm text-ink placeholder:text-ink-muted focus:border-focus focus:bg-canvas focus:outline-none [&::-webkit-search-cancel-button]:hidden"
      />
      {value && (
        <button
          type="button"
          aria-label="Clear search"
          onClick={() => onChange("")}
          className="absolute top-1/2 right-2 -translate-y-1/2 text-ink-muted"
        >
          <X size={15} />
        </button>
      )}
    </div>
  );
}
