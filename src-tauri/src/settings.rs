use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpeechProvider {
    LocalWhisper,
    Deepgram,
    AssemblyAi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LlmProvider {
    Ollama,
    OpenAiCompatible,
    Anthropic,
}

/// User preferences. API keys are deliberately absent: they live in the macOS Keychain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub speech_provider: SpeechProvider,
    pub whisper_model: String,
    pub language: String,
    pub llm_provider: LlmProvider,
    pub ollama_url: String,
    pub ollama_model: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            speech_provider: SpeechProvider::LocalWhisper,
            whisper_model: "base.en".into(),
            language: "en".into(),
            llm_provider: LlmProvider::Ollama,
            ollama_url: "http://127.0.0.1:11434".into(),
            ollama_model: "llama3.2:3b".into(),
        }
    }
}

pub fn load(path: &Path) -> io::Result<Settings> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e),
    }
}

pub fn save(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(settings)?;
    // Write-then-rename so a crash mid-write never leaves a truncated settings file.
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let s = load(&dir.path().join("settings.json")).unwrap();
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn save_then_load_round_trips_and_creates_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config/settings.json");
        let s = Settings {
            speech_provider: SpeechProvider::Deepgram,
            llm_provider: LlmProvider::Anthropic,
            ..Settings::default()
        };
        save(&path, &s).unwrap();
        assert_eq!(load(&path).unwrap(), s);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn partial_file_fills_missing_fields_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"speechProvider":"assembly-ai"}"#).unwrap();
        let s = load(&path).unwrap();
        assert_eq!(s.speech_provider, SpeechProvider::AssemblyAi);
        assert_eq!(s.ollama_model, Settings::default().ollama_model);
    }

    #[test]
    fn corrupt_file_is_an_error_not_silent_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{not json").unwrap();
        assert_eq!(load(&path).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn wire_format_matches_typescript_types() {
        let v = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(v["speechProvider"], "local-whisper");
        assert_eq!(v["llmProvider"], "ollama");
        assert_eq!(v["ollamaUrl"], "http://127.0.0.1:11434");
    }
}
