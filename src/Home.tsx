import { Ellipsis, FileText, Folder as FolderIcon, Mic, Pencil, Search, Sparkles, Square, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { AskBar } from "./AskBar";
import { ComingUp } from "./ComingUp";
import { EmptyArt } from "./EmptyArt";
import { FolderNameForm, useFolders } from "./Folders";
import { Menu } from "./Menu";
import { useRecording } from "./RecordingContext";
import { Folder, checkFolderName, deleteFolder, renameFolder } from "./lib/folders";
import { greeting, groupByDay, longDate, timeOfDay } from "./lib/dates";
import { MeetingDetected, MeetingSummary, formatMinutes, listMeetings, onStoreReady, searchMeetings, splitSnippet } from "./lib/library";
import { formatDuration } from "./lib/recording";

interface Props {
  hotkey: string;
  detected: MeetingDetected | null;
  onDismissDetected: () => void;
  onOpenMeeting: (id: string) => void;
  onOpenRecording: () => void;
  onOpenSettings: () => void;
  onAsk: (question: string) => void;
  /** Changes when ⌘K asks for the search box. */
  focusSearch: number;
  /** Set on a folder's page: everything here is scoped to it. */
  folder?: Folder;
  onFolderDeleted?: () => void;
}

export function Home({
  hotkey,
  detected,
  onDismissDetected,
  onOpenMeeting,
  onOpenRecording,
  onOpenSettings,
  onAsk,
  focusSearch,
  folder,
  onFolderDeleted,
}: Props) {
  const rec = useRecording();
  const folders = useFolders();
  const folderId = folder?.id ?? null;
  const [renaming, setRenaming] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [meetings, setMeetings] = useState<MeetingSummary[] | null>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<MeetingSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const search = useRef<HTMLInputElement>(null);

  const refresh = useCallback(() => {
    listMeetings(folderId)
      .then((m) => {
        setMeetings(m);
        setError(null);
      })
      .catch((e) => setError(String(e)));
  }, [folderId]);

  useEffect(() => {
    refresh();
    const sub = onStoreReady(refresh);
    return () => {
      sub.then((fn) => fn());
    };
  }, [refresh]);

  // A recording starting or finishing changes the list.
  useEffect(refresh, [rec.phase, refresh]);

  useEffect(() => {
    if (focusSearch) search.current?.focus();
  }, [focusSearch]);

  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setResults(null);
      return;
    }
    const t = setTimeout(() => {
      searchMeetings(q, folderId).then(setResults).catch((e) => setError(String(e)));
    }, 200);
    return () => clearTimeout(t);
  }, [query, folderId]);

  const removeFolder = async () => {
    if (!folder) return;
    try {
      await deleteFolder(folder.id);
      folders.refresh();
      onFolderDeleted?.();
    } catch (e) {
      setError(String(e));
      setConfirmDelete(false);
    }
  };

  const shown = results ?? meetings ?? [];
  const groups = groupByDay(shown);
  const recordingId = rec.phase === "recording" || rec.phase === "stopping" ? rec.meeting?.id : undefined;
  const now = new Date();

  return (
    <div className="page home">
      <div className="page-scroll">
        {folder ? (
          <header className="page-header folder-header">
            <p className="eyebrow">
              <FolderIcon size={14} aria-hidden />
              Folder · {folder.meetingCount} {folder.meetingCount === 1 ? "meeting" : "meetings"}
            </p>
            {renaming ? (
              <FolderNameForm
                initial={folder.name}
                submitLabel="Rename"
                onCancel={() => setRenaming(false)}
                onSubmit={async (name) => {
                  const checked = checkFolderName(name, folders.folders, folder.id);
                  if ("error" in checked) throw checked.error;
                  await renameFolder(folder.id, checked.name);
                  folders.refresh();
                  setRenaming(false);
                }}
              />
            ) : (
              <div className="title-row">
                <h1 className="display">{folder.name}</h1>
                <Menu
                  label="Folder actions"
                  align="end"
                  triggerClassName="icon-button"
                  trigger={<Ellipsis size={18} aria-hidden />}
                  items={[
                    { label: "Rename folder", icon: <Pencil size={16} />, onSelect: () => setRenaming(true) },
                    "separator",
                    { label: "Delete folder", icon: <Trash2 size={16} />, danger: true, onSelect: () => setConfirmDelete(true) },
                  ]}
                />
              </div>
            )}
            {confirmDelete && (
              <div className="confirm inline" role="alertdialog" aria-labelledby="folder-confirm">
                <Trash2 size={18} aria-hidden />
                <p id="folder-confirm">Delete the folder “{folder.name}”? Its meetings stay in your library, just without a folder.</p>
                <button className="secondary" onClick={() => setConfirmDelete(false)} autoFocus>Cancel</button>
                <button className="danger" onClick={removeFolder}>Delete folder</button>
              </div>
            )}
          </header>
        ) : (
          <header className="page-header">
            <p className="eyebrow">{longDate(now)}</p>
            <h1 className="display">{greeting(now)}</h1>
          </header>
        )}

        {recordingId ? (
          <section className="hero-card recording" aria-label="Recording in progress">
            <span className="rec-dot" aria-hidden />
            <div className="hero-text">
              <strong>Recording {rec.meeting?.title}</strong>
              <span className="muted tabular">{formatDuration(rec.levels.seconds)}</span>
            </div>
            <button className="secondary" onClick={onOpenRecording}>Open notes</button>
            <button className="primary" onClick={() => void rec.stop()} disabled={rec.phase === "stopping"}>
              <Square size={14} aria-hidden fill="currentColor" />
              {rec.phase === "stopping" ? "Finishing…" : "Stop"}
            </button>
          </section>
        ) : detected ? (
          <section className="hero-card detected" aria-label="Meeting detected">
            <span className="hero-icon" aria-hidden><Mic size={20} /></span>
            <div className="hero-text">
              <strong>{detected.appName} is using your microphone</strong>
              <span className="muted">Taking notes on this call? Let the others know you're recording.</span>
            </div>
            <button className="icon-button" aria-label="Not now" onClick={onDismissDetected}>
              <X size={16} aria-hidden />
            </button>
            <button
              className="primary"
              onClick={() => {
                onDismissDetected();
                void rec.start(folderId);
              }}
              disabled={rec.phase === "starting"}
            >
              <Mic size={16} aria-hidden />
              Start recording
            </button>
          </section>
        ) : (
          <section className="hero-card" aria-label="New recording">
            <span className="hero-icon" aria-hidden><Mic size={20} /></span>
            <div className="hero-text">
              <strong>{folder ? `Record a meeting in ${folder.name}` : "Ready for your next meeting"}</strong>
              <span className="muted">
                Start when the call begins, or press <kbd>{hotkey}</kbd> from any app.
              </span>
            </div>
            <button className="primary" onClick={() => void rec.start(folderId)} disabled={rec.phase === "starting"}>
              <Mic size={16} aria-hidden />
              {rec.phase === "starting" ? "Starting…" : "Start recording"}
            </button>
          </section>
        )}

        {!folder && <ComingUp onOpenSettings={onOpenSettings} />}

        <div className="search-field">
          <Search size={16} aria-hidden />
          <input
            ref={search}
            type="search"
            spellCheck={false}
            autoCorrect="off"
            autoCapitalize="off"
            placeholder={folder ? `Search in ${folder.name}` : "Search titles, notes and transcripts"}
            aria-label="Search meetings"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setQuery("")}
          />
          {!query && <kbd aria-hidden>/</kbd>}
        </div>

        {error && <p className="error" role="alert">{error}</p>}

        {meetings === null && !error ? (
          <div className="skeleton-list" aria-label="Loading meetings">
            {[0, 1, 2].map((i) => <div key={i} className="skeleton-row" />)}
          </div>
        ) : groups.length === 0 ? (
          <div className="empty-state">
            {results ? (
              <p>Nothing matches “{query.trim()}”. Try a name, a topic or a phrase someone said.</p>
            ) : folder ? (
              <>
                <EmptyArt />
                <p className="empty-title">Nothing in {folder.name} yet</p>
                <p className="muted">Record here, or open a meeting and pick this folder from its folder chip.</p>
              </>
            ) : (
              <>
                <EmptyArt />
                <p className="empty-title">Your meetings will live here</p>
                <p className="muted">Start a recording when a call begins. Notes, transcript and AI summary land in this list.</p>
              </>
            )}
          </div>
        ) : (
          groups.map((g, gi) => (
            <section key={g.key} className="day-group" style={{ animationDelay: `${Math.min(gi, 5) * 50}ms` }}>
              <h2 className="day-label">{g.label}</h2>
              <ul className="meeting-list">
                {g.items.map((m) => {
                  const live = m.id === recordingId;
                  const sub = [!folder && folders.byId(m.folderId)?.name, m.hasEnhanced && "AI notes ready"].filter(Boolean).join(" · ");
                  return (
                    <li key={m.id}>
                      <button className="meeting-row" onClick={() => (live ? onOpenRecording() : onOpenMeeting(m.id))}>
                        <span className={`row-icon${m.hasEnhanced ? " enhanced" : ""}`} aria-hidden>
                          {live ? <span className="rec-dot" /> : m.hasEnhanced ? <Sparkles size={16} /> : <FileText size={16} />}
                        </span>
                        <span className="row-main">
                          <span className="row-title">{m.title}</span>
                          {m.snippet ? (
                            <span className="row-snippet">
                              {splitSnippet(m.snippet).map((p, i) => (p.hit ? <mark key={i}>{p.text}</mark> : <span key={i}>{p.text}</span>))}
                            </span>
                          ) : (
                            sub && <span className="row-sub">{sub}</span>
                          )}
                        </span>
                        <span className="row-meta">
                          {live ? "Recording now" : timeOfDay(m.startedAtMs)}
                          {!live && m.durationSecs ? <span className="muted"> · {formatMinutes(m.durationSecs)}</span> : null}
                        </span>
                      </button>
                    </li>
                  );
                })}
              </ul>
            </section>
          ))
        )}
      </div>
      <AskBar onAsk={onAsk} placeholder={folder ? `Ask about ${folder.name}` : undefined} />
    </div>
  );
}
