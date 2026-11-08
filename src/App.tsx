import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Item, listItems, onHistoryChanged } from "./api";
import { firstLine, relativeTime } from "./format";

// One bundle serves every window; the window label picks the view.
const label = getCurrentWindow().label;

function History() {
  const [items, setItems] = useState<Item[]>([]);

  useEffect(() => {
    const load = () => listItems("").then(setItems);
    load();
    return onHistoryChanged(load);
  }, []);

  if (items.length === 0) {
    return <p>Nothing copied yet. Copy something and it'll show up here.</p>;
  }
  return (
    <ul style={{ listStyle: "none", padding: 0 }}>
      {items.map((item) => (
        <li key={item.id} style={{ display: "flex", gap: 12, padding: "6px 0" }}>
          <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {firstLine(item.content)}
          </span>
          <span style={{ opacity: 0.6 }}>{relativeTime(item.copiedAt)}</span>
        </li>
      ))}
    </ul>
  );
}

export default function App() {
  return (
    <main style={{ fontFamily: "Segoe UI Variable, Segoe UI, sans-serif", padding: 24 }}>
      {label === "settings" ? <h1>Settings</h1> : <History />}
    </main>
  );
}
