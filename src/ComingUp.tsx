import { CalendarDays, Mic, Users, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useRecording } from "./RecordingContext";
import { CalendarEvent, attendeeSummary, canRecordNow, connectCalendar, startsIn, timeRange, upcomingEvents } from "./lib/calendar";
import { daysAgo } from "./lib/dates";
import { Settings, getSettings } from "./lib/settings";

const DISMISS_KEY = "parley.calendarPromptDismissed";
const REFRESH_MS = 60_000;

const endOfTomorrow = (now: Date) => new Date(now.getFullYear(), now.getMonth(), now.getDate() + 2).getTime();

/** Today's remaining and tomorrow's events from the Mac's calendars, with one-click recording. */
export function ComingUp({ onOpenSettings }: { onOpenSettings: () => void }) {
  const rec = useRecording();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [events, setEvents] = useState<CalendarEvent[] | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [notice, setNotice] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(() => {
    try {
      return localStorage.getItem(DISMISS_KEY) === "1";
    } catch {
      return false;
    }
  });

  const load = useCallback(() => {
    const t = Date.now();
    setNow(t);
    upcomingEvents(t - 60 * 60 * 1000, endOfTomorrow(new Date(t)))
      .then((list) => setEvents(list.filter((e) => e.endMs > t && !e.allDay)))
      .catch(() => setEvents([]));
  }, []);

  useEffect(() => {
    getSettings().then(setSettings).catch(() => {});
  }, []);

  useEffect(() => {
    if (!settings?.calendarEnabled) return;
    load();
    const id = setInterval(load, REFRESH_MS);
    return () => clearInterval(id);
  }, [settings?.calendarEnabled, load]);

  const connect = async () => {
    setNotice(null);
    try {
      const status = await connectCalendar();
      if (status === "fullAccess") {
        setSettings(await getSettings());
      } else if (status === "denied" || status === "restricted") {
        setNotice("Calendar access is off. Turn on Parley in System Settings > Privacy & Security > Calendars, then try again.");
      } else {
        setNotice("Parley needs full calendar access to read your events.");
      }
    } catch (e) {
      setNotice(String(e));
    }
  };

  if (!settings) return null;

  if (!settings.calendarEnabled) {
    if (dismissed) return null;
    return (
      <section className="connect-card" aria-label="Connect your calendar">
        <span className="hero-icon" aria-hidden><CalendarDays size={20} /></span>
        <div className="hero-text">
          <strong>See your upcoming meetings here</strong>
          <span className="muted">Parley reads the calendars on this Mac (Google, Outlook and iCloud included) to name each recording after its meeting. Nothing is uploaded.</span>
          {notice && <span className="error" role="alert">{notice}</span>}
        </div>
        <button
          className="icon-button"
          aria-label="Not now"
          onClick={() => {
            setDismissed(true);
            try {
              localStorage.setItem(DISMISS_KEY, "1");
            } catch {
              // Only hides the prompt for this session then.
            }
          }}
        >
          <X size={16} aria-hidden />
        </button>
        <button className="secondary" onClick={connect}>Connect calendar</button>
      </section>
    );
  }

  const busy = rec.phase !== "idle";
  const today = (events ?? []).filter((e) => daysAgo(new Date(e.startMs), new Date(now)) >= 0);
  const tomorrow = (events ?? []).filter((e) => daysAgo(new Date(e.startMs), new Date(now)) < 0);

  return (
    <section className="coming-up" aria-labelledby="coming-up-title">
      <div className="section-head">
        <h2 id="coming-up-title" className="day-label">Coming up</h2>
        <button className="ghost small-ghost" onClick={onOpenSettings}>Calendars</button>
      </div>
      {events === null ? (
        <div className="skeleton-row" aria-label="Loading events" />
      ) : events.length === 0 ? (
        <p className="muted coming-empty">Nothing else on your calendar today or tomorrow.</p>
      ) : (
        <div className="event-card">
          {[
            { label: "Today", items: today },
            { label: "Tomorrow", items: tomorrow },
          ]
            .filter((g) => g.items.length > 0)
            .map((g) => (
              <div key={g.label} className="event-day">
                <h3 className="event-day-label">{g.label}</h3>
                <ul>
                  {g.items.map((e) => {
                    const live = canRecordNow(e, now);
                    const people = attendeeSummary(e.attendees);
                    return (
                      <li key={e.id} className={`event-row${live ? " live" : ""}`}>
                        <div className="event-main">
                          <span className="event-title">{e.title || "Untitled event"}</span>
                          <span className="event-meta">
                            {timeRange(e)}
                            {people && (
                              <>
                                <Users size={13} aria-hidden />
                                <span title={e.attendees.map((a) => a.name || a.email).join(", ")}>{people}</span>
                              </>
                            )}
                          </span>
                        </div>
                        {live ? (
                          <button className="primary" disabled={busy} onClick={() => void rec.start(null, e)}>
                            <Mic size={15} aria-hidden />
                            Record
                          </button>
                        ) : (
                          <span className="event-when">{startsIn(e, now)}</span>
                        )}
                      </li>
                    );
                  })}
                </ul>
              </div>
            ))}
        </div>
      )}
    </section>
  );
}
