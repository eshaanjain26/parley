import { ReactNode, createContext, useContext, useEffect, useRef, useState } from "react";
import { AudioLevels, onAudioLevels, onToggleRecording, startRecording, stopRecording } from "./lib/recording";
import { MODEL_MISSING, ModelStatus, TranscriptLine, applyLine, listModels, onTranscriptError, onTranscriptLine } from "./lib/transcript";
import { missingKeyMessage } from "./lib/settings";
import { defaultTitle } from "./lib/meetings";
import { setMeetingFolder } from "./lib/folders";
import {
  CalendarEvent,
  currentEvent,
  followCall,
  joinCall,
  keepRecording,
  linkMeetingEvent,
  onAutoRecord,
  onCallChange,
} from "./lib/calendar";

/** The calendar lookup must never hold up a recording for long. */
const EVENT_LOOKUP_MS = 1500;

function findCurrentEvent(): Promise<CalendarEvent | null> {
  const timeout = new Promise<null>((resolve) => setTimeout(() => resolve(null), EVENT_LOOKUP_MS));
  return Promise.race([currentEvent().catch(() => null), timeout]);
}

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
  /**
   * Starts recording. With `folderId` the meeting is filed there. With `event` (or, without one,
   * the calendar event happening now) the meeting takes the event's title and attendees.
   */
  start: (folderId?: string | null, event?: CalendarEvent | null) => Promise<boolean>;
  /** Opens the event's call and records it, following the call so it can stop on its own. */
  join: (event: CalendarEvent) => Promise<void>;
  /** Set while the call has ended and an automatic stop is counting down. */
  callEndsAt: number | null;
  /** Cancels the automatic stop. */
  keep: () => void;
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
  const [callEndsAt, setCallEndsAt] = useState<number | null>(null);
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

  const start = async (folderId: string | null = null, event?: CalendarEvent | null) => {
    setError(null);
    setWarning(null);
    setLevels(SILENT);
    setLines([]);
    setMissingModel(null);
    setPhase("starting");
    const matched = event === undefined ? await findCurrentEvent() : event;
    const title = matched?.title.trim() || defaultTitle(new Date());
    try {
      const started = await startRecording(title);
      const notes = [
        started.warning && `System audio is off: ${started.warning}. Recording your microphone only.`,
        started.micWarning && `Echo cancellation is off (${started.micWarning}). Use headphones, or the call may also appear under "You".`,
      ].filter(Boolean);
      setWarning(notes.length ? notes.join(" ") : null);
      let filed: string | null = null;
      // Recording has started either way; a failed filing or link only leaves the meeting
      // unfiled or unlinked.
      if (folderId) {
        filed = await setMeetingFolder(started.id, folderId).then(
          () => folderId,
          () => null,
        );
      }
      if (matched) {
        const seriesFolder = await linkMeetingEvent(started.id, matched).catch(() => null);
        filed = filed ?? seriesFolder;
      }
      setMeeting({ id: started.id, title, folderId: filed });
      setPhase("recording");
      setCallEndsAt(null);
      callbacks.current.onStarted();
      return true;
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
      return false;
    }
  };

  const join = async (event: CalendarEvent) => {
    if (!event.callLink) return;
    try {
      await joinCall(event.callLink.url);
    } catch (e) {
      setError(String(e));
      callbacks.current.onNeedsAttention();
      return;
    }
    if (phase === "idle" && (await start(null, event))) {
      await followCall({ provider: event.callLink.provider }).catch(() => {});
    }
  };

  const keep = () => {
    setCallEndsAt(null);
    void keepRecording();
  };

  const stop = async () => {
    setCallEndsAt(null);
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
  // Automatic recording: the backend saw a calendar call's app join (or leave) the call.
  const autoRef = useRef({ start, stop, phase });
  autoRef.current = { start, stop, phase };
  useEffect(() => {
    const subs = [
      onToggleRecording(() => toggleRef.current()),
      onAutoRecord(async ({ event, bundleId }) => {
        if (autoRef.current.phase !== "idle") return;
        if (await autoRef.current.start(null, event)) await followCall({ bundleId }).catch(() => {});
      }),
      onCallChange((c) => {
        if (c.kind === "ended") setCallEndsAt(Date.now() + c.seconds * 1000);
        else if (c.kind === "resumed") setCallEndsAt(null);
        else if (autoRef.current.phase === "recording") void autoRef.current.stop();
      }),
    ];
    return () => {
      subs.forEach((s) => s.then((fn) => fn()));
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
    join,
    callEndsAt,
    keep,
    toggle: () => toggleRef.current(),
    clearMissingModel: () => setMissingModel(null),
    setFolder: (folderId) => setMeeting((m) => (m ? { ...m, folderId } : m)),
  };
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}
