import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { accentColor, hasMica, initPaths } from "./api";
import History from "./views/History";
import Popup from "./views/Popup";
import Settings from "./views/Settings";
import Welcome from "./views/Welcome";
import "./styles.css";

// One bundle serves every window; the window label picks the view.
const views: Record<string, () => React.JSX.Element | null> = {
  main: History,
  popup: Popup,
  settings: Settings,
  welcome: Welcome,
};

/** Pulls the Windows accent color into CSS; re-run on focus to follow changes. */
async function applyAccent() {
  const accent = await accentColor().catch(() => null);
  if (!accent) return;
  const root = document.documentElement.style;
  root.setProperty("--accent-light-theme", accent.lightTheme);
  root.setProperty("--accent-dark-theme", accent.darkTheme);
}

async function start() {
  const label = getCurrentWindow().label;
  document.documentElement.dataset.window = label;
  // Never let a failed setup call stop the window from rendering.
  await Promise.allSettled([
    initPaths(),
    applyAccent(),
    hasMica().then((ok) => !ok && document.documentElement.classList.add("no-mica")),
  ]).then((results) =>
    results.forEach((r) => r.status === "rejected" && console.error(r.reason)),
  );
  window.addEventListener("focus", applyAccent);
  // No browser context menu: this is an app, not a page.
  window.addEventListener("contextmenu", (e) => {
    if (!(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) e.preventDefault();
  });

  const View = views[label] ?? History;
  ReactDOM.createRoot(document.getElementById("root")!).render(
    <React.StrictMode>
      <View />
    </React.StrictMode>,
  );
}

start();
