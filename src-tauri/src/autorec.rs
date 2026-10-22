//! Calendar-driven automation: reminders before calls, starting a recording when the call app
//! joins, and stopping it after the call app lets go of the microphone. Pure logic; the mic
//! watcher in `shell.rs` feeds it and acts on what it returns.

use crate::calendar::CalendarEvent;
use crate::links::Provider;
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Reminders go out this long before a call starts.
pub const REMIND_BEFORE_MS: i64 = 75_000;
/// How long the call app must stay off the microphone before an automatic stop.
pub const STOP_GRACE: Duration = Duration::from_secs(30);

/// Events with a call link that start within the reminder window and haven't had a reminder.
/// Marks them as reminded.
pub fn due_reminders<'a>(
    events: &'a [CalendarEvent],
    now_ms: i64,
    reminded: &mut HashSet<String>,
) -> Vec<&'a CalendarEvent> {
    events
        .iter()
        .filter(|e| !e.all_day && e.call_link.is_some())
        .filter(|e| {
            let until = e.start_ms - now_ms;
            until > 0 && until <= REMIND_BEFORE_MS
        })
        .filter(|e| reminded.insert(e.id.clone()))
        .collect()
}

/// The event to record when `bundle_id` (a meeting app that just took the microphone) joins a
/// call: the current event, if its call link belongs to a provider that runs in that app.
pub fn event_to_auto_record<'a>(
    bundle_id: &str,
    current: Option<&'a CalendarEvent>,
) -> Option<&'a CalendarEvent> {
    current.filter(|e| {
        e.call_link
            .as_ref()
            .is_some_and(|l| l.provider.runs_in(bundle_id))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Matcher {
    App(String),
    Provider(Provider),
}

impl Matcher {
    fn matches(&self, bundle_id: &str) -> bool {
        match self {
            Matcher::App(id) => id == bundle_id,
            Matcher::Provider(p) => p.runs_in(bundle_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallChange {
    /// The call app let go of the microphone; recording stops after this many seconds.
    Ended { seconds: u64 },
    /// It came back before the grace period ran out.
    Resumed,
    /// Grace period over: stop the recording.
    Stop,
}

/// Watches the call app of the current recording and decides when it has really left the call.
#[derive(Debug, Default)]
pub struct CallWatch {
    matcher: Option<Matcher>,
    seen: bool,
    gone_since: Option<Instant>,
}

impl CallWatch {
    /// Follow one app (the one whose joining started an automatic recording).
    pub fn follow_app(&mut self, bundle_id: &str) {
        *self = Self {
            matcher: Some(Matcher::App(bundle_id.into())),
            ..Self::default()
        };
    }

    /// Follow whichever app runs this provider's call (after Join opened the link).
    pub fn follow_provider(&mut self, provider: Provider) {
        *self = Self {
            matcher: Some(Matcher::Provider(provider)),
            ..Self::default()
        };
    }

    /// Stop watching: the recording ended, or the user chose to keep recording.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn is_active(&self) -> bool {
        self.matcher.is_some()
    }

    /// Feed the apps holding the microphone right now.
    pub fn update(&mut self, apps: &[String], now: Instant) -> Option<CallChange> {
        let matcher = self.matcher.as_ref()?;
        if apps.iter().any(|a| matcher.matches(a)) {
            self.seen = true;
            return self.gone_since.take().map(|_| CallChange::Resumed);
        }
        // Until the call app has joined at least once, its absence means nothing.
        if !self.seen {
            return None;
        }
        match self.gone_since {
            None => {
                self.gone_since = Some(now);
                Some(CallChange::Ended {
                    seconds: STOP_GRACE.as_secs(),
                })
            }
            Some(since) if now.duration_since(since) >= STOP_GRACE => {
                self.clear();
                Some(CallChange::Stop)
            }
            Some(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links::CallLink;

    fn event(id: &str, start_ms: i64, link: Option<Provider>) -> CalendarEvent {
        CalendarEvent {
            id: id.into(),
            series_id: None,
            calendar_id: "work".into(),
            title: id.into(),
            start_ms,
            end_ms: start_ms + 30 * 60_000,
            all_day: false,
            attendees: vec![],
            url: String::new(),
            location: String::new(),
            notes: String::new(),
            call_link: link.map(|provider| CallLink {
                provider,
                url: "https://example.test".into(),
            }),
        }
    }

    fn apps(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn reminds_once_shortly_before_calls_only() {
        let events = vec![
            event("call", 100_000, Some(Provider::Zoom)),
            event("no-link", 100_000, None),
            event("later", 1_000_000, Some(Provider::Meet)),
        ];
        let mut reminded = HashSet::new();
        assert!(
            due_reminders(&events, 0, &mut reminded).is_empty(),
            "100 s ahead is too early"
        );
        let due = due_reminders(&events, 30_000, &mut reminded);
        assert_eq!(
            due.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            ["call"]
        );
        assert!(
            due_reminders(&events, 40_000, &mut reminded).is_empty(),
            "only once"
        );
        assert!(
            due_reminders(&events, 100_000, &mut HashSet::new()).is_empty(),
            "not after it started"
        );
    }

    #[test]
    fn auto_records_only_when_the_app_runs_the_events_call() {
        let zoom_call = event("standup", 0, Some(Provider::Zoom));
        assert_eq!(
            event_to_auto_record("us.zoom.xos", Some(&zoom_call)).map(|e| e.id.as_str()),
            Some("standup")
        );
        assert_eq!(
            event_to_auto_record("com.google.Chrome", Some(&zoom_call)).map(|e| e.id.as_str()),
            Some("standup")
        );
        assert_eq!(
            event_to_auto_record("com.microsoft.teams2", Some(&zoom_call)),
            None
        );
        assert_eq!(
            event_to_auto_record("us.zoom.xos", Some(&event("focus", 0, None))),
            None
        );
        assert_eq!(event_to_auto_record("us.zoom.xos", None), None);
    }

    #[test]
    fn stops_after_the_call_app_stays_away_for_the_grace_period() {
        let t0 = Instant::now();
        let mut w = CallWatch::default();
        assert_eq!(w.update(&apps(&[]), t0), None, "not watching yet");
        w.follow_app("us.zoom.xos");
        assert_eq!(w.update(&apps(&["us.zoom.xos"]), t0), None);
        assert_eq!(
            w.update(&apps(&[]), t0 + Duration::from_secs(1)),
            Some(CallChange::Ended { seconds: 30 })
        );
        assert_eq!(w.update(&apps(&[]), t0 + Duration::from_secs(20)), None);
        assert_eq!(
            w.update(&apps(&[]), t0 + Duration::from_secs(31)),
            Some(CallChange::Stop)
        );
        assert!(!w.is_active());
        assert_eq!(w.update(&apps(&[]), t0 + Duration::from_secs(40)), None);
    }

    #[test]
    fn a_brief_drop_resumes_and_resets_the_countdown() {
        let t0 = Instant::now();
        let mut w = CallWatch::default();
        w.follow_provider(Provider::Teams);
        // Join opened the link but Teams hasn't taken the mic yet: nothing happens.
        assert_eq!(w.update(&apps(&[]), t0), None);
        assert_eq!(
            w.update(
                &apps(&["com.microsoft.teams2"]),
                t0 + Duration::from_secs(5)
            ),
            None
        );
        assert!(matches!(
            w.update(&apps(&[]), t0 + Duration::from_secs(6)),
            Some(CallChange::Ended { .. })
        ));
        assert_eq!(
            w.update(
                &apps(&["com.microsoft.teams2"]),
                t0 + Duration::from_secs(10)
            ),
            Some(CallChange::Resumed)
        );
        assert!(matches!(
            w.update(&apps(&[]), t0 + Duration::from_secs(50)),
            Some(CallChange::Ended { .. })
        ));
        assert_eq!(
            w.update(&apps(&[]), t0 + Duration::from_secs(60)),
            None,
            "countdown restarted at 50 s"
        );
    }

    #[test]
    fn clearing_cancels_a_pending_stop() {
        let t0 = Instant::now();
        let mut w = CallWatch::default();
        w.follow_app("us.zoom.xos");
        w.update(&apps(&["us.zoom.xos"]), t0);
        w.update(&apps(&[]), t0 + Duration::from_secs(1));
        w.clear();
        assert_eq!(w.update(&apps(&[]), t0 + Duration::from_secs(60)), None);
    }
}
