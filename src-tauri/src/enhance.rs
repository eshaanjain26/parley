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
    /// One line for the template gallery; empty for custom templates.
    #[serde(default)]
    pub description: String,
}

fn built_in(id: &str, name: &str, description: &str, instructions: &str) -> Template {
    Template {
        id: id.into(),
        name: name.into(),
        instructions: instructions.into(),
        built_in: true,
        description: description.into(),
    }
}

pub fn built_in_templates() -> Vec<Template> {
    vec![
        built_in(
            "general",
            "General meeting",
            "Summary, key points, decisions and action items for any meeting.",
            "## Summary\nTwo or three sentences on what the meeting was for and where it landed.\n\n\
             ## Key points\nThe substance of the discussion, grouped by topic.\n\n\
             ## Decisions\nWhat was agreed, with who agreed when it matters.\n\n\
             ## Action items\n- [ ] Task (owner, due date if one was said)\n\n\
             ## Open questions\nAnything raised and left unresolved.",
        ),
        built_in(
            "one-on-one",
            "1:1",
            "Updates, feedback, blockers and growth between a manager and a report, or two peers.",
            "## Updates\nWhat each person shared since the last 1:1.\n\n\
             ## Feedback\nFeedback given in either direction, stated specifically.\n\n\
             ## Blockers and support needed\n\n\
             ## Growth and career\nOnly if it came up.\n\n\
             ## Action items\n- [ ] Task (owner, due date if one was said)",
        ),
        built_in(
            "sales-call",
            "Sales call",
            "Pain, requirements, budget, timeline and objections from a prospect call.",
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
            "What each person did, plans next and is blocked on.",
            "## Progress\nWhat got done, per person when names are known.\n\n\
             ## Plans\nWhat each person is working on next.\n\n\
             ## Blockers\nEach blocker and who can unblock it.\n\n\
             ## Action items\n- [ ] Task (owner)",
        ),
        built_in(
            "customer-discovery",
            "Customer discovery",
            "Learn a prospect's world: their job, current tools and the problem worth solving.",
            "## Who we spoke with\nNames, roles, company, team size.\n\n\
             ## Their workflow today\nHow they do the job now, step by step, with the tools they use.\n\n\
             ## Problems and impact\nEach pain point in their words, how often it happens and what it costs them.\n\n\
             ## What they've tried\nWorkarounds and alternatives, and why they fell short.\n\n\
             ## Buying signals and concerns\n\n\
             ## Quotes worth keeping\nShort verbatim quotes, attributed.\n\n\
             ## Next steps\n- [ ] Step (owner, date)",
        ),
        built_in(
            "product-demo",
            "Product demo",
            "What was shown, how the audience reacted, questions asked and follow-ups owed.",
            "## Audience\nWho attended and what they care about.\n\n\
             ## What we showed\nFeatures and flows demonstrated, in order.\n\n\
             ## Reactions\nWhat landed and what didn't, with quotes where useful.\n\n\
             ## Questions\nEach question and the answer given; mark any we still owe.\n\n\
             ## Requests\nFeatures or integrations they asked for.\n\n\
             ## Follow-ups\n- [ ] Item (owner, date)",
        ),
        built_in(
            "customer-check-in",
            "Customer check-in",
            "Account health, adoption, issues and expansion for an existing customer.",
            "## Account snapshot\nWho attended, plan or contract, renewal date if mentioned.\n\n\
             ## How it's going\nAdoption, wins and results they reported.\n\n\
             ## Issues and risks\nProblems, open tickets and anything that threatens renewal.\n\n\
             ## Opportunities\nNew teams, use cases or upgrades discussed.\n\n\
             ## Commitments\n- [ ] What we promised (owner, date)\n- [ ] What they promised (owner, date)",
        ),
        built_in(
            "interview",
            "Interview",
            "Evidence for and against a candidate, kept to what was said.",
            "## Candidate and role\n\n\
             ## Experience discussed\nRelevant roles and projects, with specifics they gave.\n\n\
             ## Strengths\nEach with the evidence from the conversation.\n\n\
             ## Concerns\nEach with the evidence; note what wasn't covered.\n\n\
             ## Their questions\nWhat the candidate asked and how it was answered.\n\n\
             ## Logistics\nNotice period, compensation expectations, location, only if stated.\n\n\
             ## Next steps\n- [ ] Step (owner)\n\n\
             Stick to what was said in the interview. No hiring recommendation unless the user's notes give one.",
        ),
        built_in(
            "project-kickoff",
            "Project kickoff",
            "Goals, scope, roles, milestones and risks agreed at the start of a project.",
            "## Goal\nWhat success looks like and how it will be measured.\n\n\
             ## Scope\nIn scope and explicitly out of scope.\n\n\
             ## Roles\nWho owns what.\n\n\
             ## Milestones and dates\n\n\
             ## Risks and dependencies\n\n\
             ## Decisions\n\n\
             ## Action items\n- [ ] Task (owner, due date)",
        ),
        built_in(
            "retrospective",
            "Retrospective",
            "What went well, what didn't, and the changes the team agreed to try.",
            "## What went well\n\n\
             ## What didn't go well\nFacts first, then the impact.\n\n\
             ## Root causes\nOnly causes the team actually discussed.\n\n\
             ## What we'll change\n- [ ] Change (owner, by when)",
        ),
        built_in(
            "weekly-team",
            "Weekly team meeting",
            "Updates by topic, metrics, decisions and owners for a recurring team sync.",
            "## Headlines\nThe two or three things everyone should know.\n\n\
             ## Updates by topic\nGrouped by project or area, with owners.\n\n\
             ## Metrics mentioned\nNumbers exactly as stated.\n\n\
             ## Decisions\n\n\
             ## Action items\n- [ ] Task (owner, due date)",
        ),
        built_in(
            "exec-update",
            "Exec or board update",
            "A brief for leaders: status, numbers, risks and the asks that need a decision.",
            "## Bottom line\nOne paragraph: where things stand.\n\n\
             ## Results and numbers\nKPIs and figures exactly as stated, with comparisons given.\n\n\
             ## Risks\nEach risk, its likely impact and the mitigation discussed.\n\n\
             ## Decisions and asks\nWhat leadership approved, rejected or still needs to decide.\n\n\
             ## Follow-ups\n- [ ] Item (owner, date)",
        ),
        built_in(
            "brainstorm",
            "Brainstorm",
            "Every idea grouped by theme, with the ones the group picked to pursue.",
            "## Problem\nThe question the session set out to answer.\n\n\
             ## Ideas\nGrouped by theme; keep each idea short and credit its author when known.\n\n\
             ## Favourites\nIdeas the group liked most, and why.\n\n\
             ## Parked\nIdeas set aside, with the reason.\n\n\
             ## Next steps\n- [ ] Step (owner)",
        ),
        built_in(
            "troubleshooting",
            "Troubleshooting",
            "Symptoms, what was tried, the cause found and the fix, for incidents and support calls.",
            "## Problem\nSymptoms, who is affected, since when.\n\n\
             ## Environment\nVersions, systems and settings mentioned.\n\n\
             ## What was tried\nIn order, with the result of each step.\n\n\
             ## Cause\nThe root cause if found; otherwise the leading theory, marked as unconfirmed.\n\n\
             ## Fix or workaround\n\n\
             ## Follow-ups\n- [ ] Task (owner)",
        ),
        built_in(
            "partner-negotiation",
            "Partner or negotiation",
            "Positions, terms on the table, concessions and open points between two parties.",
            "## Parties\nWho represented each side.\n\n\
             ## Goals\nWhat each side wants, as they stated it.\n\n\
             ## Terms discussed\nPrice, scope, dates and conditions, with exact figures.\n\n\
             ## Agreed\n\n\
             ## Open points\nWhere the sides still differ.\n\n\
             ## Next steps\n- [ ] Step (owner, date)",
        ),
        built_in(
            "learning",
            "Lecture or learning session",
            "Study notes: core concepts, examples, definitions and questions to follow up.",
            "## Topic\n\n\
             ## Key concepts\nEach concept explained in a sentence or two.\n\n\
             ## Examples\nExamples or cases the speaker used.\n\n\
             ## Definitions\nTerms introduced, with their meaning.\n\n\
             ## Questions to follow up\n\n\
             ## Resources mentioned\nBooks, links and tools, as named.",
        ),
    ]
}

