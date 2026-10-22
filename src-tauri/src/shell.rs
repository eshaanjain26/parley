//! macOS integration outside the main window: the menu-bar item, the global hotkey, the
//! "meeting detected" watcher (which also starts and stops automatic recordings) and calendar
//! reminders.

use crate::autorec::{self, CallChange};
use crate::calendar::{self, CalendarEvent};
use crate::detect::{self, Detector};
use crate::{CallWatchState, RecorderState, settings, settings_path};
use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_notification::NotificationExt;

/// Start or stop recording from anywhere: ⌥⇧⌘R. Three modifiers keep it clear of app shortcuts.
pub const HOTKEY: &str = "Alt+Shift+Super+KeyR";
pub const HOTKEY_LABEL: &str = "⌥⇧⌘R";

const TRAY_ID: &str = "parley";
const MENU_TOGGLE: &str = "toggle";
const MENU_SHOW: &str = "show";
const MENU_QUIT: &str = "quit";

/// The menu-bar item whose label follows the recording state.
pub struct TrayToggle(MenuItem<Wry>);

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// The UI owns the start/stop flow (title, warnings, model download), so the menu bar and the
/// hotkey ask it to toggle instead of duplicating that logic here.
pub fn request_toggle(app: &AppHandle) {
    let _ = app.emit("toggle-recording", ());
}

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(
        app,
        MENU_TOGGLE,
        "Start recording",
        true,
        Some("Alt+Shift+Cmd+R"),
    )?;
    let show = MenuItem::with_id(app, MENU_SHOW, "Show Parley", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit Parley", true, Some("Cmd+Q"))?;
    let menu = Menu::with_items(
        app,
        &[&toggle, &show, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!(
            "../icons/tray-template.png"
        ))?)
        .icon_as_template(true)
        .tooltip("Parley")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_TOGGLE => request_toggle(app),
            MENU_SHOW => show_window(app),
            MENU_QUIT => app.exit(0),
            _ => {}
        })
        .build(app)?;
    app.manage(TrayToggle(toggle));
    Ok(())
}

/// Keeps the menu-bar label and the "REC" marker in step with the recorder.
pub fn set_recording_indicator(app: &AppHandle, recording: bool) {
    if let Some(toggle) = app.try_state::<TrayToggle>() {
        let _ = toggle.0.set_text(if recording {
            "Stop recording"
        } else {
            "Start recording"
        });
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_title(if recording { Some("REC") } else { None });
    }
}

pub fn global_shortcut_plugin() -> tauri::plugin::TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                request_toggle(app);
            }
        })
        .build()
}

