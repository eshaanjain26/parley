import { useEffect, useState } from "react";
import "./App.css";
import { Recorder } from "./Recorder";
import { AppInfo, getAppInfo } from "./lib/settings";
import { SettingsView } from "./SettingsView";

type View = "meetings" | "settings";

function App() {
  const [view, setView] = useState<View>("meetings");
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    getAppInfo().then(setInfo).catch(() => setInfo(null));
  }, []);

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark" aria-hidden>P</span>
          <span className="brand-name">Parley</span>
        </div>
        <nav>
          <button className={view === "meetings" ? "active" : ""} onClick={() => setView("meetings")}>
            Meetings
          </button>
          <button className={view === "settings" ? "active" : ""} onClick={() => setView("settings")}>
            Settings
          </button>
        </nav>
        <div className="version">{info ? `v${info.version}` : ""}</div>
      </aside>
      <main className="content">
        {/* Stays mounted so a recording in progress keeps its screen while Settings is open. */}
        <div hidden={view !== "meetings"} className="view">
          <Recorder />
        </div>
        {view === "settings" && <SettingsView />}
      </main>
    </div>
  );
}

export default App;
