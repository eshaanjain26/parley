//! Meetings, notes and transcripts in an encrypted SQLite database (SQLCipher, AES-256 via
//! Apple CommonCrypto). The 256-bit key lives in the macOS Keychain, never on disk.

use crate::calendar::Attendee;
use crate::enhance::Template;
use crate::transcribe::{Speaker, TranscriptLine};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

pub const KEY_LEN: usize = 32;

pub struct Store(Mutex<Connection>);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub started_at_ms: i64,
    /// `None` while the meeting is still being recorded.
    pub duration_secs: Option<f64>,
    /// Editor document (TipTap JSON). Empty until the first save.
    pub notes_json: String,
    /// Plain text of the notes, kept alongside for search and AI prompts.
    pub notes_text: String,
    pub updated_at_ms: i64,
    /// AI-enhanced notes (Markdown); empty until the user runs Enhance.
    pub enhanced_md: String,
    pub enhanced_template_id: String,
    pub enhanced_at_ms: Option<i64>,
    /// The folder the meeting is filed in, if any.
    pub folder_id: Option<String>,
    /// The calendar event this recording belongs to, if one was matched.
    pub event_id: Option<String>,
    pub attendees: Vec<Attendee>,
    pub transcript: Vec<TranscriptLine>,
}

const MIGRATIONS: &[&str] = &[
    // v1
    "CREATE TABLE meetings (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        started_at_ms INTEGER NOT NULL,
        duration_secs REAL,
        notes_json TEXT NOT NULL DEFAULT '',
        notes_text TEXT NOT NULL DEFAULT '',
        updated_at_ms INTEGER NOT NULL
    );
    CREATE TABLE transcript_lines (
        meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
        speaker TEXT NOT NULL CHECK (speaker IN ('me', 'them')),
        segment_id INTEGER NOT NULL,
        start_ms INTEGER NOT NULL,
        end_ms INTEGER NOT NULL,
        text TEXT NOT NULL,
        PRIMARY KEY (meeting_id, speaker, segment_id)
    );
    CREATE INDEX meetings_by_start ON meetings(started_at_ms DESC);",
    // v2: AI-enhanced notes and the user's own templates
    "ALTER TABLE meetings ADD COLUMN enhanced_md TEXT NOT NULL DEFAULT '';
    ALTER TABLE meetings ADD COLUMN enhanced_template_id TEXT NOT NULL DEFAULT '';
    ALTER TABLE meetings ADD COLUMN enhanced_at_ms INTEGER;
    CREATE TABLE templates (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        instructions TEXT NOT NULL,
        updated_at_ms INTEGER NOT NULL
    );",
    // v3: full-text search over every meeting's title, notes, enhanced notes and transcript
    "CREATE VIRTUAL TABLE meeting_fts USING fts5(
        meeting_id UNINDEXED, title, notes, enhanced, transcript,
        tokenize = 'unicode61 remove_diacritics 2'
    );
    INSERT INTO meeting_fts (meeting_id, title, notes, enhanced, transcript)
        SELECT m.id, m.title, m.notes_text, m.enhanced_md,
               coalesce((SELECT group_concat(t.text, ' ') FROM transcript_lines t WHERE t.meeting_id = m.id), '')
        FROM meetings m;",
    // v4: folders. Deleting a folder unfiles its meetings; it never deletes them.
    "CREATE TABLE folders (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        created_at_ms INTEGER NOT NULL
    );
    ALTER TABLE meetings ADD COLUMN folder_id TEXT REFERENCES folders(id) ON DELETE SET NULL;
    CREATE INDEX meetings_by_folder ON meetings(folder_id, started_at_ms DESC);",
    // v5: the calendar event a recording belongs to, and who was invited
    "ALTER TABLE meetings ADD COLUMN event_id TEXT;
    ALTER TABLE meetings ADD COLUMN series_id TEXT;
    ALTER TABLE meetings ADD COLUMN attendees_json TEXT NOT NULL DEFAULT '[]';
    CREATE INDEX meetings_by_series ON meetings(series_id, started_at_ms DESC);",
];

/// Longest folder name, in characters.
pub const FOLDER_NAME_MAX: usize = 60;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub meeting_count: u32,
}

/// Trims and checks a folder name typed by the user.
pub fn clean_folder_name(name: &str) -> Result<String, String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err("Give the folder a name.".into());
    }
    if name.chars().count() > FOLDER_NAME_MAX {
        return Err(format!(
            "Folder names can be up to {FOLDER_NAME_MAX} characters."
        ));
    }
    Ok(name)
}

/// A random 128-bit id as hex.
pub fn new_id() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(hex(&bytes))
}

