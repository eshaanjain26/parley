import { useEffect, useState } from "react";
import "./App.css";
import { Recorder } from "./Recorder";
import { ApiKeyStatus, apiKeyStatus, deleteApiKey, hasKeyFor, setApiKey } from "./lib/settings";
import { ModelStatus, formatBytes, listModels } from "./lib/transcript";
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
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [keys, setKeys] = useState<ApiKeyStatus | null>(null);
  const [keyDraft, setKeyDraft] = useState("");
  const [keyStatus, setKeyStatus] = useState("");

  useEffect(() => {
    listModels().then(setModels).catch(() => setModels([]));
    apiKeyStatus().then(setKeys).catch(() => setKeys(null));
  }, []);

  useEffect(() => {
    getSettings()
      .then(setSettings)
      .catch((e) => setStatus(`Couldn't load settings: ${e}`));
  }, []);

  if (!settings) return <section className="panel">{status || "Loading…"}</section>;

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) =>
    setSettings({ ...settings, [key]: value });

  const cloud = settings.speechProvider !== "local-whisper";
  const providerName = settings.speechProvider === "deepgram" ? "Deepgram" : "AssemblyAI";

  const saveKey = async () => {
    try {
      await setApiKey(settings.speechProvider, keyDraft);
      setKeyDraft("");
      setKeys(await apiKeyStatus());
      setKeyStatus("Saved in your Keychain");
    } catch (e) {
      setKeyStatus(`Couldn't save the key: ${e}`);
    }
  };

  const removeKey = async () => {
    try {
      await deleteApiKey(settings.speechProvider);
      setKeys(await apiKeyStatus());
      setKeyStatus("Key removed");
    } catch (e) {
      setKeyStatus(`Couldn't remove the key: ${e}`);
    }
  };

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
      {cloud && (
        <div className="key-field">
          <label>
            {providerName} API key
            <input
              type="password"
              autoComplete="off"
              placeholder={hasKeyFor(keys, settings.speechProvider) ? "A key is saved. Paste a new one to replace it." : "Paste your key"}
              value={keyDraft}
              onChange={(e) => setKeyDraft(e.target.value)}
            />
          </label>
          <div className="actions">
            <button onClick={saveKey} disabled={!keyDraft.trim()}>Save key</button>
            {hasKeyFor(keys, settings.speechProvider) && <button onClick={removeKey}>Remove key</button>}
            <span className="status" role="status">{keyStatus}</span>
          </div>
          <p className="hint">Audio is sent to {providerName} while you record. The key is stored in the macOS Keychain.</p>
        </div>
      )}
      {!cloud && (
        <label>
          On-device model
          <select value={settings.whisperModel} onChange={(e) => set("whisperModel", e.target.value)}>
            {models.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}, {formatBytes(m.sizeBytes)}{m.installed ? " (downloaded)" : ""}
              </option>
            ))}
          </select>
        </label>
      )}
      <label className="check">
        <input type="checkbox" checked={settings.keepAudio} onChange={(e) => set("keepAudio", e.target.checked)} />
        <span>
          Keep audio recordings
          <span className="hint">Off: audio is deleted once the transcript is saved. Transcripts and notes are always encrypted.</span>
        </span>
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
