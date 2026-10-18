import { describe, expect, it } from "vitest";
import { PALETTE, nextColor } from "./palette";
import { formatHotkey } from "./ipc";

describe("nextColor", () => {
  it("returns the first unused palette color, ignoring case", () => {
    expect(nextColor([])).toBe(PALETTE[0]);
    expect(nextColor([PALETTE[0].toUpperCase()])).toBe(PALETTE[1]);
  });

  it("cycles once every color is taken", () => {
    expect(nextColor([...PALETTE])).toBe(PALETTE[PALETTE.length % PALETTE.length]);
    expect(nextColor([...PALETTE, PALETTE[0]])).toBe(PALETTE[1]);
  });
});

describe("formatHotkey", () => {
  it("shows Super as the Windows key", () => {
    expect(formatHotkey("Super+Shift+J")).toBe("Win+Shift+J");
    expect(formatHotkey("Ctrl+Shift+J")).toBe("Ctrl+Shift+J");
  });
});