/// Rebuilds one meeting's search row from its current data. Called after every write.
fn refresh_fts(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM meeting_fts WHERE meeting_id = ?1", params![id])?;
    conn.execute(
        "INSERT INTO meeting_fts (meeting_id, title, notes, enhanced, transcript)
         SELECT m.id, m.title, m.notes_text, m.enhanced_md,
                coalesce((SELECT group_concat(t.text, ' ') FROM transcript_lines t WHERE t.meeting_id = m.id), '')
         FROM meetings m WHERE m.id = ?1",
        params![id],
    )?;
    Ok(())
}

/// Turns free text into a safe FTS5 query: each word becomes a quoted prefix term, so
/// operators or quotes typed by the user can't produce a syntax error.
pub fn fts_query(input: &str, any_term: bool) -> Option<String> {
    let terms: Vec<String> = input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{}\"*", w.to_lowercase()))
        .collect();
    if terms.is_empty() {
        return None;
    }
    Some(terms.join(if any_term { " OR " } else { " AND " }))
}

/// Marks a search snippet's matched text: \u{1} opens a highlight, \u{2} closes it.
pub const HIGHLIGHT_START: char = '\u{1}';
pub const HIGHLIGHT_END: char = '\u{2}';

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingSummary {
    pub id: String,
    pub title: String,
    pub started_at_ms: i64,
    pub duration_secs: Option<f64>,
    pub has_enhanced: bool,
    pub folder_id: Option<String>,
    /// For search results: the best-matching passage with highlight markers.
    pub snippet: Option<String>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn parse_key(hex_key: &str) -> Result<[u8; KEY_LEN], String> {
    let bytes: Vec<u8> = (0..hex_key.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex_key.get(i..i + 2).unwrap_or("zz"), 16))
        .collect::<Result<_, _>>()
        .map_err(|_| "stored database key is corrupt")?;
    bytes
        .try_into()
        .map_err(|_| "stored database key has the wrong length".to_string())
}

pub fn new_key() -> Result<[u8; KEY_LEN], String> {
    let mut key = [0u8; KEY_LEN];
    getrandom::fill(&mut key).map_err(|e| format!("no system randomness: {e}"))?;
    Ok(key)
}

pub fn key_to_hex(key: &[u8; KEY_LEN]) -> String {
    hex(key)
}

