import { Check, ChevronDown, PenLine, Sparkles, Square } from "lucide-react";
import { ReactNode, useEffect, useRef, useState } from "react";
import { Menu } from "./Menu";
import { NotesEditor } from "./NotesEditor";
import { templateIcon } from "./templateIcons";
import { Template, cancelEnhance, enhanceMeeting, listTemplates, onEnhanceDelta, renderMarkdown, suggestTemplate } from "./lib/enhance";
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
  onSaved?: () => void;
  /** Chips under the title (date, duration). */
  meta?: ReactNode;
}

type Tab = "mine" | "enhanced";

export function MeetingNotes({ meetingId, title, notesJson, enhancedMd, enhancedTemplateId, canEnhance, onSaved, meta }: Props) {
  const [tab, setTab] = useState<Tab>(enhancedMd ? "enhanced" : "mine");
  const [templates, setTemplates] = useState<Template[]>([]);
  const [templateId, setTemplateId] = useState(enhancedTemplateId || "general");
  const [enhanced, setEnhanced] = useState(enhancedMd);
  const [streaming, setStreaming] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const flushNotes = useRef<() => Promise<void>>(async () => {});
  // A fast double click lands before React re-renders, so guard with a ref, not state.
  const inFlight = useRef(false);

  const [suggested, setSuggested] = useState<string | null>(null);

  useEffect(() => {
    listTemplates().then(setTemplates).catch(() => setTemplates([]));
    // A meeting named "Weekly 1:1" or "Acme demo" starts on the matching template.
    suggestTemplate(title)
      .then((id) => {
        setSuggested(id);
        if (id && !enhancedTemplateId) setTemplateId(id);
      })
      .catch(() => {});
    // The title at mount is what the meeting (or its calendar event) was called.
  }, []);

  const enhance = async () => {
    if (inFlight.current) return;
    inFlight.current = true;
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
      onSaved?.();
    } catch (e) {
      const message = String(e);
      if (message !== "cancelled") setError(missingKeyMessage(message) ?? message);
    } finally {
      unlisten();
      setStreaming(null);
      inFlight.current = false;
    }
  };

  const shown = streaming ?? enhanced;
  const busy = streaming !== null;

  const current = templates.find((t) => t.id === templateId);

  const toolbar = (
    <>
      {meta && <div className="meta-row">{meta}</div>}
      <div className="notes-toolbar">
        <div className="segmented" role="tablist" aria-label="Notes view">
          <button role="tab" aria-selected={tab === "mine"} className={tab === "mine" ? "active" : ""} onClick={() => setTab("mine")}>
            <PenLine size={15} aria-hidden />
            My notes
          </button>
          <button
            role="tab"
            aria-selected={tab === "enhanced"}
            className={tab === "enhanced" ? "active" : ""}
            onClick={() => setTab("enhanced")}
            disabled={!enhanced && !busy}
          >
            <Sparkles size={15} aria-hidden />
            AI notes
          </button>
        </div>
        {canEnhance && (
          <div className="enhance-controls">
            <Menu
              label="Template"
              heading="Templates"
              align="end"
              disabled={busy}
              trigger={
                <>
                  <span aria-hidden>{templateIcon(templateId)}</span>
                  {current?.name ?? "Template"}
                  <ChevronDown size={14} aria-hidden />
                </>
              }
              items={templates.map((t) => ({
                label: t.name,
                icon: templateIcon(t.id),
                trailing:
                  t.id === templateId ? (
                    <Check size={16} aria-label="Selected" />
                  ) : t.id === suggested ? (
                    <span className="menu-badge">Suggested</span>
                  ) : undefined,
                onSelect: () => setTemplateId(t.id),
              }))}
            />
            {busy ? (
              <button className="secondary" onClick={() => cancelEnhance()}>
                <Square size={13} aria-hidden fill="currentColor" />
                Stop
              </button>
            ) : (
              <button className="primary" onClick={enhance}>
                <Sparkles size={15} aria-hidden />
                {enhanced ? "Rewrite AI notes" : "Write AI notes"}
              </button>
            )}
          </div>
        )}
      </div>
      {error && <p className="error" role="alert">{error}</p>}
    </>
  );

  return (
    <div className="meeting-notes">
      <NotesEditor
        meetingId={meetingId}
        initialTitle={title}
        initialJson={notesJson}
        onFlushReady={(flush) => (flushNotes.current = flush)}
        onSaved={onSaved}
        toolbar={toolbar}
        hideBody={tab !== "mine"}
      />
      {tab === "enhanced" && (
        <div className="enhanced" aria-live={busy ? "polite" : "off"} aria-busy={busy}>
          {shown ? (
            <div className="prose" dangerouslySetInnerHTML={{ __html: renderMarkdown(shown) }} />
          ) : (
            <p className="status">Writing notes from your notes and the transcript…</p>
          )}
        </div>
      )}
    </div>
  );
}
