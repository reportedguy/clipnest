import type { ReactNode } from "react";

// Quiet, flat callouts: cream for things to know, coral-tinted for errors (DESIGN.md).
const tones = {
  error: "border-signature-coral/30 bg-signature-coral/8 text-ink",
  warn: "border-transparent bg-signature-cream text-[#181d26]",
} as const;

export function Notice({ tone, children }: { tone: keyof typeof tones; children: ReactNode }) {
  return (
    <p
      role={tone === "error" ? "alert" : "status"}
      className={`rounded-[var(--radius-card)] border px-5 py-4 text-sm leading-[1.4] ${tones[tone]}`}
    >
      {children}
    </p>
  );
}
