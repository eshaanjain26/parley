import {
  Folder as FolderIcon,
  FolderPlus,
  House,
  Lock,
  MessageCircleQuestion,
  Mic,
  Monitor,
  Moon,
  PhoneOff,
  Plus,
  Search,
  Settings as SettingsIcon,
  Square,
  Sun,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import "./App.css";
import { AskView } from "./AskView";
import { Command, CommandPalette } from "./CommandPalette";
import { ToastProvider, useToast } from "./Toasts";
import { Logo } from "./Logo";
import { FolderNameForm, FoldersProvider, useFolders } from "./Folders";
import { Home } from "./Home";
import { MeetingPage } from "./MeetingPage";
import { Onboarding } from "./Onboarding";
import { RecordingPage } from "./Recorder";
import { RecordingProvider, useRecording } from "./RecordingContext";
import { SettingsView } from "./SettingsView";
import { MeetingDetected, onMeetingDetected } from "./lib/library";
import { formatDuration } from "./lib/recording";
import { AppInfo, Settings, getAppInfo, getSettings, updateSettings } from "./lib/settings";
import { UpdateInfo, installUpdate, onUpdateReady } from "./lib/update";
import { Theme, applyAppearance } from "./lib/theme";

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
    getSettings()
      .then((s) => {
        setSettings(s);
        applyAppearance(s.theme, s.accent);
      })
      .catch(() => setSettings(null));
  }, []);

  if (settings && !settings.onboarded) {
    return <Onboarding settings={settings} hotkey={info?.hotkey ?? "⌥⇧⌘R"} onDone={setSettings} />;
  }

  return (
    <ToastProvider>
      <FoldersProvider>
        <RecordingProvider
          onStarted={() => setRoute({ name: "recording" })}
          onNeedsAttention={() => setRoute({ name: "recording" })}
          onFinished={(id) => setRoute({ name: "meeting", id })}
        >
          <Shell route={route} setRoute={setRoute} info={info} />
        </RecordingProvider>
      </FoldersProvider>
    </ToastProvider>
  );
}

function Shell({ route, setRoute, info }: { route: Route; setRoute: (r: Route) => void; info: AppInfo | null }) {
  const rec = useRecording();
  const folders = useFolders();
  const [creatingFolder, setCreatingFolder] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const toast = useToast();
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
    // "/" jumps to the page's search box, unless the user is typing somewhere.
    const target = e.target as HTMLElement | null;
    const typing = target?.closest("input, textarea, select, [contenteditable='true']");
    if (e.key === "/" && !e.metaKey && !e.ctrlKey && !e.altKey && !typing) {
      if (route.name !== "home" && route.name !== "folder") setRoute({ name: "home" });
      setFocusSearch(Date.now());
      e.preventDefault();
      return;
    }
    if (!e.metaKey || e.altKey || e.ctrlKey) return;
    const k = e.key.toLowerCase();
    if (k === "n") rec.toggle();
    else if (k === "k") setPaletteOpen((o) => !o);
    else if (k === "j") ask();
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

  const setTheme = async (theme: Theme) => {
    try {
      const current = await getSettings();
      applyAppearance(theme, current.accent);
      await updateSettings({ ...current, theme });
      toast(theme === "system" ? "Theme follows your Mac" : `${theme === "dark" ? "Dark" : "Light"} theme on`);
    } catch {
      // The palette closes either way; Settings shows the saved theme.
    }
  };

  const commands: Command[] = [
    {
      id: "record",
      group: "Actions",
      label: recording ? "Stop recording" : "Start recording",
      icon: recording ? <Square size={15} /> : <Mic size={16} />,
      shortcut: "⌘N",
      run: rec.toggle,
    },
    { id: "ask", group: "Actions", label: "Ask your meetings", icon: <MessageCircleQuestion size={16} />, shortcut: "⌘J", run: () => ask() },
    {
      id: "new-folder",
      group: "Actions",
      label: "New folder",
      icon: <FolderPlus size={16} />,
      run: () => setCreatingFolder(true),
    },
    { id: "home", group: "Go to", label: "Home", icon: <House size={16} />, run: () => setRoute({ name: "home" }) },
    { id: "settings", group: "Go to", label: "Settings", icon: <SettingsIcon size={16} />, shortcut: "⌘,", run: () => setRoute({ name: "settings" }) },
    ...folders.folders.map((f) => ({
      id: `folder:${f.id}`,
      group: "Folders" as const,
      label: f.name,
      icon: <FolderIcon size={16} />,
      shortcut: `${f.meetingCount}`,
      run: () => setRoute({ name: "folder", id: f.id }),
    })),
    { id: "theme-light", group: "Appearance", label: "Light theme", icon: <Sun size={16} />, run: () => void setTheme("light") },
    { id: "theme-dark", group: "Appearance", label: "Dark theme", icon: <Moon size={16} />, run: () => void setTheme("dark") },
    { id: "theme-system", group: "Appearance", label: "Match system theme", icon: <Monitor size={16} />, run: () => void setTheme("system") },
  ];
  const openFolder = route.name === "folder" ? folders.byId(route.id) : undefined;

  return (
    <div className="shell">
      {paletteOpen && (
        <CommandPalette
          commands={commands}
          onClose={() => setPaletteOpen(false)}
          onOpenMeeting={(id) => setRoute({ name: "meeting", id })}
        />
      )}
      <aside className="sidebar">
        <div className="brand">
          <Logo size={24} />
          <span className="brand-name">Parley</span>
        </div>
        <button className="palette-trigger" onClick={() => setPaletteOpen(true)}>
          <Search size={15} aria-hidden />
          <span>Search or jump to…</span>
          <kbd aria-hidden>⌘K</kbd>
        </button>
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
                toast(`Folder “${folder.name}” created`);
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
        {rec.callEndsAt !== null && recording && <CallEndingToast endsAt={rec.callEndsAt} onKeep={rec.keep} onStop={() => void rec.stop()} />}
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
            onDeleted={() => {
              setRoute({ name: "home" });
              toast("Meeting deleted");
            }}
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

/** The call app left the call; the recording stops when the countdown ends unless the user keeps it. */
function CallEndingToast({ endsAt, onKeep, onStop }: { endsAt: number; onKeep: () => void; onStop: () => void }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);
  const seconds = Math.max(0, Math.ceil((endsAt - now) / 1000));
  return (
    <div className="toast" role="status">
      <PhoneOff size={18} aria-hidden />
      <p>
        <strong>The call ended.</strong> Recording stops in <span className="tabular">{seconds}</span> s.
      </p>
      <button className="secondary" onClick={onKeep}>Keep recording</button>
      <button className="primary" onClick={onStop}>Stop now</button>
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
