import { useEffect, useRef, useState } from "react";
import {
  AudioLevels,
  dbToPercent,
  formatDuration,
  onAudioLevels,
  onToggleRecording,
  startRecording,
  stopRecording,
} from "./lib/recording";
import {
  DownloadProgress,
  MODEL_MISSING,
  ModelStatus,
  TranscriptLine,
  applyLine,
  cancelModelDownload,
  downloadModel,
  formatBytes,
  formatTimestamp,
  listModels,
  onDownloadProgress,
  onTranscriptError,
  onTranscriptLine,
} from "./lib/transcript";
import { missingKeyMessage } from "./lib/settings";
import { Meeting, defaultTitle } from "./lib/meetings";
import { MeetingNotes } from "./MeetingNotes";

type Phase = "idle" | "starting" | "recording" | "stopping";

const SILENT: AudioLevels = { micDbfs: -100, systemDbfs: -100, seconds: 0, micEnded: false, systemEnded: false };

/** The meeting on screen: the one being recorded, or the one that just finished. */
interface OpenMeeting {
  id: string;
  title: string;
  notesJson: string;
}

interface RecorderProps {
  /** A new meeting row exists (recording started). */
  onStarted?: (meetingId: string) => void;
  /** Recording stopped and the meeting was saved; the parent opens it. */
  onFinished?: (meetingId: string) => void;
}

export function Recorder({ onStarted, onFinished }: RecorderProps) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [levels, setLevels] = useState<AudioLevels>(SILENT);
  const [lines, setLines] = useState<TranscriptLine[]>([]);
  const [warning, setWarning] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [missingModel, setMissingModel] = useState<ModelStatus | null>(null);
  const [open, setOpen] = useState<OpenMeeting | null>(null);
  const [finished, setFinished] = useState<Meeting | null>(null);

  useEffect(() => {
    if (phase !== "recording" && phase !== "stopping") return;
    const subs = [
      onAudioLevels(setLevels),
      onTranscriptLine((l) => setLines((prev) => applyLine(prev, l))),
      onTranscriptError((m) => setWarning(`Transcription stopped: ${m}`)),
    ];
    return () => {
      subs.forEach((s) => s.then((fn) => fn()));
    };
  }, [phase]);

  const start = async () => {
    setError(null);
    setWarning(null);
    setLevels(SILENT);
    setLines([]);
    setFinished(null);
    setOpen(null);
    setPhase("starting");
    const title = defaultTitle(new Date());
    try {
      const started = await startRecording(title);
      const notes = [
        started.warning && `System audio is off: ${started.warning}. Recording your microphone only.`,
        started.micWarning && `Echo cancellation is off (${started.micWarning}). Use headphones, or the call may also appear under "You".`,
      ].filter(Boolean);
      setWarning(notes.length ? notes.join(" ") : null);
      setOpen({ id: started.id, title, notesJson: "" });
      setPhase("recording");
      onStarted?.(started.id);
    } catch (e) {
      const message = String(e);
      const keyMessage = missingKeyMessage(message);
      if (keyMessage) {
        setError(`${keyMessage} Or switch transcription to "On this Mac".`);
      } else if (message.startsWith(MODEL_MISSING)) {
        const id = message.slice(MODEL_MISSING.length);
        const model = (await listModels()).find((m) => m.id === id) ?? null;
        setMissingModel(model);
      } else {
        setError(message);
      }
      setPhase("idle");
    }
  };

  const stop = async () => {
    setPhase("stopping");
    try {
      const meeting = await stopRecording();
      // The finished meeting opens in the library's detail view, so this view resets. Keeping
      // its editor mounted too would mean two autosaving editors on one meeting.
      setOpen(null);
      setFinished(null);
      setLines([]);
      onFinished?.(meeting.id);
    } catch (e) {
      setError(String(e));
    }
    setPhase("idle");
  };

  // The menu bar and the global hotkey ask the UI to toggle, so they share this exact flow.
  const toggle = useRef<() => void>(() => {});
  toggle.current = () => {
    if (phase === "recording") void stop();
    else if (phase === "idle") void start();
  };
  useEffect(() => {
    const sub = onToggleRecording(() => toggle.current());
    return () => {
      sub.then((fn) => fn());
    };
  }, []);

  if (!open) {
    return (
      <section className="recorder-idle">
        {missingModel ? (
          <ModelDownload model={missingModel} onDone={() => setMissingModel(null)} />
        ) : (
          <div className="empty">
            <h1>Start a new meeting</h1>
            <p>
              Parley records your mic and the call audio on this Mac, writes a live transcript, and keeps your notes next to it, all
              encrypted on this Mac.
            </p>
            <button className="primary" onClick={start} disabled={phase === "starting"}>
              {phase === "starting" ? "Starting…" : "Start recording"}
            </button>
            {error && <p className="error" role="alert">{error}</p>}
          </div>
        )}
      </section>
    );
  }

  const recording = phase === "recording" || phase === "stopping";
  return (
    <section className="meeting">
      <div className="rec-bar">
        {recording ? (
          <>
            <div className="rec-header">
              <span className="rec-dot" aria-hidden />
              <span className="rec-time">{formatDuration(levels.seconds)}</span>
            </div>
            <div className="rec-meters">
              <Meter label="You" db={levels.micDbfs} />
              <Meter label="Others on the call" db={levels.systemDbfs} />
            </div>
            <button className="primary" onClick={stop} disabled={phase === "stopping"}>
              {phase === "stopping" ? "Finishing transcript…" : "Stop"}
            </button>
          </>
        ) : (
          <>
            <p className="rec-summary">
              {finished
                ? `Saved. ${formatDuration(finished.durationSecs ?? 0)} recorded, ${finished.transcript.length} transcript ${finished.transcript.length === 1 ? "line" : "lines"}.`
                : "Recording stopped."}
            </p>
            <button className="primary" onClick={start}>
              New recording
            </button>
          </>
        )}
      </div>
      {warning && recording && <p className="warning" role="status">{warning}</p>}
      {error && <p className="error" role="alert">{error}</p>}
      {(levels.micEnded || levels.systemEnded) && recording && (
        <p className="error" role="alert">
          {levels.systemEnded ? "Call audio" : "Your microphone"} stopped recording unexpectedly. The rest of the meeting is still being
          captured where possible.
        </p>
      )}
      <div className="meeting-body">
        <MeetingNotes
          key={open.id}
          meetingId={open.id}
          title={open.title}
          notesJson={open.notesJson}
          enhancedMd={finished?.enhancedMd ?? ""}
          enhancedTemplateId={finished?.enhancedTemplateId ?? ""}
          canEnhance={!recording && finished !== null}
        />
        <aside className="meeting-transcript" aria-label="Transcript">
          <h2>Transcript</h2>
          <Transcript lines={lines} live={recording} />
        </aside>
      </div>
    </section>
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
    <div className="empty">
      <h1>Download the speech model</h1>
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
