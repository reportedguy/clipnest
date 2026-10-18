// Small shared building blocks. Radix supplies behavior and accessibility; styling follows
// DESIGN.md: near-black primary + white hairline secondary, 12px action radius, 6px inputs,
// weights 400/500 only, pressed (active) states but no hover styling.
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { Dialog as RDialog, Switch as RSwitch } from "radix-ui";
import { X } from "lucide-react";

type Variant = "primary" | "secondary" | "ghost" | "danger";
type Size = "md" | "sm";

const variants: Record<Variant, string> = {
  primary: "bg-primary text-on-primary active:bg-primary-active",
  secondary: "border border-line bg-surface-raised text-ink active:bg-surface",
  ghost: "text-ink-muted active:bg-surface",
  danger: "bg-signature-coral text-white active:brightness-90",
};

const sizes: Record<Size, string> = {
  md: "gap-2 px-5 py-2.5 text-[15px]",
  sm: "gap-1.5 px-3.5 py-1.5 text-sm",
};

export const Button = forwardRef<HTMLButtonElement, ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; size?: Size }>(
  function Button({ variant = "secondary", size = "md", className = "", ...props }, ref) {
    return (
      <button
        ref={ref}
        type="button"
        className={`inline-flex items-center justify-center rounded-[var(--radius-action)] leading-[1.4] font-medium transition-colors duration-100 disabled:opacity-40 ${sizes[size]} ${variants[variant]} ${className}`}
        {...props}
      />
    );
  },
);

/**
 * Icon-only button; `label` is required for screen readers and shown as a tooltip.
 * `circle` is the design system's 40px circular icon button (hairline, canvas fill);
 * `plain` is the compact borderless form for dense rows.
 */
export const IconButton = forwardRef<
  HTMLButtonElement,
  ButtonHTMLAttributes<HTMLButtonElement> & { label: string; children: ReactNode; shape?: "plain" | "circle" }
>(function IconButton({ label, className = "", children, shape = "plain", ...props }, ref) {
  const look =
    shape === "circle"
      ? "h-10 w-10 rounded-full border border-line bg-surface-raised text-ink active:bg-surface"
      : "h-8 w-8 rounded-[var(--radius-input)] text-ink-muted active:bg-surface active:text-ink";
  return (
    <button
      ref={ref}
      type="button"
      aria-label={label}
      title={label}
      className={`inline-flex shrink-0 items-center justify-center transition-colors duration-100 disabled:pointer-events-none disabled:opacity-30 ${look} ${className}`}
      {...props}
    >
      {children}
    </button>
  );
});

export function Switch({
  checked,
  onChange,
  label,
  description,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  description?: string;
}) {
  return (
    <label className="flex cursor-pointer items-center justify-between gap-4 py-3">
      <span>
        <span className="block text-sm text-ink">{label}</span>
        {description && <span className="mt-0.5 block text-[13px] text-ink-muted">{description}</span>}
      </span>
      <RSwitch.Root
        checked={checked}
        onCheckedChange={onChange}
        className="relative h-6 w-10 shrink-0 rounded-full bg-surface-strong transition-colors duration-150 data-[state=checked]:bg-primary"
      >
        <RSwitch.Thumb className="block h-5 w-5 translate-x-0.5 rounded-full bg-canvas shadow-sm transition-transform duration-150 data-[state=checked]:translate-x-[18px]" />
      </RSwitch.Root>
    </label>
  );
}

/** A centered modal with a title and close button. */
export function Modal({
  open,
  onOpenChange,
  title,
  children,
  wide = false,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  title: string;
  children: ReactNode;
  wide?: boolean;
}) {
  return (
    <RDialog.Root open={open} onOpenChange={onOpenChange}>
      <RDialog.Portal>
        <RDialog.Overlay className="fixed inset-0 z-40 bg-[#181d26]/50" />
        <RDialog.Content
          className={`fixed top-1/2 left-1/2 z-50 w-[calc(100vw-2rem)] -translate-x-1/2 -translate-y-1/2 rounded-[var(--radius-action)] border border-line bg-surface-raised p-6 text-body ${
            wide ? "max-w-2xl" : "max-w-md"
          }`}
        >
          <div className="mb-5 flex items-center justify-between">
            <RDialog.Title className="type-display text-xl">{title}</RDialog.Title>
            <RDialog.Close asChild>
              <IconButton label="Close">
                <X size={16} />
              </IconButton>
            </RDialog.Close>
          </div>
          <RDialog.Description className="sr-only">{title}</RDialog.Description>
          {children}
        </RDialog.Content>
      </RDialog.Portal>
    </RDialog.Root>
  );
}

export const inputClass =
  "w-full rounded-[var(--radius-input)] border border-line bg-canvas px-3 py-2 text-sm text-ink placeholder:text-ink-muted focus:border-focus focus:outline-none";

/** Shared look for Radix dropdown menus. Highlight follows keyboard focus as well as the pointer. */
export const menu = {
  content:
    "z-50 min-w-52 rounded-[var(--radius-card)] border border-line bg-surface-raised p-1.5 text-sm text-ink shadow-[0_8px_24px_rgba(24,29,38,0.10)]",
  item: "flex cursor-default items-center gap-2.5 rounded-[var(--radius-input)] px-2.5 py-2 outline-none select-none data-[highlighted]:bg-surface data-[disabled]:opacity-40",
  separator: "my-1.5 h-px bg-line",
};