/// Picks a template from a meeting's (or calendar event's) title, or `None` for the default.
/// Specific kinds are checked before broad ones ("weekly 1:1" is a 1:1, not a team meeting).
pub fn suggest_template(title: &str) -> Option<&'static str> {
    let t = format!(
        " {} ",
        title
            .to_lowercase()
            .replace(['-', '_', '/', '|', ':', '(', ')', ','], " ")
    );
    let has = |words: &[&str]| words.iter().any(|w| t.contains(&format!(" {w} ")));
    // "1:1" became "1 1" above; check the original too.
    let raw = title.to_lowercase();
    if raw.contains("1:1")
        || raw.contains("1-1")
        || raw.contains("1on1")
        || has(&["one on one", "1 on 1", "1 1"])
    {
        return Some("one-on-one");
    }
    let rules: &[(&[&str], &str)] = &[
        (
            &["standup", "stand up", "daily sync", "daily scrum", "scrum"],
            "standup",
        ),
        (
            &[
                "retro",
                "retrospective",
                "post mortem",
                "postmortem",
                "lessons learned",
            ],
            "retrospective",
        ),
        (
            &[
                "interview",
                "phone screen",
                "screening",
                "candidate",
                "hiring panel",
            ],
            "interview",
        ),
        (
            &[
                "incident",
                "outage",
                "troubleshooting",
                "troubleshoot",
                "debug",
                "bug bash",
                "escalation",
            ],
            "troubleshooting",
        ),
        (&["kickoff", "kick off"], "project-kickoff"),
        (&["demo", "walkthrough", "walk through"], "product-demo"),
        (
            &[
                "discovery",
                "intro call",
                "introductory call",
                "exploratory",
            ],
            "customer-discovery",
        ),
        (
            &[
                "qbr",
                "check in",
                "checkin",
                "account review",
                "renewal",
                "success review",
            ],
            "customer-check-in",
        ),
        (
            &[
                "board",
                "exec",
                "executive",
                "steering",
                "leadership review",
                "investor update",
            ],
            "exec-update",
        ),
        (
            &["brainstorm", "ideation", "workshop", "whiteboard"],
            "brainstorm",
        ),
        (
            &[
                "negotiation",
                "contract",
                "partnership",
                "partner",
                "term sheet",
            ],
            "partner-negotiation",
        ),
        (
            &[
                "lecture", "class", "course", "training", "webinar", "seminar", "lesson",
            ],
            "learning",
        ),
        (&["sales", "pricing", "proposal", "prospect"], "sales-call"),
        (
            &[
                "weekly",
                "team sync",
                "team meeting",
                "staff meeting",
                "all hands",
                "all-hands",
            ],
            "weekly-team",
        ),
    ];
    rules
        .iter()
        .find(|(words, _)| has(words))
        .map(|(_, id)| *id)
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
    fn built_in_templates_are_unique_described_and_structured() {
        let all = built_in_templates();
        assert_eq!(all.len(), 16);
        let ids: std::collections::HashSet<_> = all.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids.len(), all.len(), "ids are unique");
        assert_eq!(all[0].id, "general", "the default comes first");
        for t in &all {
            assert!(
                t.built_in && t.instructions.contains("## "),
                "{} has sections",
                t.id
            );
            assert!(
                !t.description.is_empty() && !t.description.contains('\u{2014}'),
                "{} has a plain description",
                t.id
            );
            let prompt = build_prompt(t, "Title", "notes", &[line(Speaker::Me, 0, "hello")], 1000);
            assert!(
                prompt.user.contains(&t.instructions),
                "{} builds a prompt",
                t.id
            );
        }
    }

    #[test]
    fn suggests_a_template_from_the_title() {
        let cases = [
            ("Priya / Eshaan 1:1", Some("one-on-one")),
            ("Weekly 1-1 with Marco", Some("one-on-one")),
            ("Daily Standup", Some("standup")),
            ("Q3 Sprint Retro", Some("retrospective")),
            ("Interview: Senior Engineer (Ana Ruiz)", Some("interview")),
            ("Acme <> Parley intro call", Some("customer-discovery")),
            ("Product demo for Globex", Some("product-demo")),
            ("Globex QBR", Some("customer-check-in")),
            ("Board meeting", Some("exec-update")),
            ("Outage review: payments", Some("troubleshooting")),
            ("Project Atlas kick-off", Some("project-kickoff")),
            ("Weekly team sync", Some("weekly-team")),
            ("Pricing proposal review", Some("sales-call")),
            ("Lunch", None),
            ("Meeting on Oct 21 at 2:16 PM", None),
        ];
        for (title, want) in cases {
            assert_eq!(suggest_template(title), want, "{title}");
        }
        assert!(
            cases
                .iter()
                .filter_map(|(_, id)| *id)
                .all(|id| built_in_templates().iter().any(|t| t.id == id)),
            "every suggestion is a real template"
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
