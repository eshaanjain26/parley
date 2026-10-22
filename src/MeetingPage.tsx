import { CalendarDays, ChevronLeft, Clock, Ellipsis, FileDown, FileText, MessageSquareText, PanelRight, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { AskBar } from "./AskBar";
import { FolderChip } from "./Folders";
import { MeetingNotes } from "./MeetingNotes";
import { Menu } from "./Menu";
import { Transcript } from "./Recorder";
import { dayLabel, timeOfDay } from "./lib/dates";
import { renderMarkdown } from "./lib/enhance";
import { deleteMeeting, exportMarkdown, formatMinutes, meetingMarkdown, printPage } from "./lib/library";
import { Meeting, getMeeting } from "./lib/meetings";

interface Props {
  id: string;
  onBack: () => void;
  onDeleted: () => void;
  onAsk: (question: string) => void;
}

const TRANSCRIPT_KEY = "parley.transcriptOpen";

export function MeetingPage({ id, onBack, onDeleted, onAsk }: Props) {
  const [meeting, setMeeting] = useState<Meeting | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [printHtml, setPrintHtml] = useState<string | null>(null);
  const [printRequest, setPrintRequest] = useState(0);
  const [transcriptOpen, setTranscriptOpen] = useState(() => {
    try {
      return localStorage.getItem(TRANSCRIPT_KEY) !== "0";
    } catch {
      return true;
    }
  });

  useEffect(() => {
    getMeeting(id)
      .then((m) => (m ? setMeeting(m) : setError("This meeting no longer exists.")))
      .catch((e) => setError(String(e)));
  }, [id]);

  // The print document stays mounted (hidden on screen): the print sheet renders its preview
  // after the print call returns, so removing it here would print a blank page.
  useEffect(() => {
    if (printRequest === 0) return;
    const frame = requestAnimationFrame(() => {
      printPage().catch((e) => setStatus(`Couldn't open the print sheet: ${e}`));
    });
    return () => cancelAnimationFrame(frame);
  }, [printRequest]);

  const toggleTranscript = () => {
    setTranscriptOpen((open) => {
      try {
        localStorage.setItem(TRANSCRIPT_KEY, open ? "0" : "1");
      } catch {
        // Preference only; the panel still toggles.
      }
      return !open;
    });
  };

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

  const lineCount = meeting?.transcript.length ?? 0;

  return (
    <div className="page meeting-page">
      <div className="topbar">
        <button className="ghost" onClick={onBack}>
          <ChevronLeft size={16} aria-hidden />
          Meetings
        </button>
        <span className="spacer" />
        {meeting && (
          <>
            <button className={`ghost${transcriptOpen ? " pressed" : ""}`} aria-pressed={transcriptOpen} onClick={toggleTranscript}>
              <PanelRight size={16} aria-hidden />
              Transcript
            </button>
            <Menu
              label="More actions"
              align="end"
              triggerClassName="icon-button"
              trigger={<Ellipsis size={18} aria-hidden />}
              items={[
                { label: "Export Markdown…", icon: <FileText size={16} />, onSelect: saveMarkdown },
                { label: "Save as PDF…", icon: <FileDown size={16} />, onSelect: savePdf },
                "separator",
                { label: "Delete meeting", icon: <Trash2 size={16} />, danger: true, onSelect: () => setConfirmDelete(true) },
              ]}
            />
          </>
        )}
      </div>

      {confirmDelete && (
        <div className="confirm" role="alertdialog" aria-labelledby="confirm-title">
          <Trash2 size={18} aria-hidden />
          <p id="confirm-title">Delete this meeting? Its notes, AI notes and transcript are removed from this Mac for good.</p>
          <button className="secondary" onClick={() => setConfirmDelete(false)} autoFocus>Cancel</button>
          <button className="danger" onClick={remove}>Delete</button>
        </div>
      )}
      {status && <p className="status page-status" role="status">{status}</p>}
      {error && <p className="error page-status" role="alert">{error}</p>}

      {meeting && (
        <div className={`meeting-layout${transcriptOpen ? " with-transcript" : ""}`}>
          <div className="page-scroll">
            <div className="column">
              <MeetingNotes
                meetingId={meeting.id}
                title={meeting.title}
                notesJson={meeting.notesJson}
                enhancedMd={meeting.enhancedMd}
                enhancedTemplateId={meeting.enhancedTemplateId}
                canEnhance
                meta={
                  <>
                    <FolderChip
                      meetingId={meeting.id}
                      folderId={meeting.folderId}
                      onChange={(folderId) => setMeeting({ ...meeting, folderId })}
                    />
                    <span className="chip static">
                      <CalendarDays size={14} aria-hidden />
                      {dayLabel(meeting.startedAtMs)}, {timeOfDay(meeting.startedAtMs)}
                    </span>
                    {meeting.durationSecs ? (
                      <span className="chip static">
                        <Clock size={14} aria-hidden />
                        {formatMinutes(meeting.durationSecs)}
                      </span>
                    ) : null}
                    <span className="chip static">
                      <MessageSquareText size={14} aria-hidden />
                      {lineCount} transcript {lineCount === 1 ? "line" : "lines"}
                    </span>
                  </>
                }
              />
            </div>
          </div>
          {transcriptOpen && (
            <aside className="transcript-panel" aria-label="Transcript">
              <h2 className="panel-label">Transcript</h2>
              <Transcript lines={meeting.transcript} />
            </aside>
          )}
        </div>
      )}
      <AskBar onAsk={onAsk} />
      {printHtml !== null && createPortal(<div className="print-doc" dangerouslySetInnerHTML={{ __html: printHtml }} />, document.body)}
    </div>
  );
}
