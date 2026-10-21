//! Turns a meeting (title, the user's rough notes, transcript) into a prompt for "Enhance".
//! The user's notes are the outline; the transcript supplies the detail behind them.

use crate::transcribe::{Speaker, TranscriptLine};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub name: String,
    /// What the notes should contain, in the user's words for custom templates.
    pub instructions: String,
    pub built_in: bool,
}

fn built_in(id: &str, name: &str, instructions: &str) -> Template {
    Template {
        id: id.into(),
        name: name.into(),
        instructions: instructions.into(),
        built_in: true,
    }
}

pub fn built_in_templates() -> Vec<Template> {
    vec![
        built_in(
            "general",
            "General meeting",
            "## Summary\nTwo or three sentences on what the meeting was for and where it landed.\n\n\
             ## Key points\nThe substance of the discussion, grouped by topic.\n\n\
             ## Decisions\nWhat was agreed, with who agreed when it matters.\n\n\
             ## Action items\n- [ ] Task (owner, due date if one was said)\n\n\
             ## Open questions\nAnything raised and left unresolved.",
        ),
        built_in(
            "one-on-one",
            "1:1",
            "## Updates\nWhat each person shared since the last 1:1.\n\n\
             ## Feedback\nFeedback given in either direction, stated specifically.\n\n\
             ## Blockers and support needed\n\n\
             ## Growth and career\nOnly if it came up.\n\n\
             ## Action items\n- [ ] Task (owner, due date if one was said)",
        ),
        built_in(
            "sales-call",
            "Sales call",
            "## Customer and context\nWho was on the call, their company, role and situation.\n\n\
             ## Pain points\nProblems in the customer's own words where possible.\n\n\
             ## Current approach\nWhat they use today and why it falls short.\n\n\
             ## Requirements\nMust-haves and nice-to-haves.\n\n\
             ## Budget, timeline and decision process\nNumbers, dates and names exactly as stated.\n\n\
             ## Objections and concerns\nEach objection and how it was answered.\n\n\
             ## Next steps\n- [ ] Step (owner, date)",
        ),
        built_in(
            "standup",
            "Standup",
            "## Progress\nWhat got done, per person when names are known.\n\n\
             ## Plans\nWhat each person is working on next.\n\n\
             ## Blockers\nEach blocker and who can unblock it.\n\n\
             ## Action items\n- [ ] Task (owner)",
        ),
    ]
}

const SYSTEM_PROMPT: &str = "\
You write meeting notes from two sources: the notes the user typed during the meeting, and a \
transcript of the meeting.

How to use them:
- The user's notes show what mattered to them. Keep every point they wrote and expand it with \
  detail from the transcript. Put more weight on topics they noted than on topics they skipped.
- Use the transcript for facts, names, numbers, dates and commitments. Quote figures exactly.
- Never add anything that is in neither source. If something is unclear in the transcript, \
  leave it out or mark it as unclear.
- \"You\" in the transcript is the user; \"Them\" is everyone else on the call. Refer to people \
  by name when the transcript names them.
- Transcripts come from speech recognition and contain errors. Correct obvious mishearings \
  only when the context makes the right word certain.

Output format:
- Markdown, following the template's structure. Leave out any section with nothing to put in \
  it instead of writing \"none\".
- Short bullet points. No preamble, no closing remarks, no mention of these instructions.
- Write in the language of the transcript.";

