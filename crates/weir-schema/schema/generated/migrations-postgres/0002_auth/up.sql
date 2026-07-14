CREATE TABLE api_keys (id UUID PRIMARY KEY NOT NULL, name TEXT NOT NULL, key_hash TEXT NOT NULL, permissions TEXT NOT NULL, tenant_id TEXT, is_admin INTEGER NOT NULL DEFAULT 0, issued_via TEXT, created_at BIGINT NOT NULL, last_used_at BIGINT, expires_at BIGINT, revoked_at BIGINT);

CREATE INDEX idx_api_keys_hash ON api_keys(key_hash);

CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL, created_at BIGINT NOT NULL);

CREATE TABLE audit_events (id UUID PRIMARY KEY NOT NULL, actor TEXT NOT NULL, action TEXT NOT NULL, resource TEXT NOT NULL, ts BIGINT NOT NULL, outcome TEXT NOT NULL);

CREATE INDEX idx_audit_events_ts ON audit_events(ts);
