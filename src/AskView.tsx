import { ArrowUp, BookOpen, Folder as FolderIcon, MessageCircleQuestion, Square, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { cancelEnhance, renderMarkdown } from "./lib/enhance";
import { Source, askMeetings, formatWhen, onAskDelta } from "./lib/library";
import { missingKeyMessage } from "./lib/settings";
import { Folder } from "./lib/folders";

const EXAMPLES = [
  "What did we decide about the budget?",
  "What action items do I own?",
  "What concerns did customers raise this month?",
];

interface Props {
  /** Asked straight away (from the Ask bar). */
  initialQuestion?: string;
  /** Answers only from this folder's meetings. */
  folder?: Folder;
  onClearFolder: () => void;
  onOpenMeeting: (id: string) => void;
}

export function AskView({ initialQuestion, folder, onClearFolder, onOpenMeeting }: Props) {
  const [question, setQuestion] = useState(initialQuestion ?? "");
  const [answer, setAnswer] = useState("");
  const [sources, setSources] = useState<Source[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // State updates lag a fast double click; a ref guards against sending the question twice.
  const inFlight = useRef(false);

  const ask = async (q = question) => {
    if (!q.trim() || inFlight.current) return;
    inFlight.current = true;
    setQuestion(q);
    setBusy(true);
    setError(null);
    setAnswer("");
    setSources([]);
    const unlisten = await onAskDelta((t) => setAnswer((prev) => prev + t));
    try {
      const result = await askMeetings(q, folder?.id ?? null);
      setAnswer(result.answer);
      setSources(result.sources);
    } catch (e) {
      const message = String(e);
      if (message !== "cancelled") setError(missingKeyMessage(message) ?? message);
    } finally {
      unlisten();
      setBusy(false);
      inFlight.current = false;
    }
  };

  useEffect(() => {
    if (initialQuestion) void ask(initialQuestion);
    // Only the question this view was opened with; later questions come from the form.
  }, []);

  return (
    <div className="page">
      <div className="page-scroll">
        <section className="ask column">
          <header className="page-header">
            <p className="eyebrow">{folder ? "Only meetings in this folder" : "Across every meeting on this Mac"}</p>
            <h1 className="display">{folder ? `Ask about ${folder.name}` : "Ask your meetings"}</h1>
            {folder && (
              <button className="chip scope-chip" onClick={onClearFolder} aria-label={`Ask across all meetings instead of only ${folder.name}`}>
                <FolderIcon size={14} aria-hidden />
                {folder.name}
                <X size={13} aria-hidden />
              </button>
            )}
          </header>
          <form
            className="ask-bar inline"
            onSubmit={(e) => {
              e.preventDefault();
              ask();
            }}
          >
            <MessageCircleQuestion size={18} aria-hidden className="field-icon" />
            <input
              aria-label="Question"
              autoFocus={!initialQuestion}
              placeholder="What did we agree with the vendor last week?"
              value={question}
              onChange={(e) => setQuestion(e.target.value)}
              disabled={busy}
            />
            {busy ? (
              <button className="secondary" type="button" onClick={() => cancelEnhance()}>
                <Square size={13} aria-hidden fill="currentColor" />
                Stop
              </button>
            ) : (
              <button className="icon-button primary" type="submit" aria-label="Ask" disabled={!question.trim()}>
                <ArrowUp size={18} aria-hidden />
              </button>
            )}
          </form>
          {!answer && !busy && !error && (
            <div className="chip-row">
              {EXAMPLES.map((q) => (
                <button key={q} className="chip" onClick={() => ask(q)}>{q}</button>
              ))}
            </div>
          )}
          {error && <p className="error" role="alert">{error}</p>}
          {(answer || busy) && (
            <div className="answer-card" aria-live="polite" aria-busy={busy}>
              {answer ? <div className="prose" dangerouslySetInnerHTML={{ __html: renderMarkdown(answer) }} /> : <p className="status">Reading your meetings…</p>}
            </div>
          )}
          {sources.length > 0 && (
            <div className="sources">
              <h2 className="panel-label">Sources</h2>
              <ol>
                {sources.map((s) => (
                  <li key={s.meetingId}>
                    <button className="source" onClick={() => onOpenMeeting(s.meetingId)}>
                      <span className="source-index" aria-hidden>{s.index}</span>
                      <BookOpen size={15} aria-hidden />
                      <span className="source-title">{s.title}</span>
                      <span className="muted">{formatWhen(s.startedAtMs)}</span>
                    </button>
                  </li>
                ))}
              </ol>
            </div>
          )}
          <p className="hint">Answers come from your saved notes and transcripts, sent to the AI provider chosen in Settings.</p>
        </section>
      </div>
    </div>
  );
}
