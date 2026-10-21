mod capture;
mod dsp;
mod enhance;
mod llm;
mod secrets;
mod settings;
mod store;
mod stt;
mod transcribe;

use capture::{Recording, Sidecars};
use settings::{LlmProvider, Settings, SpeechProvider};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use store::Store;
use stt::whisper::{WhisperEngine, WhisperModel};
use stt::{SpeechToText, models};
use tauri::{Emitter, Manager, State};
use transcribe::{Speaker, TranscriptLine};

fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|d| d.join("settings.json"))
        .map_err(|e| e.to_string())
}

fn models_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("models"))
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

#[tauri::command]
fn list_models(app: tauri::AppHandle) -> Result<Vec<models::ModelStatus>, String> {
    Ok(models::statuses(&models_dir(&app)?))
}

#[derive(Default)]
struct DownloadState(Mutex<Option<Arc<AtomicBool>>>);

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    id: String,
    received: u64,
    total: u64,
}

#[tauri::command]
async fn download_model(
    app: tauri::AppHandle,
    downloads: State<'_, DownloadState>,
    id: String,
) -> Result<(), String> {
    let total = models::find(&id)?.size_bytes;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut current = downloads.0.lock().map_err(|e| e.to_string())?;
        if current.is_some() {
            return Err("A model is already downloading".into());
        }
        *current = Some(cancel.clone());
    }
    let dir = models_dir(&app)?;
    let emitter = app.clone();
    let model_id = id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        models::download(
            &dir,
            &model_id,
            |received| {
                let _ = emitter.emit(
                    "model-download",
                    DownloadProgress {
                        id: model_id.clone(),
                        received,
                        total,
                    },
                );
            },
            &cancel,
        )
    })
    .await
    .map_err(|e| e.to_string());
    *downloads.0.lock().map_err(|e| e.to_string())? = None;
    result??;
    Ok(())
}

#[tauri::command]
fn cancel_model_download(downloads: State<'_, DownloadState>) -> Result<(), String> {
    if let Some(flag) = downloads.0.lock().map_err(|e| e.to_string())?.as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// Loading a model takes a moment, so keep the last one for the next meeting.
#[derive(Default)]
struct ModelCache(Mutex<Option<(PathBuf, WhisperModel)>>);

impl ModelCache {
    fn get(&self, path: PathBuf) -> Result<WhisperModel, String> {
        let mut cache = self.0.lock().map_err(|e| e.to_string())?;
        if let Some((cached, model)) = cache.as_ref()
            && *cached == path
        {
            return Ok(model.clone());
        }
        let model = WhisperModel::load(&path)?;
        *cache = Some((path, model.clone()));
        Ok(model)
    }
}

/// Keychain accounts for API keys the user can set.
const KEY_ACCOUNTS: &[&str] = &["deepgram", "assemblyai", "openai", "anthropic"];

/// Keychain account for a speech provider's API key; `None` for on-device engines.
fn key_account(provider: SpeechProvider) -> Option<&'static str> {
    match provider {
        SpeechProvider::LocalWhisper => None,
        SpeechProvider::Deepgram => Some("deepgram"),
        SpeechProvider::AssemblyAi => Some("assemblyai"),
    }
}

/// Error prefix the UI recognises to send the user to Settings for a missing key.
const KEY_MISSING: &str = "api-key-missing:";

fn required_key(account: &str) -> Result<String, String> {
    secrets::get(account)?.ok_or_else(|| format!("{KEY_MISSING}{account}"))
}

fn cloud_key(provider: SpeechProvider) -> Result<String, String> {
    required_key(key_account(provider).ok_or("not a cloud provider")?)
}

fn checked_account(account: &str) -> Result<&str, String> {
    KEY_ACCOUNTS
        .iter()
        .find(|a| **a == account)
        .copied()
        .ok_or_else(|| format!("unknown key {account:?}"))
}

#[tauri::command]
fn set_api_key(account: String, key: String) -> Result<(), String> {
    secrets::set(checked_account(&account)?, &key)
}

#[tauri::command]
fn delete_api_key(account: String) -> Result<(), String> {
    secrets::delete(checked_account(&account)?)
}

/// Which keys exist, by account; the keys themselves never leave the Rust side.
#[tauri::command]
fn api_key_status() -> Result<std::collections::BTreeMap<&'static str, bool>, String> {
    KEY_ACCOUNTS
        .iter()
        .map(|a| Ok((*a, secrets::get(a)?.is_some())))
        .collect()
}

