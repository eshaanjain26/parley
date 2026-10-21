import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import type { TranscriptLine } from "./transcript";

// Mirrors the structs returned by start_recording / stop_recording and the audio-levels event in src-tauri/src/lib.rs.
export interface RecordingStarted {
  id: string;
  systemAudio: boolean;
  warning: string | null;
  micWarning: string | null;
}

export interface RecordingFinished {
  id: string;
  durationSecs: number;
  micPath: string;
  systemPath: string | null;
  transcript: TranscriptLine[];
}

export interface AudioLevels {
  micDbfs: number;
  systemDbfs: number;
  seconds: number;
  micEnded: boolean;
  systemEnded: boolean;
}

export const startRecording = () => invoke<RecordingStarted>("start_recording");
export const stopRecording = () => invoke<RecordingFinished>("stop_recording");
export const onAudioLevels = (cb: (l: AudioLevels) => void): Promise<UnlistenFn> =>
  listen<AudioLevels>("audio-levels", (e) => cb(e.payload));

const METER_FLOOR_DB = -60;

/** Maps dBFS onto 0–100 for a meter bar; anything at or below -60 dB reads as empty. */
export function dbToPercent(db: number): number {
  if (!Number.isFinite(db) || db <= METER_FLOOR_DB) return 0;
  if (db >= 0) return 100;
  return Math.round(((db - METER_FLOOR_DB) / -METER_FLOOR_DB) * 100);
}

/** 75 -> "1:15", 3725 -> "1:02:05". */
export function formatDuration(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}