impl Store {
    pub fn open(path: &Path, key: &[u8; KEY_LEN]) -> Result<Self, String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        // The x'…' form passes a raw 256-bit key, skipping SQLCipher's passphrase derivation.
        conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex(key)))
            .map_err(|e| e.to_string())?;
        // SQLCipher only checks the key on first read; fail here, not on some later query.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(|_| {
            "the meeting database can't be opened with the key in your Keychain".to_string()
        })?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
            .map_err(|e| e.to_string())?;
        let store = Self(Mutex::new(conn));
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        let version: usize = conn
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())? as usize;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version) {
            conn.execute_batch(&format!(
                "BEGIN; {sql}; PRAGMA user_version = {}; COMMIT;",
                i + 1
            ))
            .map_err(|e| format!("database migration {} failed: {e}", i + 1))?;
        }
        Ok(())
    }

    pub fn create_meeting(&self, id: &str, title: &str, started_at_ms: i64) -> Result<(), String> {
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO meetings (id, title, started_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, ?3)",
            params![id, title, started_at_ms],
        )
        .map_err(|e| e.to_string())?;
        refresh_fts(&conn, id).map_err(|e| e.to_string())
    }

    pub fn save_notes(
        &self,
        id: &str,
        title: &str,
        notes_json: &str,
        notes_text: &str,
        updated_at_ms: i64,
    ) -> Result<(), String> {
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        let changed = conn
            .execute(
                "UPDATE meetings SET title = ?2, notes_json = ?3, notes_text = ?4, updated_at_ms = ?5
                 WHERE id = ?1",
                params![id, title, notes_json, notes_text, updated_at_ms],
            )
            .map_err(|e| e.to_string())?;
        if changed == 0 {
            return Err(format!("meeting {id} not found"));
        }
        refresh_fts(&conn, id).map_err(|e| e.to_string())
    }

    /// Records the duration and replaces the transcript in one transaction.
    pub fn finish_meeting(
        &self,
        id: &str,
        duration_secs: f64,
        lines: &[TranscriptLine],
    ) -> Result<(), String> {
        let mut conn = self.0.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE meetings SET duration_secs = ?2 WHERE id = ?1",
            params![id, duration_secs],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM transcript_lines WHERE meeting_id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
        {
            let mut insert = tx
                .prepare(
                    "INSERT INTO transcript_lines (meeting_id, speaker, segment_id, start_ms, end_ms, text)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .map_err(|e| e.to_string())?;
            for l in lines {
                insert
                    .execute(params![
                        id,
                        speaker_str(l.speaker),
                        l.segment_id as i64,
                        l.start_ms as i64,
                        l.end_ms as i64,
                        l.text
                    ])
                    .map_err(|e| e.to_string())?;
            }
        }
        refresh_fts(&tx, id).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn get_meeting(&self, id: &str) -> Result<Option<Meeting>, String> {
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        let meeting = conn
            .query_row(
                "SELECT id, title, started_at_ms, duration_secs, notes_json, notes_text, updated_at_ms,
                        enhanced_md, enhanced_template_id, enhanced_at_ms, folder_id,
                        event_id, attendees_json
                 FROM meetings WHERE id = ?1",
                params![id],
                |r| {
                    Ok(Meeting {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        started_at_ms: r.get(2)?,
                        duration_secs: r.get(3)?,
                        notes_json: r.get(4)?,
                        notes_text: r.get(5)?,
                        updated_at_ms: r.get(6)?,
                        enhanced_md: r.get(7)?,
                        enhanced_template_id: r.get(8)?,
                        enhanced_at_ms: r.get(9)?,
                        folder_id: r.get(10)?,
                        event_id: r.get(11)?,
                        // Written only by link_event; an unreadable list just shows no one.
                        attendees: serde_json::from_str(&r.get::<_, String>(12)?).unwrap_or_default(),
                        transcript: Vec::new(),
                    })
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let Some(mut meeting) = meeting else {
            return Ok(None);
        };
        let mut stmt = conn
            .prepare(
                "SELECT speaker, segment_id, start_ms, end_ms, text FROM transcript_lines
                 WHERE meeting_id = ?1 ORDER BY start_ms, speaker",
            )
            .map_err(|e| e.to_string())?;
        meeting.transcript = stmt
            .query_map(params![id], |r| {
                let speaker: String = r.get(0)?;
                Ok(TranscriptLine {
                    speaker: if speaker == "them" {
                        Speaker::Them
                    } else {
                        Speaker::Me
                    },
                    segment_id: r.get::<_, i64>(1)? as u64,
                    is_final: true,
                    start_ms: r.get::<_, i64>(2)? as u64,
                    end_ms: r.get::<_, i64>(3)? as u64,
                    text: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        Ok(Some(meeting))
    }
}

impl Store {
    pub fn save_enhanced(
        &self,
        id: &str,
        markdown: &str,
        template_id: &str,
        at_ms: i64,
    ) -> Result<(), String> {
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        let changed = conn
            .execute(
                "UPDATE meetings SET enhanced_md = ?2, enhanced_template_id = ?3, enhanced_at_ms = ?4
                 WHERE id = ?1",
                params![id, markdown, template_id, at_ms],
            )
            .map_err(|e| e.to_string())?;
        if changed == 0 {
            return Err(format!("meeting {id} not found"));
        }
        refresh_fts(&conn, id).map_err(|e| e.to_string())
    }

    /// Most recent first; only the given folder's meetings when `folder` is set.
    pub fn list_meetings(
        &self,
        limit: u32,
        offset: u32,
        folder: Option<&str>,
    ) -> Result<Vec<MeetingSummary>, String> {
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, started_at_ms, duration_secs, enhanced_md != '', folder_id
                 FROM meetings WHERE ?3 IS NULL OR folder_id = ?3
                 ORDER BY started_at_ms DESC LIMIT ?1 OFFSET ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![limit, offset, folder], |r| {
                Ok(MeetingSummary {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    started_at_ms: r.get(2)?,
                    duration_secs: r.get(3)?,
                    has_enhanced: r.get(4)?,
                    folder_id: r.get(5)?,
                    snippet: None,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    /// Ranked full-text search. `any_term` matches meetings with any of the words (for
    /// finding context to answer a question) instead of all of them (the search box).
    /// `folder` limits the search to one folder.
    pub fn search(
        &self,
        query: &str,
        any_term: bool,
        limit: u32,
        folder: Option<&str>,
    ) -> Result<Vec<MeetingSummary>, String> {
        let Some(fts) = fts_query(query, any_term) else {
            return Ok(Vec::new());
        };
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT m.id, m.title, m.started_at_ms, m.duration_secs, m.enhanced_md != '', m.folder_id,
                        snippet(meeting_fts, -1, '{HIGHLIGHT_START}', '{HIGHLIGHT_END}', '…', 14)
                 FROM meeting_fts JOIN meetings m ON m.id = meeting_fts.meeting_id
                 WHERE meeting_fts MATCH ?1 AND (?3 IS NULL OR m.folder_id = ?3)
                 ORDER BY bm25(meeting_fts, 0.0, 5.0, 3.0, 2.0, 1.0)
                 LIMIT ?2"
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![fts, limit, folder], |r| {
                Ok(MeetingSummary {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    started_at_ms: r.get(2)?,
                    duration_secs: r.get(3)?,
                    has_enhanced: r.get(4)?,
                    folder_id: r.get(5)?,
                    snippet: Some(r.get(6)?),
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    /// Removes a meeting with its notes, transcript and search entry.
    pub fn delete_meeting(&self, id: &str) -> Result<(), String> {
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM meetings WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM meeting_fts WHERE meeting_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Every folder with its meeting count, sorted by name.
    pub fn list_folders(&self) -> Result<Vec<Folder>, String> {
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT f.id, f.name, (SELECT count(*) FROM meetings m WHERE m.folder_id = f.id)
                 FROM folders f ORDER BY f.name COLLATE NOCASE",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Folder {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    meeting_count: r.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    fn name_taken(conn: &Connection, name: &str, except_id: &str) -> Result<bool, String> {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM folders WHERE name = ?1 COLLATE NOCASE AND id != ?2)",
            params![name, except_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())
    }

    pub fn create_folder(&self, id: &str, name: &str, at_ms: i64) -> Result<Folder, String> {
        let name = clean_folder_name(name)?;
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        if Self::name_taken(&conn, &name, id)? {
            return Err(format!("A folder called \"{name}\" already exists."));
        }
        conn.execute(
            "INSERT INTO folders (id, name, created_at_ms) VALUES (?1, ?2, ?3)",
            params![id, name, at_ms],
        )
        .map_err(|e| e.to_string())?;
        Ok(Folder {
            id: id.into(),
            name,
            meeting_count: 0,
        })
    }

    pub fn rename_folder(&self, id: &str, name: &str) -> Result<(), String> {
        let name = clean_folder_name(name)?;
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        if Self::name_taken(&conn, &name, id)? {
            return Err(format!("A folder called \"{name}\" already exists."));
        }
        let changed = conn
            .execute(
                "UPDATE folders SET name = ?2 WHERE id = ?1",
                params![id, name],
            )
            .map_err(|e| e.to_string())?;
        if changed == 0 {
            return Err("That folder no longer exists.".into());
        }
        Ok(())
    }

    /// Deletes the folder only; its meetings stay, unfiled.
    pub fn delete_folder(&self, id: &str) -> Result<(), String> {
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM folders WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Files a meeting in a folder, or takes it out of its folder with `None`.
    pub fn set_meeting_folder(
        &self,
        meeting_id: &str,
        folder_id: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        let changed = conn
            .execute(
                "UPDATE meetings SET folder_id = ?2 WHERE id = ?1",
                params![meeting_id, folder_id],
            )
            .map_err(|e| {
                if e.to_string().contains("FOREIGN KEY") {
                    "That folder no longer exists.".to_string()
                } else {
                    e.to_string()
                }
            })?;
        if changed == 0 {
            return Err(format!("meeting {meeting_id} not found"));
        }
        Ok(())
    }

    /// Ties a meeting to its calendar event. An unfiled meeting from a repeating event goes into
    /// the folder its most recent filed occurrence is in; returns that folder when it does.
    pub fn link_event(
        &self,
        meeting_id: &str,
        event_id: &str,
        series_id: Option<&str>,
        attendees: &[Attendee],
    ) -> Result<Option<String>, String> {
        let attendees_json = serde_json::to_string(attendees).map_err(|e| e.to_string())?;
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        let changed = conn
            .execute(
                "UPDATE meetings SET event_id = ?2, series_id = ?3, attendees_json = ?4 WHERE id = ?1",
                params![meeting_id, event_id, series_id, attendees_json],
            )
            .map_err(|e| e.to_string())?;
        if changed == 0 {
            return Err(format!("meeting {meeting_id} not found"));
        }
        let Some(series_id) = series_id else {
            return Ok(None);
        };
        let filed: Option<String> = conn
            .query_row(
                "SELECT folder_id FROM meetings WHERE id = ?1",
                params![meeting_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if filed.is_some() {
            return Ok(None);
        }
        let folder: Option<String> = conn
            .query_row(
                "SELECT folder_id FROM meetings
                 WHERE series_id = ?1 AND id != ?2 AND folder_id IS NOT NULL
                 ORDER BY started_at_ms DESC LIMIT 1",
                params![series_id, meeting_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(folder) = &folder {
            conn.execute(
                "UPDATE meetings SET folder_id = ?2 WHERE id = ?1",
                params![meeting_id, folder],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(folder)
    }

    pub fn custom_templates(&self) -> Result<Vec<Template>, String> {
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, name, instructions FROM templates ORDER BY name COLLATE NOCASE")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Template {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    instructions: r.get(2)?,
                    built_in: false,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
    }

    /// Creates or replaces a custom template.
    pub fn save_template(&self, t: &Template, at_ms: i64) -> Result<(), String> {
        if t.name.trim().is_empty() || t.instructions.trim().is_empty() {
            return Err("a template needs a name and instructions".into());
        }
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO templates (id, name, instructions, updated_at_ms) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET name = ?2, instructions = ?3, updated_at_ms = ?4",
            params![t.id, t.name.trim(), t.instructions.trim(), at_ms],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_template(&self, id: &str) -> Result<(), String> {
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM templates WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn speaker_str(s: Speaker) -> &'static str {
    match s {
        Speaker::Me => "me",
        Speaker::Them => "them",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; KEY_LEN] = [7; KEY_LEN];

    fn line(speaker: Speaker, segment_id: u64, start_ms: u64, text: &str) -> TranscriptLine {
        TranscriptLine {
            speaker,
            segment_id,
            is_final: true,
            text: text.into(),
            start_ms,
            end_ms: start_ms + 900,
        }
    }

    #[test]
    fn meeting_round_trip_with_notes_and_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("parley.db"), &KEY).unwrap();
        store.create_meeting("m1", "Weekly sync", 1_000).unwrap();
        store
            .save_notes(
                "m1",
                "Budget sync",
                r#"{"type":"doc"}"#,
                "move review to Friday",
                2_000,
            )
            .unwrap();
        store
            .finish_meeting(
                "m1",
                61.5,
                &[
                    line(Speaker::Them, 0, 5_000, "Numbers by Thursday."),
                    line(Speaker::Me, 0, 1_000, "Morning!"),
                ],
            )
            .unwrap();

        let m = store.get_meeting("m1").unwrap().unwrap();
        assert_eq!(m.title, "Budget sync");
        assert_eq!(m.duration_secs, Some(61.5));
        assert_eq!(m.notes_text, "move review to Friday");
        assert_eq!(m.updated_at_ms, 2_000);
        let texts: Vec<&str> = m.transcript.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["Morning!", "Numbers by Thursday."]);
        assert_eq!(m.transcript[1].speaker, Speaker::Them);
    }

    #[test]
    fn finishing_twice_replaces_the_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("parley.db"), &KEY).unwrap();
        store.create_meeting("m1", "t", 0).unwrap();
        store
            .finish_meeting("m1", 1.0, &[line(Speaker::Me, 0, 0, "old")])
            .unwrap();
        store
            .finish_meeting("m1", 2.0, &[line(Speaker::Me, 0, 0, "new")])
            .unwrap();
        let m = store.get_meeting("m1").unwrap().unwrap();
        assert_eq!(m.transcript.len(), 1);
        assert_eq!(m.transcript[0].text, "new");
    }

    #[test]
    fn missing_meetings_are_none_and_unknown_saves_fail() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("parley.db"), &KEY).unwrap();
        assert!(store.get_meeting("nope").unwrap().is_none());
        assert!(store.save_notes("nope", "t", "", "", 0).is_err());
    }

    #[test]
    fn database_file_is_encrypted_at_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("parley.db");
        {
            let store = Store::open(&path, &KEY).unwrap();
            store
                .create_meeting("m1", "Acquisition of Contoso", 0)
                .unwrap();
            store
                .save_notes(
                    "m1",
                    "Acquisition of Contoso",
                    "",
                    "confidential price 42 million",
                    1,
                )
                .unwrap();
        }
        let mut raw = std::fs::read(&path).unwrap();
        if let Ok(wal) = std::fs::read(path.with_extension("db-wal")) {
            raw.extend(wal);
        }
        assert!(
            !raw.starts_with(b"SQLite format 3"),
            "plain SQLite header on disk"
        );
        let needle = b"confidential price";
        assert!(
            !raw.windows(needle.len()).any(|w| w == needle),
            "notes readable on disk"
        );
    }

    #[test]
    fn same_key_reopens_and_wrong_key_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("parley.db");
        Store::open(&path, &KEY)
            .unwrap()
            .create_meeting("m1", "t", 0)
            .unwrap();
        assert!(
            Store::open(&path, &KEY)
                .unwrap()
                .get_meeting("m1")
                .unwrap()
                .is_some()
        );
        let err = Store::open(&path, &[8; KEY_LEN]).err().unwrap();
        assert!(err.contains("can't be opened"), "{err}");
    }

    #[test]
    fn enhanced_notes_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("parley.db"), &KEY).unwrap();
        store.create_meeting("m1", "t", 0).unwrap();
        assert_eq!(
            store.get_meeting("m1").unwrap().unwrap().enhanced_at_ms,
            None
        );
        store
            .save_enhanced("m1", "## Decisions\n- Ship Friday", "general", 9)
            .unwrap();
        let m = store.get_meeting("m1").unwrap().unwrap();
        assert_eq!(m.enhanced_md, "## Decisions\n- Ship Friday");
        assert_eq!(m.enhanced_template_id, "general");
        assert_eq!(m.enhanced_at_ms, Some(9));
        assert!(store.save_enhanced("nope", "x", "general", 1).is_err());
    }

    #[test]
    fn custom_templates_create_update_list_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("parley.db"), &KEY).unwrap();
        let t = |id: &str, name: &str| Template {
            id: id.into(),
            name: name.into(),
            instructions: "## Risks".into(),
            built_in: false,
        };
        store.save_template(&t("b", "Board prep"), 1).unwrap();
        store.save_template(&t("a", "account review"), 1).unwrap();
        store.save_template(&t("b", "Board prep v2"), 2).unwrap();
        let names: Vec<String> = store
            .custom_templates()
            .unwrap()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, ["account review", "Board prep v2"]);
        store.delete_template("a").unwrap();
        assert_eq!(store.custom_templates().unwrap().len(), 1);
        assert!(store.save_template(&t("c", "  "), 1).is_err());
    }

    #[test]
    fn a_v1_database_upgrades_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("parley.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex(&KEY)))
                .unwrap();
            conn.execute_batch(&format!(
                "BEGIN; {}; PRAGMA user_version = 1; COMMIT;",
                MIGRATIONS[0]
            ))
            .unwrap();
            conn.execute(
                "INSERT INTO meetings (id, title, started_at_ms, notes_text, updated_at_ms)
                 VALUES ('old', 'Before upgrade', 1, 'kept', 1)",
                [],
            )
            .unwrap();
        }
        let store = Store::open(&path, &KEY).unwrap();
        let m = store.get_meeting("old").unwrap().unwrap();
        assert_eq!(m.notes_text, "kept");
        assert_eq!(m.enhanced_md, "");
        assert!(store.custom_templates().unwrap().is_empty());
    }

    fn seeded() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("parley.db"), &KEY).unwrap();
        store.create_meeting("a", "Q3 budget sync", 1_000).unwrap();
        store
            .save_notes("a", "Q3 budget sync", "", "Priya owns the forecast", 2)
            .unwrap();
        store
            .finish_meeting(
                "a",
                60.0,
                &[line(
                    Speaker::Them,
                    0,
                    0,
                    "Revenue numbers are due Thursday.",
                )],
            )
            .unwrap();
        store.create_meeting("b", "Hiring standup", 5_000).unwrap();
        store
            .finish_meeting(
                "b",
                30.0,
                &[line(Speaker::Me, 0, 0, "We closed two backend roles.")],
            )
            .unwrap();
        store
            .save_enhanced("b", "## Decisions\n- Pause frontend hiring", "standup", 6)
            .unwrap();
        (dir, store)
    }

    #[test]
    fn list_is_newest_first_and_flags_enhanced() {
        let (_d, store) = seeded();
        let list = store.list_meetings(10, 0, None).unwrap();
        let ids: Vec<&str> = list.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["b", "a"]);
        assert!(list[0].has_enhanced && !list[1].has_enhanced);
        assert_eq!(store.list_meetings(1, 1, None).unwrap()[0].id, "a");
    }

    #[test]
    fn search_covers_notes_transcript_and_enhanced_with_highlights() {
        let (_d, store) = seeded();
        let by_note = store.search("priya", false, 10, None).unwrap();
        assert_eq!(by_note.len(), 1);
        assert_eq!(by_note[0].id, "a");
        assert!(
            by_note[0]
                .snippet
                .as_deref()
                .unwrap()
                .contains("\u{1}Priya\u{2}")
        );
        assert_eq!(
            store.search("revenue thursday", false, 10, None).unwrap()[0].id,
            "a"
        );
        assert_eq!(
            store.search("frontend", false, 10, None).unwrap()[0].id,
            "b"
        );
        // Prefix matching: "forec" finds "forecast".
        assert_eq!(store.search("forec", false, 10, None).unwrap().len(), 1);
        // All words must match by default; any_term widens it.
        assert!(
            store
                .search("priya backend", false, 10, None)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store.search("priya backend", true, 10, None).unwrap().len(),
            2
        );
    }

    #[test]
    fn search_survives_operators_and_punctuation() {
        let (_d, store) = seeded();
        for q in ["\"", "AND OR NOT", "budget*", "(q3", "-", "NEAR(", ""] {
            assert!(store.search(q, false, 10, None).is_ok(), "{q:?}");
        }
        assert_eq!(
            fts_query("Q3 budget!", false).unwrap(),
            "\"q3\"* AND \"budget\"*"
        );
        assert_eq!(fts_query("  ?! ", false), None);
    }

    #[test]
    fn edits_keep_the_index_current_and_delete_removes_everything() {
        let (_d, store) = seeded();
        store
            .save_notes("a", "Q3 budget sync", "", "Marco owns it now", 3)
            .unwrap();
        assert!(store.search("priya", false, 10, None).unwrap().is_empty());
        assert_eq!(store.search("marco", false, 10, None).unwrap().len(), 1);
        store.delete_meeting("a").unwrap();
        assert!(store.get_meeting("a").unwrap().is_none());
        assert!(store.search("revenue", false, 10, None).unwrap().is_empty());
        assert_eq!(store.list_meetings(10, 0, None).unwrap().len(), 1);
    }

    #[test]
    fn migration_v3_indexes_meetings_that_already_exist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("parley.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex(&KEY)))
                .unwrap();
            conn.execute_batch(&format!(
                "BEGIN; {}; {}; PRAGMA user_version = 2; COMMIT;",
                MIGRATIONS[0], MIGRATIONS[1]
            ))
            .unwrap();
            conn.execute(
                "INSERT INTO meetings (id, title, started_at_ms, notes_text, updated_at_ms)
                 VALUES ('old', 'Vendor review', 1, 'renewal pricing', 1)",
                [],
            )
            .unwrap();
        }
        let store = Store::open(&path, &KEY).unwrap();
        assert_eq!(
            store.search("renewal", false, 10, None).unwrap()[0].id,
            "old"
        );
    }

    #[test]
    fn folders_file_meetings_and_scope_list_and_search() {
        let (_dir, store) = seeded();
        let acme = store.create_folder("f1", "  Acme   renewal ", 10).unwrap();
        assert_eq!(acme.name, "Acme renewal");
        store.create_folder("f2", "Hiring", 11).unwrap();
        store.set_meeting_folder("a", Some("f1")).unwrap();
        store.set_meeting_folder("b", Some("f2")).unwrap();

        let in_acme = store.list_meetings(10, 0, Some("f1")).unwrap();
        assert_eq!(
            in_acme.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["a"]
        );
        assert_eq!(in_acme[0].folder_id.as_deref(), Some("f1"));
        assert_eq!(store.list_meetings(10, 0, None).unwrap().len(), 2);
        assert_eq!(
            store
                .get_meeting("a")
                .unwrap()
                .unwrap()
                .folder_id
                .as_deref(),
            Some("f1")
        );

        // "priya backend" matches both meetings with any_term; the folder narrows it to one.
        assert_eq!(
            store.search("priya backend", true, 10, None).unwrap().len(),
            2
        );
        let scoped = store.search("priya backend", true, 10, Some("f2")).unwrap();
        assert_eq!(
            scoped.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["b"]
        );

        let folders = store.list_folders().unwrap();
        assert_eq!(
            folders
                .iter()
                .map(|f| (f.name.as_str(), f.meeting_count))
                .collect::<Vec<_>>(),
            [("Acme renewal", 1), ("Hiring", 1)]
        );

        store.set_meeting_folder("a", None).unwrap();
        assert!(store.list_meetings(10, 0, Some("f1")).unwrap().is_empty());
    }

    #[test]
    fn folder_names_are_cleaned_unique_and_renamable() {
        let (_dir, store) = seeded();
        store.create_folder("f1", "Acme", 1).unwrap();
        assert!(
            store
                .create_folder("f2", "acme", 2)
                .unwrap_err()
                .contains("already exists")
        );
        assert!(store.create_folder("f2", "   ", 2).is_err());
        assert!(
            store
                .create_folder("f2", &"x".repeat(FOLDER_NAME_MAX + 1), 2)
                .is_err()
        );
        // Renaming to its own name in a different case is fine; to another folder's name is not.
        store.rename_folder("f1", "ACME").unwrap();
        store.create_folder("f2", "Globex", 2).unwrap();
        assert!(store.rename_folder("f2", "acme").is_err());
        assert!(store.rename_folder("missing", "New").is_err());
        assert_eq!(store.list_folders().unwrap()[0].name, "ACME");
    }

    #[test]
    fn deleting_a_folder_keeps_its_meetings() {
        let (_dir, store) = seeded();
        store.create_folder("f1", "Acme", 1).unwrap();
        store.set_meeting_folder("a", Some("f1")).unwrap();
        store.delete_folder("f1").unwrap();
        let a = store.get_meeting("a").unwrap().unwrap();
        assert_eq!(a.folder_id, None);
        assert_eq!(store.list_meetings(10, 0, None).unwrap().len(), 2);
        assert!(store.list_folders().unwrap().is_empty());
        // Filing into a folder that is gone fails cleanly instead of leaving a dangling id.
        assert!(
            store
                .set_meeting_folder("a", Some("f1"))
                .unwrap_err()
                .contains("no longer exists")
        );
        assert!(store.set_meeting_folder("missing", None).is_err());
    }

    #[test]
    fn migration_v4_adds_folders_to_an_existing_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("parley.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", hex(&KEY)))
                .unwrap();
            conn.execute_batch(&format!(
                "BEGIN; {}; {}; {}; PRAGMA user_version = 3; COMMIT;",
                MIGRATIONS[0], MIGRATIONS[1], MIGRATIONS[2]
            ))
            .unwrap();
            conn.execute(
                "INSERT INTO meetings (id, title, started_at_ms, updated_at_ms) VALUES ('old', 'Vendor review', 1, 1)",
                [],
            )
            .unwrap();
        }
        let store = Store::open(&path, &KEY).unwrap();
        assert_eq!(store.get_meeting("old").unwrap().unwrap().folder_id, None);
        store.create_folder("f1", "Vendors", 2).unwrap();
        store.set_meeting_folder("old", Some("f1")).unwrap();
        assert_eq!(store.list_folders().unwrap()[0].meeting_count, 1);
    }

    #[test]
    fn linking_an_event_stores_attendees_and_files_repeat_meetings() {
        let (_dir, store) = seeded();
        store.create_folder("f1", "1:1s", 1).unwrap();
        let priya = Attendee {
            name: "Priya Shah".into(),
            email: "priya@example.com".into(),
        };
        // Last week's occurrence, filed by hand.
        assert_eq!(
            store
                .link_event("a", "ev:1", Some("weekly"), std::slice::from_ref(&priya))
                .unwrap(),
            None
        );
        store.set_meeting_folder("a", Some("f1")).unwrap();
        // This week's occurrence follows it into the folder.
        assert_eq!(
            store
                .link_event("b", "ev:2", Some("weekly"), std::slice::from_ref(&priya))
                .unwrap()
                .as_deref(),
            Some("f1")
        );
        let b = store.get_meeting("b").unwrap().unwrap();
        assert_eq!(b.folder_id.as_deref(), Some("f1"));
        assert_eq!(b.event_id.as_deref(), Some("ev:2"));
        assert_eq!(b.attendees, vec![priya]);
    }

    #[test]
    fn linking_never_moves_a_filed_meeting_or_one_off_events() {
        let (_dir, store) = seeded();
        store.create_folder("f1", "Old", 1).unwrap();
        store.create_folder("f2", "Chosen", 2).unwrap();
        store.link_event("a", "ev:1", Some("weekly"), &[]).unwrap();
        store.set_meeting_folder("a", Some("f1")).unwrap();
        // Already filed (e.g. recorded from a folder page): keeps its folder.
        store.set_meeting_folder("b", Some("f2")).unwrap();
        assert_eq!(
            store.link_event("b", "ev:2", Some("weekly"), &[]).unwrap(),
            None
        );
        assert_eq!(
            store
                .get_meeting("b")
                .unwrap()
                .unwrap()
                .folder_id
                .as_deref(),
            Some("f2")
        );
        // One-off events have no series to follow.
        store.set_meeting_folder("b", None).unwrap();
        assert_eq!(store.link_event("b", "ev:3", None, &[]).unwrap(), None);
        assert!(store.link_event("missing", "ev:4", None, &[]).is_err());
        assert!(
            store
                .get_meeting("b")
                .unwrap()
                .unwrap()
                .attendees
                .is_empty()
        );
    }

    #[test]
    fn new_ids_are_random_hex() {
        let a = new_id().unwrap();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, new_id().unwrap());
    }

    #[test]
    fn keys_round_trip_through_hex() {
        let key = new_key().unwrap();
        assert_eq!(parse_key(&key_to_hex(&key)).unwrap(), key);
        assert!(parse_key("abc").is_err());
        assert!(parse_key(&"zz".repeat(32)).is_err());
    }
}
