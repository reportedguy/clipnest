/** The design system's signature surfaces, so every category color is on-brand. */
export const PALETTE = ["#aa2d00", "#0a2e0e", "#181d26", "#a8d8c4", "#d9a441", "#fcab79", "#f4d35e", "#f5e9d4"];

/** A palette color not yet used by any category (cycles once all are taken). */
export function nextColor(used: string[]): string {
  const taken = new Set(used.map((c) => c.toLowerCase()));
  return PALETTE.find((c) => !taken.has(c)) ?? PALETTE[used.length % PALETTE.length];
}
