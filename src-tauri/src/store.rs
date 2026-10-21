//! Meetings, notes and transcripts in an encrypted SQLite database (SQLCipher, AES-256 via
//! Apple CommonCrypto). The 256-bit key lives in the macOS Keychain, never on disk.

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
];

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
        Ok(())
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
        Ok(())
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
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn get_meeting(&self, id: &str) -> Result<Option<Meeting>, String> {
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        let meeting = conn
            .query_row(
                "SELECT id, title, started_at_ms, duration_secs, notes_json, notes_text, updated_at_ms,
                        enhanced_md, enhanced_template_id, enhanced_at_ms
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
        Ok(())
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

    #[test]
    fn keys_round_trip_through_hex() {
        let key = new_key().unwrap();
        assert_eq!(parse_key(&key_to_hex(&key)).unwrap(), key);
        assert!(parse_key("abc").is_err());
        assert!(parse_key(&"zz".repeat(32)).is_err());
    }
}