/// Registers or removes the hotkey to match the user's setting.
pub fn apply_hotkey(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    let registered = shortcuts.is_registered(HOTKEY);
    match (enabled, registered) {
        (true, false) => shortcuts
            .register(HOTKEY)
            .map_err(|e| format!("couldn't register {HOTKEY_LABEL}: {e}")),
        (false, true) => shortcuts.unregister(HOTKEY).map_err(|e| e.to_string()),
        _ => Ok(()),
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct MeetingDetected {
    bundle_id: String,
    app_name: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AutoRecord {
    event: CalendarEvent,
    bundle_id: String,
    app_name: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CallChangePayload {
    /// "ended", "resumed" or "stop".
    kind: &'static str,
    seconds: u64,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn load_settings(app: &AppHandle) -> settings::Settings {
    settings_path(app)
        .ok()
        .and_then(|p| settings::load(&p).ok())
        .unwrap_or_default()
}

/// The calendar event happening now, if the calendar is on.
fn event_now(prefs: &settings::Settings) -> Option<CalendarEvent> {
    if !prefs.calendar_enabled {
        return None;
    }
    let now = now_ms();
    let events = calendar::events(
        now - 12 * 60 * 60 * 1000,
        now + calendar::EARLY_START_MS,
        &prefs.calendar_ids,
    )
    .ok()?;
    calendar::current_event(&events, now).cloned()
}

/// Runs the mic watcher sidecar for the life of the app and turns its reports into
/// notifications (when enabled in Settings) and a `meeting-detected` event for the UI.
pub fn start_meeting_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let child = Command::new(exe.with_file_name("parley-micwatch"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else {
            eprintln!("meeting detection unavailable: couldn't start parley-micwatch");
            return;
        };
        // Held open for the life of the app; the watcher exits when it closes.
        let _stdin = child.stdin.take();
        let Some(stdout) = child.stdout.take() else {
            return;
        };
        let mut detector = Detector::new(Duration::from_secs(3));
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Some(apps) = detect::parse_line(&line) else {
                continue;
            };
            let recording = app
                .state::<RecorderState>()
                .0
                .lock()
                .map(|r| r.is_some())
                .unwrap_or(false);
            follow_call(&app, &apps, recording);
            for hit in detector.update(&apps, Instant::now(), recording) {
                let prefs = load_settings(&app);
                if prefs.auto_record {
                    let current = event_now(&prefs);
                    if let Some(event) =
                        autorec::event_to_auto_record(&hit.bundle_id, current.as_ref())
                    {
                        let _ = app
                            .notification()
                            .builder()
                            .title(format!("Recording \u{201c}{}\u{201d}", event.title))
                            .body(format!(
                                "{} joined the call, so Parley started notes. Let everyone know you're recording.",
                                hit.app_name
                            ))
                            .show();
                        let _ = app.emit(
                            "auto-record",
                            AutoRecord {
                                event: event.clone(),
                                bundle_id: hit.bundle_id.clone(),
                                app_name: hit.app_name.clone(),
                            },
                        );
                        continue;
                    }
                }
                if !prefs.detect_meetings {
                    continue;
                }
                let _ = app.emit(
                    "meeting-detected",
                    MeetingDetected {
                        bundle_id: hit.bundle_id.clone(),
                        app_name: hit.app_name.clone(),
                    },
                );
                let _ = app
                    .notification()
                    .builder()
                    .title(format!("Meeting in {}?", hit.app_name))
                    .body(format!(
                        "Press {HOTKEY_LABEL} or use Parley in the menu bar to take notes."
                    ))
                    .show();
            }
        }
    });
}

/// Tells the UI when the recording's call app leaves the call, comes back, or has been gone
/// long enough to stop.
fn follow_call(app: &AppHandle, apps: &[String], recording: bool) {
    let state = app.state::<CallWatchState>();
    let Ok(mut watch) = state.0.lock() else {
        return;
    };
    if !recording || !watch.is_active() {
        return;
    }
    let payload = match watch.update(apps, Instant::now()) {
        Some(CallChange::Ended { seconds }) => CallChangePayload {
            kind: "ended",
            seconds,
        },
        Some(CallChange::Resumed) => CallChangePayload {
            kind: "resumed",
            seconds: 0,
        },
        Some(CallChange::Stop) => CallChangePayload {
            kind: "stop",
            seconds: 0,
        },
        None => return,
    };
    let _ = app.emit("call-change", payload);
}

/// Checks the calendar every 30 seconds and sends a notification shortly before each call.
pub fn start_reminders(app: AppHandle) {
    std::thread::spawn(move || {
        let mut reminded = HashSet::new();
        loop {
            let prefs = load_settings(&app);
            if prefs.calendar_enabled && prefs.event_reminders {
                let now = now_ms();
                if let Ok(events) = calendar::events(now, now + 3 * 60 * 1000, &prefs.calendar_ids)
                {
                    for e in autorec::due_reminders(&events, now, &mut reminded) {
                        let provider = e
                            .call_link
                            .as_ref()
                            .map(|l| l.provider.label())
                            .unwrap_or("The call");
                        let _ = app
                            .notification()
                            .builder()
                            .title(format!("\u{201c}{}\u{201d} starts in a minute", e.title))
                            .body(format!("{provider} call. Join from Parley's Home to record it, or press {HOTKEY_LABEL} once you're in."))
                            .show();
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(30));
        }
    });
}
