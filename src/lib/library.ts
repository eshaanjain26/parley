import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

// Mirrors MeetingSummary in src-tauri/src/store.rs.
export interface MeetingSummary {
  id: string;
  title: string;
  startedAtMs: number;
  durationSecs: number | null;
  hasEnhanced: boolean;
  /** Search results only: the best passage, with \u0001 / \u0002 around matched words. */
  snippet: string | null;
}

// Mirrors ask::Source.
export interface Source {
  index: number;
  meetingId: string;
  title: string;
  startedAtMs: number;
}

export interface Answer {
  answer: string;
  sources: Source[];
}

/** Minutes east of UTC, the way the backend formats local dates. */
export const utcOffsetMinutes = (date = new Date()) => -date.getTimezoneOffset();

export const listMeetings = (limit = 200, offset = 0) => invoke<MeetingSummary[]>("list_meetings", { limit, offset });
export const searchMeetings = (query: string) => invoke<MeetingSummary[]>("search_meetings", { query });
export const deleteMeeting = (id: string) => invoke<void>("delete_meeting", { id });
export const askMeetings = (question: string) =>
  invoke<Answer>("ask_meetings", { question, utcOffsetMinutes: utcOffsetMinutes() });
export const onAskDelta = (cb: (text: string) => void): Promise<UnlistenFn> =>
  listen<{ text: string }>("ask-delta", (e) => cb(e.payload.text));
export const exportMarkdown = (id: string) =>
  invoke<string | null>("export_markdown", { id, utcOffsetMinutes: utcOffsetMinutes() });
export const meetingMarkdown = (id: string) => invoke<string>("meeting_markdown", { id, utcOffsetMinutes: utcOffsetMinutes() });
export const printPage = () => invoke<void>("print_page");
/** Fires once the encrypted database has been unlocked (it can wait on a Keychain prompt). */
export const onStoreReady = (cb: () => void): Promise<UnlistenFn> => listen("store-ready", () => cb());

export interface SnippetPart {
  text: string;
  hit: boolean;
}

/** Splits a search snippet into plain and highlighted runs. */
export function splitSnippet(snippet: string): SnippetPart[] {
  const parts: SnippetPart[] = [];
  let hit = false;
  let buf = "";
  for (const ch of snippet) {
    if (ch === "\u0001" || ch === "\u0002") {
      if (buf) parts.push({ text: buf, hit });
      buf = "";
      hit = ch === "\u0001";
    } else {
      buf += ch;
    }
  }
  if (buf) parts.push({ text: buf, hit });
  return parts;
}

/** "Today 9:05 AM", "Yesterday 4:30 PM", or "Oct 14, 9:05 AM" (with the year if not this year). */
export function formatWhen(ms: number, now = new Date(), locale?: string): string {
  const d = new Date(ms);
  const time = d.toLocaleTimeString(locale, { hour: "numeric", minute: "2-digit" });
  const startOfDay = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const days = Math.round((startOfDay(now) - startOfDay(d)) / 86_400_000);
  if (days === 0) return `Today ${time}`;
  if (days === 1) return `Yesterday ${time}`;
  const sameYear = d.getFullYear() === now.getFullYear();
  const date = d.toLocaleDateString(locale, { month: "short", day: "numeric", ...(sameYear ? {} : { year: "numeric" }) });
  return `${date}, ${time}`;
}

/** 0 -> "", 42 -> "1 min", 3725 -> "1 h 2 min". */
export function formatMinutes(secs: number | null): string {
  if (secs === null || secs < 1) return "";
  const mins = Math.max(1, Math.round(secs / 60));
  return mins >= 60 ? `${Math.floor(mins / 60)} h ${mins % 60} min` : `${mins} min`;
}
