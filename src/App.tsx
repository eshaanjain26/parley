import { Folder as FolderIcon, House, Lock, MessageCircleQuestion, Plus, Settings as SettingsIcon, Square } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import "./App.css";
import { AskView } from "./AskView";
import { FolderNameForm, FoldersProvider, useFolders } from "./Folders";
import { Home } from "./Home";
import { MeetingPage } from "./MeetingPage";
import { Onboarding } from "./Onboarding";
import { RecordingPage } from "./Recorder";
import { RecordingProvider, useRecording } from "./RecordingContext";
import { SettingsView } from "./SettingsView";
import { MeetingDetected, onMeetingDetected } from "./lib/library";
import { formatDuration } from "./lib/recording";
import { AppInfo, Settings, getAppInfo, getSettings } from "./lib/settings";
import { UpdateInfo, installUpdate, onUpdateReady } from "./lib/update";

type Route =
  | { name: "home" }
  | { name: "meeting"; id: string }
  | { name: "recording" }
  | { name: "folder"; id: string }
  | { name: "ask"; question?: string; folderId?: string; nonce: number }
  | { name: "settings" };

const NAV = [
  { name: "home", label: "Home", icon: House, shortcut: "" },
  { name: "ask", label: "Ask", icon: MessageCircleQuestion, shortcut: "⌘J" },
  { name: "settings", label: "Settings", icon: SettingsIcon, shortcut: "⌘," },
] as const;

function App() {
  const [route, setRoute] = useState<Route>({ name: "home" });
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);

  useEffect(() => {
    getAppInfo().then(setInfo).catch(() => setInfo(null));
    getSettings().then(setSettings).catch(() => setSettings(null));
  }, []);

  if (settings && !settings.onboarded) {
    return <Onboarding settings={settings} hotkey={info?.hotkey ?? "⌥⇧⌘R"} onDone={setSettings} />;
  }

  return (
    <FoldersProvider>
      <RecordingProvider
        onStarted={() => setRoute({ name: "recording" })}
        onNeedsAttention={() => setRoute({ name: "recording" })}
        onFinished={(id) => setRoute({ name: "meeting", id })}
      >
        <Shell route={route} setRoute={setRoute} info={info} />
      </RecordingProvider>
    </FoldersProvider>
  );
}

