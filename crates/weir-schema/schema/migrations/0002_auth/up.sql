-- Logical migration (diesel-dualdb): control-plane auth baseline, mirroring Colliery
-- cloacina ([[WEIR-A-0008]] / [[WEIR-I-0017]]). Regenerate per-backend SQL + schema.rs with
-- `angreal schema gen`. Client-generated keys (UUID / TEXT); `*_at`/`ts` are epoch-millis BIGINT.

-- API keys — the one auth primitive. The secret is never stored, only its SHA-256 hex hash
-- (keys are high-entropy: a plain hash gives O(1) lookup + caching, no argon2 needed).
-- `permissions` is the role (read|write|admin); `tenant_id` NULL == a global/admin key;
-- `issued_via` is provenance for minted keys (oidc:<iss>:<sub> / local:<id>), NULL for manual;
-- `expires_at` NULL == no expiry (manual key), set for short-lived OIDC-minted keys.
CREATE TABLE api_keys (
    id UUID PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL,
    permissions TEXT NOT NULL,
    tenant_id TEXT,
    is_admin INTEGER NOT NULL DEFAULT 0,
    issued_via TEXT,
    created_at BIGINT NOT NULL,
    last_used_at BIGINT,
    expires_at BIGINT,
    revoked_at BIGINT
);

CREATE INDEX idx_api_keys_hash ON api_keys (key_hash);

-- Tenants — row-level multi-tenancy ([[WEIR-A-0004]], decided): tenant-scoped resources carry
-- a `tenant_id`; access is gated by the authz seam ([[WEIR-T-0085]]).
CREATE TABLE tenants (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

-- Audit trail: one row per mutation ([[WEIR-T-0085]]). `outcome` = ok|denied|error.
CREATE TABLE audit_events (
    id UUID PRIMARY KEY NOT NULL,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    resource TEXT NOT NULL,
    ts BIGINT NOT NULL,
    outcome TEXT NOT NULL
);

CREATE INDEX idx_audit_events_ts ON audit_events (ts);
