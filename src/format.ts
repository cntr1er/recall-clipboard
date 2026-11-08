import type { Item } from "./api";

const DAY = 86_400_000;

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
  const days = Math.ceil((startOfToday - ms) / DAY);
  if (days === 1) return "Yesterday";
  if (days < 7) return `${days}d`;
  return new Date(ms).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export type Group = "Pinned" | "Today" | "Yesterday" | "This week" | "Older";

export function groupOf(item: Item, now = Date.now()): Group {
  if (item.pinned) return "Pinned";
  const startOfToday = new Date(now).setHours(0, 0, 0, 0);
  if (item.copiedAt >= startOfToday) return "Today";
  if (item.copiedAt >= startOfToday - DAY) return "Yesterday";
  if (item.copiedAt >= startOfToday - 6 * DAY) return "This week";
  return "Older";
}

export const imageLabel = (item: Item) => `Image · ${item.width}×${item.height}`;

export const filePaths = (item: Item) => item.content.split("\n").filter(Boolean);
export const fileName = (path: string) => path.split(/[\\/]/).pop() || path;

/** "report.pdf", or "3 files · report.pdf, notes.txt, …". */
export function filesLabel(item: Item): string {
  const names = filePaths(item).map(fileName);
  return names.length === 1 ? names[0] : `${names.length} files · ${names.join(", ")}`;
}

/** The single-line label for any item. */
export function rowLabel(item: Item): string {
  if (item.kind === "image") return imageLabel(item);
  if (item.kind === "files") return filesLabel(item);
  return firstLine(item.content) || "(blank)";
}

/** "Ctrl+Shift+V" → ["Ctrl", "Shift", "V"], with "Super" shown as "Win". */
export const hotkeyParts = (hotkey: string) =>
  hotkey.split("+").map((k) => (k === "Super" ? "Win" : k));

/**
 * Turns a keydown into a shortcut string for the backend, or null while only
 * modifiers are held. Requires Ctrl, Alt or Win so plain typing can't be captured.
 */
export function shortcutFromEvent(e: KeyboardEvent): string | null {
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"];
  if (!e.ctrlKey && !e.altKey && !e.metaKey) return null;
  if (/^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/.test(e.code)) return null;
  const key = e.code.replace(/^Key(?=[A-Z]$)/, "").replace(/^Digit(?=\d$)/, "");
  return [...mods.filter(Boolean), key].join("+");
}
