//! Markdown export of a meeting. The user's notes are stored as the editor's document JSON
//! (TipTap / ProseMirror), converted here so headings, lists and emphasis survive.

use crate::ask::local_date;
use crate::enhance::format_transcript;
use crate::store::Meeting;
use serde_json::Value;

fn marks_wrap(text: &str, marks: Option<&Vec<Value>>) -> String {
    let mut out = text.to_string();
    let Some(marks) = marks else { return out };
    for m in marks {
        out = match m["type"].as_str() {
            Some("bold") => format!("**{out}**"),
            Some("italic") => format!("*{out}*"),
            Some("strike") => format!("~~{out}~~"),
            Some("code") => format!("`{out}`"),
            Some("link") => match m["attrs"]["href"].as_str() {
                Some(href) => format!("[{out}]({href})"),
                None => out,
            },
            _ => out,
        };
    }
    out
}

fn inline(node: &Value) -> String {
    let Some(children) = node["content"].as_array() else {
        return String::new();
    };
    children
        .iter()
        .map(|c| match c["type"].as_str() {
            Some("text") => marks_wrap(c["text"].as_str().unwrap_or(""), c["marks"].as_array()),
            Some("hardBreak") => "  \n".to_string(),
            _ => inline(c),
        })
        .collect()
}

