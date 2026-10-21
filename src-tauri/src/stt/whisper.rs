//! On-device transcription with whisper.cpp (Metal on Apple Silicon).

use super::segmenter::{Cut, Segmenter, SegmenterConfig};
use super::{SpeechToText, TranscriptEvent};
use crate::dsp::TARGET_RATE;
use std::path::Path;
use std::sync::{Arc, Once};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

/// A loaded model. Cheap to share; each track gets its own decoding state from it.
#[derive(Clone)]
pub struct WhisperModel(Arc<WhisperContext>);

impl WhisperModel {
    pub fn load(path: &Path) -> Result<Self, String> {
        static QUIET: Once = Once::new();
        // Routes whisper.cpp's console chatter into the `log` facade instead of stderr.
        QUIET.call_once(whisper_rs::install_logging_hooks);
        let ctx = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .map_err(|e| format!("could not load speech model {path:?}: {e}"))?;
        Ok(Self(Arc::new(ctx)))
    }
}

pub struct WhisperEngine {
    state: WhisperState,
    segmenter: Segmenter,
    language: String,
}

impl WhisperEngine {
    pub fn new(model: &WhisperModel, language: &str) -> Result<Self, String> {
        Ok(Self {
            state: model.0.create_state().map_err(|e| e.to_string())?,
            segmenter: Segmenter::new(SegmenterConfig::default()),
            language: language.to_string(),
        })
    }

    pub fn transcribe(&mut self, audio: &[f32]) -> Result<String, String> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(&self.language));
        params.set_n_threads(4);
        params.set_no_context(true);
        params.set_single_segment(false);
        params.set_suppress_blank(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        self.state.full(params, audio).map_err(|e| e.to_string())?;
        let text: Vec<String> = self
            .state
            .as_iter()
            .filter_map(|seg| seg.to_str_lossy().ok().map(|s| s.into_owned()))
            .collect();
        Ok(clean_text(&text.join(" ")))
    }

    fn handle(&mut self, cuts: Vec<Cut>) -> Result<Vec<TranscriptEvent>, String> {
        let mut events = Vec::new();
        for cut in collapse(cuts) {
            let (id, start_ms, audio, is_final) = match cut {
                Cut::Partial {
                    id,
                    start_ms,
                    audio,
                } => (id, start_ms, audio, false),
                Cut::Final {
                    id,
                    start_ms,
                    audio,
                } => (id, start_ms, audio, true),
            };
            let text = self.transcribe(&audio)?;
            // An empty partial adds nothing; an empty final still closes the segment in the UI.
            if text.is_empty() && !is_final {
                continue;
            }
            events.push(TranscriptEvent {
                segment_id: id,
                is_final,
                text,
                start_ms,
                end_ms: start_ms + audio.len() as u64 * 1000 / TARGET_RATE as u64,
            });
        }
        Ok(events)
    }
}

impl SpeechToText for WhisperEngine {
    fn push(&mut self, samples: &[f32]) -> Result<Vec<TranscriptEvent>, String> {
        let cuts = self.segmenter.push(samples);
        self.handle(cuts)
    }

    fn finish(&mut self) -> Result<Vec<TranscriptEvent>, String> {
        let cuts = self.segmenter.finish();
        self.handle(cuts)
    }
}

/// After a backlog, one batch can hold several partials for the same utterance. Only the
/// newest one is worth decoding, and none at all if the utterance already has its final.
pub fn collapse(cuts: Vec<Cut>) -> Vec<Cut> {
    let superseded = |i: usize, id: u64| {
        cuts[i + 1..].iter().any(|later| match later {
            Cut::Partial { id: l, .. } | Cut::Final { id: l, .. } => *l == id,
        })
    };
    let keep: Vec<bool> = cuts
        .iter()
        .enumerate()
        .map(|(i, c)| match c {
            Cut::Partial { id, .. } => !superseded(i, *id),
            Cut::Final { .. } => true,
        })
        .collect();
    cuts.into_iter()
        .zip(keep)
        .filter_map(|(c, k)| k.then_some(c))
        .collect()
}

/// Whisper narrates non-speech as "[BLANK_AUDIO]", "(music)", "[Applause]" and so on.
/// Drop those annotations and collapse whitespace.
pub fn clean_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut depth = 0i32;
    for c in raw.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_text_drops_annotations_and_tidies_spaces() {
        assert_eq!(clean_text(" [BLANK_AUDIO] "), "");
        assert_eq!(clean_text("(music)"), "");
        assert_eq!(
            clean_text("  Let's ship it [Applause]  on   Friday. "),
            "Let's ship it on Friday."
        );
    }

    #[test]
    fn collapse_keeps_only_the_newest_partial_and_all_finals() {
        let p = |id, n| Cut::Partial {
            id,
            start_ms: 0,
            audio: vec![0.0; n],
        };
        let f = |id| Cut::Final {
            id,
            start_ms: 0,
            audio: vec![],
        };
        let out = collapse(vec![p(0, 1), p(0, 2), f(0), p(1, 3), p(1, 4)]);
        assert_eq!(out, vec![f(0), p(1, 4)]);
    }

    #[test]
    fn clean_text_keeps_plain_speech() {
        assert_eq!(
            clean_text("The budget is $40k, not $14k."),
            "The budget is $40k, not $14k."
        );
    }

    /// Needs a model; run with `PARLEY_TEST_MODEL=/path/ggml-tiny.en.bin cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn transcribes_the_fixture_recording() {
        let model_path = std::env::var("PARLEY_TEST_MODEL").expect("set PARLEY_TEST_MODEL");
        let model = WhisperModel::load(Path::new(&model_path)).unwrap();
        let mut engine = WhisperEngine::new(&model, "en").unwrap();

        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/hello-parley.wav"
        );
        let samples: Vec<f32> = hound::WavReader::open(fixture)
            .unwrap()
            .into_samples::<i16>()
            .map(|s| s.unwrap() as f32 / i16::MAX as f32)
            .collect();

        let mut events = Vec::new();
        for chunk in samples.chunks(1_600) {
            events.extend(engine.push(chunk).unwrap());
        }
        events.extend(engine.finish().unwrap());

        let finals: Vec<&TranscriptEvent> = events.iter().filter(|e| e.is_final).collect();
        assert!(!finals.is_empty(), "no final segments: {events:?}");
        let text = finals
            .iter()
            .map(|e| e.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        for word in ["budget", "friday"] {
            assert!(text.contains(word), "missing {word:?} in {text:?}");
        }
    }
}
