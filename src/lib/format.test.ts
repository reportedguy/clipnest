import { describe, expect, it } from "vitest";
import { dueLabel, formatBytes, shortUrl, timeAgo } from "./format";

const HOUR = 3_600_000;
const DAY = 24 * HOUR;
// A fixed local noon, so "today" and "tomorrow" don't depend on when the tests run.
const now = new Date(2026, 9, 14, 12, 0, 0).getTime();

describe("dueLabel", () => {
  it("flags past deadlines as overdue", () => {
    expect(dueLabel(now - HOUR, now)).toEqual({ text: "overdue", urgent: true });
  });

  it("counts calendar days, not 24-hour spans", () => {
    expect(dueLabel(now + 6 * HOUR, now)).toEqual({ text: "today", urgent: true });
    expect(dueLabel(now + 13 * HOUR, now)).toEqual({ text: "tomorrow", urgent: true });
    expect(dueLabel(now + 3 * DAY, now)).toEqual({ text: "in 3 days", urgent: false });
  });

  it("shows a date beyond two weeks", () => {
    const far = dueLabel(now + 30 * DAY, now);
    expect(far.urgent).toBe(false);
    expect(far.text).toMatch(/2026/);
  });
});

describe("timeAgo", () => {
  it("rounds recent times to just now", () => {
    expect(timeAgo(now - 10_000, now)).toBe("just now");
  });

  it("uses relative units up to a week", () => {
    expect(timeAgo(now - 5 * 60_000, now)).toBe("5 minutes ago");
    expect(timeAgo(now - 3 * HOUR, now)).toBe("3 hours ago");
    expect(timeAgo(now - DAY, now)).toBe("yesterday");
  });
});

describe("formatBytes", () => {
  it("picks a readable unit", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(1_280_835_840)).toBe("1.2 GB");
    expect(formatBytes(50 * 1024 * 1024)).toBe("50 MB");
  });
});

describe("shortUrl", () => {
  it("drops the scheme and www", () => {
    expect(shortUrl("https://www.example.com/docs")).toBe("example.com/docs");
    expect(shortUrl("example.com")).toBe("example.com");
  });

  it("falls back to the raw text", () => {
    expect(shortUrl("not a url")).toBe("not a url");
  });
});
