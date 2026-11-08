/** First non-blank line of a clip, for single-line rows. CSS adds the "…". */
export function firstLine(text: string): string {
  return text.split(/\r?\n/).find((l) => l.trim() !== "")?.trim() ?? "";
}

/** "now", "2m", "3h", "Yesterday", "4d", or a short date. */
export function relativeTime(ms: number, now = Date.now()): string {
  const min = Math.floor((now - ms) / 60_000);
  if (min < 1) return "now";
  if (min < 60) return `${min}m`;
  const startOfToday = new Date(now).setHours(0, 0, 0, 0);
  if (ms >= startOfToday) return `${Math.floor(min / 60)}h`;
  const days = Math.ceil((startOfToday - ms) / 86_400_000);
  if (days === 1) return "Yesterday";
  if (days < 7) return `${days}d`;
  return new Date(ms).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
