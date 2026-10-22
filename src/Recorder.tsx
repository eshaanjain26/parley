import { ChevronLeft, Mic, Square } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { MeetingNotes } from "./MeetingNotes";
import { useRecording } from "./RecordingContext";
import { dbToPercent, formatDuration } from "./lib/recording";
import {
  DownloadProgress,
  ModelStatus,
  TranscriptLine,
  cancelModelDownload,
  downloadModel,
  formatBytes,
  formatTimestamp,
  onDownloadProgress,
} from "./lib/transcript";

/** The live meeting: timer, meters, notes and the transcript as it is written. */
export function RecordingPage({ onBack }: { onBack: () => void }) {
  const rec = useRecording();
  const recording = rec.phase === "recording" || rec.phase === "stopping";

  if (rec.missingModel) {
    return (
      <div className="page">
        <div className="topbar">
          <button className="ghost" onClick={onBack}>
            <ChevronLeft size={16} aria-hidden />
            Meetings
          </button>
        </div>
        <div className="page-scroll">
          <ModelDownload
            model={rec.missingModel}
            onDone={() => {
              rec.clearMissingModel();
              void rec.start();
            }}
          />
        </div>
      </div>
    );
  }

  if (!recording || !rec.meeting) {
    return (
      <div className="page">
        <div className="topbar">
          <button className="ghost" onClick={onBack}>
            <ChevronLeft size={16} aria-hidden />
            Meetings
          </button>
        </div>
        <div className="page-scroll">
          <div className="empty-state centered">
            {rec.phase === "starting" ? (
              <p className="empty-title">Starting the recording…</p>
            ) : (
              <>
                <p className="empty-title">Not recording</p>
                {rec.error && <p className="error" role="alert">{rec.error}</p>}
                <button className="primary" onClick={() => void rec.start()}>
                  <Mic size={16} aria-hidden />
                  {rec.error ? "Try again" : "Start recording"}
                </button>
              </>
            )}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="page recording-page">
      <div className="topbar rec-topbar">
        <button className="ghost" onClick={onBack}>
          <ChevronLeft size={16} aria-hidden />
          Meetings
        </button>
        <span className="rec-dot" aria-hidden />
        <span className="rec-time" role="timer" aria-label="Recording time">{formatDuration(rec.levels.seconds)}</span>
        <div className="rec-meters">
          <Meter label="You" db={rec.levels.micDbfs} />
          <Meter label="Call audio" db={rec.levels.systemDbfs} />
        </div>
        <button className="primary" onClick={() => void rec.stop()} disabled={rec.phase === "stopping"}>
          <Square size={13} aria-hidden fill="currentColor" />
          {rec.phase === "stopping" ? "Finishing transcript…" : "Stop"}
        </button>
      </div>
      {rec.warning && <p className="notice page-status" role="status">{rec.warning}</p>}
      {rec.error && <p className="error page-status" role="alert">{rec.error}</p>}
      {(rec.levels.micEnded || rec.levels.systemEnded) && (
        <p className="error page-status" role="alert">
          {rec.levels.systemEnded ? "Call audio" : "Your microphone"} stopped recording unexpectedly. The rest of the meeting is still being
          captured where possible.
        </p>
      )}
      <div className="meeting-layout with-transcript">
        <div className="page-scroll">
          <div className="column">
            <MeetingNotes
              key={rec.meeting.id}
              meetingId={rec.meeting.id}
              title={rec.meeting.title}
              notesJson=""
              enhancedMd=""
              enhancedTemplateId=""
              canEnhance={false}
            />
          </div>
        </div>
        <aside className="transcript-panel" aria-label="Live transcript">
          <h2 className="panel-label">Live transcript</h2>
          <Transcript lines={rec.lines} live />
        </aside>
      </div>
    </div>
  );
}

export function Transcript({ lines, live }: { lines: TranscriptLine[]; live?: boolean }) {
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (live) end.current?.scrollIntoView({ block: "end", behavior: "smooth" });
  }, [lines, live]);

  if (lines.length === 0) {
    return <p className="transcript-empty">{live ? "Listening… the transcript appears here as people talk." : "No speech was transcribed."}</p>;
  }
  return (
    <div className="transcript" aria-live={live ? "polite" : "off"}>
      {lines.map((l) => (
        <div key={`${l.speaker}:${l.segmentId}`} className={`line line-${l.speaker}${l.isFinal ? "" : " line-partial"}`}>
          <span className="line-meta">
            <span className="line-speaker">{l.speaker === "me" ? "You" : "Them"}</span>
            <span className="line-time">{formatTimestamp(l.startMs)}</span>
          </span>
          <span className="line-text">{l.text}</span>
        </div>
      ))}
      <div ref={end} />
    </div>
  );
}

export function ModelDownload({ model, onDone }: { model: ModelStatus; onDone: () => void }) {
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const downloading = progress !== null && !error;

  useEffect(() => {
    const sub = onDownloadProgress((p) => p.id === model.id && setProgress(p));
    return () => {
      sub.then((fn) => fn());
    };
  }, [model.id]);

  const start = async () => {
    setError(null);
    setProgress({ id: model.id, received: 0, total: model.sizeBytes });
    try {
      await downloadModel(model.id);
      onDone();
    } catch (e) {
      setError(String(e));
      setProgress(null);
    }
  };

  const pct = progress ? Math.floor((progress.received / progress.total) * 100) : 0;
  return (
    <div className="empty-state centered">
      <h1 className="display small">Download the speech model</h1>
      <p>
        Parley transcribes on this Mac with the {model.label} model ({formatBytes(model.sizeBytes)}). It downloads once and is checked
        against a known checksum.
      </p>
      {downloading ? (
        <>
          <div className="meter-track" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
            <div className="meter-fill" style={{ width: `${pct}%` }} />
          </div>
          <p className="status">
            {formatBytes(progress.received)} of {formatBytes(progress.total)}
          </p>
          <button onClick={() => cancelModelDownload()}>Cancel</button>
        </>
      ) : (
        <button className="primary" onClick={start}>
          Download {formatBytes(model.sizeBytes)}
        </button>
      )}
      {error && <p className="error" role="alert">{error}</p>}
    </div>
  );
}

function Meter({ label, db }: { label: string; db: number }) {
  const pct = dbToPercent(db);
  return (
    <div className="meter">
      <span className="meter-label">{label}</span>
      <div className="meter-track" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
        <div className="meter-fill" style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}
