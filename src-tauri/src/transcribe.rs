//! Runs one speech engine per track on its own thread and labels the output by speaker.

use crate::stt::{SpeechToText, TranscriptEvent};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    /// The microphone.
    Me,
    /// Everyone heard through system audio.
    Them,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptLine {
    pub speaker: Speaker,
    pub segment_id: u64,
    pub is_final: bool,
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

impl TranscriptLine {
    fn new(speaker: Speaker, e: TranscriptEvent) -> Self {
        Self {
            speaker,
            segment_id: e.segment_id,
            is_final: e.is_final,
            text: e.text,
            start_ms: e.start_ms,
            end_ms: e.end_ms,
        }
    }
}

pub type OnLine = Arc<dyn Fn(TranscriptLine) + Send + Sync>;

/// Consumes 16 kHz audio until the sender side closes, then flushes the engine and returns the
/// final lines. An engine error is reported once and the rest of the audio is ignored, so a
/// transcription failure never stops the recording itself.
pub fn spawn(
    mut engine: Box<dyn SpeechToText>,
    speaker: Speaker,
    rx: Receiver<Vec<f32>>,
    on_line: OnLine,
    on_error: Arc<dyn Fn(String) + Send + Sync>,
) -> JoinHandle<Vec<TranscriptLine>> {
    std::thread::spawn(move || {
        let mut finals = Vec::new();
        let mut failed = false;
        let emit = |events: Vec<TranscriptEvent>, finals: &mut Vec<TranscriptLine>| {
            for e in events {
                let line = TranscriptLine::new(speaker, e);
                if line.is_final && !line.text.is_empty() {
                    finals.push(line.clone());
                }
                on_line(line);
            }
        };
        while let Ok(first) = rx.recv() {
            if failed {
                continue;
            }
            // If decoding fell behind, take everything queued at once so the engine catches up
            // in one pass instead of transcribing stale partials one by one.
            let mut batch = first;
            while let Ok(more) = rx.try_recv() {
                batch.extend(more);
            }
            match engine.push(&batch) {
                Ok(events) => emit(events, &mut finals),
                Err(e) => {
                    failed = true;
                    on_error(e);
                }
            }
        }
        if !failed {
            match engine.finish() {
                Ok(events) => emit(events, &mut finals),
                Err(e) => on_error(e),
            }
        }
        finals
    })
}

/// Both speakers' final lines in time order.
pub fn merge(mut a: Vec<TranscriptLine>, b: Vec<TranscriptLine>) -> Vec<TranscriptLine> {
    a.extend(b);
    a.sort_by_key(|l| (l.start_ms, l.speaker == Speaker::Them));
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::mpsc;

    /// Emits a partial per push and one final with the total sample count at the end.
    struct Counting {
        samples: usize,
        pushes: usize,
        fail_on_push: Option<usize>,
    }

    impl SpeechToText for Counting {
        fn push(&mut self, s: &[f32]) -> Result<Vec<TranscriptEvent>, String> {
            self.pushes += 1;
            if Some(self.pushes) == self.fail_on_push {
                return Err("engine broke".into());
            }
            self.samples += s.len();
            Ok(vec![TranscriptEvent {
                segment_id: 0,
                is_final: false,
                text: format!("{}", self.samples),
                start_ms: 0,
                end_ms: 0,
            }])
        }

        fn finish(&mut self) -> Result<Vec<TranscriptEvent>, String> {
            Ok(vec![TranscriptEvent {
                segment_id: 0,
                is_final: true,
                text: format!("total {}", self.samples),
                start_ms: 10,
                end_ms: 20,
            }])
        }
    }

    fn run(
        engine: Counting,
        chunks: Vec<Vec<f32>>,
    ) -> (Vec<TranscriptLine>, Vec<TranscriptLine>, Vec<String>) {
        let (tx, rx) = mpsc::channel();
        for c in chunks {
            tx.send(c).unwrap();
        }
        drop(tx);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let errors = Arc::new(Mutex::new(Vec::new()));
        let (s, e) = (seen.clone(), errors.clone());
        let finals = spawn(
            Box::new(engine),
            Speaker::Them,
            rx,
            Arc::new(move |l| s.lock().unwrap().push(l)),
            Arc::new(move |m| e.lock().unwrap().push(m)),
        )
        .join()
        .unwrap();
        let seen = seen.lock().unwrap().clone();
        let errors = errors.lock().unwrap().clone();
        (finals, seen, errors)
    }

    #[test]
    fn backlog_is_batched_and_finals_are_labelled() {
        let engine = Counting {
            samples: 0,
            pushes: 0,
            fail_on_push: None,
        };
        let (finals, seen, errors) = run(engine, vec![vec![0.0; 100]; 5]);
        assert!(errors.is_empty());
        // All five chunks were queued before the thread started, so they arrive as one batch.
        assert_eq!(seen.iter().filter(|l| !l.is_final).count(), 1);
        assert_eq!(finals.len(), 1);
        assert_eq!(finals[0].text, "total 500");
        assert_eq!(finals[0].speaker, Speaker::Them);
    }

    #[test]
    fn engine_errors_are_reported_once_and_do_not_panic() {
        let engine = Counting {
            samples: 0,
            pushes: 0,
            fail_on_push: Some(1),
        };
        let (finals, _, errors) = run(engine, vec![vec![0.0; 10]]);
        assert_eq!(errors, vec!["engine broke".to_string()]);
        assert!(finals.is_empty());
    }

    #[test]
    fn merge_orders_by_time_with_me_first_on_ties() {
        let line = |speaker, start_ms| TranscriptLine {
            speaker,
            segment_id: 0,
            is_final: true,
            text: String::new(),
            start_ms,
            end_ms: start_ms + 1,
        };
        let merged = merge(
            vec![line(Speaker::Me, 500), line(Speaker::Me, 100)],
            vec![line(Speaker::Them, 100), line(Speaker::Them, 50)],
        );
        let order: Vec<(Speaker, u64)> = merged.iter().map(|l| (l.speaker, l.start_ms)).collect();
        assert_eq!(
            order,
            vec![
                (Speaker::Them, 50),
                (Speaker::Me, 100),
                (Speaker::Them, 100),
                (Speaker::Me, 500)
            ]
        );
    }

    #[test]
    fn transcript_line_wire_format() {
        let v = serde_json::to_value(TranscriptLine {
            speaker: Speaker::Me,
            segment_id: 3,
            is_final: true,
            text: "hi".into(),
            start_ms: 1,
            end_ms: 2,
        })
        .unwrap();
        assert_eq!(v["speaker"], "me");
        assert_eq!(v["segmentId"], 3);
        assert_eq!(v["isFinal"], true);
    }
}
