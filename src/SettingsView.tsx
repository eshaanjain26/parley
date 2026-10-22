import { AudioLines, CalendarClock, KeyRound, RefreshCw, Sparkles } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import {
  ApiKeyStatus,
  KEY_LABELS,
  KeyAccount,
  LLM_PROVIDERS,
  SPEECH_PROVIDERS,
  Settings,
  apiKeyStatus,
  deleteApiKey,
  getAppInfo,
  getSettings,
  llmKeyAccount,
  setApiKey,
  speechKeyAccount,
  updateSettings,
} from "./lib/settings";
import { ModelStatus, formatBytes, listModels } from "./lib/transcript";
import { TemplatesSection } from "./TemplatesSection";
import { createAutosaver } from "./lib/meetings";
import { checkForUpdate } from "./lib/update";

export function SettingsView() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [status, setStatus] = useState<string>("");
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [keys, setKeys] = useState<ApiKeyStatus>({});
  const [hotkey, setHotkey] = useState("⌥⇧⌘R");
  const [version, setVersion] = useState("");
  const [updateStatus, setUpdateStatus] = useState("");

  const refreshKeys = () => apiKeyStatus().then(setKeys).catch(() => setKeys({}));

  // Settings save themselves shortly after each change; typing in a field saves once at the end.
  const saver = useMemo(
    () =>
      createAutosaver(
        async (next: Settings) => {
          await updateSettings(next);
          setStatus("All changes saved");
        },
        500,
        (e) => setStatus(`Couldn't save: ${e}`),
      ),
    [],
  );
  useEffect(
    () => () => {
      void saver.flush();
    },
    [saver],
  );

  useEffect(() => {
    listModels().then(setModels).catch(() => setModels([]));
    refreshKeys();
    getAppInfo()
      .then((i) => {
        setHotkey(i.hotkey);
        setVersion(i.version);
      })
      .catch(() => {});
    getSettings()
      .then(setSettings)
      .catch((e) => setStatus(`Couldn't load settings: ${e}`));
  }, []);

  if (!settings) {
    return (
      <div className="page">
        <div className="page-scroll">
          <section className="panel column">{status || "Loading…"}</section>
        </div>
      </div>
    );
  }

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    const next = { ...settings, [key]: value };
    setSettings(next);
    setStatus("Saving…");
    saver.schedule(next);
  };

  const checkUpdates = async () => {
    setUpdateStatus("Checking…");
    try {
      const update = await checkForUpdate();
      setUpdateStatus(update ? `Parley ${update.version} is ready. Use Restart to update in the sidebar.` : "Parley is up to date.");
    } catch (e) {
      setUpdateStatus(`Couldn't check for updates: ${e}`);
    }
  };

  const speechAccount = speechKeyAccount(settings.speechProvider);
  const llmAccount = llmKeyAccount(settings.llmProvider);

  return (
    <div className="page">
      <div className="page-scroll">
        <section className="panel column">
          <header className="page-header">
            <p className="eyebrow">Parley {version}</p>
            <h1 className="display">Settings</h1>
            <p className="status settings-status" role="status">{status || "Changes save automatically."}</p>
          </header>

          <div className="settings-card">
            <h2 className="section-title"><AudioLines size={17} aria-hidden />Transcription</h2>
            <label>
              Engine
              <select value={settings.speechProvider} onChange={(e) => set("speechProvider", e.target.value as Settings["speechProvider"])}>
                {SPEECH_PROVIDERS.map((p) => (
                  <option key={p.value} value={p.value}>{p.label}</option>
                ))}
              </select>
            </label>
            {speechAccount ? (
              <KeyField account={speechAccount} saved={!!keys[speechAccount]} onChange={refreshKeys} sends="audio" />
            ) : (
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

          </div>

          <div className="settings-card">
            <h2 className="section-title"><CalendarClock size={17} aria-hidden />Meetings</h2>
            <label className="check">
              <input type="checkbox" checked={settings.detectMeetings} onChange={(e) => set("detectMeetings", e.target.checked)} />
              <span>
                Notify me when a meeting starts
                <span className="hint">When Zoom, Teams, Meet, Slack or FaceTime starts using your microphone. Parley only checks which apps use the mic; it doesn't listen.</span>
              </span>
            </label>
            <label className="check">
              <input type="checkbox" checked={settings.hotkeyEnabled} onChange={(e) => set("hotkeyEnabled", e.target.checked)} />
              <span>
                Start and stop recording with {hotkey}
                <span className="hint">Works from any app. Parley also lives in the menu bar.</span>
              </span>
            </label>

          </div>

          <div className="settings-card">
            <h2 className="section-title"><RefreshCw size={17} aria-hidden />Updates</h2>
            <label className="check">
              <input type="checkbox" checked={settings.autoUpdate} onChange={(e) => set("autoUpdate", e.target.checked)} />
              <span>
                Check for updates automatically
                <span className="hint">Parley asks GitHub for the latest release shortly after launch. Updates are signed and verified before they install.</span>
              </span>
            </label>
            <div className="actions update-actions">
              <button onClick={checkUpdates}>Check for updates</button>
              <span className="status" role="status">
                {updateStatus || (version ? `You have Parley ${version}.` : "")}
              </span>
          </div>

          </div>

          <div className="settings-card">
            <h2 className="section-title"><Sparkles size={17} aria-hidden />AI notes</h2>
            <label>
              Provider
              <select value={settings.llmProvider} onChange={(e) => set("llmProvider", e.target.value as Settings["llmProvider"])}>
                {LLM_PROVIDERS.map((p) => (
                  <option key={p.value} value={p.value}>{p.label}</option>
                ))}
              </select>
            </label>
            {settings.llmProvider === "ollama" && (
              <>
                <label>
                  Ollama URL
                  <input value={settings.ollamaUrl} onChange={(e) => set("ollamaUrl", e.target.value)} />
                </label>
                <label>
                  Ollama model
                  <input value={settings.ollamaModel} onChange={(e) => set("ollamaModel", e.target.value)} />
                </label>
                <p className="hint">Runs on this Mac. Install Ollama and pull the model first, for example `ollama pull {settings.ollamaModel}`.</p>
              </>
            )}
            {settings.llmProvider === "open-ai-compatible" && (
              <>
                <label>
                  API base URL
                  <input value={settings.openaiBaseUrl} onChange={(e) => set("openaiBaseUrl", e.target.value)} />
                </label>
                <label>
                  Model
                  <input value={settings.openaiModel} placeholder="The model name your provider uses" onChange={(e) => set("openaiModel", e.target.value)} />
                </label>
              </>
            )}
            {settings.llmProvider === "anthropic" && (
              <label>
                Claude model
                <input value={settings.anthropicModel} onChange={(e) => set("anthropicModel", e.target.value)} />
              </label>
            )}
            {llmAccount && <KeyField account={llmAccount} saved={!!keys[llmAccount]} onChange={refreshKeys} sends="your notes and transcript" />}

          </div>

          <div className="settings-card">
              <TemplatesSection />
          </div>
        </section>
      </div>
    </div>
  );
}

function KeyField({ account, saved, onChange, sends }: { account: KeyAccount; saved: boolean; onChange: () => void; sends: string }) {
  const [draft, setDraft] = useState("");
  const [status, setStatus] = useState("");
  const label = KEY_LABELS[account];

  const save = async () => {
    try {
      await setApiKey(account, draft);
      setDraft("");
      onChange();
      setStatus("Saved in your Keychain");
    } catch (e) {
      setStatus(`Couldn't save the key: ${e}`);
    }
  };

  const remove = async () => {
    try {
      await deleteApiKey(account);
      onChange();
      setStatus("Key removed");
    } catch (e) {
      setStatus(`Couldn't remove the key: ${e}`);
    }
  };

  return (
    <div className="key-field">
      <label>
        <span className="label-with-icon"><KeyRound size={14} aria-hidden />{label} API key</span>
        <input
          type="password"
          autoComplete="off"
          placeholder={saved ? "A key is saved. Paste a new one to replace it." : "Paste your key"}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
        />
      </label>
      <div className="actions">
        <button onClick={save} disabled={!draft.trim()}>Save key</button>
        {saved && <button onClick={remove}>Remove key</button>}
        <span className="status" role="status">{status}</span>
      </div>
      <p className="hint">Using {label} sends {sends} to {label}. The key is stored in the macOS Keychain.</p>
    </div>
  );
}
