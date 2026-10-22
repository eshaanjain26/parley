//! Upcoming meetings from the macOS Calendar, read by the `parley-calendar` helper (EventKit).
//! Google, Exchange and iCloud calendars added to the Mac all come through it; nothing here
//! talks to a calendar server.

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attendee {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    /// Unique per occurrence (a repeating event's occurrences differ by start time).
    pub id: String,
    /// Shared by every occurrence of a repeating event; `None` for one-off events.
    pub series_id: Option<String>,
    pub calendar_id: String,
    pub title: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub all_day: bool,
    #[serde(default)]
    pub attendees: Vec<Attendee>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub location: String,
    /// Invite text, used only to find call links. Never sent to the interface or stored.
    #[serde(default, skip_serializing)]
    pub notes: String,
    /// The video-call link, found in the URL, location or notes.
    #[serde(default)]
    pub call_link: Option<crate::links::CallLink>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarInfo {
    pub id: String,
    pub title: String,
    pub source: String,
    pub color: String,
}

/// How early a recording can start and still belong to an event.
pub const EARLY_START_MS: i64 = 10 * 60 * 1000;

fn helper() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(exe.with_file_name("parley-calendar"))
}

/// Runs the helper and returns its stdout, or the error it reported.
fn run(args: &[&str]) -> Result<String, String> {
    let out = Command::new(helper()?)
        .args(args)
        .output()
        .map_err(|e| format!("couldn't run the calendar helper: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if !out.status.success() {
        return Err(helper_error(&stdout).unwrap_or_else(|| "the calendar helper failed".into()));
    }
    Ok(stdout)
}

fn helper_error(stdout: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct E {
        error: String,
    }
    serde_json::from_str::<E>(stdout.trim())
        .ok()
        .map(|e| e.error)
}

pub fn parse_status(stdout: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct S {
        status: String,
    }
    serde_json::from_str::<S>(stdout.trim())
        .map(|s| s.status)
        .map_err(|e| format!("unexpected calendar status: {e}"))
}

/// Parses the helper's events and finds each one's call link. Any `callLink` in the input is
/// ignored: links come only from our own extraction.
pub fn parse_events(stdout: &str) -> Result<Vec<CalendarEvent>, String> {
    let mut events: Vec<CalendarEvent> = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("unexpected calendar data: {e}"))?;
    for e in &mut events {
        e.call_link = crate::links::find(&e.url, &e.location, &e.notes);
    }
    Ok(events)
}

/// "fullAccess", "notDetermined", "denied", "restricted" or "writeOnly".
pub fn status() -> Result<String, String> {
    parse_status(&run(&["status"])?)
}

/// Shows macOS's calendar prompt the first time; returns the resulting status. Blocks until the
/// user answers, so call it off the main thread.
pub fn request_access() -> Result<String, String> {
    parse_status(&run(&["request"])?)
}

pub fn calendars() -> Result<Vec<CalendarInfo>, String> {
    serde_json::from_str(run(&["calendars"])?.trim())
        .map_err(|e| format!("unexpected calendar list: {e}"))
}

/// Events between two times, limited to `calendar_ids` (empty means every calendar).
pub fn events(
    from_ms: i64,
    to_ms: i64,
    calendar_ids: &[String],
) -> Result<Vec<CalendarEvent>, String> {
    let all = parse_events(&run(&["events", &from_ms.to_string(), &to_ms.to_string()])?)?;
    Ok(only_calendars(all, calendar_ids))
}

pub fn only_calendars(events: Vec<CalendarEvent>, calendar_ids: &[String]) -> Vec<CalendarEvent> {
    if calendar_ids.is_empty() {
        return events;
    }
    events
        .into_iter()
        .filter(|e| calendar_ids.contains(&e.calendar_id))
        .collect()
}

