//! Notices when a meeting app starts using the microphone, so Parley can offer to record.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Apps whose microphone use almost always means a call. Browsers are included for Google Meet,
/// Teams and Zoom on the web; the notification names the browser in that case.
const MEETING_APPS: &[(&str, &str)] = &[
    ("us.zoom.xos", "Zoom"),
    ("com.microsoft.teams2", "Microsoft Teams"),
    ("com.microsoft.teams", "Microsoft Teams"),
    ("com.tinyspeck.slackmacgap", "Slack"),
    ("com.apple.FaceTime", "FaceTime"),
    ("com.cisco.webexmeetingsapp", "Webex"),
    ("Cisco-Systems.Spark", "Webex"),
    ("com.hnc.Discord", "Discord"),
    ("net.whatsapp.WhatsApp", "WhatsApp"),
    ("com.google.Chrome", "Google Chrome"),
    ("com.apple.Safari", "Safari"),
    ("company.thebrowser.Browser", "Arc"),
    ("org.mozilla.firefox", "Firefox"),
    ("com.microsoft.edgemac", "Microsoft Edge"),
    ("com.brave.Browser", "Brave"),
];

pub fn meeting_app_name(bundle_id: &str) -> Option<&'static str> {
    MEETING_APPS
        .iter()
        .find(|(id, _)| *id == bundle_id)
        .map(|(_, name)| *name)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detection {
    pub bundle_id: String,
    pub app_name: String,
}

/// Turns the once-a-second list of apps using the mic into "a meeting started" events.
/// An app counts once it has held the mic for `settle` (skipping a quick mic test), and is
/// reported once per session: it re-arms only after it stops using the mic.
pub struct Detector {
    settle: Duration,
    since: HashMap<String, Instant>,
    reported: HashSet<String>,
}

impl Detector {
    pub fn new(settle: Duration) -> Self {
        Self {
            settle,
            since: HashMap::new(),
            reported: HashSet::new(),
        }
    }

    /// `suppress` is true while Parley is already recording: nothing is reported then, and apps
    /// that were in a call during the recording aren't reported afterwards either.
    pub fn update(&mut self, apps: &[String], now: Instant, suppress: bool) -> Vec<Detection> {
        let active: HashSet<&str> = apps
            .iter()
            .map(String::as_str)
            .filter(|id| meeting_app_name(id).is_some())
            .collect();
        self.since.retain(|id, _| active.contains(id.as_str()));
        self.reported.retain(|id| active.contains(id.as_str()));

        let mut out = Vec::new();
        for id in active {
            let first_seen = *self.since.entry(id.to_string()).or_insert(now);
            if self.reported.contains(id) {
                continue;
            }
            if suppress {
                self.reported.insert(id.to_string());
                continue;
            }
            if now.duration_since(first_seen) >= self.settle {
                self.reported.insert(id.to_string());
                out.push(Detection {
                    bundle_id: id.to_string(),
                    app_name: meeting_app_name(id).unwrap_or(id).to_string(),
                });
            }
        }
        out.sort_by(|a, b| a.bundle_id.cmp(&b.bundle_id));
        out
    }
}

/// Parses one line of `parley-micwatch` output.
pub fn parse_line(line: &str) -> Option<Vec<String>> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    Some(
        v["apps"]
            .as_array()?
            .iter()
            .filter_map(|a| a.as_str().map(str::to_string))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apps(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_meeting_app_is_reported_once_after_settling() {
        let t0 = Instant::now();
        let mut d = Detector::new(Duration::from_secs(3));
        assert!(d.update(&apps(&["us.zoom.xos"]), t0, false).is_empty());
        assert!(
            d.update(&apps(&["us.zoom.xos"]), t0 + Duration::from_secs(2), false)
                .is_empty()
        );
        let hit = d.update(&apps(&["us.zoom.xos"]), t0 + Duration::from_secs(3), false);
        assert_eq!(
            hit,
            vec![Detection {
                bundle_id: "us.zoom.xos".into(),
                app_name: "Zoom".into()
            }]
        );
        // Still in the same call: no repeat.
        assert!(
            d.update(&apps(&["us.zoom.xos"]), t0 + Duration::from_secs(60), false)
                .is_empty()
        );
    }

    #[test]
    fn a_quick_mic_check_is_ignored() {
        let t0 = Instant::now();
        let mut d = Detector::new(Duration::from_secs(3));
        d.update(&apps(&["com.apple.FaceTime"]), t0, false);
        d.update(&apps(&[]), t0 + Duration::from_secs(1), false);
        assert!(
            d.update(
                &apps(&["com.apple.FaceTime"]),
                t0 + Duration::from_secs(3),
                false
            )
            .is_empty()
        );
    }

    #[test]
    fn a_new_call_after_hanging_up_is_reported_again() {
        let t0 = Instant::now();
        let mut d = Detector::new(Duration::from_secs(1));
        d.update(&apps(&["com.microsoft.teams2"]), t0, false);
        assert_eq!(
            d.update(
                &apps(&["com.microsoft.teams2"]),
                t0 + Duration::from_secs(1),
                false
            )
            .len(),
            1
        );
        d.update(&apps(&[]), t0 + Duration::from_secs(5), false);
        d.update(
            &apps(&["com.microsoft.teams2"]),
            t0 + Duration::from_secs(10),
            false,
        );
        assert_eq!(
            d.update(
                &apps(&["com.microsoft.teams2"]),
                t0 + Duration::from_secs(11),
                false
            )
            .len(),
            1
        );
    }

    #[test]
    fn unknown_apps_and_parley_itself_are_ignored() {
        let t0 = Instant::now();
        let mut d = Detector::new(Duration::ZERO);
        let hits = d.update(
            &apps(&[
                "com.eshaanjain.parley",
                "com.example.voicememos",
                "com.google.Chrome",
            ]),
            t0,
            false,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].app_name, "Google Chrome");
    }

    #[test]
    fn calls_during_a_recording_are_not_reported_later() {
        let t0 = Instant::now();
        let mut d = Detector::new(Duration::ZERO);
        assert!(d.update(&apps(&["us.zoom.xos"]), t0, true).is_empty());
        // Recording stopped but the same call continues: still quiet.
        assert!(
            d.update(&apps(&["us.zoom.xos"]), t0 + Duration::from_secs(5), false)
                .is_empty()
        );
    }

    #[test]
    fn parses_watcher_lines() {
        assert_eq!(
            parse_line(r#"{"apps":["us.zoom.xos","com.apple.Safari"]}"#),
            Some(apps(&["us.zoom.xos", "com.apple.Safari"]))
        );
        assert_eq!(parse_line(r#"{"apps":[]}"#), Some(vec![]));
        assert_eq!(parse_line("garbage"), None);
    }
}
