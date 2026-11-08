import { KeyboardEvent, useEffect, useRef, useState } from "react";
import { Search16Regular } from "@fluentui/react-icons";
import { deleteItem, hidePopup, on, pasteItem, setPinned } from "../api";
import { ItemRow, Kbd } from "../components";
import { useItems } from "../hooks";

/** The Ctrl+Shift+V command-palette popup. */
export default function Popup() {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const items = useItems({ query, limit: 200 });
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLUListElement>(null);
  const rows = useRef<(HTMLLIElement | null)[]>([]);

  // The window is reused: every open starts fresh with the search box focused.
  useEffect(
    () =>
      on("popup-opened", () => {
        setQuery("");
        setSelected(0);
        list.current?.scrollTo(0, 0);
        input.current?.focus();
      }),
    [],
  );

  const count = items?.length ?? 0;
  useEffect(() => {
    setSelected((s) => Math.min(s, Math.max(count - 1, 0)));
  }, [count]);
  // Block body: scrollIntoView returns a Promise in current WebView2, which React would treat as a cleanup.
  useEffect(() => {
    rows.current[selected]?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const current = items?.[selected];
  const paste = (id: number) => pasteItem(id).catch(console.error);

  function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    const move = (d: number) => {
      e.preventDefault();
      setSelected((s) => Math.min(Math.max(s + d, 0), count - 1));
    };
    if (e.key === "ArrowDown") move(1);
    else if (e.key === "ArrowUp") move(-1);
    else if (e.key === "PageDown") move(8);
    else if (e.key === "PageUp") move(-8);
    else if (e.key === "Escape") {
      e.preventDefault();
      hidePopup();
    } else if (e.key === "Enter" && current) {
      e.preventDefault();
      paste(current.id);
    } else if (e.ctrlKey && e.key.toLowerCase() === "p" && current) {
      e.preventDefault();
      setPinned(current.id, !current.pinned);
    } else if (e.key === "Delete" && current) {
      // Delete still edits the search text while there's text after the caret.
      const el = e.currentTarget;
      if (el.selectionStart === el.value.length && el.selectionEnd === el.value.length) {
        e.preventDefault();
        deleteItem(current.id);
      }
    }
  }

  return (
    <div className="popup">
      <div className="search">
        <Search16Regular className="search-icon" />
        <input
          ref={input}
          autoFocus
          value={query}
          placeholder="Search clipboard history"
          aria-label="Search clipboard history"
          aria-controls="popup-list"
          onChange={(e) => {
            setQuery(e.target.value);
            setSelected(0);
          }}
          onKeyDown={onKeyDown}
          spellCheck={false}
        />
      </div>

      {items && count === 0 ? (
        <div className="empty">
          {query ? "No matches." : "Nothing copied yet. Copy something and it'll show up here."}
        </div>
      ) : (
        <ul ref={list} className="list" id="popup-list" role="listbox" aria-label="Clipboard history">
          {items?.map((item, i) => (
            <ItemRow
              key={item.id}
              ref={(el) => {
                rows.current[i] = el;
              }}
              item={item}
              selected={i === selected}
              onSelect={() => paste(item.id)}
              onActivate={() => paste(item.id)}
            />
          ))}
        </ul>
      )}

      <footer className="hints">
        <span><Kbd>↑↓</Kbd> Move</span>
        <span><Kbd>Enter</Kbd> Paste</span>
        <span><Kbd>Ctrl+P</Kbd> Pin</span>
        <span><Kbd>Del</Kbd> Delete</span>
        <span><Kbd>Esc</Kbd> Close</span>
      </footer>
    </div>
  );
}
