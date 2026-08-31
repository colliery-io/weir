//! Connection persistence — the `connections` row↔[`Connection`] mapping in **one place**
//! ([[WEIR-I-0030]]).
//!
//! Before this module the 12-column layout was transcribed as a positional tuple at six call
//! sites (a type alias, two `.select`s, an update, an insert, and a destructure), so adding a
//! column meant a dozen coordinated, order-dependent edits. Here it is a single [`ConnectionRow`]:
//! the `#[derive]`s carry load (`Queryable`/`Selectable`), insert (`Insertable`), and update
//! (`AsChangeset`, which skips the `(tenant_id, name)` key) — a new column is one field, checked
//! by the compiler.

use diesel::prelude::*;
use diesel_dualdb::DualConnection;
use weir_schema::{connections, work_units};

use crate::{AppError, Connection, json};

/// A `connections` row — the storage-facing twin of [`Connection`] (which carries no tenant; the
/// tenant is the caller's context). This struct is the single owner of the column set.
#[derive(Queryable, Selectable, Insertable, AsChangeset, Identifiable)]
#[diesel(table_name = connections, primary_key(tenant_id, name), treat_none_as_null = true)]
pub(crate) struct ConnectionRow {
    pub tenant_id: String,
    pub name: String,
    pub source_ref: String,
    pub dest_ref: String,
    pub stream: String,
    pub source_config: String,
    pub dest_config: String,
    pub every_secs: Option<f32>,
    pub cron: Option<String>,
    pub sync_mode: String,
    pub write_mode: String,
    pub business_keys: Option<String>,
    pub cursor_field: Option<String>,
    pub execution_mode: String,
}

impl ConnectionRow {
    /// Build a row from a **resolved** connection — the manifest has already been baked onto the
    /// `source`/`dest` refs + per-side configs by the caller ([[WEIR-I-0029]]).
    pub(crate) fn from_resolved(
        tenant: &str,
        c: &Connection,
        source_ref: String,
        source_config: String,
        dest_ref: String,
        dest_config: String,
    ) -> Result<Self, AppError> {
        Ok(Self {
            tenant_id: tenant.to_string(),
            name: c.name.clone(),
            source_ref,
            dest_ref,
            stream: c.stream.clone(),
            source_config,
            dest_config,
            // `every_secs` is REAL (f32) on disk; `Connection` keeps f64.
            every_secs: c.every_secs.map(|s| s as f32),
            cron: c.cron.clone(),
            sync_mode: c.sync_mode.clone(),
            write_mode: c.write_mode.clone(),
            // Business keys persist as a JSON array; empty → NULL so `append` rows stay clean.
            business_keys: if c.business_keys.is_empty() {
                None
            } else {
                Some(json(&c.business_keys)?)
            },
            cursor_field: c.cursor_field.clone(),
            execution_mode: c.execution_mode.clone(),
        })
    }

    /// Map a loaded row back to the domain [`Connection`] — the inverse of [`from_resolved`].
    pub(crate) fn into_connection(self) -> Result<Connection, AppError> {
        Ok(Connection {
            name: self.name,
            source: serde_json::from_str(&self.source_ref)
                .map_err(|e| AppError::Config(e.to_string()))?,
            dest: serde_json::from_str(&self.dest_ref)
                .map_err(|e| AppError::Config(e.to_string()))?,
            stream: self.stream,
            source_config: self.source_config,
            dest_config: self.dest_config,
            every_secs: self.every_secs.map(|f| f as f64),
            cron: self.cron,
            sync_mode: self.sync_mode,
            write_mode: self.write_mode,
            business_keys: self
                .business_keys
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default(),
            cursor_field: self.cursor_field,
            execution_mode: self.execution_mode,
        })
    }
}

/// Upsert a connection, keyed `(tenant_id, name)`. Portable update-then-insert — MultiBackend has
/// no `on_conflict`. `AsChangeset` writes every non-key column, so `treat_none_as_null` clears a
/// field that was previously set (e.g. dropping a `cron`).
pub(crate) fn upsert(conn: &mut DualConnection, row: &ConnectionRow) -> Result<(), AppError> {
    let updated = diesel::update(
        connections::table.filter(
            connections::tenant_id
                .eq(&row.tenant_id)
                .and(connections::name.eq(&row.name)),
        ),
    )
    .set(row)
    .execute(conn)?;
    if updated == 0 {
        diesel::insert_into(connections::table)
            .values(row)
            .execute(conn)?;
    }
    Ok(())
}

