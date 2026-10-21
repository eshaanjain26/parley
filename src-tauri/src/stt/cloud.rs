//! Deepgram and AssemblyAI live transcription, both on top of `StreamingEngine`.

use super::TranscriptEvent;
use super::streaming::{StreamConfig, StreamParser, StreamingEngine};
use serde_json::Value;

fn secs_to_ms(v: &Value) -> u64 {
    (v.as_f64().unwrap_or(0.0) * 1000.0).round().max(0.0) as u64
}

/// Deepgram sends interim results for the audio it's working on, then an `is_final` result for
/// the same span. Each final closes a segment; the next interim starts a new one.
#[derive(Default)]
pub struct DeepgramParser {
    next_id: u64,
}

impl StreamParser for DeepgramParser {
    fn parse(&mut self, message: &str) -> Result<Vec<TranscriptEvent>, String> {
        let v: Value =
            serde_json::from_str(message).map_err(|e| format!("unexpected reply: {e}"))?;
        if v["type"] != "Results" {
            return Ok(vec![]);
        }
        let text = v["channel"]["alternatives"][0]["transcript"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();
        let is_final = v["is_final"].as_bool().unwrap_or(false);
        if text.is_empty() {
            return Ok(vec![]);
        }
        let start_ms = secs_to_ms(&v["start"]);
        let event = TranscriptEvent {
            segment_id: self.next_id,
            is_final,
            text,
            start_ms,
            end_ms: start_ms + secs_to_ms(&v["duration"]),
        };
        if is_final {
            self.next_id += 1;
        }
        Ok(vec![event])
    }
}

pub fn deepgram(api_key: &str, language: &str) -> Result<StreamingEngine, String> {
    let url = format!(
        "wss://api.deepgram.com/v1/listen?model=nova-3&language={language}&encoding=linear16\
         &sample_rate=16000&channels=1&interim_results=true&smart_format=true&punctuate=true\
         &endpointing=300"
    );
    StreamingEngine::connect(
        StreamConfig {
            provider: "Deepgram",
            url,
            authorization: format!("Token {}", api_key.trim()),
            close_message: r#"{"type":"CloseStream"}"#,
        },
        DeepgramParser::default(),
    )
}

/// AssemblyAI's v3 streaming API reports one "turn" at a time. With `format_turns=true`, a turn
/// is final only once the punctuated, cased version (`turn_is_formatted`) arrives.
#[derive(Default)]
pub struct AssemblyAiParser;

impl StreamParser for AssemblyAiParser {
    fn parse(&mut self, message: &str) -> Result<Vec<TranscriptEvent>, String> {
        let v: Value =
            serde_json::from_str(message).map_err(|e| format!("unexpected reply: {e}"))?;
        if let Some(err) = v["error"].as_str() {
            return Err(err.to_string());
        }
        if v["type"] != "Turn" {
            return Ok(vec![]);
        }
        let text = v["transcript"].as_str().unwrap_or("").trim().to_string();
        let is_final = v["end_of_turn"].as_bool().unwrap_or(false)
            && v["turn_is_formatted"].as_bool().unwrap_or(false);
        if text.is_empty() && !is_final {
            return Ok(vec![]);
        }
        let words = v["words"].as_array();
        let start_ms = words
            .and_then(|w| w.first())
            .and_then(|w| w["start"].as_u64())
            .unwrap_or(0);
        let end_ms = words
            .and_then(|w| w.last())
            .and_then(|w| w["end"].as_u64())
            .unwrap_or(start_ms);
        Ok(vec![TranscriptEvent {
            segment_id: v["turn_order"].as_u64().unwrap_or(0),
            is_final,
            text,
            start_ms,
            end_ms,
        }])
    }
}

pub fn assembly_ai(api_key: &str) -> Result<StreamingEngine, String> {
    StreamingEngine::connect(
        StreamConfig {
            provider: "AssemblyAI",
            url: "wss://streaming.assemblyai.com/v3/ws?sample_rate=16000&encoding=pcm_s16le&format_turns=true"
                .into(),
            authorization: api_key.trim().to_string(),
            close_message: r#"{"type":"Terminate"}"#,
        },
        AssemblyAiParser,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dg(text: &str, is_final: bool, start: f64, duration: f64) -> String {
        serde_json::json!({
            "type": "Results",
            "start": start,
            "duration": duration,
            "is_final": is_final,
            "channel": {"alternatives": [{"transcript": text, "confidence": 0.98}]}
        })
        .to_string()
    }

    #[test]
    fn deepgram_interims_share_an_id_until_the_final() {
        let mut p = DeepgramParser::default();
        let a = p.parse(&dg("the budget", false, 1.0, 0.8)).unwrap();
        let b = p
            .parse(&dg("The budget moves to Friday.", true, 1.0, 1.9))
            .unwrap();
        let c = p.parse(&dg("please", false, 3.2, 0.4)).unwrap();
        assert_eq!((a[0].segment_id, a[0].is_final), (0, false));
        assert_eq!((b[0].segment_id, b[0].is_final), (0, true));
        assert_eq!((b[0].start_ms, b[0].end_ms), (1_000, 2_900));
        assert_eq!(c[0].segment_id, 1);
    }

    #[test]
    fn deepgram_ignores_metadata_and_empty_results() {
        let mut p = DeepgramParser::default();
        assert!(
            p.parse(r#"{"type":"Metadata","request_id":"x"}"#)
                .unwrap()
                .is_empty()
        );
        assert!(p.parse(&dg("", true, 0.0, 1.0)).unwrap().is_empty());
        // An empty final doesn't consume a segment id.
        assert_eq!(
            p.parse(&dg("hi", false, 0.0, 1.0)).unwrap()[0].segment_id,
            0
        );
    }

    fn turn(order: u64, text: &str, end: bool, formatted: bool) -> String {
        serde_json::json!({
            "type": "Turn",
            "turn_order": order,
            "transcript": text,
            "end_of_turn": end,
            "turn_is_formatted": formatted,
            "words": [
                {"text": "send", "start": 4200, "end": 4500, "word_is_final": true},
                {"text": "numbers", "start": 4500, "end": 5100, "word_is_final": true}
            ]
        })
        .to_string()
    }

    #[test]
    fn assemblyai_turn_is_final_only_once_formatted() {
        let mut p = AssemblyAiParser;
        let partial = p.parse(&turn(2, "send numbers", false, false)).unwrap();
        let unformatted_end = p.parse(&turn(2, "send numbers", true, false)).unwrap();
        let formatted = p.parse(&turn(2, "Send numbers.", true, true)).unwrap();
        assert!(!partial[0].is_final);
        assert!(!unformatted_end[0].is_final);
        assert!(formatted[0].is_final);
        assert_eq!(formatted[0].text, "Send numbers.");
        assert_eq!(formatted[0].segment_id, 2);
        assert_eq!((formatted[0].start_ms, formatted[0].end_ms), (4_200, 5_100));
    }

    #[test]
    fn assemblyai_ignores_session_messages_and_surfaces_errors() {
        let mut p = AssemblyAiParser;
        assert!(
            p.parse(r#"{"type":"Begin","id":"s1","expires_at":1}"#)
                .unwrap()
                .is_empty()
        );
        assert!(
            p.parse(r#"{"type":"Termination","audio_duration_seconds":12}"#)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            p.parse(r#"{"error":"Invalid API key"}"#).unwrap_err(),
            "Invalid API key"
        );
    }
}
