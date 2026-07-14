---
id: schema-tenant-scoping-tenants-crud
level: task
title: "Schema — tenant scoping + tenants CRUD + default-tenant backfill"
short_code: "WEIR-T-0089"
created_at: 2026-07-05T23:57:16.945067+00:00
updated_at: 2026-07-06T00:12:54.183716+00:00
parent: WEIR-I-0018
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0018
---

# Schema — tenant scoping + tenants CRUD + default-tenant backfill

## Parent Initiative

[[WEIR-I-0018]] — the schema spine. Governed by [[WEIR-A-0036]] (decision 2: composite `(tenant_id, name)`;
decision 4: implicit `default` tenant).

## Objective

Add `tenant_id` to every tenant-scoped control-plane table and recompose connection identity to
`(tenant_id, name)`, in one diesel-dualdb migration that **backfills existing rows to a `default` tenant** so
current single-tenant deploys keep working. This is the foundation the store/runner/compile tasks build on.

## Reference

- Schema is generated: `angreal schema gen` from the logical DDL (`crates/weir-schema/schema/*.sql`) → per-backend
  migrations + `schema.rs`. The auth migration `0002_auth` ([[WEIR-T-0083]]) is the multi-migration pattern to follow.
- Current PKs: `connections(name)`, `connectors(name,version)`, `work_units(id)` with `connection` (name) FK.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] A `0003_tenancy` migration (pg + sqlite): add `tenant_id TEXT NOT NULL` to `connections`, `work_units`,
  `connectors`, `run_logs`, `dead_letters`, `stream_state`, `outbox`, `schedules`.
- [ ] `connections` PK becomes `(tenant_id, name)`; `work_units` carries `(tenant_id, connection)`; other
  scoped tables carry `tenant_id`. Indexes updated (esp. the `claim()` hot path: `(tenant_id, state, next_attempt_at)`).
- [ ] The `tenants` table ([[WEIR-T-0083]]) gains a store: create/list/delete; a `default` tenant row is
  ensured at `init`/migrate.
- [ ] Backfill: the migration sets `tenant_id = 'default'` on all existing rows; the `default` tenant exists.
- [ ] `schema.rs` regenerated; `weir-schema`/`weir-app` models updated to include `tenant_id`; workspace builds;
  the migration is idempotent + applies on a fresh **and** a populated DB (both backends).

## Implementation Notes

- Keep the `default` tenant id a stable constant (e.g. `"default"`) referenced from one place.
- The composite PK ripples into `weir-app`/`weir-orchestrator` models + queries — those adjustments land in
  [[WEIR-T-0090]]/[[WEIR-T-0091]]; here, get the schema + models compiling with `tenant_id` present.
- Migration ordering + `__weir_schema_version` tracking already exist ([[WEIR-T-0083]]).

## Status Updates

### 2026-07-06 — implementation approach (diesel-dualdb capability confirmed)

Investigated the schema-gen translator (`~/Desktop/diesel-dualdb/diesel-dualdb-cli/src/lib.rs`):
- Parses SQL via `sqlparser`; **`ALTER` passes through**; handles `AddColumn`/`DropColumn`/`AddConstraint`.
- **Per-backend tagged blocks**: `-- dualdb:postgres` … `-- dualdb:end` / `-- dualdb:sqlite` … `-- dualdb:end`
  (default = shared). This is the mechanism for the PK recompose.

**Plan for `0003_tenancy/up.sql`:**
- **Shared** `ALTER TABLE <t> ADD COLUMN tenant_id TEXT NOT NULL DEFAULT 'default';` for surrogate/independent-PK
  tables (existing rows backfill via the DEFAULT).
- **PK recompose** (SQLite can't `ALTER` a PK) via tagged blocks — `connections` (`name`→`(tenant_id,name)`) and
  `connectors` (`(name,version)`→`(tenant_id,name,version)`): Postgres = `ADD COLUMN` + `DROP CONSTRAINT`/`ADD
  PRIMARY KEY`; SQLite = table-recreate (CREATE new, `INSERT SELECT 'default'`, `DROP`, `ALTER RENAME`).
- **Default tenant row**: ensure `tenants('default', …)` in **Rust** (`migrate()`/init) to dodge the
  `ON CONFLICT` (pg) vs `INSERT OR IGNORE` (sqlite) split.
- **Index**: `(tenant_id, state, next_attempt_at)` on `work_units` (the `claim()` hot path, [[WEIR-T-0091]]).

Then `angreal schema gen` → register `0003_tenancy` in `MIGRATIONS` (lib.rs) → add `tenant_id` to models so the
workspace compiles (query rewrites land in [[WEIR-T-0090]]/[[WEIR-T-0091]]).

### 2026-07-06 — done (`c66b9b0`), **folded into baseline** per approved decision

diesel-dualdb can't model a PK recompose in a follow-on migration (tagged blocks are ignored by the schema
model; the model-builder only does `ADD COLUMN`/`ADD FK`). User approved **folding tenancy into the baseline
`0001_init`** (safe pre-release — no deployed DBs). So there's no `0003` + no runtime backfill; the schema *is*
tenant-scoped from creation and legacy rows are covered by `DEFAULT 'default'`.

- **`0001_init/up.sql`** rewritten: `tenant_id TEXT NOT NULL DEFAULT 'default'` on `dead_letters`,
  `stream_state`, `outbox`, `run_logs`, `connections`, `connectors`, `work_units`, `schedules`; composite PKs
  `connections=(tenant_id,name)`, `connectors=(tenant_id,name,version)`, `stream_state=(tenant_id,connection,stream)`;
  `idx_work_units_claim (tenant_id,state,next_attempt_at)`. `angreal schema gen` → `schema.rs` shows the composite
  PKs. Dual-backend migrate test **6/6**.
- **`weir-app/src/tenant.rs`**: `DEFAULT_TENANT` + `Tenant` + `create_tenant`/`list_tenants`/`delete_tenant`
  (default protected) + `ensure_default_tenant`, called in `App::open`. 2 unit tests.
- Existing queries default `tenant_id` → single-tenant behavior unchanged; **workspace builds, app suite 15/15,
  clippy clean**. Scoping the queries + `/tenants/*` routes = [[WEIR-T-0090]]; runner claim-by-tenant = [[WEIR-T-0091]].

**Complete.** (Title says "backfill" — moot under the baseline fold; kept for traceability.)
