-- =============================================================================
-- 001_initial_schema.sql
-- Initial database schema for HA-VoIP.
-- Supports both SQLite and PostgreSQL.
-- =============================================================================

-- ---------------------------------------------------------------------------
-- Schema version tracking
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS schema_version (
    version     INTEGER PRIMARY KEY,
    applied_at  TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------------------
-- Extensions (SIP endpoints / users)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS extensions (
    id              INTEGER PRIMARY KEY,
    number          TEXT UNIQUE NOT NULL,
    display_name    TEXT,
    password_hash   TEXT NOT NULL,
    realm           TEXT,
    user_agent      TEXT,
    codec_prefs     TEXT,
    created_at      TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_extensions_number ON extensions(number);
CREATE INDEX IF NOT EXISTS idx_extensions_realm ON extensions(realm);

-- ---------------------------------------------------------------------------
-- Call history
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS call_history (
    id                  INTEGER PRIMARY KEY,
    call_id             TEXT UNIQUE,
    caller              TEXT,
    callee              TEXT,
    start_time          TIMESTAMP,
    answer_time         TIMESTAMP,
    end_time            TIMESTAMP,
    duration_seconds    REAL,
    status              TEXT,
    hangup_cause        TEXT,
    codec               TEXT,
    quality_score       REAL
);

CREATE INDEX IF NOT EXISTS idx_call_history_call_id ON call_history(call_id);
CREATE INDEX IF NOT EXISTS idx_call_history_caller ON call_history(caller);
CREATE INDEX IF NOT EXISTS idx_call_history_callee ON call_history(callee);
CREATE INDEX IF NOT EXISTS idx_call_history_start_time ON call_history(start_time);
CREATE INDEX IF NOT EXISTS idx_call_history_status ON call_history(status);

-- ---------------------------------------------------------------------------
-- Voicemails
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS voicemails (
    id                  INTEGER PRIMARY KEY,
    extension_id        INTEGER REFERENCES extensions(id),
    caller              TEXT,
    timestamp           TIMESTAMP,
    duration_seconds    REAL,
    file_path           TEXT,
    read                BOOLEAN DEFAULT FALSE,
    transcription       TEXT
);

CREATE INDEX IF NOT EXISTS idx_voicemails_extension_id ON voicemails(extension_id);
CREATE INDEX IF NOT EXISTS idx_voicemails_timestamp ON voicemails(timestamp);
CREATE INDEX IF NOT EXISTS idx_voicemails_read ON voicemails(read);

-- ---------------------------------------------------------------------------
-- Recordings
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS recordings (
    id                  INTEGER PRIMARY KEY,
    call_id             TEXT,
    file_path           TEXT,
    encryption_key_id   TEXT,
    duration_seconds    REAL,
    size_bytes          INTEGER,
    created_at          TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_recordings_call_id ON recordings(call_id);
CREATE INDEX IF NOT EXISTS idx_recordings_created_at ON recordings(created_at);

-- ---------------------------------------------------------------------------
-- Routing rules
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS routing_rules (
    id                  INTEGER PRIMARY KEY,
    pattern             TEXT,
    priority            INTEGER,
    destination         TEXT,
    destination_type    TEXT,
    time_conditions     TEXT,
    enabled             BOOLEAN DEFAULT TRUE
);

CREATE INDEX IF NOT EXISTS idx_routing_rules_pattern ON routing_rules(pattern);
CREATE INDEX IF NOT EXISTS idx_routing_rules_priority ON routing_rules(priority);
CREATE INDEX IF NOT EXISTS idx_routing_rules_enabled ON routing_rules(enabled);

-- ---------------------------------------------------------------------------
-- Configuration key-value store
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS config (
    key         TEXT PRIMARY KEY,
    value       TEXT,
    updated_at  TIMESTAMP
);

-- ---------------------------------------------------------------------------
-- Record initial schema version
-- ---------------------------------------------------------------------------
INSERT INTO schema_version (version, applied_at) VALUES (1, CURRENT_TIMESTAMP);