#[tauri::command]
fn list_templates(store: State<'_, StoreState>) -> Result<Vec<enhance::Template>, String> {
    list_templates_from(store.get()?)
}

#[tauri::command]
fn save_template(
    store: State<'_, StoreState>,
    mut template: enhance::Template,
) -> Result<enhance::Template, String> {
    let now = now_ms()?;
    if template.id.is_empty() {
        template.id = format!("custom-{now}");
    }
    if enhance::built_in_templates()
        .iter()
        .any(|t| t.id == template.id)
    {
        return Err("built-in templates can't be changed; save a copy instead".into());
    }
    template.built_in = false;
    store.get()?.save_template(&template, now)?;
    Ok(template)
}

#[tauri::command]
fn delete_template(store: State<'_, StoreState>, id: String) -> Result<(), String> {
    store.get()?.delete_template(&id)
}

fn llm_provider(settings: &Settings) -> Result<llm::Provider, String> {
    Ok(match settings.llm_provider {
        LlmProvider::Ollama => llm::Provider::Ollama {
            url: settings.ollama_url.clone(),
            model: settings.ollama_model.clone(),
        },
        LlmProvider::OpenAiCompatible => {
            if settings.openai_model.trim().is_empty() {
                return Err(
                    "Set the model name for your OpenAI-compatible provider in Settings.".into(),
                );
            }
            llm::Provider::OpenAiCompatible {
                base_url: settings.openai_base_url.clone(),
                model: settings.openai_model.trim().to_string(),
                api_key: required_key("openai")?,
            }
        }
        LlmProvider::Anthropic => llm::Provider::Anthropic {
            base_url: "https://api.anthropic.com".into(),
            model: settings.anthropic_model.trim().to_string(),
            api_key: required_key("anthropic")?,
        },
    })
}

#[derive(Default)]
struct EnhanceState(Mutex<Option<Arc<AtomicBool>>>);

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EnhanceDelta {
    meeting_id: String,
    text: String,
}

/// Writes enhanced notes for a meeting, streaming `enhance-delta` events as text arrives, and
/// saves the result. One enhancement runs at a time.
#[tauri::command]
async fn enhance_meeting(
    app: tauri::AppHandle,
    store: State<'_, StoreState>,
    running: State<'_, EnhanceState>,
    meeting_id: String,
    template_id: String,
) -> Result<store::Meeting, String> {
    let store = store.get()?;
    let meeting = store.get_meeting(&meeting_id)?.ok_or("meeting not found")?;
    let template = list_templates_from(store)?
        .into_iter()
        .find(|t| t.id == template_id)
        .ok_or("template not found")?;
    let settings = settings::load(&settings_path(&app)?).map_err(|e| e.to_string())?;
    let provider = llm_provider(&settings)?;
    let prompt = enhance::build_prompt(
        &template,
        &meeting.title,
        &meeting.notes_text,
        &meeting.transcript,
        provider.transcript_budget_chars(),
    );

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = running.0.lock().map_err(|e| e.to_string())?;
        if slot.is_some() {
            return Err("Already enhancing a meeting".into());
        }
        *slot = Some(cancel.clone());
    }
    let emitter = app.clone();
    let id = meeting_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        llm::stream(
            &provider,
            &prompt.system,
            &prompt.user,
            |text| {
                let _ = emitter.emit(
                    "enhance-delta",
                    EnhanceDelta {
                        meeting_id: id.clone(),
                        text: text.to_string(),
                    },
                );
            },
            &cancel,
        )
    })
    .await
    .map_err(|e| e.to_string());
    *running.0.lock().map_err(|e| e.to_string())? = None;
    let markdown = result??;
    store.save_enhanced(&meeting_id, markdown.trim(), &template.id, now_ms()?)?;
    store
        .get_meeting(&meeting_id)?
        .ok_or_else(|| "meeting disappeared after saving".into())
}

fn list_templates_from(store: &Store) -> Result<Vec<enhance::Template>, String> {
    let mut all = enhance::built_in_templates();
    all.extend(store.custom_templates()?);
    Ok(all)
}

