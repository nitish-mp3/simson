//! Database layer for ha-voip engine.
//!
//! Provides abstracted storage for extensions, call history, voicemails,
//! recordings metadata, and routing rules. Uses SQLite by default with
//! PostgreSQL support for larger deployments.

use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument};

/// Database abstraction layer
pub struct Database {
    conn: Mutex<Connection>,
    db_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extension {
    pub id: i64,
    pub number: String,
    pub display_name: String,
    pub password_hash: String,
    pub realm: String,
    pub user_agent: Option<String>,
    pub codec_prefs: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallHistoryEntry {
    pub id: i64,
    pub call_id: String,
    pub caller: String,
    pub callee: String,
    pub start_time: String,
    pub answer_time: Option<String>,
    pub end_time: Option<String>,
    pub duration_seconds: Option<f64>,
    pub status: String,
    pub hangup_cause: Option<String>,
    pub codec: Option<String>,
    pub quality_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voicemail {
    pub id: i64,
    pub extension_id: i64,
    pub caller: String,
    pub timestamp: String,
    pub duration_seconds: f64,
    pub file_path: String,
    pub read: bool,
    pub transcription: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    pub id: i64,
    pub call_id: String,
    pub file_path: String,
    pub encryption_key_id: Option<String>,
    pub duration_seconds: f64,
    pub size_bytes: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub id: i64,
    pub pattern: String,
    pub priority: i32,
    pub destination: String,
    pub destination_type: String,
    pub time_conditions: Option<String>,
    pub enabled: bool,
}

impl Database {
    /// Create a new database connection
    pub fn new(path: &str, db_type: &str) -> Result<Self> {
        let conn = if db_type == "sqlite" {
            let p = Path::new(path);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)
                    .context("Failed to create database directory")?;
            }
            let conn = Connection::open(path)
                .context("Failed to open SQLite database")?;
            conn.execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA synchronous=NORMAL;
                 PRAGMA foreign_keys=ON;
                 PRAGMA busy_timeout=5000;"
            )?;
            conn
        } else {
            // For PostgreSQL, we'd use a different driver; for now SQLite is the
            // bundled default. PostgreSQL support uses the same schema via the
            // migrations/ SQL files and a dedicated async pool (tokio-postgres).
            Connection::open(path)?
        };

        Ok(Self {
            conn: Mutex::new(conn),
            db_type: db_type.to_string(),
        })
    }

