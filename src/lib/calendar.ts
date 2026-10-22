import { invoke } from "@tauri-apps/api/core";

// Mirrors src-tauri/src/calendar.rs.
export interface Attendee {
  name: string;
  email: string;
}

export interface CalendarEvent {
  id: string;
  seriesId: string | null;
  calendarId: string;
  title: string;
  startMs: number;
  endMs: number;
  allDay: boolean;
  attendees: Attendee[];
  url: string;
  location: string;
}

export interface CalendarInfo {
  id: string;
  title: string;
  source: string;
  color: string;
}

export type CalendarStatus = "fullAccess" | "notDetermined" | "denied" | "restricted" | "writeOnly";

export const calendarStatus = () => invoke<CalendarStatus>("calendar_status");
/** Shows macOS's prompt the first time and turns the calendar on when access is granted. */
export const connectCalendar = () => invoke<CalendarStatus>("connect_calendar");
export const listCalendars = () => invoke<CalendarInfo[]>("list_calendars");
export const upcomingEvents = (fromMs: number, toMs: number) => invoke<CalendarEvent[]>("upcoming_events", { fromMs, toMs });
export const currentEvent = () => invoke<CalendarEvent | null>("current_event");
/** Returns the folder the meeting was filed in, when a repeating event decided one. */
export const linkMeetingEvent = (meetingId: string, event: CalendarEvent) =>
  invoke<string | null>("link_meeting_event", { meetingId, event });

/** Mirrors calendar::EARLY_START_MS: how early a recording can start and still belong to an event. */
export const EARLY_START_MS = 10 * 60 * 1000;

/** Whether "Record" makes sense for an event right now. */
export const canRecordNow = (e: CalendarEvent, now = Date.now()) => !e.allDay && e.startMs - EARLY_START_MS <= now && now < e.endMs;

/** "Priya Shah", "Priya Shah and Marco", "Priya Shah, Marco and 3 others". Falls back to emails. */
export function attendeeSummary(attendees: Attendee[]): string {
  const names = attendees.map((a) => a.name || a.email).filter(Boolean);
  if (names.length === 0) return "";
  if (names.length === 1) return names[0];
  if (names.length === 2) return `${names[0]} and ${names[1]}`;
  const rest = names.length - 2;
  return `${names[0]}, ${names[1]} and ${rest} ${rest === 1 ? "other" : "others"}`;
}

/** "9:00 AM to 9:30 AM" in the user's locale, or "All day". */
export function timeRange(e: CalendarEvent, locale?: string): string {
  if (e.allDay) return "All day";
  const f = (ms: number) => new Date(ms).toLocaleTimeString(locale, { hour: "numeric", minute: "2-digit" });
  return `${f(e.startMs)} to ${f(e.endMs)}`;
}

/** "in 5 min", "in 2 h", "now", for an event's start relative to `now`. */
export function startsIn(e: CalendarEvent, now = Date.now()): string {
  if (now >= e.startMs) return now < e.endMs ? "now" : "ended";
  const mins = Math.ceil((e.startMs - now) / 60000);
  if (mins < 60) return `in ${mins} min`;
  const hours = Math.round(mins / 60);
  return `in ${hours} h`;
}
