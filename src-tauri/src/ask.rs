//! "Ask your meetings": pick the meetings most relevant to a question, pull out the passages
//! that matter, and build a prompt that answers only from them, with numbered citations.

use crate::enhance::{Prompt, format_transcript};
use crate::store::Meeting;
use crate::transcribe::TranscriptLine;
use serde::Serialize;

const STOPWORDS: &[&str] = &[
    "a", "about", "after", "all", "an", "and", "any", "are", "as", "at", "be", "been", "but", "by",
    "can", "could", "did", "do", "does", "for", "from", "had", "has", "have", "how", "i", "if",
    "in", "into", "is", "it", "its", "last", "me", "meeting", "meetings", "my", "of", "on", "or",
    "our", "said", "say", "should", "so", "than", "that", "the", "their", "them", "then", "there",
    "these", "they", "this", "to", "us", "was", "we", "were", "what", "when", "where", "which",
    "who", "why", "will", "with", "would", "you", "your",
];

/// The words of a question worth searching for.
pub fn keywords(question: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in question.split(|c: char| !c.is_alphanumeric()) {
        let w = w.to_lowercase();
        if w.chars().count() < 2 || STOPWORDS.contains(&w.as_str()) || out.contains(&w) {
            continue;
        }
        out.push(w);
    }
    out
}

/// Transcript lines mentioning any keyword, each with one line of context either side, in
/// order and without duplicates. Falls back to the opening lines when nothing matches.
pub fn relevant_lines(
    lines: &[TranscriptLine],
    keywords: &[String],
    fallback: usize,
) -> Vec<TranscriptLine> {
    let hits: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let text = l.text.to_lowercase();
            keywords.iter().any(|k| text.contains(k.as_str()))
        })
        .map(|(i, _)| i)
        .collect();
    if hits.is_empty() {
        return lines.iter().take(fallback).cloned().collect();
    }
    let mut keep = vec![false; lines.len()];
    for i in hits {
        let end = (i + 1).min(lines.len() - 1);
        keep[i.saturating_sub(1)..=end].fill(true);
    }
    lines
        .iter()
        .zip(keep)
        .filter(|&(_l, k)| k)
        .map(|(l, _k)| l.clone())
        .collect()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut} […]")
}

/// "2026-10-21" in the user's time zone, from a Unix-ms timestamp and a UTC offset.
pub fn local_date(ms: i64, utc_offset_minutes: i32) -> String {
    let days = (ms / 1000 + utc_offset_minutes as i64 * 60).div_euclid(86_400);
    // Howard Hinnant's days-to-civil algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// The number the answer cites, e.g. [2].
    pub index: usize,
    pub meeting_id: String,
    pub title: String,
    pub started_at_ms: i64,
}

const SYSTEM_PROMPT: &str = "\
You answer questions about the user's past meetings using only the meeting excerpts provided.

- Cite the meetings you rely on with their numbers in square brackets, like [1] or [2][3].
- If the excerpts don't contain the answer, say so plainly instead of guessing.
- Quote names, numbers, dates and commitments exactly as they appear.
- \"You\" in a transcript is the user; \"Them\" is everyone else on that call.
- Be concise: a direct answer first, then supporting points if they help. Markdown is fine.";

