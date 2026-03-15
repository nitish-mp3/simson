-- =============================================================================
-- 002_add_federation.sql
-- Adds federation support: peer servers, shared extensions, and routes.
-- =============================================================================

-- ---------------------------------------------------------------------------
-- Federation peers (trusted remote VoIP servers)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS federation_peers (
    id              INTEGER PRIMARY KEY,
    name            TEXT NOT NULL,
    host            TEXT UNIQUE NOT NULL,
    port            INTEGER DEFAULT 5061,
    transport       TEXT DEFAULT 'tls',
    auth_token_hash TEXT,
    tls_fingerprint TEXT,
    status          TEXT DEFAULT 'active',
    last_seen       TIMESTAMP,
    created_at      TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at      TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_federation_peers_host ON federation_peers(host);
CREATE INDEX IF NOT EXISTS idx_federation_peers_status ON federation_peers(status);

-- ---------------------------------------------------------------------------
-- Federation extensions (extensions shared across peers)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS federation_extensions (
    id              INTEGER PRIMARY KEY,
    extension_id    INTEGER REFERENCES extensions(id),
    peer_id         INTEGER REFERENCES federation_peers(id),
    remote_number   TEXT NOT NULL,
    direction       TEXT DEFAULT 'both',
    enabled         BOOLEAN DEFAULT TRUE,
    created_at      TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_federation_extensions_extension_id ON federation_extensions(extension_id);
CREATE INDEX IF NOT EXISTS idx_federation_extensions_peer_id ON federation_extensions(peer_id);
CREATE INDEX IF NOT EXISTS idx_federation_extensions_remote_number ON federation_extensions(remote_number);
CREATE UNIQUE INDEX IF NOT EXISTS idx_federation_extensions_unique
    ON federation_extensions(extension_id, peer_id);

-- ---------------------------------------------------------------------------
-- Federation routes (how to reach federated extensions)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS federation_routes (
    id              INTEGER PRIMARY KEY,
    pattern         TEXT NOT NULL,
    peer_id         INTEGER REFERENCES federation_peers(id),
    priority        INTEGER DEFAULT 100,
    prefix_strip    INTEGER DEFAULT 0,
    prefix_add      TEXT DEFAULT '',
    enabled         BOOLEAN DEFAULT TRUE,
    created_at      TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_federation_routes_pattern ON federation_routes(pattern);
CREATE INDEX IF NOT EXISTS idx_federation_routes_peer_id ON federation_routes(peer_id);
CREATE INDEX IF NOT EXISTS idx_federation_routes_priority ON federation_routes(priority);

-- ---------------------------------------------------------------------------
-- Add federation columns to existing tables
-- ---------------------------------------------------------------------------

-- Add federation_peer_id to call_history to track federated calls
ALTER TABLE call_history ADD COLUMN federation_peer_id INTEGER REFERENCES federation_peers(id);
CREATE INDEX IF NOT EXISTS idx_call_history_federation_peer_id ON call_history(federation_peer_id);

-- Add federation_origin to extensions to track where an extension was provisioned
ALTER TABLE extensions ADD COLUMN federation_origin TEXT;
CREATE INDEX IF NOT EXISTS idx_extensions_federation_origin ON extensions(federation_origin);

-- Add federation flag to routing_rules
ALTER TABLE routing_rules ADD COLUMN federation_peer_id INTEGER REFERENCES federation_peers(id);
CREATE INDEX IF NOT EXISTS idx_routing_rules_federation_peer_id ON routing_rules(federation_peer_id);

-- ---------------------------------------------------------------------------
-- Record schema version
-- ---------------------------------------------------------------------------
INSERT INTO schema_version (version, applied_at) VALUES (2, CURRENT_TIMESTAMP);
