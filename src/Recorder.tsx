import { useEffect, useState } from "react";
import {
  AudioLevels,
  RecordingFinished,
  dbToPercent,
  formatDuration,
  onAudioLevels,
  startRecording,
  stopRecording,
} from "./lib/recording";

type Phase = "idle" | "starting" | "recording" | "stopping";

const SILENT: AudioLevels = { micDbfs: -100, systemDbfs: -100, seconds: 0 };

export function Recorder() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [levels, setLevels] = useState<AudioLevels>(SILENT);
  const [warning, setWarning] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [last, setLast] = useState<RecordingFinished | null>(null);

  useEffect(() => {
    if (phase !== "recording") return;
    const unlisten = onAudioLevels(setLevels);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [phase]);

  const start = async () => {
    setError(null);
    setWarning(null);
    setLevels(SILENT);
    setPhase("starting");
    try {
      const started = await startRecording();
      setWarning(started.warning);
      setPhase("recording");
    } catch (e) {
      setError(String(e));
      setPhase("idle");
    }
  };

  const stop = async () => {
    setPhase("stopping");
    try {
      setLast(await stopRecording());
    } catch (e) {
      setError(String(e));
    }
    setPhase("idle");
  };

  if (phase === "idle" || phase === "starting") {
    return (
      <section className="empty">
        <h1>{last ? "Recording saved" : "No meetings yet"}</h1>
        <p>
          {last
            ? `${formatDuration(last.durationSecs)} recorded${last.systemPath ? " (you and the call)" : " (microphone only)"}. Transcription arrives in the next release.`
            : "Parley records your mic and the call audio on this Mac, writes a live transcript, and turns your rough notes into clean ones."}
        </p>
        <button className="primary" onClick={start} disabled={phase === "starting"}>
          {phase === "starting" ? "Starting…" : "Start recording"}
        </button>
        {error && <p className="error" role="alert">{error}</p>}
      </section>
    );
  }

  return (
    <section className="recorder">
      <div className="rec-header">
        <span className="rec-dot" aria-hidden />
        <span className="rec-time" aria-live="off">{formatDuration(levels.seconds)}</span>
      </div>
      <Meter label="You" db={levels.micDbfs} />
      <Meter label="Others on the call" db={levels.systemDbfs} disabled={!!warning} />
      {warning && <p className="warning" role="status">System audio is off: {warning}. Recording your microphone only.</p>}
      <button className="primary" onClick={stop} disabled={phase === "stopping"}>
        {phase === "stopping" ? "Saving…" : "Stop"}
      </button>
    </section>
  );
}

function Meter({ label, db, disabled }: { label: string; db: number; disabled?: boolean }) {
  const pct = disabled ? 0 : dbToPercent(db);
  return (
    <div className={`meter${disabled ? " meter-off" : ""}`}>
      <span className="meter-label">{label}</span>
      <div className="meter-track" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
        <div className="meter-fill" style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}