/// Builds the prompt and the numbered source list. `budget_chars` bounds all excerpts together.
pub fn build_prompt(
    question: &str,
    meetings: &[Meeting],
    utc_offset_minutes: i32,
    budget_chars: usize,
) -> (Prompt, Vec<Source>) {
    let words = keywords(question);
    let per_meeting = budget_chars / meetings.len().max(1);
    let mut sources = Vec::new();
    let mut blocks = Vec::new();
    for (i, m) in meetings.iter().enumerate() {
        let index = i + 1;
        sources.push(Source {
            index,
            meeting_id: m.id.clone(),
            title: m.title.clone(),
            started_at_ms: m.started_at_ms,
        });
        let notes = if !m.enhanced_md.trim().is_empty() {
            m.enhanced_md.trim()
        } else {
            m.notes_text.trim()
        };
        let transcript = format_transcript(&relevant_lines(&m.transcript, &words, 20));
        let notes_part = truncate_chars(notes, per_meeting / 2);
        let transcript_budget = per_meeting.saturating_sub(notes_part.chars().count());
        blocks.push(format!(
            "<meeting index=\"{index}\" title=\"{title}\" date=\"{date}\">\nNotes:\n{notes}\n\nTranscript excerpts:\n{transcript}\n</meeting>",
            title = m.title.replace('"', "'"),
            date = local_date(m.started_at_ms, utc_offset_minutes),
            notes = if notes_part.is_empty() { "(none)".to_string() } else { notes_part },
            transcript = if transcript.is_empty() {
                "(none)".to_string()
            } else {
                truncate_chars(&transcript, transcript_budget)
            },
        ));
    }
    let user = if blocks.is_empty() {
        format!(
            "No meetings matched this question.\n\nQuestion: {}",
            question.trim()
        )
    } else {
        format!("{}\n\nQuestion: {}", blocks.join("\n\n"), question.trim())
    };
    (
        Prompt {
            system: SYSTEM_PROMPT.to_string(),
            user,
        },
        sources,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcribe::Speaker;

    fn line(start_ms: u64, text: &str) -> TranscriptLine {
        TranscriptLine {
            speaker: Speaker::Them,
            segment_id: start_ms,
            is_final: true,
            text: text.into(),
            start_ms,
            end_ms: start_ms + 1000,
        }
    }

    fn meeting(
        id: &str,
        title: &str,
        enhanced: &str,
        notes: &str,
        lines: Vec<TranscriptLine>,
    ) -> Meeting {
        Meeting {
            id: id.into(),
            title: title.into(),
            started_at_ms: 1_792_454_400_000, // 2026-10-20 00:00 UTC
            duration_secs: Some(60.0),
            notes_json: String::new(),
            notes_text: notes.into(),
            updated_at_ms: 0,
            enhanced_md: enhanced.into(),
            enhanced_template_id: String::new(),
            enhanced_at_ms: None,
            folder_id: None,
            transcript: lines,
        }
    }

    #[test]
    fn keywords_drop_stopwords_short_words_and_repeats() {
        assert_eq!(
            keywords("What did Priya say about the Q3 budget? Budget!"),
            ["priya", "q3", "budget"]
        );
        assert!(keywords("what was it?").is_empty());
    }

    #[test]
    fn relevant_lines_keep_matches_with_neighbours() {
        let lines: Vec<TranscriptLine> = (0..10)
            .map(|i| line(i * 1000, &format!("line {i}")))
            .collect();
        let mut with_hit = lines.clone();
        with_hit[5].text = "the budget moves".into();
        let picked: Vec<u64> = relevant_lines(&with_hit, &["budget".into()], 3)
            .iter()
            .map(|l| l.start_ms)
            .collect();
        assert_eq!(picked, [4_000, 5_000, 6_000]);
        // No match: the opening lines stand in.
        assert_eq!(relevant_lines(&lines, &["budget".into()], 3).len(), 3);
    }

    #[test]
    fn local_date_respects_the_time_zone() {
        assert_eq!(local_date(1_792_454_400_000, 0), "2026-10-20");
        assert_eq!(local_date(1_792_454_400_000, -420), "2026-10-19"); // Pacific
        assert_eq!(local_date(0, 0), "1970-01-01");
        assert_eq!(local_date(951_782_400_000, 0), "2000-02-29");
    }

    #[test]
    fn prompt_numbers_meetings_and_prefers_enhanced_notes() {
        let ms = vec![
            meeting(
                "a",
                "Q3 budget sync",
                "## Decisions\n- Review moves to Friday",
                "rough notes",
                vec![line(0, "Priya owns the budget")],
            ),
            meeting("b", "Hiring", "", "closed two roles", vec![]),
        ];
        let (p, sources) = build_prompt("Who owns the budget?", &ms, 0, 20_000);
        assert!(p.system.contains("only the meeting excerpts"));
        assert!(
            p.user
                .contains("<meeting index=\"1\" title=\"Q3 budget sync\" date=\"2026-10-20\">")
        );
        assert!(p.user.contains("Review moves to Friday"));
        assert!(!p.user.contains("rough notes"));
        assert!(p.user.contains("[0:00] Them: Priya owns the budget"));
        assert!(p.user.contains("<meeting index=\"2\" title=\"Hiring\""));
        assert!(p.user.contains("closed two roles"));
        assert!(p.user.ends_with("Question: Who owns the budget?"));
        assert_eq!(sources.iter().map(|s| s.index).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(sources[1].meeting_id, "b");
    }

    #[test]
    fn prompt_stays_within_budget_and_handles_no_matches() {
        let long = "word ".repeat(5_000);
        let ms = vec![meeting("a", "Long", &long, "", vec![line(0, &long)])];
        let (p, _) = build_prompt("word", &ms, 0, 4_000);
        assert!(p.user.chars().count() < 4_600, "{}", p.user.chars().count());
        let (empty, sources) = build_prompt("anything", &[], 0, 4_000);
        assert!(empty.user.starts_with("No meetings matched"));
        assert!(sources.is_empty());
    }
}