#[tauri::command]
fn cancel_enhance(running: State<'_, EnhanceState>) -> Result<(), String> {
    if let Some(flag) = running.0.lock().map_err(|e| e.to_string())?.as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// One engine for the mic track and one for system audio.
type EnginePair = (Box<dyn SpeechToText>, Box<dyn SpeechToText>);

/// Error prefix the UI recognises to offer a model download instead of a plain error.
const MODEL_MISSING: &str = "speech-model-missing:";

fn speech_engines(
    app: &tauri::AppHandle,
    settings: &Settings,
    cache: &ModelCache,
) -> Result<EnginePair, String> {
    match settings.speech_provider {
        SpeechProvider::LocalWhisper => {
            let model = models::find(&settings.whisper_model)?;
            let dir = models_dir(app)?;
            if !models::is_installed(&dir, model) {
                return Err(format!("{MODEL_MISSING}{}", model.id));
            }
            let loaded = cache.get(models::path(&dir, model.id))?;
            Ok((
                Box::new(WhisperEngine::new(&loaded, &settings.language)?),
                Box::new(WhisperEngine::new(&loaded, &settings.language)?),
            ))
        }
        SpeechProvider::Deepgram => {
            let key = cloud_key(SpeechProvider::Deepgram)?;
            Ok((
                Box::new(stt::cloud::deepgram(&key, &settings.language)?),
                Box::new(stt::cloud::deepgram(&key, &settings.language)?),
            ))
        }
        SpeechProvider::AssemblyAi => {
            let key = cloud_key(SpeechProvider::AssemblyAi)?;
            Ok((
                Box::new(stt::cloud::assembly_ai(&key)?),
                Box::new(stt::cloud::assembly_ai(&key)?),
            ))
        }
    }
}

/// Opened once at startup. Holds the error instead if the database couldn't be opened, so
/// every command can report it.
#[derive(Default)]
struct StoreState(OnceLock<Result<Store, String>>);

impl StoreState {
    fn get(&self) -> Result<&Store, String> {
        match self.0.get() {
            Some(Ok(store)) => Ok(store),
            Some(Err(e)) => Err(e.clone()),
            None => Err(
                "Parley is still unlocking your meeting database. If macOS asks for \
                         Keychain access, choose Always Allow."
                    .into(),
            ),
        }
    }
}

const DB_KEY_ACCOUNT: &str = "database-key";

fn open_store(app: &tauri::AppHandle) -> Result<Store, String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("parley.db");
    let key = match secrets::get(DB_KEY_ACCOUNT)? {
        Some(hex) => store::parse_key(&hex)?,
        // Never mint a new key for an existing database: it could not be read with it.
        None if path.exists() => {
            return Err("your meeting database's key is missing from the Keychain".into());
        }
        None => {
            let key = store::new_key()?;
            secrets::set(DB_KEY_ACCOUNT, &store::key_to_hex(&key))?;
            key
        }
    };
    Store::open(&path, &key)
}

#[tauri::command]
fn get_meeting(store: State<'_, StoreState>, id: String) -> Result<Option<store::Meeting>, String> {
    store.get()?.get_meeting(&id)
}

#[tauri::command]
fn save_notes(
    store: State<'_, StoreState>,
    id: String,
    title: String,
    notes_json: String,
    notes_text: String,
) -> Result<(), String> {
    store
        .get()?
        .save_notes(&id, &title, &notes_json, &notes_text, now_ms()?)
}

fn now_ms() -> Result<i64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64)
}

struct ActiveRecording {
    id: String,
    keep_audio: bool,
    recording: Recording,
    meters_running: Arc<AtomicBool>,
    transcribers: Vec<JoinHandle<Vec<TranscriptLine>>>,
}

#[derive(Default)]
struct RecorderState(Mutex<Option<ActiveRecording>>);

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordingStarted {
    id: String,
    system_audio: bool,
    warning: Option<String>,
    /// Set when echo cancellation couldn't start and the mic fell back to plain capture.
    mic_warning: Option<String>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AudioLevels {
    mic_dbfs: f32,
    system_dbfs: f32,
    seconds: f64,
    /// A capture stream stopped by itself mid-recording (see <track>.log in the recording).
    mic_ended: bool,
    system_ended: bool,
}

/// The Swift sidecars are bundled next to the main executable (Contents/MacOS in the .app).
fn sidecars() -> Result<Sidecars, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(Sidecars {
        system_audio: exe.with_file_name("parley-audiotap"),
        mic: exe.with_file_name("parley-mic"),
    })
}

