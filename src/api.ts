import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { appDataDir, join, sep } from "@tauri-apps/api/path";

/** Mirrors `storage::Item` in Rust. */
export interface Item {
  id: number;
  kind: "text" | "image" | "files";
  /** The text (truncated in lists; use `getItem` for all of it), the image's content hash, or file paths one per line. */
  content: string;
  width: number | null;
  height: number | null;
  /** Lowercase exe name, e.g. "chrome.exe". */
  sourceApp: string | null;
  /** Friendly name, e.g. "Google Chrome". */
  sourceName: string | null;
  pinned: boolean;
  /** Unix milliseconds of the most recent copy. */
  copiedAt: number;
}

export interface Filter {
  query?: string;
  kind?: "text" | "image" | "files" | null;
  pinnedOnly?: boolean;
  sourceApp?: string | null;
  limit?: number;
}

export interface App {
  exe: string;
  name: string;
}

/** Mirrors `settings::Settings` in Rust. */
export interface Settings {
  hotkey: string;
  retentionDays: number;
  maxItems: number;
  paused: boolean;
  excludedApps: string[];
  welcomed: boolean;
  recordText: boolean;
  recordImages: boolean;
  recordFiles: boolean;
}

export const listItems = (filter: Filter) => invoke<Item[]>("list_items", { filter });
export const getItem = (id: number) => invoke<Item | null>("get_item", { id });
export const listApps = () => invoke<App[]>("list_apps");
export const setPinned = (id: number, pinned: boolean) => invoke<void>("set_pinned", { id, pinned });
export const deleteItem = (id: number) => invoke<void>("delete_item", { id });
export const clearHistory = () => invoke<void>("clear_history");
export const copyItem = (id: number) => invoke<void>("copy_item", { id });
export const pasteItem = (id: number) => invoke<void>("paste_item", { id });
export const hidePopup = () => invoke<void>("hide_popup");
export const openWindow = (label: "main" | "settings") => invoke<void>("open_window", { label });
export const getSettings = () => invoke<Settings>("get_settings");
export const setSettings = (settings: Settings) => invoke<Settings>("set_settings", { settings });
export const accentColor = () =>
  invoke<{ lightTheme: string; darkTheme: string } | null>("accent_color");
export const hasMica = () => invoke<boolean>("has_mica");

/** Subscribes to a backend event. Returns an unsubscribe function for useEffect. */
export function on<T = unknown>(event: string, fn: (payload: T) => void): () => void {
  const un = listen<T>(event, (e) => fn(e.payload));
  return () => void un.then((f) => f());
}

let imagesDir = "";
let iconsDir = "";

/** Resolves on-disk folders once at startup so image URLs can be built synchronously. */
export async function initPaths() {
  const dir = await appDataDir();
  [imagesDir, iconsDir] = await Promise.all([join(dir, "images"), join(dir, "icons")]);
}

export const imageSrc = (hash: string) => convertFileSrc(`${imagesDir}${sep()}${hash}.png`);
export const thumbSrc = (hash: string) => convertFileSrc(`${imagesDir}${sep()}${hash}.thumb.png`);
export const iconSrc = (exe: string) => convertFileSrc(`${iconsDir}${sep()}${exe}.png`);
