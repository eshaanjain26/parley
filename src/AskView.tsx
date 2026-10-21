import { useRef, useState } from "react";
import { cancelEnhance, renderMarkdown } from "./lib/enhance";
import { Source, askMeetings, formatWhen, onAskDelta } from "./lib/library";
import { missingKeyMessage } from "./lib/settings";

const EXAMPLES = [
  "What did we decide about the budget?",
  "What action items do I own?",
  "What concerns did customers raise this month?",
];

export function AskView({ onOpenMeeting }: { onOpenMeeting: (id: string) => void }) {
  const [question, setQuestion] = useState("");
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
      const result = await askMeetings(q);
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

  return (
    <section className="ask">
      <h1>Ask your meetings</h1>
      <form
        className="ask-form"
        onSubmit={(e) => {
          e.preventDefault();
          ask();
        }}
      >
        <input
          aria-label="Question"
          placeholder="What did we agree with the vendor last week?"
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          disabled={busy}
        />
        {busy ? (
          <button type="button" onClick={() => cancelEnhance()}>Stop</button>
        ) : (
          <button className="primary" type="submit" disabled={!question.trim()}>Ask</button>
        )}
      </form>
      {!answer && !busy && !error && (
        <div className="ask-examples">
          {EXAMPLES.map((q) => (
            <button key={q} onClick={() => ask(q)}>{q}</button>
          ))}
        </div>
      )}
      {error && <p className="error" role="alert">{error}</p>}
      {(answer || busy) && (
        <div className="ask-answer" aria-live="polite" aria-busy={busy}>
          {answer ? <div className="enhanced-body" dangerouslySetInnerHTML={{ __html: renderMarkdown(answer) }} /> : <p className="status">Reading your meetings…</p>}
        </div>
      )}
      {sources.length > 0 && (
        <div className="ask-sources">
          <h2>Sources</h2>
          <ol>
            {sources.map((s) => (
              <li key={s.meetingId} value={s.index}>
                <button className="link" onClick={() => onOpenMeeting(s.meetingId)}>{s.title}</button>
                <span className="row-meta"> · {formatWhen(s.startedAtMs)}</span>
              </li>
            ))}
          </ol>
        </div>
      )}
      <p className="hint">Answers come from your saved notes and transcripts, sent to the AI provider chosen in Settings.</p>
    </section>
  );
}
