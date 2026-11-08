import { Fragment, useEffect, useRef, useState } from "react";
import {
  Copy20Regular,
  Delete20Regular,
  Pin20Filled,
  Pin20Regular,
  Search16Regular,
  Settings20Regular,
} from "@fluentui/react-icons";
import {
  App,
  Item,
  copyItem,
  deleteItem,
  getItem,
  imageSrc,
  listApps,
  on,
  openWindow,
  setPinned,
} from "../api";
import { AppIcon, ItemRow } from "../components";
import { firstLine, groupOf } from "../format";
import { useItems, useMinuteTick } from "../hooks";

type Kind = "all" | "text" | "image" | "pinned";
const KINDS: [Kind, string][] = [
  ["all", "All"],
  ["text", "Text"],
  ["image", "Images"],
  ["pinned", "Pinned"],
];

/** The full history window (tray → Open). */
export default function History() {
  useMinuteTick();
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<Kind>("all");
  const [source, setSource] = useState<string>("");
  const [apps, setApps] = useState<App[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const rows = useRef(new Map<number, HTMLLIElement>());

  const items = useItems({
    query,
    kind: kind === "text" || kind === "image" ? kind : null,
    pinnedOnly: kind === "pinned",
    sourceApp: source || null,
    limit: 2000,
  });

  useEffect(() => {
    const load = () => listApps().then(setApps, console.error);
    load();
    return on("history-changed", load);
  }, []);

  // Keep a valid selection as the list changes.
  const index = items?.findIndex((i) => i.id === selectedId) ?? -1;
  useEffect(() => {
    if (items && index === -1) setSelectedId(items[0]?.id ?? null);
  }, [items, index]);
  useEffect(() => {
    if (selectedId !== null) rows.current.get(selectedId)?.scrollIntoView({ block: "nearest" });
  }, [selectedId]);

  const selected = index >= 0 ? items![index] : null;

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (!items?.length || document.querySelector(".dialog-backdrop")) return;
      const move = (d: number) => {
        e.preventDefault();
        const next = Math.min(Math.max(index + d, 0), items.length - 1);
        setSelectedId(items[next].id);
      };
      const typing = e.target instanceof HTMLInputElement && e.target.type === "text";
      if (e.key === "ArrowDown") move(1);
      else if (e.key === "ArrowUp") move(-1);
      else if (!selected) return;
      else if (e.ctrlKey && e.key.toLowerCase() === "p") {
        e.preventDefault();
        setPinned(selected.id, !selected.pinned);
      } else if (e.key === "Delete" && !typing) deleteItem(selected.id);
      else if (e.key === "Enter" && !(e.target instanceof HTMLButtonElement)) copyItem(selected.id);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [items, index, selected]);

  return (
    <div className="history">
      <header className="toolbar">
        <h1>Clipboard history</h1>
        <div className="search grow">
          <Search16Regular className="search-icon" />
          <input
            value={query}
            placeholder="Search"
            aria-label="Search clipboard history"
            onChange={(e) => setQuery(e.target.value)}
            spellCheck={false}
          />
        </div>
        <button className="btn subtle icon" title="Settings" onClick={() => openWindow("settings")}>
          <Settings20Regular />
        </button>
      </header>

      <div className="filters">
        <div className="segmented" role="tablist" aria-label="Show">
          {KINDS.map(([k, label]) => (
            <button key={k} role="tab" aria-selected={kind === k} onClick={() => setKind(k)}>
              {label}
            </button>
          ))}
        </div>
        <select
          className="select"
          value={source}
          onChange={(e) => setSource(e.target.value)}
          aria-label="Filter by app"
        >
          <option value="">All apps</option>
          {apps.map((a) => (
            <option key={a.exe} value={a.exe}>
              {a.name}
            </option>
          ))}
        </select>
      </div>

      <div className="split">
        <section className="pane list-pane">
          {items && items.length === 0 ? (
            <div className="empty">
              {query || kind !== "all" || source
                ? "Nothing matches these filters."
                : "Nothing copied yet. Copy something and it'll show up here."}
            </div>
          ) : (
            <ul className="list" role="listbox" aria-label="Clipboard history">
              {items?.map((item, i) => {
                const group = groupOf(item);
                const header = i === 0 || groupOf(items[i - 1]) !== group;
                return (
                  <Fragment key={item.id}>
                    {header && <li className="group-header" role="presentation">{group}</li>}
                    <ItemRow
                      ref={(el) => {
                        if (el) rows.current.set(item.id, el);
                        else rows.current.delete(item.id);
                      }}
                      item={item}
                      selected={item.id === selectedId}
                      onSelect={() => setSelectedId(item.id)}
                      onActivate={() => copyItem(item.id)}
                    />
                  </Fragment>
                );
              })}
            </ul>
          )}
        </section>

        <section className="pane preview-pane">
          {selected ? <Preview item={selected} /> : <div className="empty">Select an item to preview it.</div>}
        </section>
      </div>
    </div>
  );
}

function Preview({ item }: { item: Item }) {
  const [full, setFull] = useState<Item | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    setCopied(false);
    if (item.kind === "text") getItem(item.id).then(setFull, console.error);
  }, [item.id, item.kind]);

  const text = full?.id === item.id ? full.content : item.content;
  const copy = () =>
    copyItem(item.id).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }, console.error);

  const details = [
    item.kind === "image"
      ? `${item.width} × ${item.height}`
      : `${text.length.toLocaleString()} characters`,
    new Date(item.copiedAt).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }),
  ];

  return (
    <div className="preview">
      <div className="preview-head">
        <div className="preview-title">
          <AppIcon exe={item.sourceApp} name={item.sourceName} size={20} />
          <div>
            <div className="preview-name">
              {item.kind === "image" ? "Image" : firstLine(text).slice(0, 80) || "Text"}
            </div>
            <div className="caption">
              {item.sourceName ?? "Unknown app"} · {details.join(" · ")}
            </div>
          </div>
        </div>
        <div className="preview-actions">
          <button className="btn accent" onClick={copy}>
            <Copy20Regular /> {copied ? "Copied" : "Copy"}
          </button>
          <button
            className="btn"
            aria-pressed={item.pinned}
            onClick={() => setPinned(item.id, !item.pinned)}
          >
            {item.pinned ? <Pin20Filled /> : <Pin20Regular />} {item.pinned ? "Unpin" : "Pin"}
          </button>
          <button className="btn icon" title="Delete" onClick={() => deleteItem(item.id)}>
            <Delete20Regular />
          </button>
        </div>
      </div>
      <div className="preview-body">
        {item.kind === "image" ? (
          <div className="checker">
            <img src={imageSrc(item.content)} alt={`Copied image, ${item.width} by ${item.height}`} />
          </div>
        ) : (
          <pre>{text}</pre>
        )}
      </div>
    </div>
  );
}
