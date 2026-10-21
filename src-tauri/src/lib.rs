mod capture;
mod dsp;
mod settings;

use capture::Recording;
use settings::Settings;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager, State};

fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|d| d.join("settings.json"))
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    name: String,
    version: String,
}

#[tauri::command]
fn app_info(app: tauri::AppHandle) -> AppInfo {
    let pkg = app.package_info();
    AppInfo {
        name: pkg.name.clone(),
        version: pkg.version.to_string(),
    }
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> Result<Settings, String> {
    settings::load(&settings_path(&app)?).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_settings(app: tauri::AppHandle, settings: Settings) -> Result<Settings, String> {
    settings::save(&settings_path(&app)?, &settings).map_err(|e| e.to_string())?;
    Ok(settings)
}

struct ActiveRecording {
    id: String,
    recording: Recording,
    meters_running: Arc<AtomicBool>,
}

#[derive(Default)]
struct RecorderState(Mutex<Option<ActiveRecording>>);

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordingStarted {
    id: String,
    system_audio: bool,
    warning: Option<String>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AudioLevels {
    mic_dbfs: f32,
    system_dbfs: f32,
    seconds: f64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordingFinished {
    id: String,
    duration_secs: f64,
    mic_path: String,
    system_path: Option<String>,
}

/// The Swift sidecar is bundled next to the main executable (Contents/MacOS in the .app).
fn sidecar_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(exe.with_file_name("parley-audiotap"))
}

// Async so Tauri runs it off the main thread: starting can wait on permission prompts.
#[tauri::command]
async fn start_recording(
    app: tauri::AppHandle,
    state: State<'_, RecorderState>,
) -> Result<RecordingStarted, String> {
    let mut active = state.0.lock().map_err(|e| e.to_string())?;
    if active.is_some() {
        return Err("Already recording".into());
    }
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis()
        .to_string();
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("recordings")
        .join(&id);
    let recording = Recording::start(dir, &sidecar_path()?)?;

    let meters_running = Arc::new(AtomicBool::new(true));
    let (mic, system, running) = (
        recording.mic_stats.clone(),
        recording.system_stats.clone(),
        meters_running.clone(),
    );
    let emitter = app.clone();
    std::thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            let _ = emitter.emit(
                "audio-levels",
                AudioLevels {
                    mic_dbfs: mic.level_dbfs(),
                    system_dbfs: system.level_dbfs(),
                    seconds: mic.seconds().max(system.seconds()),
                },
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    });

    let started = RecordingStarted {
        id: id.clone(),
        system_audio: recording.system_warning.is_none(),
        warning: recording.system_warning.clone(),
    };
    *active = Some(ActiveRecording {
        id,
        recording,
        meters_running,
    });
    Ok(started)
}

#[tauri::command]
async fn stop_recording(state: State<'_, RecorderState>) -> Result<RecordingFinished, String> {
    let active = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("Not recording")?;
    active.meters_running.store(false, Ordering::Relaxed);
    let done = active.recording.stop()?;
    Ok(RecordingFinished {
        id: active.id,
        duration_secs: done.duration_secs,
        mic_path: done.mic_path.display().to_string(),
        system_path: done.system_path.map(|p| p.display().to_string()),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(RecorderState::default())
        .invoke_handler(tauri::generate_handler![
            app_info,
            get_settings,
            update_settings,
            start_recording,
            stop_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running Parley");
}
