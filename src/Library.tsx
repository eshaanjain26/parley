import { useCallback, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { MeetingNotes } from "./MeetingNotes";
import { Recorder, Transcript } from "./Recorder";
import { renderMarkdown } from "./lib/enhance";
import {
  MeetingSummary,
  deleteMeeting,
  exportMarkdown,
  formatMinutes,
  formatWhen,
  listMeetings,
  meetingMarkdown,
  onStoreReady,
  printPage,
  searchMeetings,
  splitSnippet,
} from "./lib/library";
import { Meeting, getMeeting } from "./lib/meetings";

/** What the right-hand pane shows. */
type Pane = { kind: "recorder" } | { kind: "meeting"; id: string };

interface Props {
  /** Set from outside (e.g. an Ask source link) to open a meeting. */
  openRequest: { id: string; nonce: number } | null;
}

export function Library({ openRequest }: Props) {
  const [meetings, setMeetings] = useState<MeetingSummary[]>([]);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<MeetingSummary[] | null>(null);
  const [pane, setPane] = useState<Pane>({ kind: "recorder" });
  const [recordingId, setRecordingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    listMeetings()
      .then((m) => {
        setMeetings(m);
        setError(null);
      })
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    refresh();
    const sub = onStoreReady(refresh);
    return () => {
      sub.then((fn) => fn());
    };
  }, [refresh]);

  useEffect(() => {
    if (openRequest) setPane({ kind: "meeting", id: openRequest.id });
  }, [openRequest]);

  // Search as you type, after a short pause.
  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setResults(null);
      return;
    }
    const t = setTimeout(() => {
      searchMeetings(q).then(setResults).catch((e) => setError(String(e)));
    }, 200);
    return () => clearTimeout(t);
  }, [query]);

  const open = (id: string) => setPane(id === recordingId ? { kind: "recorder" } : { kind: "meeting", id });
  const shown = results ?? meetings;
  const selectedId = pane.kind === "meeting" ? pane.id : recordingId;

  return (
    <div className="library">
      <aside className="library-list" aria-label="Meetings">
        <button className="primary new-recording" onClick={() => setPane({ kind: "recorder" })}>
          {recordingId ? "● Recording" : "New recording"}
        </button>
        <input
          className="search"
          type="search"
          spellCheck={false}
          autoCorrect="off"
          autoCapitalize="off"
          placeholder="Search notes and transcripts"
          aria-label="Search meetings"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {error && <p className="error" role="alert">{error}</p>}
        <ul>
          {shown.map((m) => (
            <li key={m.id}>
              <button className={`meeting-row${selectedId === m.id ? " active" : ""}`} onClick={() => open(m.id)}>
                <span className="row-title">{m.title}</span>
                <span className="row-meta">
                  {m.id === recordingId ? "Recording now" : formatWhen(m.startedAtMs)}
                  {m.durationSecs ? ` · ${formatMinutes(m.durationSecs)}` : ""}
                  {m.hasEnhanced ? " · Enhanced" : ""}
                </span>
                {m.snippet && (
                  <span className="row-snippet">
                    {splitSnippet(m.snippet).map((p, i) => (p.hit ? <mark key={i}>{p.text}</mark> : <span key={i}>{p.text}</span>))}
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
        {shown.length === 0 && (
          <p className="list-empty">{results ? `Nothing matches "${query.trim()}".` : "Your meetings will appear here."}</p>
        )}
      </aside>

      <section className="library-pane">
        {/* The recorder stays mounted so a recording keeps running while you browse. */}
        <div hidden={pane.kind !== "recorder"} className="view">
          <Recorder
            onStarted={(id) => {
              setRecordingId(id);
              refresh();
            }}
            onFinished={(id) => {
              setRecordingId(null);
              refresh();
              setPane({ kind: "meeting", id });
            }}
          />
        </div>
        {pane.kind === "meeting" && (
          <MeetingDetail
            key={pane.id}
            id={pane.id}
            onChanged={refresh}
            onDeleted={() => {
              refresh();
              setQuery("");
              setPane({ kind: "recorder" });
            }}
          />
        )}
      </section>
    </div>
  );
}

function MeetingDetail({ id, onChanged, onDeleted }: { id: string; onChanged: () => void; onDeleted: () => void }) {
  const [meeting, setMeeting] = useState<Meeting | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [printHtml, setPrintHtml] = useState<string | null>(null);
  const [title, setTitle] = useState("");

  useEffect(() => {
    getMeeting(id)
      .then((m) => {
        if (!m) return setError("This meeting no longer exists.");
        setMeeting(m);
        setTitle(m.title);
      })
      .catch((e) => setError(String(e)));
  }, [id]);

  // Once the print view has rendered, open the print sheet (Save as PDF lives there). The
  // print document stays mounted (it's hidden on screen): the sheet renders its preview after
  // the print call returns, so removing it here would print a blank page.
  const [printRequest, setPrintRequest] = useState(0);
  useEffect(() => {
    if (printRequest === 0) return;
    const frame = requestAnimationFrame(() => {
      printPage().catch((e) => setStatus(`Couldn't open the print sheet: ${e}`));
    });
    return () => cancelAnimationFrame(frame);
  }, [printRequest]);

  if (error) return <p className="error" role="alert">{error}</p>;
  if (!meeting) return <p className="status">Loading…</p>;

  const saveMarkdown = async () => {
    try {
      const path = await exportMarkdown(id);
      setStatus(path ? `Saved to ${path}` : "");
    } catch (e) {
      setStatus(`Couldn't export: ${e}`);
    }
  };

  const savePdf = async () => {
    try {
      setPrintHtml(renderMarkdown(await meetingMarkdown(id)));
      setPrintRequest((n) => n + 1);
    } catch (e) {
      setStatus(`Couldn't prepare the PDF: ${e}`);
    }
  };

  const remove = async () => {
    try {
      await deleteMeeting(id);
      onDeleted();
    } catch (e) {
      setStatus(`Couldn't delete: ${e}`);
      setConfirmDelete(false);
    }
  };

  return (
    <div className="meeting">
      <div className="rec-bar meeting-actions">
        <div className="meeting-heading">
          <h1>{title}</h1>
          <p className="rec-summary">
          {formatWhen(meeting.startedAtMs)}
          {meeting.durationSecs ? ` · ${formatMinutes(meeting.durationSecs)}` : ""}
          {` · ${meeting.transcript.length} transcript ${meeting.transcript.length === 1 ? "line" : "lines"}`}
          </p>
        </div>
        <button onClick={saveMarkdown}>Export Markdown</button>
        <button onClick={savePdf}>Save as PDF</button>
        {confirmDelete ? (
          <>
            <button className="danger" onClick={remove}>Delete for good</button>
            <button onClick={() => setConfirmDelete(false)}>Cancel</button>
          </>
        ) : (
          <button onClick={() => setConfirmDelete(true)}>Delete</button>
        )}
      </div>
      {status && <p className="status" role="status">{status}</p>}
      <div className="meeting-body">
        <MeetingNotes
          meetingId={meeting.id}
          title={meeting.title}
          notesJson={meeting.notesJson}
          enhancedMd={meeting.enhancedMd}
          enhancedTemplateId={meeting.enhancedTemplateId}
          canEnhance
          onSaved={() => {
            onChanged();
            // Keep the header title in step with edits made in the notes editor.
            getMeeting(id).then((m) => m && setTitle(m.title)).catch(() => {});
          }}
        />
        <aside className="meeting-transcript" aria-label="Transcript">
          <h2>Transcript</h2>
          <Transcript lines={meeting.transcript} />
        </aside>
      </div>
      {/* Rendered beside the app shell, so print CSS can hide the whole shell. */}
      {printHtml !== null && createPortal(<div className="print-doc" dangerouslySetInnerHTML={{ __html: printHtml }} />, document.body)}
    </div>
  );
}