fn timestamp(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

pub fn format_transcript(lines: &[TranscriptLine]) -> String {
    lines
        .iter()
        .map(|l| {
            let who = match l.speaker {
                Speaker::Me => "You",
                Speaker::Them => "Them",
            };
            format!("[{}] {who}: {}", timestamp(l.start_ms), l.text.trim())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Fits a transcript into `max_chars`, keeping the opening (context, introductions) and the
/// larger closing part (decisions and next steps tend to come last), with a marker in between.
pub fn fit_transcript(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let lines: Vec<&str> = text.lines().collect();
    let head_budget = max_chars * 2 / 5;
    let tail_budget = max_chars - head_budget;

    let mut head = Vec::new();
    let mut used = 0;
    for line in &lines {
        let len = line.chars().count() + 1;
        if used + len > head_budget {
            break;
        }
        head.push(*line);
        used += len;
    }
    let mut tail = Vec::new();
    used = 0;
    for line in lines[head.len()..].iter().rev() {
        let len = line.chars().count() + 1;
        if used + len > tail_budget {
            break;
        }
        tail.push(*line);
        used += len;
    }
    tail.reverse();
    let omitted = lines.len() - head.len() - tail.len();
    format!(
        "{}\n[… {omitted} transcript lines omitted for length …]\n{}",
        head.join("\n"),
        tail.join("\n")
    )
}

pub struct Prompt {
    pub system: String,
    pub user: String,
}

pub fn build_prompt(
    template: &Template,
    title: &str,
    notes_text: &str,
    transcript: &[TranscriptLine],
    max_transcript_chars: usize,
) -> Prompt {
    let notes = if notes_text.trim().is_empty() {
        "(The user took no notes. Build the notes from the transcript alone.)".to_string()
    } else {
        notes_text.trim().to_string()
    };
    let transcript = if transcript.is_empty() {
        "(No speech was transcribed.)".to_string()
    } else {
        fit_transcript(&format_transcript(transcript), max_transcript_chars)
    };
    Prompt {
        system: SYSTEM_PROMPT.to_string(),
        user: format!(
            "Meeting: {title}\n\nTemplate ({name}):\n{instructions}\n\n<user_notes>\n{notes}\n</user_notes>\n\n<transcript>\n{transcript}\n</transcript>",
            name = template.name,
            instructions = template.instructions.trim(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(speaker: Speaker, start_ms: u64, text: &str) -> TranscriptLine {
        TranscriptLine {
            speaker,
            segment_id: start_ms,
            is_final: true,
            text: text.into(),
            start_ms,
            end_ms: start_ms + 1000,
        }
    }

    #[test]
    fn built_in_templates_are_the_four_promised_and_unique() {
        let ids: Vec<String> = built_in_templates().into_iter().map(|t| t.id).collect();
        assert_eq!(ids, ["general", "one-on-one", "sales-call", "standup"]);
        assert!(
            built_in_templates()
                .iter()
                .all(|t| t.built_in && t.instructions.contains("##"))
        );
    }

    #[test]
    fn transcript_lines_carry_time_and_speaker() {
        let text = format_transcript(&[
            line(Speaker::Me, 4_000, " Morning all. "),
            line(Speaker::Them, 83_000, "Numbers by Thursday."),
        ]);
        assert_eq!(
            text,
            "[0:04] You: Morning all.\n[1:23] Them: Numbers by Thursday."
        );
    }

    #[test]
    fn prompt_includes_title_template_notes_and_transcript() {
        let t = &built_in_templates()[0];
        let p = build_prompt(
            t,
            "Q3 budget",
            "move review to Fri",
            &[line(Speaker::Them, 0, "Agreed.")],
            10_000,
        );
        assert!(p.system.contains("Never add anything"));
        assert!(p.user.starts_with("Meeting: Q3 budget"));
        assert!(p.user.contains("Template (General meeting)"));
        assert!(
            p.user
                .contains("<user_notes>\nmove review to Fri\n</user_notes>")
        );
        assert!(p.user.contains("[0:00] Them: Agreed."));
    }

    #[test]
    fn missing_notes_and_transcript_are_explained_not_left_blank() {
        let p = build_prompt(&built_in_templates()[3], "Standup", "  ", &[], 10_000);
        assert!(p.user.contains("took no notes"));
        assert!(p.user.contains("No speech was transcribed"));
    }

    #[test]
    fn long_transcripts_keep_both_ends_within_the_budget() {
        let lines: Vec<TranscriptLine> = (0..500)
            .map(|i| {
                line(
                    Speaker::Them,
                    i * 1000,
                    &format!("line number {i} of the meeting"),
                )
            })
            .collect();
        let full = format_transcript(&lines);
        let fitted = fit_transcript(&full, 4_000);
        assert!(
            fitted.chars().count() <= 4_100,
            "{}",
            fitted.chars().count()
        );
        assert!(fitted.contains("line number 0 of"));
        assert!(fitted.contains("line number 499 of"));
        assert!(fitted.contains("transcript lines omitted"));
        assert_eq!(fit_transcript("short", 4_000), "short");
    }
}
