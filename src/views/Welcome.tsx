import { useState } from "react";
import { ShieldCheckmark20Regular } from "@fluentui/react-icons";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { disable, enable } from "@tauri-apps/plugin-autostart";
import { Kbd } from "../components";
import { hotkeyParts } from "../format";
import { useSettings } from "../hooks";
import { Logo } from "../Logo";

/** Shown once, on first launch. */
export default function Welcome() {
  const { settings, update } = useSettings();
  const [startWithWindows, setStartWithWindows] = useState(true);

  if (!settings) return null;

  const done = async () => {
    await (startWithWindows ? enable() : disable()).catch(console.error);
    await update({ welcomed: true });
    await getCurrentWindow().hide();
  };

  return (
    <div className="welcome">
      <Logo size={56} />
      <h1>Welcome to Recall Clipboard</h1>
      <p className="lead">
        Press{" "}
        {hotkeyParts(settings.hotkey).map((k) => (
          <Kbd key={k}>{k}</Kbd>
        ))}{" "}
        anywhere to open your clipboard history.
      </p>
      <p className="privacy">
        <ShieldCheckmark20Regular />
        <span>All data stays on this PC. Copies from password managers are skipped.</span>
      </p>
      <label className="checkbox">
        <input
          type="checkbox"
          checked={startWithWindows}
          onChange={(e) => setStartWithWindows(e.target.checked)}
        />
        Start with Windows
      </label>
      <button className="btn accent wide" autoFocus onClick={done}>
        Got it
      </button>
    </div>
  );
}
