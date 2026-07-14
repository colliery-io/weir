CREATE TABLE dead_letters (id BLOB PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL DEFAULT 'default', connection TEXT NOT NULL, stream TEXT NOT NULL, record TEXT NOT NULL, reason TEXT NOT NULL, ts BIGINT NOT NULL);

CREATE TABLE stream_state (tenant_id TEXT NOT NULL DEFAULT 'default', connection TEXT NOT NULL, stream TEXT NOT NULL, cursor TEXT, opaque BLOB NOT NULL, PRIMARY KEY (tenant_id, connection, stream));

CREATE TABLE stream_schemas (tenant_id TEXT NOT NULL DEFAULT 'default', connection TEXT NOT NULL, stream TEXT NOT NULL, schema TEXT NOT NULL, broken TEXT, PRIMARY KEY (tenant_id, connection, stream));

CREATE TABLE outbox (id BLOB PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL DEFAULT 'default', connection TEXT NOT NULL, stream TEXT NOT NULL, seq BIGINT NOT NULL, processed INTEGER NOT NULL);

CREATE TABLE run_logs (id BLOB PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL DEFAULT 'default', connection TEXT NOT NULL, stream TEXT NOT NULL, level TEXT NOT NULL, message TEXT NOT NULL, ts BIGINT NOT NULL);

CREATE TABLE connections (tenant_id TEXT NOT NULL DEFAULT 'default', name TEXT NOT NULL, source_ref TEXT NOT NULL, dest_ref TEXT NOT NULL, stream TEXT NOT NULL, source_config TEXT NOT NULL DEFAULT '{}', dest_config TEXT NOT NULL DEFAULT '{}', every_secs REAL, cron TEXT, sync_mode TEXT NOT NULL DEFAULT 'full_refresh', write_mode TEXT NOT NULL DEFAULT 'append', business_keys TEXT, cursor_field TEXT, PRIMARY KEY (tenant_id, name));

CREATE TABLE connectors (tenant_id TEXT NOT NULL DEFAULT 'default', name TEXT NOT NULL, version TEXT NOT NULL, roles TEXT NOT NULL, config_schema TEXT NOT NULL, contract_version BIGINT NOT NULL, supported_sync_modes TEXT NOT NULL, origin TEXT NOT NULL, status TEXT NOT NULL, location TEXT NOT NULL, kind TEXT NOT NULL, manifest TEXT, created_at BIGINT NOT NULL, updated_at BIGINT NOT NULL, PRIMARY KEY (tenant_id, name, version));

CREATE TABLE work_units (id BIGINT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL DEFAULT 'default', connection TEXT NOT NULL, stream TEXT NOT NULL, source_ref TEXT NOT NULL, dest_ref TEXT NOT NULL, source_config TEXT NOT NULL DEFAULT '{}', dest_config TEXT NOT NULL DEFAULT '{}', state TEXT NOT NULL, attempt BIGINT NOT NULL DEFAULT 0, next_attempt_at BIGINT NOT NULL DEFAULT 0, lease_owner TEXT, lease_expires_at BIGINT, state_key TEXT, seed_cursor TEXT, error TEXT, rows_written BIGINT NOT NULL DEFAULT 0, dead_lettered BIGINT NOT NULL DEFAULT 0, started_at BIGINT, finished_at BIGINT, partition TEXT NOT NULL DEFAULT 'null');

CREATE INDEX idx_work_units_claim ON work_units(tenant_id,state,next_attempt_at);

CREATE TABLE schedules (id BIGINT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL DEFAULT 'default', connection TEXT NOT NULL, spec TEXT NOT NULL, every_ms BIGINT NOT NULL, next_due_at BIGINT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1, cron TEXT);

CREATE TABLE leader_leases (name TEXT PRIMARY KEY NOT NULL, owner TEXT NOT NULL, expires_at BIGINT NOT NULL);
