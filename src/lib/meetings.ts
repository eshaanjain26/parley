import { invoke } from "@tauri-apps/api/core";
import type { Attendee } from "./calendar";
import type { TranscriptLine } from "./transcript";

// Mirrors Meeting in src-tauri/src/store.rs.
export interface Meeting {
  id: string;
  title: string;
  startedAtMs: number;
  durationSecs: number | null;
  notesJson: string;
  notesText: string;
  updatedAtMs: number;
  enhancedMd: string;
  enhancedTemplateId: string;
  enhancedAtMs: number | null;
  folderId: string | null;
  eventId: string | null;
  attendees: Attendee[];
  transcript: TranscriptLine[];
}

export interface NotesUpdate {
  title: string;
  notesJson: string;
  notesText: string;
}

export const getMeeting = (id: string) => invoke<Meeting | null>("get_meeting", { id });
export const saveNotes = (id: string, update: NotesUpdate) => invoke<void>("save_notes", { id, ...update });

/** "Meeting on Oct 21 at 9:05 AM", in the user's locale. */
export function defaultTitle(date: Date, locale?: string): string {
  const day = date.toLocaleDateString(locale, { month: "short", day: "numeric" });
  const time = date.toLocaleTimeString(locale, { hour: "numeric", minute: "2-digit" });
  return `Meeting on ${day} at ${time}`;
}

/**
 * Debounced saver: `schedule` coalesces rapid edits into one save after `delayMs` of quiet,
 * and `flush` saves the latest pending edit immediately (before stopping or leaving).
 * Saves never overlap; a change made during a save is saved right after it.
 */
export function createAutosaver<T>(save: (value: T) => Promise<void>, delayMs: number, onError?: (e: unknown) => void) {
  let pending: { value: T } | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let inFlight: Promise<void> | null = null;

  const run = async (): Promise<void> => {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
    if (inFlight) {
      await inFlight;
    }
    if (!pending) return;
    const { value } = pending;
    pending = null;
    inFlight = save(value)
      .catch((e) => onError?.(e))
      .finally(() => {
        inFlight = null;
      });
    await inFlight;
  };

  return {
    schedule(value: T) {
      pending = { value };
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => void run(), delayMs);
    },
    flush: run,
    hasPending: () => pending !== null,
  };
}
