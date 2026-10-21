import { useEffect, useState } from "react";
import {
  ApiKeyStatus,
  KEY_LABELS,
  KeyAccount,
  LLM_PROVIDERS,
  SPEECH_PROVIDERS,
  Settings,
  apiKeyStatus,
  deleteApiKey,
  getSettings,
  llmKeyAccount,
  setApiKey,
  speechKeyAccount,
  updateSettings,
} from "./lib/settings";
import { ModelStatus, formatBytes, listModels } from "./lib/transcript";
import { TemplatesSection } from "./TemplatesSection";

export function SettingsView() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [status, setStatus] = useState<string>("");
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [keys, setKeys] = useState<ApiKeyStatus>({});

  const refreshKeys = () => apiKeyStatus().then(setKeys).catch(() => setKeys({}));

  useEffect(() => {
    listModels().then(setModels).catch(() => setModels([]));
    refreshKeys();
    getSettings()
      .then(setSettings)
      .catch((e) => setStatus(`Couldn't load settings: ${e}`));
  }, []);

  if (!settings) return <section className="panel">{status || "Loading…"}</section>;

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) => setSettings({ ...settings, [key]: value });

  const save = async () => {
    try {
      setSettings(await updateSettings(settings));
      setStatus("Saved");
    } catch (e) {
      setStatus(`Couldn't save: ${e}`);
    }
  };

  const speechAccount = speechKeyAccount(settings.speechProvider);
  const llmAccount = llmKeyAccount(settings.llmProvider);

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

      <h2>AI notes</h2>
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

      <div className="actions">
        <button className="primary" onClick={save}>Save</button>
        <span className="status" role="status">{status}</span>
      </div>

      <TemplatesSection />
    </section>
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
        {label} API key
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
