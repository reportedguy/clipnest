export function Kbd({ children }: { children: string }) {
  return (
    <kbd className="rounded-[var(--radius-input)] border border-line-strong/50 bg-canvas px-1.5 py-0.5 font-sans text-[13px] font-medium text-ink">
      {children}
    </kbd>
  );
}
