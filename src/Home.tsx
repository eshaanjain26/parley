import { FileText, Mic, Search, Sparkles, Square, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { AskBar } from "./AskBar";
import { useRecording } from "./RecordingContext";
import { greeting, groupByDay, longDate, timeOfDay } from "./lib/dates";
import { MeetingDetected, MeetingSummary, formatMinutes, listMeetings, onStoreReady, searchMeetings, splitSnippet } from "./lib/library";
import { formatDuration } from "./lib/recording";

interface Props {
  hotkey: string;
  detected: MeetingDetected | null;
  onDismissDetected: () => void;
  onOpenMeeting: (id: string) => void;
  onOpenRecording: () => void;
  onAsk: (question: string) => void;
  /** Changes when ⌘K asks for the search box. */
  focusSearch: number;
}

export function Home({ hotkey, detected, onDismissDetected, onOpenMeeting, onOpenRecording, onAsk, focusSearch }: Props) {
  const rec = useRecording();
  const [meetings, setMeetings] = useState<MeetingSummary[] | null>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<MeetingSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const search = useRef<HTMLInputElement>(null);

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
      searchMeetings(q).then(setResults).catch((e) => setError(String(e)));
    }, 200);
    return () => clearTimeout(t);
  }, [query]);

  const shown = results ?? meetings ?? [];
  const groups = groupByDay(shown);
  const recordingId = rec.phase === "recording" || rec.phase === "stopping" ? rec.meeting?.id : undefined;
  const now = new Date();

  return (
    <div className="page home">
      <div className="page-scroll">
        <header className="page-header">
          <p className="eyebrow">{longDate(now)}</p>
          <h1 className="display">{greeting(now)}</h1>
        </header>

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
                void rec.start();
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
              <strong>Ready for your next meeting</strong>
              <span className="muted">
                Start when the call begins, or press <kbd>{hotkey}</kbd> from any app.
              </span>
            </div>
            <button className="primary" onClick={() => void rec.start()} disabled={rec.phase === "starting"}>
              <Mic size={16} aria-hidden />
              {rec.phase === "starting" ? "Starting…" : "Start recording"}
            </button>
          </section>
        )}

        <div className="search-field">
          <Search size={16} aria-hidden />
          <input
            ref={search}
            type="search"
            spellCheck={false}
            autoCorrect="off"
            autoCapitalize="off"
            placeholder="Search titles, notes and transcripts"
            aria-label="Search meetings"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setQuery("")}
          />
          {!query && <kbd aria-hidden>⌘K</kbd>}
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
            ) : (
              <>
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
                            m.hasEnhanced && <span className="row-sub">AI notes ready</span>
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
      <AskBar onAsk={onAsk} />
    </div>
  );
}
