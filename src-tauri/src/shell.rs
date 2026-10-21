//! macOS integration outside the main window: the menu-bar item, the global hotkey, and the
//! "meeting detected" watcher.

use crate::detect::{self, Detector};
use crate::{RecorderState, settings, settings_path};
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
            for hit in detector.update(&apps, Instant::now(), recording) {
                let enabled = settings_path(&app)
                    .ok()
                    .and_then(|p| settings::load(&p).ok())
                    .map(|s| s.detect_meetings)
                    .unwrap_or(true);
                if !enabled {
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
