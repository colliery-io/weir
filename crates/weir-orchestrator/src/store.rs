//! Work-unit persistence — the `work_units` row↔struct mapping in one place ([[WEIR-I-0030]]),
//! mirroring weir-app's connection store. `WorkSpecRow` is the spec subset the executor loads;
//! `RunFeedRow` is the live-feed subset. The claim/lease guarded `UPDATE` and the genuinely-partial
//! projections (the claim candidate's `(id, attempt)`, `active_tenants`) stay hand-written — they
//! are conditional writes / ad-hoc projections, not row mappings.

use diesel::prelude::*;
use diesel_dualdb::DualConnection;
use weir_schema::work_units;

use crate::{ExecutorError, RunRow, WorkSpec, from_json, json};

/// The `work_units` columns that make up a [`WorkSpec`] (a subset of the table). `Selectable`
/// selects exactly these by name; the JSON columns are decoded in [`into_work_spec`].
#[derive(Queryable, Selectable)]
#[diesel(table_name = work_units)]
pub(crate) struct WorkSpecRow {
    pub tenant_id: String,
    pub connection: String,
    pub stream: String,
    pub source_ref: String,
    pub dest_ref: String,
    pub source_config: String,
    pub dest_config: String,
    pub state_key: Option<String>,
    pub seed_cursor: Option<String>,
    pub partition: String,
    pub execution_mode: String,
}

impl WorkSpecRow {
    pub(crate) fn into_work_spec(self) -> Result<WorkSpec, ExecutorError> {
        Ok(WorkSpec {
            tenant: self.tenant_id,
            connection: self.connection,
            stream: from_json(&self.stream)?,
            source: from_json(&self.source_ref)?,
            dest: from_json(&self.dest_ref)?,
            source_config: from_json(&self.source_config)?,
            dest_config: from_json(&self.dest_config)?,
            state_key: self.state_key,
            seed_cursor: self.seed_cursor,
            partition: from_json(&self.partition)?,
            execution_mode: from_json(&self.execution_mode)?,
        })
    }
}

/// Insert a new `pending` unit for `spec` under `id`. The spec→column serialization (the JSON
/// columns) lives here, so the three `plan*` entry points share one mapping.
pub(crate) fn insert_pending(
    conn: &mut DualConnection,
    id: i64,
    spec: &WorkSpec,
) -> Result<(), ExecutorError> {
    diesel::insert_into(work_units::table)
        .values((
            work_units::id.eq(id),
            work_units::tenant_id.eq(&spec.tenant),
            work_units::connection.eq(&spec.connection),
            work_units::stream.eq(json(&spec.stream)?),
            work_units::source_ref.eq(json(&spec.source)?),
            work_units::dest_ref.eq(json(&spec.dest)?),
            work_units::source_config.eq(json(&spec.source_config)?),
            work_units::dest_config.eq(json(&spec.dest_config)?),
            work_units::state.eq("pending"),
            work_units::state_key.eq(spec.state_key.as_deref()),
            work_units::seed_cursor.eq(spec.seed_cursor.as_deref()),
            work_units::partition.eq(json(&spec.partition)?),
            work_units::execution_mode.eq(json(&spec.execution_mode)?),
        ))
        .execute(conn)?;
    Ok(())
}

/// Load the spec row for `id`, if present.
pub(crate) fn load_spec(
    conn: &mut DualConnection,
    id: i64,
) -> Result<Option<WorkSpecRow>, ExecutorError> {
    Ok(work_units::table
        .filter(work_units::id.eq(id))
        .select(WorkSpecRow::as_select())
        .first(conn)
        .optional()?)
}

/// A `work_units` row for the live feed (a subset of the table's columns); `duration_ms` is a
/// view over the two timestamps, computed in [`into_run_row`] rather than stored.
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

impl RunFeedRow {
    pub(crate) fn into_run_row(self) -> RunRow {
        RunRow {
            id: self.id,
            connection: self.connection,
            state: self.state,
            attempt: self.attempt,
            rows_written: self.rows_written,
            dead_lettered: self.dead_lettered,
            duration_ms: match (self.started_at, self.finished_at) {
                (Some(s), Some(f)) => Some((f - s).max(0)),
                _ => None,
            },
            error: self.error,
        }
    }
}

/// The most recent `limit` work units across all connections, newest first.
pub(crate) fn run_feed(
    conn: &mut DualConnection,
    limit: i64,
) -> Result<Vec<RunFeedRow>, ExecutorError> {
    Ok(work_units::table
        .order(work_units::id.desc())
        .limit(limit)
        .select(RunFeedRow::as_select())
        .load(conn)?)
}