/// All connections under `tenant`, name-ordered.
pub(crate) fn list(
    conn: &mut DualConnection,
    tenant: &str,
) -> Result<Vec<ConnectionRow>, AppError> {
    Ok(connections::table
        .filter(connections::tenant_id.eq(tenant))
        .order(connections::name.asc())
        .select(ConnectionRow::as_select())
        .load(conn)?)
}

/// One connection by `(tenant, name)`, if present.
pub(crate) fn get(
    conn: &mut DualConnection,
    tenant: &str,
    name: &str,
) -> Result<Option<ConnectionRow>, AppError> {
    Ok(connections::table
        .filter(
            connections::tenant_id
                .eq(tenant)
                .and(connections::name.eq(name)),
        )
        .select(ConnectionRow::as_select())
        .first(conn)
        .optional()?)
}

/// A `work_units` row as the run feed reads it (a subset of the table's columns). Kept here beside
/// the connection row so the app has one storage module; the derived `duration_ms` view lives in
/// [`crate::App::recent_runs`].
#[derive(Queryable, Selectable)]
#[diesel(table_name = work_units)]
pub(crate) struct RunFeedRow {
    pub id: i64,
    pub connection: String,
    pub state: String,
    pub attempt: i64,
    pub rows_written: i64,
    pub dead_lettered: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub error: Option<String>,
}

/// The most recent `limit` work units for `tenant`, newest first.
pub(crate) fn run_feed(
    conn: &mut DualConnection,
    tenant: &str,
    limit: i64,
) -> Result<Vec<RunFeedRow>, AppError> {
    // The recent-by-id feed (terminal + in-flight), capped.
    let recent: Vec<RunFeedRow> = work_units::table
        .filter(work_units::tenant_id.eq(tenant))
        .order(work_units::id.desc())
        .limit(limit)
        .select(RunFeedRow::as_select())
        .load(conn)?;
    // ALWAYS include active (pending/leased/running) units, even past the cap ([[WEIR-I-0035]]):
    // a long-lived **resident** run has an old id and gets crowded out of the recent window by
    // frequently-firing connections — which made the UI show a running resident as "stopped".
    // Active work must never be hidden by a recency limit.
    let active: Vec<RunFeedRow> = work_units::table
        .filter(
            work_units::tenant_id
                .eq(tenant)
                .and(work_units::state.eq_any(["pending", "leased", "running"])),
        )
        .order(work_units::id.desc())
        .select(RunFeedRow::as_select())
        .load(conn)?;
    // Union (active ∪ recent), dedup by id, newest first.
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<RunFeedRow> = Vec::with_capacity(recent.len() + active.len());
    for r in active.into_iter().chain(recent) {
        if seen.insert(r.id) {
            out.push(r);
        }
    }
    out.sort_by_key(|b| std::cmp::Reverse(b.id));
    Ok(out)
}

/// One strict history page ([[WEIR-T-0189]]): units with `id < before`, newest
/// first, capped. Ids are monotonic (timestamp-ordered), so walking `before =
/// smallest id of the previous page` visits every unit exactly once — no dupes,
/// no gaps. Unlike the first-page live feed, no active-union: this is history.
pub(crate) fn run_feed_before(
    conn: &mut DualConnection,
    tenant: &str,
    limit: i64,
    before: i64,
) -> Result<Vec<RunFeedRow>, AppError> {
    Ok(work_units::table
        .filter(
            work_units::tenant_id
                .eq(tenant)
                .and(work_units::id.lt(before)),
        )
        .order(work_units::id.desc())
        .limit(limit)
        .select(RunFeedRow::as_select())
        .load(conn)?)
}

/// One unit by `(tenant, id)` — the single-run fetch ([[WEIR-T-0189]]). A row
/// belonging to another tenant is simply not found (no cross-tenant leak).
pub(crate) fn run_by_id(
    conn: &mut DualConnection,
    tenant: &str,
    id: i64,
) -> Result<Option<RunDetailRow>, AppError> {
    Ok(work_units::table
        .filter(work_units::tenant_id.eq(tenant).and(work_units::id.eq(id)))
        .select(RunDetailRow::as_select())
        .first(conn)
        .optional()?)
}

/// The single-run row ([[WEIR-T-0189]]) — the feed columns plus `stream`.
#[derive(Queryable, Selectable)]
#[diesel(table_name = work_units)]
pub(crate) struct RunDetailRow {
    pub id: i64,
    pub connection: String,
    pub stream: String,
    pub state: String,
    pub attempt: i64,
    pub rows_written: i64,
    pub dead_lettered: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub error: Option<String>,
}
