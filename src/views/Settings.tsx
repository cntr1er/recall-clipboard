import { ReactNode, useEffect, useState } from "react";
import {
  Add16Regular,
  AppsList20Regular,
  CalendarClock20Regular,
  DataHistogram20Regular,
  Delete20Regular,
  Dismiss16Regular,
  Keyboard20Regular,
  Pause20Regular,
  Power20Regular,
  Save20Regular,
  ShieldCheckmark20Regular,
} from "@fluentui/react-icons";
import { getVersion } from "@tauri-apps/api/app";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { App, clearHistory, listApps } from "../api";
import { AppIcon, ConfirmDialog, Kbd, Toggle } from "../components";
import { hotkeyParts, shortcutFromEvent } from "../format";
import { useSettings } from "../hooks";

function Card({
  icon,
  title,
  description,
  children,
}: {
  icon: ReactNode;
  title: string;
  description?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="card">
      <span className="card-icon" aria-hidden>
        {icon}
      </span>
      <div className="card-text">
        <div className="card-title">{title}</div>
        {description && <div className="caption">{description}</div>}
      </div>
      <div className="card-control">{children}</div>
    </div>
  );
}

/** A number field that saves on blur or Enter, so half-typed values can't delete history. */
function NumberField({
  value,
  min,
  max,
  unit,
  label,
  onCommit,
}: {
  value: number;
  min: number;
  max: number;
  unit: string;
  label: string;
  onCommit: (v: number) => void;
}) {
  const [draft, setDraft] = useState(String(value));
  useEffect(() => {
    setDraft(String(value));
  }, [value]);
  const commit = () => {
    const n = Math.round(Number(draft));
    if (Number.isFinite(n) && n >= min && n <= max) onCommit(n);
    else setDraft(String(value));
  };
  return (
    <span className="number-field">
      <input
        className="textbox"
        type="number"
        inputMode="numeric"
        min={min}
        max={max}
        value={draft}
        aria-label={label}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
      />
      <span className="caption">{unit}</span>
    </span>
  );
}

