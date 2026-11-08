import { getCurrentWindow } from "@tauri-apps/api/window";

// One bundle serves every window; the window label picks the view.
const label = getCurrentWindow().label;

export default function App() {
  return (
    <main style={{ fontFamily: "Segoe UI Variable, Segoe UI, sans-serif", padding: 24 }}>
      <h1>{label === "settings" ? "Settings" : "Recall Clipboard"}</h1>
      <p>Coming soon.</p>
    </main>
  );
}
