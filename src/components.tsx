import { ReactNode, forwardRef, useEffect, useRef, useState } from "react";
import {
  Document16Regular,
  DocumentMultiple16Regular,
  Pin16Filled,
  Window16Regular,
} from "@fluentui/react-icons";
import { Item, iconSrc, thumbSrc } from "./api";
import { filePaths, relativeTime, rowLabel } from "./format";

/** The source app's icon, or a generic window icon if we don't have one. */
export function AppIcon({ exe, name, size = 16 }: { exe: string | null; name?: string | null; size?: number }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    setFailed(false);
  }, [exe]);
  const label = name ?? exe ?? "Unknown app";
  if (!exe || failed) {
    return <Window16Regular className="app-icon generic" style={{ fontSize: size }} aria-label={label} />;
  }
  return (
    <img
      className="app-icon"
      src={iconSrc(exe)}
      width={size}
      height={size}
      alt={label}
      title={label}
      onError={() => setFailed(true)}
    />
  );
}

interface RowProps {
  item: Item;
  selected: boolean;
  onSelect: () => void;
  onActivate: () => void;
}

/** One single-line history row, shared by the popup and the history window. */
export const ItemRow = forwardRef<HTMLLIElement, RowProps>(function ItemRow(
  { item, selected, onSelect, onActivate },
  ref,
) {
  const isImage = item.kind === "image";
  const isFiles = item.kind === "files";
  const many = isFiles && filePaths(item).length > 1;
  return (
    <li
      ref={ref}
      role="option"
      aria-selected={selected}
      className={`row${selected ? " selected" : ""}`}
      onMouseDown={(e) => e.preventDefault() /* keep focus in the search box */}
      onClick={onSelect}
      onDoubleClick={onActivate}
    >
      {item.pinned && <Pin16Filled className="row-pin" aria-label="Pinned" />}
      {isImage && <img className="row-thumb" src={thumbSrc(item.content)} alt="" />}
      {isFiles &&
        (many ? (
          <DocumentMultiple16Regular className="row-kind" aria-label="Files" />
        ) : (
          <Document16Regular className="row-kind" aria-label="File" />
        ))}
      <span className={`row-text${isImage ? " muted" : ""}`}>{rowLabel(item)}</span>
      <span className="row-meta">
        <AppIcon exe={item.sourceApp} name={item.sourceName} />
        <time>{relativeTime(item.copiedAt)}</time>
      </span>
    </li>
  );
});

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd>{children}</kbd>;
}

export function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
}) {
  return (
    <label className="toggle">
      <span className="toggle-state">{checked ? "On" : "Off"}</span>
      <input
        type="checkbox"
        role="switch"
        checked={checked}
        aria-label={label}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className="toggle-track" aria-hidden />
    </label>
  );
}

/** A modal confirmation, dismissed with Esc or Cancel. */
export function ConfirmDialog({
  title,
  body,
  confirm,
  onConfirm,
  onCancel,
}: {
  title: string;
  body: string;
  confirm: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    cancelRef.current?.focus();
  }, []);
  return (
    <div
      className="dialog-backdrop"
      onKeyDown={(e) => e.key === "Escape" && onCancel()}
      onClick={(e) => e.target === e.currentTarget && onCancel()}
    >
      <div className="dialog" role="alertdialog" aria-modal aria-labelledby="dialog-title">
        <div className="dialog-body">
          <h2 id="dialog-title">{title}</h2>
          <p>{body}</p>
        </div>
        <div className="dialog-actions">
          <button className="btn danger" onClick={onConfirm}>
            {confirm}
          </button>
          <button className="btn" ref={cancelRef} onClick={onCancel}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