fn indent(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|l| {
            if l.is_empty() {
                String::new()
            } else {
                format!("{prefix}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Renders a list item's children: the first paragraph after the marker, the rest indented.
fn list_item(item: &Value, marker: &str) -> String {
    let blocks: Vec<String> = item["content"]
        .as_array()
        .map(|c| c.iter().map(block).filter(|b| !b.is_empty()).collect())
        .unwrap_or_default();
    let pad = " ".repeat(marker.chars().count());
    let mut out = String::new();
    for (i, b) in blocks.iter().enumerate() {
        if i == 0 {
            out.push_str(marker);
            out.push_str(indent(b, &pad).trim_start());
        } else {
            out.push('\n');
            out.push_str(&indent(b, &pad));
        }
    }
    if blocks.is_empty() {
        out.push_str(marker.trim_end());
    }
    out
}

fn block(node: &Value) -> String {
    let items = || node["content"].as_array().cloned().unwrap_or_default();
    match node["type"].as_str() {
        Some("paragraph") => inline(node),
        Some("heading") => {
            let level = node["attrs"]["level"].as_u64().unwrap_or(2).clamp(1, 6) as usize;
            format!("{} {}", "#".repeat(level), inline(node))
        }
        Some("bulletList") => items()
            .iter()
            .map(|i| list_item(i, "- "))
            .collect::<Vec<_>>()
            .join("\n"),
        Some("orderedList") => {
            let start = node["attrs"]["start"].as_u64().unwrap_or(1);
            items()
                .iter()
                .enumerate()
                .map(|(n, i)| list_item(i, &format!("{}. ", start + n as u64)))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Some("taskList") => items()
            .iter()
            .map(|i| {
                let marker = if i["attrs"]["checked"].as_bool() == Some(true) {
                    "- [x] "
                } else {
                    "- [ ] "
                };
                list_item(i, marker)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some("blockquote") => indent(&blocks(node), "> "),
        Some("codeBlock") => format!(
            "```{}\n{}\n```",
            node["attrs"]["language"].as_str().unwrap_or(""),
            inline(node)
        ),
        Some("horizontalRule") => "---".to_string(),
        _ => blocks(node),
    }
}

fn blocks(node: &Value) -> String {
    node["content"]
        .as_array()
        .map(|c| {
            c.iter()
                .map(block)
                .filter(|b| !b.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .unwrap_or_default()
}

/// Converts a TipTap document (as stored JSON) to Markdown.
pub fn notes_markdown(notes_json: &str) -> String {
    serde_json::from_str::<Value>(notes_json)
        .map(|doc| blocks(&doc))
        .unwrap_or_default()
}

fn duration(secs: f64) -> String {
    let s = secs.round() as u64;
    if s >= 3600 {
        format!("{}h {}m", s / 3600, (s % 3600) / 60)
    } else {
        format!("{}m {}s", s / 60, s % 60)
    }
}

pub fn meeting_markdown(m: &Meeting, utc_offset_minutes: i32) -> String {
    let mut out = format!("# {}\n\n", m.title.trim());
    let mut meta = vec![local_date(m.started_at_ms, utc_offset_minutes)];
    if let Some(d) = m.duration_secs {
        meta.push(duration(d));
    }
    out.push_str(&format!("_{}_\n", meta.join(" · ")));

    let notes = {
        let md = notes_markdown(&m.notes_json);
        if md.trim().is_empty() {
            m.notes_text.trim().to_string()
        } else {
            md
        }
    };
    if !notes.is_empty() {
        out.push_str(&format!("\n## My notes\n\n{notes}\n"));
    }
    if !m.enhanced_md.trim().is_empty() {
        // Enhanced notes use ## sections; nest them one level under their own heading.
        let nested: String = m
            .enhanced_md
            .trim()
            .lines()
            .map(|l| {
                if l.starts_with('#') {
                    format!("#{l}")
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        out.push_str(&format!("\n## Enhanced notes\n\n{nested}\n"));
    }
    if !m.transcript.is_empty() {
        let lines: String = format_transcript(&m.transcript)
            .lines()
            .map(|l| format!("- {l}\n"))
            .collect();
        out.push_str(&format!("\n## Transcript\n\n{lines}"));
    }
    out
}

/// A file name for the exported meeting: its title with path-hostile characters removed.
pub fn file_stem(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_.,()".contains(c) {
                c
            } else {
                ' '
            }
        })
        .collect();
    let squashed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if squashed.is_empty() {
        "Meeting".to_string()
    } else {
        squashed.chars().take(80).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcribe::{Speaker, TranscriptLine};

    const DOC: &str = r#"{"type":"doc","content":[
        {"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Budget"}]},
        {"type":"paragraph","content":[
            {"type":"text","text":"Review moves to "},
            {"type":"text","marks":[{"type":"bold"}],"text":"Friday"},
            {"type":"text","text":", see "},
            {"type":"text","marks":[{"type":"link","attrs":{"href":"https://x.test"}}],"text":"sheet"}]},
        {"type":"bulletList","content":[
            {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Priya owns it"}]},
                {"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"due Thu"}]}]}]}]},
            {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","marks":[{"type":"italic"}],"text":"tentative"}]}]}]},
        {"type":"orderedList","attrs":{"start":1},"content":[
            {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"first"}]}]},
            {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"second"}]}]}]},
        {"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"quoted"}]}]},
        {"type":"paragraph"}
    ]}"#;

    #[test]
    fn tiptap_document_becomes_markdown() {
        assert_eq!(
            notes_markdown(DOC),
            "## Budget\n\n\
             Review moves to **Friday**, see [sheet](https://x.test)\n\n\
             - Priya owns it\n  - due Thu\n- *tentative*\n\n\
             1. first\n2. second\n\n\
             > quoted"
        );
    }

    #[test]
    fn empty_or_invalid_notes_give_empty_markdown() {
        assert_eq!(notes_markdown(""), "");
        assert_eq!(notes_markdown("{not json"), "");
        assert_eq!(
            notes_markdown(r#"{"type":"doc","content":[{"type":"paragraph"}]}"#),
            ""
        );
    }

    #[test]
    fn meeting_export_has_all_sections_in_order() {
        let m = Meeting {
            id: "a".into(),
            title: "Q3 budget sync".into(),
            started_at_ms: 1_792_454_400_000,
            duration_secs: Some(125.0),
            notes_json: DOC.into(),
            notes_text: String::new(),
            updated_at_ms: 0,
            enhanced_md: "## Decisions\n- Friday".into(),
            enhanced_template_id: "general".into(),
            enhanced_at_ms: Some(1),
            folder_id: None,
            transcript: vec![TranscriptLine {
                speaker: Speaker::Them,
                segment_id: 0,
                is_final: true,
                text: "Numbers by Thursday.".into(),
                start_ms: 29_000,
                end_ms: 31_000,
            }],
        };
        let md = meeting_markdown(&m, 0);
        assert!(md.starts_with("# Q3 budget sync\n\n_2026-10-20 · 2m 5s_\n"));
        let order: Vec<usize> = [
            "## My notes",
            "## Enhanced notes",
            "### Decisions",
            "## Transcript",
            "- [0:29] Them: Numbers by Thursday.",
        ]
        .iter()
        .map(|s| md.find(s).unwrap_or_else(|| panic!("missing {s}")))
        .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{md}");
    }

    #[test]
    fn notes_fall_back_to_plain_text_and_sections_are_skipped_when_empty() {
        let m = Meeting {
            id: "b".into(),
            title: "Quick".into(),
            started_at_ms: 0,
            duration_secs: None,
            notes_json: String::new(),
            notes_text: "plain note".into(),
            updated_at_ms: 0,
            enhanced_md: String::new(),
            enhanced_template_id: String::new(),
            enhanced_at_ms: None,
            folder_id: None,
            transcript: vec![],
        };
        let md = meeting_markdown(&m, 0);
        assert!(md.contains("## My notes\n\nplain note"));
        assert!(!md.contains("Enhanced") && !md.contains("Transcript"));
    }

    #[test]
    fn file_stems_are_safe() {
        assert_eq!(
            file_stem("Q3 budget: sync/review?"),
            "Q3 budget sync review"
        );
        assert_eq!(file_stem("  ///  "), "Meeting");
    }
}
