import { useEffect, useState } from "react";
import "./App.css";
import { Recorder } from "./Recorder";
import {
  AppInfo,
  LLM_PROVIDERS,
  SPEECH_PROVIDERS,
  Settings,
  getAppInfo,
  getSettings,
  updateSettings,
} from "./lib/settings";

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
      <main className="content">{view === "meetings" ? <Recorder /> : <SettingsView />}</main>
    </div>
  );
}

function SettingsView() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [status, setStatus] = useState<string>("");

  useEffect(() => {
    getSettings()
      .then(setSettings)
      .catch((e) => setStatus(`Couldn't load settings: ${e}`));
  }, []);

  if (!settings) return <section className="panel">{status || "Loading…"}</section>;

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) =>
    setSettings({ ...settings, [key]: value });

  const save = async () => {
    try {
      setSettings(await updateSettings(settings));
      setStatus("Saved");
    } catch (e) {
      setStatus(`Couldn't save: ${e}`);
    }
  };

  return (
    <section className="panel">
      <h1>Settings</h1>
      <h2>Transcription</h2>
      <label>
        Engine
        <select value={settings.speechProvider} onChange={(e) => set("speechProvider", e.target.value as Settings["speechProvider"])}>
          {SPEECH_PROVIDERS.map((p) => (
            <option key={p.value} value={p.value}>{p.label}</option>
          ))}
        </select>
      </label>
      <label>
        Whisper model
        <input value={settings.whisperModel} onChange={(e) => set("whisperModel", e.target.value)} />
      </label>
      <h2>AI notes</h2>
      <label>
        Provider
        <select value={settings.llmProvider} onChange={(e) => set("llmProvider", e.target.value as Settings["llmProvider"])}>
          {LLM_PROVIDERS.map((p) => (
            <option key={p.value} value={p.value}>{p.label}</option>
          ))}
        </select>
      </label>
      <label>
        Ollama URL
        <input value={settings.ollamaUrl} onChange={(e) => set("ollamaUrl", e.target.value)} />
      </label>
      <label>
        Ollama model
        <input value={settings.ollamaModel} onChange={(e) => set("ollamaModel", e.target.value)} />
      </label>
      <div className="actions">
        <button className="primary" onClick={save}>Save</button>
        <span className="status" role="status">{status}</span>
      </div>
    </section>
  );
}

export default App;
