import { ReactNode, createContext, useContext, useEffect, useRef, useState } from "react";
import { AudioLevels, onAudioLevels, onToggleRecording, startRecording, stopRecording } from "./lib/recording";
import { MODEL_MISSING, ModelStatus, TranscriptLine, applyLine, listModels, onTranscriptError, onTranscriptLine } from "./lib/transcript";
import { missingKeyMessage } from "./lib/settings";
import { defaultTitle } from "./lib/meetings";
import { setMeetingFolder } from "./lib/folders";

export type Phase = "idle" | "starting" | "recording" | "stopping";

export const SILENT: AudioLevels = { micDbfs: -100, systemDbfs: -100, seconds: 0, micEnded: false, systemEnded: false };

export interface Recording {
  phase: Phase;
  /** The meeting being recorded. */
  meeting: { id: string; title: string; folderId: string | null } | null;
  levels: AudioLevels;
  lines: TranscriptLine[];
  warning: string | null;
  error: string | null;
  /** Set when the speech model must be downloaded before recording can start. */
  missingModel: ModelStatus | null;
  /** Starts recording; with `folderId` the new meeting is filed there straight away. */
  start: (folderId?: string | null) => Promise<void>;
  stop: () => Promise<void>;
  toggle: () => void;
  clearMissingModel: () => void;
  /** Keeps the recording's folder in step after the user moves it. */
  setFolder: (folderId: string | null) => void;
}

const Ctx = createContext<Recording | null>(null);

export function useRecording(): Recording {
  const r = useContext(Ctx);
  if (!r) throw new Error("useRecording outside RecordingProvider");
  return r;
}

interface Props {
  children: ReactNode;
  /** Recording began (from any entry point: button, menu bar, hotkey, meeting prompt). */
  onStarted: () => void;
  /** Starting needs attention (model download or an error) and the recording page shows it. */
  onNeedsAttention: () => void;
  /** Recording stopped and the meeting was saved. */
  onFinished: (meetingId: string) => void;
}

/** One recorder for the whole app, so every view can show and control it. */
export function RecordingProvider({ children, onStarted, onNeedsAttention, onFinished }: Props) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [meeting, setMeeting] = useState<Recording["meeting"]>(null);
  const [levels, setLevels] = useState<AudioLevels>(SILENT);
  const [lines, setLines] = useState<TranscriptLine[]>([]);
  const [warning, setWarning] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [missingModel, setMissingModel] = useState<ModelStatus | null>(null);
  const callbacks = useRef({ onStarted, onNeedsAttention, onFinished });
  callbacks.current = { onStarted, onNeedsAttention, onFinished };

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

  const start = async (folderId: string | null = null) => {
    setError(null);
    setWarning(null);
    setLevels(SILENT);
    setLines([]);
    setMissingModel(null);
    setPhase("starting");
    const title = defaultTitle(new Date());
    try {
      const started = await startRecording(title);
      const notes = [
        started.warning && `System audio is off: ${started.warning}. Recording your microphone only.`,
        started.micWarning && `Echo cancellation is off (${started.micWarning}). Use headphones, or the call may also appear under "You".`,
      ].filter(Boolean);
      setWarning(notes.length ? notes.join(" ") : null);
      let filed: string | null = null;
      if (folderId) {
        // Recording has started either way; a failed filing only leaves the meeting unfiled.
        filed = await setMeetingFolder(started.id, folderId).then(
          () => folderId,
          () => null,
        );
      }
      setMeeting({ id: started.id, title, folderId: filed });
      setPhase("recording");
      callbacks.current.onStarted();
    } catch (e) {
      const message = String(e);
      const keyMessage = missingKeyMessage(message);
      if (keyMessage) {
        setError(`${keyMessage} Or switch transcription to "On this Mac".`);
      } else if (message.startsWith(MODEL_MISSING)) {
        const id = message.slice(MODEL_MISSING.length);
        setMissingModel((await listModels()).find((m) => m.id === id) ?? null);
      } else {
        setError(message);
      }
      setPhase("idle");
      callbacks.current.onNeedsAttention();
    }
  };

  const stop = async () => {
    setPhase("stopping");
    try {
      const saved = await stopRecording();
      setMeeting(null);
      setLines([]);
      setPhase("idle");
      callbacks.current.onFinished(saved.id);
    } catch (e) {
      setError(String(e));
      setPhase("idle");
      callbacks.current.onNeedsAttention();
    }
  };

  // The menu bar, the global hotkey, ⌘N and the meeting prompt all toggle through this one flow.
  const toggleRef = useRef<() => void>(() => {});
  toggleRef.current = () => {
    if (phase === "recording") void stop();
    else if (phase === "idle") void start();
  };
  useEffect(() => {
    const sub = onToggleRecording(() => toggleRef.current());
    return () => {
      sub.then((fn) => fn());
    };
  }, []);

  const value: Recording = {
    phase,
    meeting,
    levels,
    lines,
    warning,
    error,
    missingModel,
    start,
    stop,
    toggle: () => toggleRef.current(),
    clearMissingModel: () => setMissingModel(null),
    setFolder: (folderId) => setMeeting((m) => (m ? { ...m, folderId } : m)),
  };
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}
