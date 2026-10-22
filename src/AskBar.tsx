import { ArrowUp, ListChecks } from "lucide-react";
import { useState } from "react";

const SUGGESTION = "What did I commit to this week?";

/** The "ask your meetings" bar pinned to the bottom of Home and meeting pages. */
export function AskBar({ onAsk, placeholder }: { onAsk: (question: string) => void; placeholder?: string }) {
  const [question, setQuestion] = useState("");
  const submit = (q: string) => {
    if (q.trim()) onAsk(q.trim());
  };

  return (
    <div className="ask-dock">
      <form
        className="ask-bar"
        onSubmit={(e) => {
          e.preventDefault();
          submit(question);
        }}
      >
        <input
          aria-label={placeholder ?? "Ask your meetings"}
          placeholder={placeholder ?? "Ask anything about your meetings"}
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
        />
        {question.trim() ? (
          <button className="icon-button primary" type="submit" aria-label="Ask">
            <ArrowUp size={18} aria-hidden />
          </button>
        ) : (
          <button className="chip" type="button" onClick={() => submit(SUGGESTION)}>
            <ListChecks size={15} aria-hidden />
            {SUGGESTION}
          </button>
        )}
      </form>
    </div>
  );
}
