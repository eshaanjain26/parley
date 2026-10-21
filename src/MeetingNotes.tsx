import { useEffect, useRef, useState } from "react";
import { NotesEditor } from "./NotesEditor";
import { Template, cancelEnhance, enhanceMeeting, listTemplates, onEnhanceDelta, renderMarkdown } from "./lib/enhance";
import { missingKeyMessage } from "./lib/settings";

interface Props {
  meetingId: string;
  title: string;
  notesJson: string;
  /** Enhanced notes already saved for this meeting, if any. */
  enhancedMd: string;
  enhancedTemplateId: string;
  /** Enhance needs the finished transcript, so it is only offered after recording stops. */
  canEnhance: boolean;
}

type Tab = "mine" | "enhanced";

export function MeetingNotes({ meetingId, title, notesJson, enhancedMd, enhancedTemplateId, canEnhance }: Props) {
  const [tab, setTab] = useState<Tab>(enhancedMd ? "enhanced" : "mine");
  const [templates, setTemplates] = useState<Template[]>([]);
  const [templateId, setTemplateId] = useState(enhancedTemplateId || "general");
  const [enhanced, setEnhanced] = useState(enhancedMd);
  const [streaming, setStreaming] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const flushNotes = useRef<() => Promise<void>>(async () => {});

  useEffect(() => {
    listTemplates().then(setTemplates).catch(() => setTemplates([]));
  }, []);

  const enhance = async () => {
    setError(null);
    setTab("enhanced");
    setStreaming("");
    // The backend reads notes from the database, so save any edit still waiting on autosave.
    await flushNotes.current();
    const unlisten = await onEnhanceDelta((d) => {
      if (d.meetingId === meetingId) setStreaming((prev) => (prev ?? "") + d.text);
    });
    try {
      const meeting = await enhanceMeeting(meetingId, templateId);
      setEnhanced(meeting.enhancedMd);
    } catch (e) {
      const message = String(e);
      if (message !== "cancelled") setError(missingKeyMessage(message) ?? message);
    } finally {
      unlisten();
      setStreaming(null);
    }
  };

  const shown = streaming ?? enhanced;
  const busy = streaming !== null;

  return (
    <div className="meeting-notes">
      <div className="notes-tabs" role="tablist">
        <button role="tab" aria-selected={tab === "mine"} className={tab === "mine" ? "active" : ""} onClick={() => setTab("mine")}>
          My notes
        </button>
        <button
          role="tab"
          aria-selected={tab === "enhanced"}
          className={tab === "enhanced" ? "active" : ""}
          onClick={() => setTab("enhanced")}
          disabled={!enhanced && !busy}
        >
          Enhanced
        </button>
        {canEnhance && (
          <div className="enhance-controls">
            <select aria-label="Template" value={templateId} onChange={(e) => setTemplateId(e.target.value)} disabled={busy}>
              {templates.map((t) => (
                <option key={t.id} value={t.id}>{t.name}</option>
              ))}
            </select>
            {busy ? (
              <button onClick={() => cancelEnhance()}>Stop</button>
            ) : (
              <button className="primary" onClick={enhance}>
                {enhanced ? "Enhance again" : "Enhance notes"}
              </button>
            )}
          </div>
        )}
      </div>
      {error && <p className="error" role="alert">{error}</p>}
      <div hidden={tab !== "mine"} className="notes-pane">
        <NotesEditor
          meetingId={meetingId}
          initialTitle={title}
          initialJson={notesJson}
          onFlushReady={(flush) => (flushNotes.current = flush)}
        />
      </div>
      {tab === "enhanced" && (
        <div className="enhanced" aria-live={busy ? "polite" : "off"} aria-busy={busy}>
          {shown ? (
            <div className="enhanced-body" dangerouslySetInnerHTML={{ __html: renderMarkdown(shown) }} />
          ) : (
            <p className="status">Writing notes from your notes and the transcript…</p>
          )}
        </div>
      )}
    </div>
  );
}