function HotkeyRecorder({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [recording, setRecording] = useState(false);

  useEffect(() => {
    if (!recording) return;
    function onKey(e: KeyboardEvent) {
      e.preventDefault();
      if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.metaKey) return setRecording(false);
      const shortcut = shortcutFromEvent(e);
      if (shortcut) {
        setRecording(false);
        onChange(shortcut);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [recording, onChange]);

  return (
    <span className="hotkey">
      {recording ? (
        <span className="caption recording">Press a shortcut… (Esc to cancel)</span>
      ) : (
        <span className="keys">
          {hotkeyParts(value).map((k) => (
            <Kbd key={k}>{k}</Kbd>
          ))}
        </span>
      )}
      <button className="btn" onClick={() => setRecording(!recording)}>
        {recording ? "Cancel" : "Change"}
      </button>
    </span>
  );
}

function ExcludedApps({ excluded, onChange }: { excluded: string[]; onChange: (v: string[]) => void }) {
  const [apps, setApps] = useState<App[]>([]);
  const [draft, setDraft] = useState("");
  useEffect(() => {
    listApps().then(setApps, console.error);
  }, []);

  const nameOf = (exe: string) => apps.find((a) => a.exe === exe)?.name ?? exe;
  const add = () => {
    let exe = draft.trim().toLowerCase();
    if (!exe) return;
    if (!exe.endsWith(".exe")) exe += ".exe";
    if (!excluded.includes(exe)) onChange([...excluded, exe]);
    setDraft("");
  };

  return (
    <div className="card expander">
      <div className="card-row">
        <span className="card-icon" aria-hidden>
          <AppsList20Regular />
        </span>
        <div className="card-text">
          <div className="card-title">Excluded apps</div>
          <div className="caption">Copies made in these apps are never saved.</div>
        </div>
      </div>
      <ul className="excluded">
        {excluded.length === 0 && <li className="caption">No apps excluded.</li>}
        {excluded.map((exe) => (
          <li key={exe}>
            <AppIcon exe={exe} name={nameOf(exe)} />
            <span className="grow">{nameOf(exe)}</span>
            <span className="caption">{exe}</span>
            <button
              className="btn subtle icon"
              title={`Stop excluding ${nameOf(exe)}`}
              onClick={() => onChange(excluded.filter((x) => x !== exe))}
            >
              <Dismiss16Regular />
            </button>
          </li>
        ))}
      </ul>
      <form
        className="add-app"
        onSubmit={(e) => {
          e.preventDefault();
          add();
        }}
      >
        <input
          className="textbox grow"
          list="known-apps"
          value={draft}
          placeholder="App, e.g. keepassxc.exe"
          aria-label="App to exclude"
          onChange={(e) => setDraft(e.target.value)}
          spellCheck={false}
        />
        <datalist id="known-apps">
          {apps
            .filter((a) => !excluded.includes(a.exe))
            .map((a) => (
              <option key={a.exe} value={a.exe}>
                {a.name}
              </option>
            ))}
        </datalist>
        <button className="btn" type="submit" disabled={!draft.trim()}>
          <Add16Regular /> Add
        </button>
      </form>
    </div>
  );
}

export default function Settings() {
  const { settings, update, error } = useSettings();
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [cleared, setCleared] = useState(false);
  const [version, setVersion] = useState("");

  useEffect(() => {
    isEnabled().then(setAutostart, () => setAutostart(false));
    getVersion().then(setVersion);
  }, []);

  if (!settings) return null;

  const toggleAutostart = async (on: boolean) => {
    await (on ? enable() : disable()).catch(console.error);
    setAutostart(await isEnabled());
  };

  return (
    <div className="settings">
      <h1>Settings</h1>
      {error && (
        <div className="infobar error" role="alert">
          {error}
        </div>
      )}

      <h2>Shortcut</h2>
      <Card icon={<Keyboard20Regular />} title="Open clipboard history" description="Shows the popup wherever you are">
        <HotkeyRecorder value={settings.hotkey} onChange={(hotkey) => update({ hotkey })} />
      </Card>

      <h2>Recording</h2>
      <Card
        icon={<Save20Regular />}
        title="What to save"
        description="Copies of unchecked kinds are ignored and never read"
      >
        <span className="checks">
          {(
            [
              ["recordText", "Text"],
              ["recordImages", "Images"],
              ["recordFiles", "Files"],
            ] as const
          ).map(([key, label]) => (
            <label key={key} className="checkbox">
              <input
                type="checkbox"
                checked={settings[key]}
                onChange={(e) => update({ [key]: e.target.checked })}
              />
              {label}
            </label>
          ))}
        </span>
      </Card>
      <Card
        icon={<Pause20Regular />}
        title="Pause recording"
        description="New copies aren't saved while paused. Also in the tray menu."
      >
        <Toggle label="Pause recording" checked={settings.paused} onChange={(paused) => update({ paused })} />
      </Card>
      <Card icon={<Power20Regular />} title="Start with Windows" description="Run in the tray when you sign in">
        {autostart !== null && (
          <Toggle label="Start with Windows" checked={autostart} onChange={toggleAutostart} />
        )}
      </Card>

      <h2>History</h2>
      <Card
        icon={<CalendarClock20Regular />}
        title="Keep items for"
        description="Older unpinned items are deleted automatically"
      >
        <NumberField
          label="Days to keep items"
          value={settings.retentionDays}
          min={1}
          max={3650}
          unit="days"
          onCommit={(retentionDays) => update({ retentionDays })}
        />
      </Card>
      <Card
        icon={<DataHistogram20Regular />}
        title="Maximum items"
        description="Pinned items don't count and are never deleted automatically"
      >
        <NumberField
          label="Maximum items"
          value={settings.maxItems}
          min={10}
          max={100000}
          unit="items"
          onCommit={(maxItems) => update({ maxItems })}
        />
      </Card>
      <Card
        icon={<Delete20Regular />}
        title="Clear history"
        description={cleared ? "History cleared." : "Deletes everything except pinned items"}
      >
        <button className="btn" onClick={() => setConfirmClear(true)}>
          Clear
        </button>
      </Card>

      <h2>Privacy</h2>
      <Card
        icon={<ShieldCheckmark20Regular />}
        title="Password managers are skipped"
        description="Copies that apps mark as private are never read or saved. Everything else stays on this PC; nothing is ever uploaded."
      />
      <ExcludedApps excluded={settings.excludedApps} onChange={(excludedApps) => update({ excludedApps })} />

      <p className="about caption">Recall Clipboard {version} · MIT License</p>

      {confirmClear && (
        <ConfirmDialog
          title="Clear clipboard history?"
          body="This permanently deletes every item except pinned ones. It can't be undone."
          confirm="Clear history"
          onCancel={() => setConfirmClear(false)}
          onConfirm={() => {
            setConfirmClear(false);
            clearHistory().then(() => setCleared(true), console.error);
          }}
        />
      )}
    </div>
  );
}