/// The event a recording starting at `now_ms` belongs to: a timed event that is in progress or
/// starts within the next few minutes. With overlaps, the one that started (or starts) closest
/// to now wins, so back-to-back meetings pick the new one.
pub fn current_event(events: &[CalendarEvent], now_ms: i64) -> Option<&CalendarEvent> {
    events
        .iter()
        .filter(|e| !e.all_day && e.start_ms - EARLY_START_MS <= now_ms && now_ms < e.end_ms)
        .min_by_key(|e| (e.start_ms - now_ms).abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    fn event(id: &str, start_min: i64, end_min: i64) -> CalendarEvent {
        CalendarEvent {
            id: id.into(),
            series_id: None,
            calendar_id: "work".into(),
            title: id.into(),
            start_ms: start_min * MIN,
            end_ms: end_min * MIN,
            all_day: false,
            attendees: vec![],
            url: String::new(),
            location: String::new(),
            notes: String::new(),
            call_link: None,
        }
    }

    #[test]
    fn parses_helper_output_and_keeps_notes_out_of_the_interface() {
        let json = r#"[{"id":"abc:1000","seriesId":"abc","calendarId":"work","title":"Weekly 1:1",
            "startMs":1000,"endMs":2000,"allDay":false,
            "attendees":[{"name":"Priya Shah","email":"priya@example.com"}],
            "url":"","location":"Room 4","notes":"Join: https://example.com/j/123"}]"#;
        let events = parse_events(json).unwrap();
        assert_eq!(events[0].series_id.as_deref(), Some("abc"));
        assert_eq!(events[0].attendees[0].email, "priya@example.com");
        assert!(events[0].notes.contains("example.com"));
        // example.com isn't a call provider, so no link.
        assert_eq!(events[0].call_link, None);
        let wire = serde_json::to_value(&events[0]).unwrap();
        assert!(wire.get("notes").is_none());
        assert_eq!(wire["startMs"], 1000);
        let with_call = parse_events(
            r#"[{"id":"x:1","seriesId":null,"calendarId":"w","title":"Sync","startMs":1,"endMs":2,"allDay":false,
                "notes":"Join https://meet.google.com/abc-defg-hij","callLink":{"provider":"zoom","url":"https://evil.example"}}]"#,
        )
        .unwrap();
        let link = with_call[0].call_link.as_ref().unwrap();
        assert_eq!(link.url, "https://meet.google.com/abc-defg-hij");
        assert_eq!(
            serde_json::to_value(&with_call[0]).unwrap()["callLink"]["provider"],
            "meet"
        );
    }

    #[test]
    fn reads_status_and_helper_errors() {
        assert_eq!(
            parse_status(r#"{"status":"fullAccess"}"#).unwrap(),
            "fullAccess"
        );
        assert!(parse_status("garbage").is_err());
        assert_eq!(
            helper_error(r#"{"error":"no calendar access"}"#).as_deref(),
            Some("no calendar access")
        );
        assert_eq!(helper_error("[]"), None);
    }

    #[test]
    fn current_event_covers_early_starts_and_skips_all_day_and_finished_events() {
        let mut all_day = event("holiday", 0, 24 * 60);
        all_day.all_day = true;
        let events = vec![all_day, event("standup", 60, 75), event("done", 0, 30)];
        // 8 minutes early still counts; 11 minutes early does not.
        assert_eq!(
            current_event(&events, 52 * MIN).map(|e| e.id.as_str()),
            Some("standup")
        );
        assert_eq!(current_event(&events, 49 * MIN), None);
        assert_eq!(
            current_event(&events, 74 * MIN).map(|e| e.id.as_str()),
            Some("standup")
        );
        assert_eq!(current_event(&events, 75 * MIN), None);
    }

    #[test]
    fn back_to_back_meetings_pick_the_one_starting_now() {
        let events = vec![event("first", 0, 30), event("second", 30, 60)];
        // Two minutes before the hand-over, both match; the next meeting is closer.
        assert_eq!(
            current_event(&events, 28 * MIN).map(|e| e.id.as_str()),
            Some("second")
        );
        assert_eq!(
            current_event(&events, 10 * MIN).map(|e| e.id.as_str()),
            Some("first")
        );
    }

    #[test]
    fn calendar_filter_keeps_chosen_calendars_or_all() {
        let mut personal = event("gym", 0, 60);
        personal.calendar_id = "personal".into();
        let events = vec![event("standup", 0, 15), personal];
        assert_eq!(only_calendars(events.clone(), &[]).len(), 2);
        let work = only_calendars(events, &["work".into()]);
        assert_eq!(
            work.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            ["standup"]
        );
    }
}
