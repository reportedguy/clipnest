const rtf = new Intl.RelativeTimeFormat("en-IN", { numeric: "auto" });
const dateFmt = new Intl.DateTimeFormat("en-IN", { day: "numeric", month: "short", year: "numeric" });

/** "just now", "5 minutes ago", "yesterday", or a date for anything older than a week. */
export function timeAgo(ms: number, now = Date.now()): string {
  const s = Math.round((ms - now) / 1000);
  const abs = Math.abs(s);
  if (abs < 45) return "just now";
  if (abs < 3600) return rtf.format(Math.round(s / 60), "minute");
  if (abs < 86400) return rtf.format(Math.round(s / 3600), "hour");
  if (abs < 7 * 86400) return rtf.format(Math.round(s / 86400), "day");
  return dateFmt.format(ms);
}

const DAY = 86_400_000;

/** "overdue", "today", "tomorrow", "in 3 days", or a date for anything past two weeks. */
export function dueLabel(deadlineMs: number, now = Date.now()): { text: string; urgent: boolean } {
  const startOfToday = new Date(now).setHours(0, 0, 0, 0);
  const days = Math.floor((deadlineMs - startOfToday) / DAY);
  if (deadlineMs < now) return { text: "overdue", urgent: true };
  if (days <= 0) return { text: "today", urgent: true };
  if (days === 1) return { text: "tomorrow", urgent: true };
  if (days <= 14) return { text: `in ${days} days`, urgent: false };
  return { text: dateFmt.format(deadlineMs), urgent: false };
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

/** Hostname for compact link display, falling back to the raw text. */
export function shortUrl(url: string): string {
  try {
    const u = new URL(/^[a-z]+:\/\//i.test(url) ? url : `https://${url}`);
    return u.hostname.replace(/^www\./, "") + (u.pathname.length > 1 ? u.pathname : "");
  } catch {
    return url;
  }
}