    /// Initialize database schema
    #[instrument(skip(self))]
    pub fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock();

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS extensions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                number TEXT UNIQUE NOT NULL,
                display_name TEXT NOT NULL DEFAULT '',
                password_hash TEXT NOT NULL,
                realm TEXT NOT NULL DEFAULT 'ha-voip',
                user_agent TEXT,
                codec_prefs TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS call_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                call_id TEXT UNIQUE NOT NULL,
                caller TEXT NOT NULL,
                callee TEXT NOT NULL,
                start_time TIMESTAMP NOT NULL,
                answer_time TIMESTAMP,
                end_time TIMESTAMP,
                duration_seconds REAL,
                status TEXT NOT NULL DEFAULT 'initiated',
                hangup_cause TEXT,
                codec TEXT,
                quality_score REAL
            );

            CREATE TABLE IF NOT EXISTS voicemails (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                extension_id INTEGER NOT NULL REFERENCES extensions(id) ON DELETE CASCADE,
                caller TEXT NOT NULL,
                timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                duration_seconds REAL NOT NULL DEFAULT 0,
                file_path TEXT NOT NULL,
                read BOOLEAN NOT NULL DEFAULT FALSE,
                transcription TEXT
            );

            CREATE TABLE IF NOT EXISTS recordings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                call_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                encryption_key_id TEXT,
                duration_seconds REAL NOT NULL DEFAULT 0,
                size_bytes INTEGER NOT NULL DEFAULT 0,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS routing_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pattern TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 0,
                destination TEXT NOT NULL,
                destination_type TEXT NOT NULL DEFAULT 'extension',
                time_conditions TEXT,
                enabled BOOLEAN NOT NULL DEFAULT TRUE
            );

            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_extensions_number ON extensions(number);
            CREATE INDEX IF NOT EXISTS idx_call_history_call_id ON call_history(call_id);
            CREATE INDEX IF NOT EXISTS idx_call_history_start ON call_history(start_time);
            CREATE INDEX IF NOT EXISTS idx_call_history_caller ON call_history(caller);
            CREATE INDEX IF NOT EXISTS idx_call_history_callee ON call_history(callee);
            CREATE INDEX IF NOT EXISTS idx_voicemails_ext ON voicemails(extension_id);
            CREATE INDEX IF NOT EXISTS idx_recordings_call ON recordings(call_id);
            CREATE INDEX IF NOT EXISTS idx_routing_rules_pattern ON routing_rules(pattern);
            CREATE INDEX IF NOT EXISTS idx_routing_rules_priority ON routing_rules(priority);

            INSERT OR IGNORE INTO schema_version (version, applied_at)
                VALUES (1, CURRENT_TIMESTAMP);"
        )?;

        info!(db_type = %self.db_type, "Database schema initialized");
        Ok(())
    }

    // ── Extension CRUD ──────────────────────────────────────────────

    #[instrument(skip(self, password_hash))]
    pub fn create_extension(
        &self,
        number: &str,
        display_name: &str,
        password_hash: &str,
        realm: &str,
    ) -> Result<Extension> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO extensions (number, display_name, password_hash, realm)
             VALUES (?1, ?2, ?3, ?4)",
            params![number, display_name, password_hash, realm],
        )?;
        let id = conn.last_insert_rowid();
        debug!(id, number, "Extension created");

        self.get_extension_by_id(id)
    }

    pub fn get_extension(&self, number: &str) -> Result<Option<Extension>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, number, display_name, password_hash, realm, user_agent,
                    codec_prefs, created_at, updated_at
             FROM extensions WHERE number = ?1",
            params![number],
            |row| {
                Ok(Extension {
                    id: row.get(0)?,
                    number: row.get(1)?,
                    display_name: row.get(2)?,
                    password_hash: row.get(3)?,
                    realm: row.get(4)?,
                    user_agent: row.get(5)?,
                    codec_prefs: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        )
        .optional()
        .context("Failed to query extension")
    }

    pub fn get_extension_by_id(&self, id: i64) -> Result<Extension> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, number, display_name, password_hash, realm, user_agent,
                    codec_prefs, created_at, updated_at
             FROM extensions WHERE id = ?1",
            params![id],
            |row| {
                Ok(Extension {
                    id: row.get(0)?,
                    number: row.get(1)?,
                    display_name: row.get(2)?,
                    password_hash: row.get(3)?,
                    realm: row.get(4)?,
                    user_agent: row.get(5)?,
                    codec_prefs: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        )
        .context("Extension not found")
    }

    pub fn list_extensions(&self) -> Result<Vec<Extension>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, number, display_name, password_hash, realm, user_agent,
                    codec_prefs, created_at, updated_at
             FROM extensions ORDER BY number"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Extension {
                id: row.get(0)?,
                number: row.get(1)?,
                display_name: row.get(2)?,
                password_hash: row.get(3)?,
                realm: row.get(4)?,
                user_agent: row.get(5)?,
                codec_prefs: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;

        let mut extensions = Vec::new();
        for row in rows {
            extensions.push(row?);
        }
        Ok(extensions)
    }

    pub fn update_extension(
        &self,
        number: &str,
        display_name: Option<&str>,
        codec_prefs: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        if let Some(name) = display_name {
            conn.execute(
                "UPDATE extensions SET display_name = ?1, updated_at = CURRENT_TIMESTAMP
                 WHERE number = ?2",
                params![name, number],
            )?;
        }
        if let Some(codecs) = codec_prefs {
            conn.execute(
                "UPDATE extensions SET codec_prefs = ?1, updated_at = CURRENT_TIMESTAMP
                 WHERE number = ?2",
                params![codecs, number],
            )?;
        }
        Ok(())
    }

    pub fn delete_extension(&self, number: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute("DELETE FROM extensions WHERE number = ?1", params![number])?;
        Ok(affected > 0)
    }

    // ── Call History ────────────────────────────────────────────────

    pub fn insert_call_history(&self, entry: &CallHistoryEntry) -> Result<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO call_history (call_id, caller, callee, start_time, answer_time,
             end_time, duration_seconds, status, hangup_cause, codec, quality_score)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                entry.call_id,
                entry.caller,
                entry.callee,
                entry.start_time,
                entry.answer_time,
                entry.end_time,
                entry.duration_seconds,
                entry.status,
                entry.hangup_cause,
                entry.codec,
                entry.quality_score,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_call_end(
        &self,
        call_id: &str,
        end_time: &str,
        duration: f64,
        status: &str,
        hangup_cause: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE call_history SET end_time = ?1, duration_seconds = ?2,
             status = ?3, hangup_cause = ?4 WHERE call_id = ?5",
            params![end_time, duration, status, hangup_cause, call_id],
        )?;
        Ok(())
    }

    pub fn query_call_history(
        &self,
        limit: usize,
        offset: usize,
        caller_filter: Option<&str>,
    ) -> Result<Vec<CallHistoryEntry>> {
        let conn = self.conn.lock();
        let sql = if let Some(caller) = caller_filter {
            format!(
                "SELECT id, call_id, caller, callee, start_time, answer_time,
                 end_time, duration_seconds, status, hangup_cause, codec, quality_score
                 FROM call_history WHERE caller = '{}' OR callee = '{}'
                 ORDER BY start_time DESC LIMIT {} OFFSET {}",
                caller, caller, limit, offset
            )
        } else {
            format!(
                "SELECT id, call_id, caller, callee, start_time, answer_time,
                 end_time, duration_seconds, status, hangup_cause, codec, quality_score
                 FROM call_history ORDER BY start_time DESC LIMIT {} OFFSET {}",
                limit, offset
            )
        };

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(CallHistoryEntry {
                id: row.get(0)?,
                call_id: row.get(1)?,
                caller: row.get(2)?,
                callee: row.get(3)?,
                start_time: row.get(4)?,
                answer_time: row.get(5)?,
                end_time: row.get(6)?,
                duration_seconds: row.get(7)?,
                status: row.get(8)?,
                hangup_cause: row.get(9)?,
                codec: row.get(10)?,
                quality_score: row.get(11)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    // ── Voicemail ──────────────────────────────────────────────────

    pub fn create_voicemail(
        &self,
        extension_id: i64,
        caller: &str,
        duration: f64,
        file_path: &str,
    ) -> Result<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO voicemails (extension_id, caller, duration_seconds, file_path)
             VALUES (?1, ?2, ?3, ?4)",
            params![extension_id, caller, duration, file_path],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_voicemails(&self, extension_id: i64) -> Result<Vec<Voicemail>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, extension_id, caller, timestamp, duration_seconds,
                    file_path, read, transcription
             FROM voicemails WHERE extension_id = ?1
             ORDER BY timestamp DESC"
        )?;

        let rows = stmt.query_map(params![extension_id], |row| {
            Ok(Voicemail {
                id: row.get(0)?,
                extension_id: row.get(1)?,
                caller: row.get(2)?,
                timestamp: row.get(3)?,
                duration_seconds: row.get(4)?,
                file_path: row.get(5)?,
                read: row.get(6)?,
                transcription: row.get(7)?,
            })
        })?;

        let mut voicemails = Vec::new();
        for row in rows {
            voicemails.push(row?);
        }
        Ok(voicemails)
    }

    pub fn mark_voicemail_read(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("UPDATE voicemails SET read = TRUE WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delete_voicemail(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute("DELETE FROM voicemails WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    // ── Recording ──────────────────────────────────────────────────

    pub fn insert_recording(&self, metadata: &RecordingMetadata) -> Result<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO recordings (call_id, file_path, encryption_key_id,
             duration_seconds, size_bytes) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                metadata.call_id,
                metadata.file_path,
                metadata.encryption_key_id,
                metadata.duration_seconds,
                metadata.size_bytes,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn query_recordings(&self, call_id: &str) -> Result<Vec<RecordingMetadata>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, call_id, file_path, encryption_key_id, duration_seconds,
                    size_bytes, created_at
             FROM recordings WHERE call_id = ?1"
        )?;

        let rows = stmt.query_map(params![call_id], |row| {
            Ok(RecordingMetadata {
                id: row.get(0)?,
                call_id: row.get(1)?,
                file_path: row.get(2)?,
                encryption_key_id: row.get(3)?,
                duration_seconds: row.get(4)?,
                size_bytes: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;

        let mut recordings = Vec::new();
        for row in rows {
            recordings.push(row?);
        }
        Ok(recordings)
    }

    pub fn delete_recording(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute("DELETE FROM recordings WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    // ── Routing Rules ──────────────────────────────────────────────

    pub fn insert_routing_rule(&self, rule: &RoutingRule) -> Result<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO routing_rules (pattern, priority, destination, destination_type,
             time_conditions, enabled) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                rule.pattern,
                rule.priority,
                rule.destination,
                rule.destination_type,
                rule.time_conditions,
                rule.enabled,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_routing_rules(&self) -> Result<Vec<RoutingRule>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, pattern, priority, destination, destination_type,
                    time_conditions, enabled
             FROM routing_rules WHERE enabled = TRUE
             ORDER BY priority DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(RoutingRule {
                id: row.get(0)?,
                pattern: row.get(1)?,
                priority: row.get(2)?,
                destination: row.get(3)?,
                destination_type: row.get(4)?,
                time_conditions: row.get(5)?,
                enabled: row.get(6)?,
            })
        })?;

        let mut rules = Vec::new();
        for row in rows {
            rules.push(row?);
        }
        Ok(rules)
    }

    pub fn delete_routing_rule(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute("DELETE FROM routing_rules WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    // ── Config KV ──────────────────────────────────────────────────

    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO config (key, value, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = CURRENT_TIMESTAMP",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT value FROM config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to query config")
    }

    // ── Schema Version ─────────────────────────────────────────────

    pub fn get_schema_version(&self) -> Result<i32> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT MAX(version) FROM schema_version",
            [],
            |row| row.get::<_, i32>(0),
        )
        .context("Failed to query schema version")
    }

    pub fn is_healthy(&self) -> bool {
        let conn = self.conn.lock();
        conn.execute_batch("SELECT 1").is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::new(":memory:", "sqlite").unwrap()
    }

    #[test]
    fn test_init_schema() {
        let db = test_db();
        db.init_schema().unwrap();
        assert_eq!(db.get_schema_version().unwrap(), 1);
    }

    #[test]
    fn test_extension_crud() {
        let db = test_db();
        db.init_schema().unwrap();

        let ext = db.create_extension("100", "Alice", "hash123", "ha-voip").unwrap();
        assert_eq!(ext.number, "100");
        assert_eq!(ext.display_name, "Alice");

        let fetched = db.get_extension("100").unwrap().unwrap();
        assert_eq!(fetched.id, ext.id);

        let list = db.list_extensions().unwrap();
        assert_eq!(list.len(), 1);

        let deleted = db.delete_extension("100").unwrap();
        assert!(deleted);

        let deleted_again = db.delete_extension("100").unwrap();
        assert!(!deleted_again);
    }

    #[test]
    fn test_call_history() {
        let db = test_db();
        db.init_schema().unwrap();

        let entry = CallHistoryEntry {
            id: 0,
            call_id: "call-001".to_string(),
            caller: "100".to_string(),
            callee: "101".to_string(),
            start_time: "2024-01-01T00:00:00Z".to_string(),
            answer_time: None,
            end_time: None,
            duration_seconds: None,
            status: "initiated".to_string(),
            hangup_cause: None,
            codec: Some("opus".to_string()),
            quality_score: None,
        };
        db.insert_call_history(&entry).unwrap();

        let history = db.query_call_history(10, 0, None).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].call_id, "call-001");
    }

    #[test]
    fn test_config_kv() {
        let db = test_db();
        db.init_schema().unwrap();

        db.set_config("test_key", "test_value").unwrap();
        let val = db.get_config("test_key").unwrap();
        assert_eq!(val.unwrap(), "test_value");

        db.set_config("test_key", "updated").unwrap();
        let val = db.get_config("test_key").unwrap();
        assert_eq!(val.unwrap(), "updated");
    }

    #[test]
    fn test_health_check() {
        let db = test_db();
        assert!(db.is_healthy());
    }
}
