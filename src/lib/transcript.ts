import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

// Mirrors TranscriptLine in src-tauri/src/transcribe.rs.
export type Speaker = "me" | "them";

export interface TranscriptLine {
  speaker: Speaker;
  segmentId: number;
  isFinal: boolean;
  text: string;
  startMs: number;
  endMs: number;
}

export interface ModelStatus {
  id: string;
  label: string;
  sizeBytes: number;
  installed: boolean;
}

export interface DownloadProgress {
  id: string;
  received: number;
  total: number;
}

/** start_recording fails with this prefix (plus the model id) when the speech model isn't downloaded. */
export const MODEL_MISSING = "speech-model-missing:";

export const onTranscriptLine = (cb: (l: TranscriptLine) => void): Promise<UnlistenFn> =>
  listen<TranscriptLine>("transcript-line", (e) => cb(e.payload));
export const onTranscriptError = (cb: (message: string) => void): Promise<UnlistenFn> =>
  listen<string>("transcript-error", (e) => cb(e.payload));
export const onDownloadProgress = (cb: (p: DownloadProgress) => void): Promise<UnlistenFn> =>
  listen<DownloadProgress>("model-download", (e) => cb(e.payload));

export const listModels = () => invoke<ModelStatus[]>("list_models");
export const downloadModel = (id: string) => invoke<void>("download_model", { id });
export const cancelModelDownload = () => invoke<void>("cancel_model_download");

const key = (l: Pick<TranscriptLine, "speaker" | "segmentId">) => `${l.speaker}:${l.segmentId}`;

/**
 * Inserts or replaces a line (partials are replaced by later partials and by their final),
 * keeping the list in time order. Empty finals remove the segment.
 */
export function applyLine(lines: TranscriptLine[], line: TranscriptLine): TranscriptLine[] {
  const rest = lines.filter((l) => key(l) !== key(line));
  if (line.isFinal && line.text.trim() === "") return rest;
  const next = [...rest, line];
  next.sort((a, b) => a.startMs - b.startMs || (a.speaker === "me" ? -1 : 1));
  return next;
}

/** 147964211 -> "148 MB". */
export function formatBytes(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  return `${Math.round(bytes / 1e6)} MB`;
}

/** 83000 -> "1:23" for transcript timestamps. */
export function formatTimestamp(ms: number): string {
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}