// Async so Tauri runs it off the main thread: starting can wait on permission prompts.
#[tauri::command]
async fn start_recording(
    app: tauri::AppHandle,
    state: State<'_, RecorderState>,
    cache: State<'_, ModelCache>,
    store: State<'_, StoreState>,
    title: String,
) -> Result<RecordingStarted, String> {
    let store = store.get()?;
    let mut active = state.0.lock().map_err(|e| e.to_string())?;
    if active.is_some() {
        return Err("Already recording".into());
    }
    let settings = settings::load(&settings_path(&app)?).map_err(|e| e.to_string())?;
    let (mic_engine, system_engine) = speech_engines(&app, &settings, &cache)?;

    let started_at = now_ms()?;
    let id = started_at.to_string();
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("recordings")
        .join(&id);

    let (mic_tx, mic_rx) = mpsc::channel();
    let (system_tx, system_rx) = mpsc::channel();
    let recording = Recording::start(dir, &sidecars()?, Some(mic_tx), Some(system_tx))?;
    store.create_meeting(&id, &title, started_at)?;

    let line_emitter = app.clone();
    let on_line: transcribe::OnLine = Arc::new(move |line| {
        let _ = line_emitter.emit("transcript-line", line);
    });
    let error_emitter = app.clone();
    let on_error: Arc<dyn Fn(String) + Send + Sync> = Arc::new(move |message| {
        let _ = error_emitter.emit("transcript-error", message);
    });
    let transcribers = vec![
        transcribe::spawn(
            mic_engine,
            Speaker::Me,
            mic_rx,
            on_line.clone(),
            on_error.clone(),
        ),
        transcribe::spawn(system_engine, Speaker::Them, system_rx, on_line, on_error),
    ];

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
                    mic_ended: mic.ended_early(),
                    system_ended: system.ended_early(),
                },
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    });

    let started = RecordingStarted {
        id: id.clone(),
        system_audio: recording.system_warning.is_none(),
        warning: recording.system_warning.clone(),
        mic_warning: recording.mic_warning.clone(),
    };
    *active = Some(ActiveRecording {
        id,
        keep_audio: settings.keep_audio,
        recording,
        meters_running,
        transcribers,
    });
    Ok(started)
}

#[tauri::command]
async fn stop_recording(
    state: State<'_, RecorderState>,
    store: State<'_, StoreState>,
) -> Result<store::Meeting, String> {
    let store = store.get()?;
    let active = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("Not recording")?;
    active.meters_running.store(false, Ordering::Relaxed);
    // Stopping closes the audio taps, which lets each transcriber flush and finish.
    let done = active.recording.stop()?;
    let mut lines = active
        .transcribers
        .into_iter()
        .map(|t| t.join().unwrap_or_default());
    let transcript = transcribe::merge(
        lines.next().unwrap_or_default(),
        lines.next().unwrap_or_default(),
    );
    store.finish_meeting(&active.id, done.duration_secs, &transcript)?;
    if !active.keep_audio {
        // The capture logs stay for troubleshooting; they hold no audio or transcript text.
        for wav in [Some(done.mic_path), done.system_path]
            .into_iter()
            .flatten()
        {
            let _ = std::fs::remove_file(wav);
        }
    }
    store
        .get_meeting(&active.id)?
        .ok_or_else(|| "meeting disappeared after saving".into())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(StoreState::default())
        .setup(|app| {
            // Reading the database key can wait on a macOS Keychain prompt. Off the main thread,
            // so the window still appears and can say what it's waiting for.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let opened = open_store(&handle);
                if let Err(e) = &opened {
                    eprintln!("could not open the meeting database: {e}");
                }
                let _ = handle.state::<StoreState>().0.set(opened);
            });
            Ok(())
        })
        .manage(RecorderState::default())
        .manage(ModelCache::default())
        .manage(DownloadState::default())
        .manage(EnhanceState::default())
        .invoke_handler(tauri::generate_handler![
            app_info,
            get_settings,
            update_settings,
            list_models,
            download_model,
            cancel_model_download,
            set_api_key,
            delete_api_key,
            api_key_status,
            list_templates,
            save_template,
            delete_template,
            enhance_meeting,
            cancel_enhance,
            get_meeting,
            save_notes,
            start_recording,
            stop_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running Parley");
}