function Shell({ route, setRoute, info }: { route: Route; setRoute: (r: Route) => void; info: AppInfo | null }) {
  const rec = useRecording();
  const folders = useFolders();
  const [creatingFolder, setCreatingFolder] = useState(false);
  const [detected, setDetected] = useState<MeetingDetected | null>(null);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [focusSearch, setFocusSearch] = useState(0);
  const recording = rec.phase === "recording" || rec.phase === "stopping";

  useEffect(() => {
    const subs = [onMeetingDetected(setDetected), onUpdateReady(setUpdate)];
    return () => {
      subs.forEach((s) => s.then((fn) => fn()));
    };
  }, []);

  useEffect(() => {
    if (recording) setDetected(null);
  }, [recording]);

  // Folder counts change when a recording starts in a folder or a meeting is saved.
  const { refresh: refreshFolders } = folders;
  useEffect(refreshFolders, [rec.phase, route, refreshFolders]);

  const ask = (question?: string, folderId?: string) => setRoute({ name: "ask", question, folderId, nonce: Date.now() });

  // App-wide shortcuts. ⌘N mirrors the global ⌥⇧⌘R so it works without the hotkey too.
  const keys = useRef<(e: KeyboardEvent) => void>(() => {});
  keys.current = (e) => {
    if (!e.metaKey || e.altKey || e.ctrlKey) return;
    const k = e.key.toLowerCase();
    if (k === "n") rec.toggle();
    else if (k === "k") {
      setRoute({ name: "home" });
      setFocusSearch(Date.now());
    } else if (k === "j") ask();
    else if (k === ",") setRoute({ name: "settings" });
    else return;
    e.preventDefault();
  };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => keys.current(e);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const active = route.name === "meeting" || route.name === "recording" ? "home" : route.name;
  const openFolder = route.name === "folder" ? folders.byId(route.id) : undefined;

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark" aria-hidden>P</span>
          <span className="brand-name">Parley</span>
        </div>
        <nav aria-label="Main">
          {NAV.map(({ name, label, icon: Icon, shortcut }) => (
            <button
              key={name}
              className={`nav-item${active === name ? " active" : ""}`}
              aria-current={active === name ? "page" : undefined}
              onClick={() => (name === "ask" ? ask() : setRoute({ name }))}
            >
              <Icon size={17} aria-hidden />
              <span>{label}</span>
              {shortcut && <kbd className="nav-kbd" aria-hidden>{shortcut}</kbd>}
            </button>
          ))}
        </nav>

        <section className="folders" aria-labelledby="folders-label">
          <div className="folders-head">
            <h2 id="folders-label" className="panel-label">Folders</h2>
            <button className="icon-button small" aria-label="New folder" onClick={() => setCreatingFolder(true)}>
              <Plus size={15} aria-hidden />
            </button>
          </div>
          {creatingFolder && (
            <FolderNameForm
              submitLabel="Create folder"
              onCancel={() => setCreatingFolder(false)}
              onSubmit={async (name) => {
                const folder = await folders.create(name);
                setCreatingFolder(false);
                setRoute({ name: "folder", id: folder.id });
              }}
            />
          )}
          {folders.folders.length === 0 && !creatingFolder ? (
            <p className="folders-empty">Group meetings by client, team or project.</p>
          ) : (
            <ul>
              {folders.folders.map((f) => {
                const current = route.name === "folder" && route.id === f.id;
                return (
                  <li key={f.id}>
                    <button
                      className={`nav-item folder-item${current ? " active" : ""}`}
                      aria-current={current ? "page" : undefined}
                      onClick={() => setRoute({ name: "folder", id: f.id })}
                    >
                      <FolderIcon size={16} aria-hidden />
                      <span>{f.name}</span>
                      <span className="count" aria-label={`${f.meetingCount} meetings`}>{f.meetingCount}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </section>

        {recording && (
          <div className="rec-pill" role="status">
            <button className="rec-pill-open" onClick={() => setRoute({ name: "recording" })}>
              <span className="rec-dot" aria-hidden />
              <span>Recording</span>
              <span className="tabular">{formatDuration(rec.levels.seconds)}</span>
            </button>
            <button className="icon-button small" aria-label="Stop recording" onClick={() => void rec.stop()} disabled={rec.phase === "stopping"}>
              <Square size={12} aria-hidden fill="currentColor" />
            </button>
          </div>
        )}

        <div className="sidebar-foot">
          {update && <UpdateNotice update={update} />}
          <p className="privacy-cue">
            <Lock size={13} aria-hidden />
            Encrypted on this Mac
          </p>
          <div className="version">{info ? `Parley ${info.version}` : ""}</div>
        </div>
      </aside>

      <main className="content">
        {route.name === "folder" && openFolder && (
          <Home
            key={openFolder.id}
            folder={openFolder}
            onFolderDeleted={() => setRoute({ name: "home" })}
            hotkey={info?.hotkey ?? "⌥⇧⌘R"}
            detected={detected}
            onDismissDetected={() => setDetected(null)}
            onOpenMeeting={(id) => setRoute({ name: "meeting", id })}
            onOpenRecording={() => setRoute({ name: "recording" })}
            onOpenSettings={() => setRoute({ name: "settings" })}
            onAsk={(q) => ask(q, openFolder.id)}
            focusSearch={focusSearch}
          />
        )}
        {route.name === "home" && (
          <Home
            hotkey={info?.hotkey ?? "⌥⇧⌘R"}
            detected={detected}
            onDismissDetected={() => setDetected(null)}
            onOpenMeeting={(id) => setRoute({ name: "meeting", id })}
            onOpenRecording={() => setRoute({ name: "recording" })}
            onOpenSettings={() => setRoute({ name: "settings" })}
            onAsk={ask}
            focusSearch={focusSearch}
          />
        )}
        {route.name === "meeting" && (
          <MeetingPage
            key={route.id}
            id={route.id}
            onBack={() => setRoute({ name: "home" })}
            onDeleted={() => setRoute({ name: "home" })}
            onAsk={ask}
          />
        )}
        {/* Stays mounted while recording so the notes editor keeps its state across views. */}
        {(route.name === "recording" || recording) && (
          <div className="view" hidden={route.name !== "recording"}>
            <RecordingPage onBack={() => setRoute({ name: "home" })} />
          </div>
        )}
        {route.name === "ask" && (
          <AskView
            key={route.nonce}
            initialQuestion={route.question}
            folder={folders.byId(route.folderId)}
            onClearFolder={() => setRoute({ name: "ask", nonce: Date.now() })}
            onOpenMeeting={(id) => setRoute({ name: "meeting", id })}
          />
        )}
        {route.name === "settings" && <SettingsView />}
      </main>
    </div>
  );
}

function UpdateNotice({ update }: { update: UpdateInfo }) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const restart = async () => {
    setBusy(true);
    setError(null);
    try {
      await installUpdate();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <div className="update-notice" role="status">
      <p>Parley {update.version} is ready.</p>
      <button className="primary" onClick={restart} disabled={busy}>
        {busy ? "Updating…" : "Restart to update"}
      </button>
      {error && <p className="error">{error}</p>}
    </div>
  );
}

export default App;
