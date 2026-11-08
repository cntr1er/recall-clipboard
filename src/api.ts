import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Mirrors `storage::Item` in Rust. */
export interface Item {
  id: number;
  kind: "text" | "image";
  /** The text (truncated in lists), or the image's content hash. */
  content: string;
  width: number | null;
  height: number | null;
  sourceApp: string | null;
  pinned: boolean;
  /** Unix milliseconds of the most recent copy. */
  copiedAt: number;
}

export const listItems = (query: string, limit = 500) =>
  invoke<Item[]>("list_items", { query, limit });

/** Calls `fn` whenever the history changes. Returns an unsubscribe function. */
export function onHistoryChanged(fn: () => void): () => void {
  const un = listen("history-changed", fn);
  return () => void un.then((f) => f());
}
