import { useCallback, useEffect, useState } from "react";
import { Filter, Item, Settings, getSettings, listItems, on, setSettings } from "./api";

/** Items matching `filter`, kept fresh as the history changes. */
export function useItems(filter: Filter) {
  const [items, setItems] = useState<Item[] | null>(null);
  // Depend on the contents, so a new but equal filter object doesn't refetch.
  const key = JSON.stringify(filter);

  useEffect(() => {
    let live = true;
    const load = () =>
      listItems(filter).then((r) => live && setItems(r), console.error);
    load();
    const off = on("history-changed", load);
    return () => {
      live = false;
      off();
    };
  }, [key]);

  return items;
}

/** Current settings plus an updater that saves immediately. */
export function useSettings() {
  const [settings, set] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getSettings().then(set, console.error);
    return on<Settings>("settings-changed", set);
  }, []);

  const update = useCallback(
    async (patch: Partial<Settings>) => {
      if (!settings) return;
      try {
        set(await setSettings({ ...settings, ...patch }));
        setError(null);
      } catch (e) {
        setError(String(e));
      }
    },
    [settings],
  );

  return { settings, update, error };
}

/** Re-renders every minute so relative times stay current. */
export function useMinuteTick() {
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), 60_000);
    return () => clearInterval(id);
  }, []);
}
