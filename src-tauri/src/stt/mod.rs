//! Speech-to-text. Every engine (local whisper.cpp, Deepgram, AssemblyAI) implements
//! `SpeechToText` over the same 16 kHz mono stream, so the recorder doesn't care which runs.

pub mod cloud;
pub mod models;
pub mod segmenter;
pub mod streaming;
pub mod whisper;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptEvent {
    /// Stable per utterance: a later event with the same id replaces the earlier text.
    pub segment_id: u64,
    pub is_final: bool,
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

pub trait SpeechToText: Send {
    /// Feeds 16 kHz mono samples and returns any transcript updates that are ready.
    fn push(&mut self, samples: &[f32]) -> Result<Vec<TranscriptEvent>, String>;
    /// Flushes the end of the recording.
    fn finish(&mut self) -> Result<Vec<TranscriptEvent>, String>;
}
