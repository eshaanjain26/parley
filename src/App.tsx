import { useEffect, useState } from "react";
import "./App.css";
import { AskView } from "./AskView";
import { Library } from "./Library";
import { SettingsView } from "./SettingsView";
import { Onboarding } from "./Onboarding";
import { AppInfo, Settings, getAppInfo, getSettings } from "./lib/settings";

type View = "meetings" | "ask" | "settings";

const NAV: { view: View; label: string }[] = [
  { view: "meetings", label: "Meetings" },
  { view: "ask", label: "Ask" },
  { view: "settings", label: "Settings" },
];

function App() {
  const [view, setView] = useState<View>("meetings");
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [openRequest, setOpenRequest] = useState<{ id: string; nonce: number } | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);

  useEffect(() => {
    getAppInfo().then(setInfo).catch(() => setInfo(null));
    getSettings().then(setSettings).catch(() => setSettings(null));
  }, []);

  const openMeeting = (id: string) => {
    setOpenRequest({ id, nonce: Date.now() });
    setView("meetings");
  };

  if (settings && !settings.onboarded) {
    return <Onboarding settings={settings} hotkey={info?.hotkey ?? "⌥⇧⌘R"} onDone={setSettings} />;
  }

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark" aria-hidden>P</span>
          <span className="brand-name">Parley</span>
        </div>
        <nav>
          {NAV.map((n) => (
            <button key={n.view} className={view === n.view ? "active" : ""} onClick={() => setView(n.view)}>
              {n.label}
            </button>
          ))}
        </nav>
        <div className="version">{info ? `v${info.version}` : ""}</div>
      </aside>
      <main className="content">
        {/* Stays mounted so a recording in progress keeps its screen while other views are open. */}
        <div hidden={view !== "meetings"} className="view">
          <Library openRequest={openRequest} />
        </div>
        {view === "ask" && <AskView onOpenMeeting={openMeeting} />}
        {view === "settings" && <SettingsView />}
      </main>
    </div>
  );
}

export default App;
