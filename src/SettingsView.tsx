import { AudioLines, CalendarClock, CalendarDays, KeyRound, Monitor, Moon, Palette, RefreshCw, Sparkles, Sun } from "lucide-react";
import { ReactNode, useEffect, useMemo, useState } from "react";
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
import { CalendarInfo, connectCalendar, listCalendars } from "./lib/calendar";
import { createAutosaver } from "./lib/meetings";
import { ACCENTS, Theme, applyAppearance } from "./lib/theme";
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
            <h2 className="section-title"><Palette size={17} aria-hidden />Appearance</h2>
            <div className="appearance-row">
              <span className="field-label" id="theme-label">Theme</span>
              <div className="segmented" role="radiogroup" aria-labelledby="theme-label">
                {(
                  [
                    { value: "light", label: "Light", icon: <Sun size={15} aria-hidden /> },
                    { value: "dark", label: "Dark", icon: <Moon size={15} aria-hidden /> },
                    { value: "system", label: "Match system", icon: <Monitor size={15} aria-hidden /> },
                  ] as { value: Theme; label: string; icon: ReactNode }[]
                ).map((t) => (
                  <button
                    key={t.value}
                    role="radio"
                    aria-checked={settings.theme === t.value}
                    className={settings.theme === t.value ? "active" : ""}
                    onClick={() => {
                      set("theme", t.value);
                      applyAppearance(t.value, settings.accent);
                    }}
                  >
                    {t.icon}
                    {t.label}
                  </button>
                ))}
              </div>
            </div>
            <div className="appearance-row">
              <span className="field-label" id="accent-label">Accent</span>
              <div className="swatches" role="radiogroup" aria-labelledby="accent-label">
                {ACCENTS.map((a) => (
                  <button
                    key={a.value}
                    role="radio"
                    aria-checked={settings.accent === a.value}
                    className={`swatch${settings.accent === a.value ? " active" : ""}`}
                    onClick={() => {
                      set("accent", a.value);
                      applyAppearance(settings.theme, a.value);
                    }}
                  >
                    <span className="swatch-dot" style={{ background: a.swatch }} aria-hidden />
                    {a.label}
                  </button>
                ))}
              </div>
            </div>
          </div>

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
            <h2 className="section-title"><CalendarDays size={17} aria-hidden />Calendar</h2>
            <CalendarSection
              settings={settings}
              set={set}
              onConnected={() => getSettings().then(setSettings).catch(() => {})}
            />
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

/** Connect the Mac's calendars and pick which ones count. Empty `calendarIds` means all. */
function CalendarSection({
  settings,
  set,
  onConnected,
}: {
  settings: Settings;
  set: <K extends keyof Settings>(key: K, value: Settings[K]) => void;
  onConnected: () => void;
}) {
  const [calendars, setCalendars] = useState<CalendarInfo[] | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    if (settings.calendarEnabled) listCalendars().then(setCalendars).catch((e) => setNotice(String(e)));
  }, [settings.calendarEnabled]);

  const connect = async () => {
    setNotice(null);
    try {
      const status = await connectCalendar();
      if (status === "fullAccess") onConnected();
      else if (status === "denied" || status === "restricted")
        setNotice("Calendar access is off. Turn on Parley in System Settings > Privacy & Security > Calendars, then connect again.");
      else setNotice("Parley needs full calendar access to read your events.");
    } catch (e) {
      setNotice(String(e));
    }
  };

  if (!settings.calendarEnabled) {
    return (
      <>
        <p className="card-text">
          Show upcoming meetings on Home and name each recording after its calendar event, with the people invited. Parley reads the
          calendars on this Mac, so Google, Outlook and iCloud accounts added in System Settings are included. Nothing is uploaded.
        </p>
        <div className="actions">
          <button className="primary" onClick={connect}>
            <CalendarDays size={15} aria-hidden />
            Connect calendar
          </button>
        </div>
        {notice && <p className="error" role="alert">{notice}</p>}
      </>
    );
  }

  const all = calendars ?? [];
  const chosen = (id: string) => settings.calendarIds.length === 0 || settings.calendarIds.includes(id);
  const toggle = (id: string, on: boolean) => {
    const current = settings.calendarIds.length === 0 ? all.map((c) => c.id) : settings.calendarIds;
    const next = on ? [...new Set([...current, id])] : current.filter((c) => c !== id);
    // Every calendar chosen is stored as "all", so calendars added later are included too.
    set("calendarIds", next.length === all.length && all.every((c) => next.includes(c.id)) ? [] : next);
  };
  const sources = [...new Set(all.map((c) => c.source))];

  return (
    <>
      <label className="check">
        <input type="checkbox" checked={settings.calendarEnabled} onChange={(e) => set("calendarEnabled", e.target.checked)} />
        <span>
          Use my calendar
          <span className="hint">Upcoming meetings on Home; recordings take the event's title and attendees.</span>
        </span>
      </label>
      <label className="check">
        <input type="checkbox" checked={settings.eventReminders} onChange={(e) => set("eventReminders", e.target.checked)} />
        <span>
          Remind me a minute before calls
          <span className="hint">For events with a Zoom, Teams, Meet or Webex link.</span>
        </span>
      </label>
      <label className="check">
        <input type="checkbox" checked={settings.autoRecord} onChange={(e) => set("autoRecord", e.target.checked)} />
        <span>
          Record calls automatically
          <span className="hint">
            When the app for a scheduled call joins, Parley starts recording, and stops 30 seconds after the call ends. Let the people on
            the call know you're recording.
          </span>
        </span>
      </label>
      {calendars === null && !notice ? (
        <p className="status">Loading calendars…</p>
      ) : (
        sources.map((source) => (
          <fieldset key={source} className="calendar-group">
            <legend>{source || "On this Mac"}</legend>
            {all
              .filter((c) => c.source === source)
              .map((c) => (
                <label key={c.id} className="check calendar-option">
                  <input type="checkbox" checked={chosen(c.id)} onChange={(e) => toggle(c.id, e.target.checked)} />
                  <span className="calendar-dot" style={{ background: c.color || "var(--muted)" }} aria-hidden />
                  <span>{c.title}</span>
                </label>
              ))}
          </fieldset>
        ))
      )}
      {notice && <p className="error" role="alert">{notice}</p>}
      <p className="hint">To stop Parley reading calendars entirely, turn it off in System Settings &gt; Privacy &amp; Security &gt; Calendars.</p>
    </>
  );
}
