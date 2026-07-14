-- Logical migration (diesel-dualdb): logical column types, one source for both
-- backends. Regenerate the per-backend SQL + schema.rs with `angreal schema gen`.
-- weir control-plane store ([[WEIR-I-0013]] / [[WEIR-A-0009]]).
--
-- Surrogate keys are client-generated UUIDs (the diesel-dualdb idiom — the
-- generator has no SERIAL/autoincrement). Recency that used rowid order is served
-- by the `ts` (epoch-millis) columns instead.
--
-- Multi-tenancy ([[WEIR-A-0004]]/[[WEIR-A-0036]], [[WEIR-I-0018]]): every scoped row carries a
-- `tenant_id` (default `'default'` — single-tenant deploys never notice it). Connection identity is
-- composite `(tenant_id, name)` so two tenants can each own a connection of the same name; the connector
-- catalog + stream state are tenant-scoped likewise. `claim()` filters by `tenant_id` for physical
-- runner isolation, so `work_units` carries it (surrogate `id` PK) with a matching index. Folded into
-- the baseline (not a follow-on migration): diesel-dualdb can't model a PK recompose in an ALTER, and
-- weir is pre-release so there are no deployed DBs to preserve.

CREATE TABLE dead_letters (
    id UUID PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    connection TEXT NOT NULL,
    stream TEXT NOT NULL,
    record TEXT NOT NULL,
    reason TEXT NOT NULL,
    ts BIGINT NOT NULL
);

CREATE TABLE stream_state (
    tenant_id TEXT NOT NULL DEFAULT 'default',
    connection TEXT NOT NULL,
    stream TEXT NOT NULL,
    cursor TEXT,
    opaque BYTEA NOT NULL,
    PRIMARY KEY (tenant_id, connection, stream)
);

-- Typed stream schemas ([[WEIR-I-0025]]) — a weir-native StreamSchema (JSON) per stream, captured at
-- run (connector-declared or inferred). Kept separate from stream_state so the engine's checkpoint
-- read/write tuple is untouched. `broken` flags a detected breaking drift ([[WEIR-T-0120]]).
CREATE TABLE stream_schemas (
    tenant_id TEXT NOT NULL DEFAULT 'default',
    connection TEXT NOT NULL,
    stream TEXT NOT NULL,
    schema TEXT NOT NULL,
    broken TEXT,
    PRIMARY KEY (tenant_id, connection, stream)
);

CREATE TABLE outbox (
    id UUID PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    connection TEXT NOT NULL,
    stream TEXT NOT NULL,
    seq BIGINT NOT NULL,
    processed INTEGER NOT NULL
);

CREATE TABLE run_logs (
    id UUID PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    connection TEXT NOT NULL,
    stream TEXT NOT NULL,
    level TEXT NOT NULL,
    message TEXT NOT NULL,
    ts BIGINT NOT NULL
);

-- weir-app: connections + the connector catalog ([[WEIR-T-0060]] / [[WEIR-I-0010]]).
-- every_secs is REAL (the generator has no DOUBLE; f32 is exact for schedule
-- intervals — the app casts f64<->f32 at the boundary).
CREATE TABLE connections (
    tenant_id TEXT NOT NULL DEFAULT 'default',
    name TEXT NOT NULL,
    source_ref TEXT NOT NULL,
    dest_ref TEXT NOT NULL,
    stream TEXT NOT NULL,
    -- Per-side connector config ([[WEIR-I-0029]]): the source and destination resolve independently, so
    -- e.g. a postgres→postgres connection can read one table and write another. A CLI/API `config`
    -- convenience sets both when the per-side ones aren't given.
    source_config TEXT NOT NULL DEFAULT '{}',
    dest_config TEXT NOT NULL DEFAULT '{}',
    every_secs REAL,
    cron TEXT,
    -- Per-connection sync/write modes ([[WEIR-I-0028]]): how the source reads + how the dest applies.
    -- `sync_mode` = full_refresh | incremental | cdc; `write_mode` = append | upsert | overwrite.
    -- `business_keys` is a JSON array (for upsert); `cursor_field` names the incremental cursor.
    sync_mode TEXT NOT NULL DEFAULT 'full_refresh',
    write_mode TEXT NOT NULL DEFAULT 'append',
    business_keys TEXT,
    cursor_field TEXT,
    PRIMARY KEY (tenant_id, name)
);

CREATE TABLE connectors (
    tenant_id TEXT NOT NULL DEFAULT 'default',
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    roles TEXT NOT NULL,
    config_schema TEXT NOT NULL,
    contract_version BIGINT NOT NULL,
    supported_sync_modes TEXT NOT NULL,
    origin TEXT NOT NULL,
    status TEXT NOT NULL,
    location TEXT NOT NULL,
    kind TEXT NOT NULL,
    manifest TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    PRIMARY KEY (tenant_id, name, version)
);

-- weir-orchestrator: the work-unit queue + schedules ([[WEIR-T-0061]]). `id` is a
-- BIGINT, client-generated (timestamp<<20 | counter — monotonic, so the FIFO
-- `ORDER BY id` claim and the run feed keep working without rowid autoincrement).
CREATE TABLE work_units (
    id BIGINT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    connection TEXT NOT NULL,
    stream TEXT NOT NULL,
    source_ref TEXT NOT NULL,
    dest_ref TEXT NOT NULL,
    -- Per-side connector config ([[WEIR-I-0029]]) — resolved independently for source + dest.
    source_config TEXT NOT NULL DEFAULT '{}',
    dest_config TEXT NOT NULL DEFAULT '{}',
    state TEXT NOT NULL,
    attempt BIGINT NOT NULL DEFAULT 0,
    next_attempt_at BIGINT NOT NULL DEFAULT 0,
    lease_owner TEXT,
    lease_expires_at BIGINT,
    state_key TEXT,
    seed_cursor TEXT,
    error TEXT,
    rows_written BIGINT NOT NULL DEFAULT 0,
    dead_lettered BIGINT NOT NULL DEFAULT 0,
    started_at BIGINT,
    finished_at BIGINT,
    partition TEXT NOT NULL DEFAULT 'null'
);

-- The claim() hot path: per-tenant FIFO over due pending units ([[WEIR-T-0091]]).
CREATE INDEX idx_work_units_claim ON work_units (tenant_id, state, next_attempt_at);

CREATE TABLE schedules (
    id BIGINT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    connection TEXT NOT NULL,
    spec TEXT NOT NULL,
    every_ms BIGINT NOT NULL,
    next_due_at BIGINT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    cron TEXT
);

-- Leader election for the autoscaler ([[WEIR-T-0104]]/[[WEIR-A-0023]]) — a singleton lease row per
-- name, heartbeated; portable (works without k8s). The leader is the only actor that scales runners.
CREATE TABLE leader_leases (
    name TEXT PRIMARY KEY NOT NULL,
    owner TEXT NOT NULL,
    expires_at BIGINT NOT NULL
);
