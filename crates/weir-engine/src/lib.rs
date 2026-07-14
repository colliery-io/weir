//! # weir-engine
//!
//! A minimal **Sync Engine** slice ([[WEIR-S-0004]]): it drives one connection's
//! `source → (map) → destination` round-trip via the [`weir_runtime`] seam and
//! commits each chunk's **checkpoint + outbox record in one transaction**
//! ([[WEIR-A-0011]] at-least-once + [[WEIR-A-0010]] transactional outbox) into a
//! [`diesel-dualdb`] store ([[WEIR-A-0009]]). Single in-process agent, no broker.
//!
//! Passthrough mapping ([[WEIR-A-0026]] stub), one partition. The store is portable
//! across Postgres + SQLite via diesel-dualdb's typed query builder over the
//! generated `weir-schema` ([[WEIR-I-0013]]).

use diesel::prelude::*;
use diesel_dualdb::Pool;
use diesel_dualdb::types::{Bytes, Uuid as DbUuid};
use weir_schema::{dead_letters, outbox, run_logs, stream_schemas, stream_state};

use futures::StreamExt;
use futures::channel::oneshot;
use futures::future::{Either, select};
use weir_connector::{
    ChangeRecord, ConfiguredStream, ConnectorError, DeadLetter, LogLevel, Partition, ReadContext,
    ReadMessage, RecordBatch, StreamState, WriteContext, WriteResult,
};
use weir_runtime::{ConnectorHandle, RuntimeError};

mod mapping;

/// Apply a [`MappingSpec`] to one `RecordBatch` between read and write
/// ([[WEIR-T-0052]]). `Rows` are mapped per-record (kept / filtered-out / dead-
/// lettered on a failed cast). `Arrow` batches pass through (Arrow-native
/// projection/filter is a follow-on). Empty spec = fast-path passthrough.
/// Apply mapping, then (if a schema is stored) enforce it — one record, in order ([[WEIR-T-0119]]).
fn process_record(
    spec: &weir_connector::MappingSpec,
    schema: Option<&weir_connector::StreamSchema>,
    v: serde_json::Value,
) -> mapping::Mapped {
    let mapped = if spec.ops.is_empty() {
        mapping::Mapped::Keep(v)
    } else {
        mapping::apply(spec, v)
    };
    match (mapped, schema) {
        (mapping::Mapped::Keep(mv), Some(s)) => mapping::enforce(s, mv),
        (other, _) => other,
    }
}

fn map_batch(
    spec: &weir_connector::MappingSpec,
    schema: Option<&weir_connector::StreamSchema>,
    batch: RecordBatch,
    pending_dead: &mut Vec<DeadLetter>,
) -> RecordBatch {
    // Nothing to do unless there's a mapping or a schema to enforce.
    let active = !spec.ops.is_empty() || schema.is_some();
    match batch {
        RecordBatch::Rows(rows) if active => {
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                match serde_json::from_str::<serde_json::Value>(&row) {
                    Ok(v) => match process_record(spec, schema, v) {
                        mapping::Mapped::Keep(mv) => out.push(mv.to_string()),
                        mapping::Mapped::Filtered => {}
                        mapping::Mapped::DeadLetter(reason) => {
                            pending_dead.push(DeadLetter {
                                record: row,
                                reason,
                            });
                        }
                    },
                    // Non-JSON record: pass through unmapped.
                    Err(_) => out.push(row),
                }
            }
            RecordBatch::Rows(out)
        }
        // CDC changes ([[WEIR-I-0026]]): map + enforce each change's `data` (so renames/casts/coercion
        // hit the row + key), preserving the op. Filter/dead-letter behave as for `Rows`.
        RecordBatch::Changes(changes) if active => {
            let mut out = Vec::with_capacity(changes.len());
            for ch in changes {
                match serde_json::from_str::<serde_json::Value>(&ch.data) {
                    Ok(v) => match process_record(spec, schema, v) {
                        mapping::Mapped::Keep(mv) => {
                            out.push(ChangeRecord {
                                op: ch.op,
                                data: mv.to_string(),
                            });
                        }
                        mapping::Mapped::Filtered => {}
                        mapping::Mapped::DeadLetter(reason) => {
                            pending_dead.push(DeadLetter {
                                record: ch.data,
                                reason,
                            });
                        }
                    },
                    Err(_) => out.push(ch),
                }
            }
            RecordBatch::Changes(out)
        }
        other => other,
    }
}

/// Records sampled for schema inference ([[WEIR-T-0118]]).
const SCHEMA_SAMPLE: usize = 200;

/// Accumulate up to `cap` record-JSON strings from a batch into `sample` (Arrow contributes none).
fn collect_records(batch: &RecordBatch, sample: &mut Vec<String>, cap: usize) {
    let room = cap.saturating_sub(sample.len());
    match batch {
        RecordBatch::Rows(rows) => sample.extend(rows.iter().take(room).cloned()),
        RecordBatch::Changes(cs) => sample.extend(cs.iter().take(room).map(|c| c.data.clone())),
        RecordBatch::Arrow(_) => {}
    }
}

#[cfg(test)]
mod change_mapping_tests {
    use super::*;
    use weir_connector::{ChangeOp, MappingOp, MappingSpec};

    #[test]
    fn mapping_transforms_a_changes_batch_preserving_op() {
        // Rename `a`→`id` over a Changes batch: data is transformed, the op is preserved.
        let spec = MappingSpec {
            ops: vec![MappingOp::Rename {
                from: "a".to_string(),
                to: "id".to_string(),
            }],
        };
        let batch = RecordBatch::Changes(vec![
            ChangeRecord {
                op: ChangeOp::Insert,
                data: r#"{"a":1}"#.to_string(),
            },
            ChangeRecord {
                op: ChangeOp::Delete,
                data: r#"{"a":2}"#.to_string(),
            },
        ]);
        let mut dead = Vec::new();
        match map_batch(&spec, None, batch, &mut dead) {
            RecordBatch::Changes(cs) => {
                assert_eq!(cs.len(), 2);
                assert_eq!(cs[0].op, ChangeOp::Insert);
                assert_eq!(cs[0].data, r#"{"id":1}"#);
                assert_eq!(cs[1].op, ChangeOp::Delete); // delete's key still maps
                assert_eq!(cs[1].data, r#"{"id":2}"#);
            }
            _ => panic!("expected a Changes batch back"),
        }
        assert!(dead.is_empty());
    }
}

/// Errors from the engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("store pool error: {0}")]
    Pool(String),
    #[error("db error: {0}")]
    Db(#[from] diesel::result::Error),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error("async runtime: {0}")]
    Rt(String),
    /// A connector ran and reported a domain failure (its `kind` drives retry).
    #[error("connector error: {0}")]
    Connector(ConnectorError),
    /// A **resident** source's `read` stream returned end-of-stream. For a
    /// long-lived source that is abnormal — the worker ([[WEIR-I-0035]] F1.3)
    /// treats it as transient and requeues, resuming from the last committed
    /// checkpoint ([[WEIR-A-0011]]).
    #[error("resident source stream ended unexpectedly")]
    ResidentStreamEnded,
}

/// Retry a DB write briefly on SQLite "database is locked" — write contention when
/// concurrent executors ([[WEIR-T-0051]]) commit checkpoints at once. The whole
/// transaction rolls back on a lock, so re-running it is safe. Runs inside the
/// engine's `spawn_blocking`, so `thread::sleep` is fine. (diesel-dualdb's pool
/// can't set a per-connection `busy_timeout`, so we retry — cf. cloacina's SQLite
/// claim retry.)
fn retry_locked_db<T>(
    mut f: impl FnMut() -> Result<T, diesel::result::Error>,
) -> Result<T, diesel::result::Error> {
    let start = std::time::Instant::now();
    let mut delay_ms = 5u64;
    loop {
        match f() {
            Err(e)
                if e.to_string().contains("locked")
                    && start.elapsed() < std::time::Duration::from_secs(30) =>
            {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                delay_ms = (delay_ms * 2).min(200);
            }
            other => return other,
        }
    }
}

/// The durable store (config/state/outbox), backed by `diesel-dualdb`.
pub struct Store {
    pool: Pool,
}

impl Store {
    /// Open a store at `url` (`":memory:"` or a path for SQLite; a `postgres://`
    /// URL for Postgres) and ensure the schema exists.
    pub fn open(url: &str) -> Result<Self, EngineError> {
        let pool = Pool::connect(url).map_err(|e| EngineError::Pool(e.to_string()))?;
        let store = Self { pool };
        store.migrate()?;
        // Single-node SQLite under a scheduled fleet contends hard in the default rollback-journal
        // mode (any reader blocks the writer). WAL — persistent + file-level, so every pooled
        // connection inherits it — lets readers run alongside one writer; NORMAL sync stays durable
        // under WAL. Postgres ignores this. ([[WEIR-I-0023]] soak finding.)
        if !url.starts_with("postgres")
            && let Ok(mut conn) = store.pool.get()
        {
            let _ = diesel::sql_query("PRAGMA journal_mode=WAL").execute(&mut conn);
            let _ = diesel::sql_query("PRAGMA synchronous=NORMAL").execute(&mut conn);
        }
        Ok(store)
    }

    fn migrate(&self) -> Result<(), EngineError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| EngineError::Pool(e.to_string()))?;
        // The generated, per-backend migration is applied via the shared runner,
        // which dispatches the right DDL by connection arm ([[WEIR-I-0013]]).
        weir_schema::migrate(&mut conn)?;
        Ok(())
    }

    /// The underlying connection pool. Lets layers above the engine (e.g. the
    /// orchestrator's work-unit queue) run their own SQL against the shared
    /// store without the engine knowing their schema.
    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    /// Count outbox rows for a connection (test/inspection helper).
    pub fn outbox_count(&self, connection: &str) -> Result<i64, EngineError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| EngineError::Pool(e.to_string()))?;
        Ok(outbox::table
            .filter(
                outbox::connection
                    .eq(connection)
                    .and(outbox::processed.eq(1)),
            )
            .count()
            .get_result(&mut conn)?)
    }

    /// The committed cursor for a `(connection, state_key)` (test/inspection).
    pub fn cursor(&self, connection: &str, state_key: &str) -> Result<Option<String>, EngineError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| EngineError::Pool(e.to_string()))?;
        let row: Option<(Option<String>, Bytes)> = stream_state::table
            .filter(
                stream_state::connection
                    .eq(connection)
                    .and(stream_state::stream.eq(state_key)),
            )
            .select((stream_state::cursor, stream_state::opaque))
            .first(&mut conn)
            .optional()?;
        Ok(row.and_then(|(cursor, _)| cursor))
    }

    /// Capture the stream's typed schema from a record `sample` if none is stored yet
    /// ([[WEIR-T-0118]]) — inference fallback (the connector-declared path preloads it). Keyed by the
    /// **logical stream** (shared across partitions); a concurrent partition losing the insert is fine.
    pub fn capture_schema(
        &self,
        connection: &str,
        stream: &str,
        sample: &[String],
    ) -> Result<(), EngineError> {
        if sample.is_empty() {
            return Ok(());
        }
        let mut conn = self
            .pool
            .get()
            .map_err(|e| EngineError::Pool(e.to_string()))?;
        let existing: Option<String> = stream_schemas::table
            .filter(
                stream_schemas::connection
                    .eq(connection)
                    .and(stream_schemas::stream.eq(stream)),
            )
            .select(stream_schemas::schema)
            .first(&mut conn)
            .optional()?;
        if existing.is_none() {
            let schema = weir_connector::StreamSchema::infer(sample);
            let json = serde_json::to_string(&schema).unwrap_or_default();
            // Best-effort: a concurrent partition may insert first (PK conflict) — that's fine.
            let _ = diesel::insert_into(stream_schemas::table)
                .values((
                    stream_schemas::connection.eq(connection),
                    stream_schemas::stream.eq(stream),
                    stream_schemas::schema.eq(&json),
                ))
                .execute(&mut conn);
        }
        Ok(())
    }

    /// The stored typed schema for a stream ([[WEIR-T-0119]]), if any — read before a sync to enforce.
    pub fn stream_schema(
        &self,
        connection: &str,
        stream: &str,
    ) -> Result<Option<weir_connector::StreamSchema>, EngineError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| EngineError::Pool(e.to_string()))?;
        let row: Option<String> = stream_schemas::table
            .filter(
                stream_schemas::connection
                    .eq(connection)
                    .and(stream_schemas::stream.eq(stream)),
            )
            .select(stream_schemas::schema)
            .first(&mut conn)
            .optional()?;
        Ok(row.and_then(|j| serde_json::from_str(&j).ok()))
    }

    /// Evolve the stored schema against the run's freshly-inferred shape ([[WEIR-T-0120]]): no stored
    /// schema → capture it; **additive** drift → merge the new fields + clear any breaking flag;
    /// **breaking** drift (a type change) → set the `broken` flag with the reasons (surfaced, never
    /// silently applied); stable → clear a stale flag. Best-effort — a concurrent partition may race.
    pub fn evolve_schema(
        &self,
        connection: &str,
        stream: &str,
        sample: &[String],
    ) -> Result<(), EngineError> {
        if sample.is_empty() {
            return Ok(());
        }
        let mut conn = self
            .pool
            .get()
            .map_err(|e| EngineError::Pool(e.to_string()))?;
        let current = weir_connector::StreamSchema::infer(sample);
        let stored_json: Option<String> = stream_schemas::table
            .filter(
                stream_schemas::connection
                    .eq(connection)
                    .and(stream_schemas::stream.eq(stream)),
            )
            .select(stream_schemas::schema)
            .first(&mut conn)
            .optional()?;
        let target = stream_schemas::table.filter(
            stream_schemas::connection
                .eq(connection)
                .and(stream_schemas::stream.eq(stream)),
        );
        match stored_json {
            None => {
                let json = serde_json::to_string(&current).unwrap_or_default();
                let _ = diesel::insert_into(stream_schemas::table)
                    .values((
                        stream_schemas::connection.eq(connection),
                        stream_schemas::stream.eq(stream),
                        stream_schemas::schema.eq(&json),
                    ))
                    .execute(&mut conn);
            }
            Some(j) => {
                let stored: weir_connector::StreamSchema =
                    serde_json::from_str(&j).unwrap_or_default();
                let d = stored.diff(&current);
                if !d.breaking.is_empty() {
                    let reason = d.breaking.join("; ");
                    let _ = diesel::update(target)
                        .set(stream_schemas::broken.eq(Some(reason)))
                        .execute(&mut conn);
                } else if !d.additive.is_empty() {
                    let json =
                        serde_json::to_string(&stored.merged(&d.additive)).unwrap_or_default();
                    let _ = diesel::update(target)
                        .set((
                            stream_schemas::schema.eq(&json),
                            stream_schemas::broken.eq(None::<String>),
                        ))
                        .execute(&mut conn);
                } else {
                    let _ = diesel::update(target)
                        .set(stream_schemas::broken.eq(None::<String>))
                        .execute(&mut conn);
                }
            }
        }
        Ok(())
    }

    /// The breaking-drift reason for a stream, if flagged ([[WEIR-T-0120]]).
    pub fn schema_broken(
        &self,
        connection: &str,
        stream: &str,
    ) -> Result<Option<String>, EngineError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| EngineError::Pool(e.to_string()))?;
        let row: Option<Option<String>> = stream_schemas::table
            .filter(
                stream_schemas::connection
                    .eq(connection)
                    .and(stream_schemas::stream.eq(stream)),
            )
            .select(stream_schemas::broken)
            .first(&mut conn)
            .optional()?;
        Ok(row.flatten())
    }

    /// Count dead-lettered records for a connection (test/inspection helper).
    pub fn dead_letter_count(&self, connection: &str) -> Result<i64, EngineError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| EngineError::Pool(e.to_string()))?;
        Ok(dead_letters::table
            .filter(dead_letters::connection.eq(connection))
            .count()
            .get_result(&mut conn)?)
    }

    /// The most-recent dead-lettered records for a connection (newest first) —
    /// the *what + why* behind the dead-letter count ([[WEIR-T-0035]]).
    pub fn dead_letters(
        &self,
        connection: &str,
        limit: i64,
    ) -> Result<Vec<DeadLetterRecord>, EngineError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| EngineError::Pool(e.to_string()))?;
        let rows: Vec<(String, String, String)> = dead_letters::table
            .filter(dead_letters::connection.eq(connection))
            .order(dead_letters::ts.desc())
            .limit(limit)
            .select((
                dead_letters::stream,
                dead_letters::record,
                dead_letters::reason,
            ))
            .load(&mut conn)?;
        Ok(rows
            .into_iter()
            .map(|(stream, record, reason)| DeadLetterRecord {
                stream,
                record,
                reason,
            })
            .collect())
    }

    /// The most-recent run logs for a connection (newest first) — connector logs +
    /// write diagnostics captured during syncs ([[WEIR-T-0036]]).
    pub fn logs(&self, connection: &str, limit: i64) -> Result<Vec<LogRecord>, EngineError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| EngineError::Pool(e.to_string()))?;
        let rows: Vec<(String, String, String, i64)> = run_logs::table
            .filter(run_logs::connection.eq(connection))
            .order(run_logs::ts.desc())
            .limit(limit)
            .select((
                run_logs::stream,
                run_logs::level,
                run_logs::message,
                run_logs::ts,
            ))
            .load(&mut conn)?;
        Ok(rows
            .into_iter()
            .map(|(stream, level, message, ts)| LogRecord {
                stream,
                level,
                message,
                ts,
            })
            .collect())
    }
}

/// A captured run log line (connector log or write diagnostic), for the API.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogRecord {
    pub stream: String,
    pub level: String,
    pub message: String,
    pub ts: i64,
}

/// Epoch-millis now (best-effort; 0 if the clock is before the epoch).
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Stable lowercase name for a log level (the `run_logs.level` column).
fn level_str(level: &LogLevel) -> &'static str {
    match level {
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    }
}

/// A dead-lettered record (rejected during a sync), for inspection / the API.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeadLetterRecord {
    pub stream: String,
    pub record: String,
    pub reason: String,
}

/// A fresh client-generated surrogate key (the generator has no SERIAL; keys are
/// UUIDs, [[WEIR-I-0013]]).
fn new_id() -> DbUuid {
    DbUuid(uuid::Uuid::new_v4())
}

/// Outcome of one [`Engine::sync`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutcome {
    /// Number of chunks pulled + committed.
    pub chunks: u64,
    /// Total rows the destination accepted (summed `WriteReceipt::accepted`).
    pub rows_written: u64,
    /// Total records dead-lettered (record-level rejections, run still succeeds).
    pub dead_lettered: u64,
    /// The final committed stream state (resume point for next run).
    pub final_state: StreamState,
}

/// Cumulative progress reported on each committed checkpoint during a sync — lets
/// the orchestrator update the in-flight work unit so the live feed shows a run's
/// rows/chunks climb as it runs ([[WEIR-T-0037]]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncProgress {
    pub rows_written: u64,
    pub dead_lettered: u64,
    pub chunks: u64,
}

/// Options for [`Engine::sync_with`]. Defaults reproduce plain [`Engine::sync`].
#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    /// Checkpoint key override (defaults to the stream name). A backfill uses a
    /// distinct key so its checkpoints never touch the live incremental cursor;
    /// each partition of a partitioned read also gets its own key.
    pub state_key: Option<String>,
    /// Initial cursor when no state exists under `state_key` (backfill lower bound).
    pub seed_cursor: Option<String>,
    /// Which partition to read ([[WEIR-T-0027]]); `None` = the whole stream (`p0`).
    pub partition: Option<Partition>,
}

/// The minimal sync engine.
/// Cooperative stop for a resident run ([`Engine::run_resident`], [[WEIR-I-0035]] F1).
/// The worker (F1.3) holds the [`StopHandle`]; calling [`StopHandle::stop`] — or simply
/// dropping it — resolves the paired [`StopToken`], so the resident loop shuts down at
/// its next poll boundary (drop-to-cancel, [[WEIR-A-0029]]). Keeps `futures` internal to
/// the engine so callers need not depend on it.
pub struct StopHandle {
    tx: oneshot::Sender<()>,
}

impl StopHandle {
    /// Signal the resident loop to stop. (Dropping the handle does the same.)
    pub fn stop(self) {
        // Best-effort: if the loop already exited, the receiver is gone.
        let _ = self.tx.send(());
    }
}

/// The engine-side half of a resident stop signal; see [`StopHandle`].
pub struct StopToken {
    rx: oneshot::Receiver<()>,
}

/// Create a linked ([`StopHandle`], [`StopToken`]) pair for a resident run.
pub fn stop_channel() -> (StopHandle, StopToken) {
    let (tx, rx) = oneshot::channel();
    (StopHandle { tx }, StopToken { rx })
}

pub struct Engine<'a> {
    store: &'a Store,
}

impl<'a> Engine<'a> {
    pub fn new(store: &'a Store) -> Self {
        Self { store }
    }

    /// Sync one `stream` of `connection` from `source` to `dest`: the chunked
    /// pull loop, committing checkpoint + outbox atomically per chunk.
    pub fn sync(
        &self,
        connection: &str,
        stream: &ConfiguredStream,
        source: &ConnectorHandle,
        dest: &ConnectorHandle,
    ) -> Result<SyncOutcome, EngineError> {
        self.sync_with(
            connection,
            stream,
            source,
            dest,
            &SyncOptions::default(),
            &mut |_| {},
        )
    }

    /// Like [`sync`](Self::sync) but with an explicit checkpoint **state key**
    /// (defaults to the stream name) and an optional **seed cursor**. A backfill
    /// ([[WEIR-T-0010]]) passes a distinct `state_key` so it reads from
    /// `seed_cursor` and checkpoints under its own key — never touching the live
    /// incremental cursor. The connector still sees the real `stream`.
    pub fn sync_with(
        &self,
        connection: &str,
        stream: &ConfiguredStream,
        source: &ConnectorHandle,
        dest: &ConnectorHandle,
        opts: &SyncOptions,
        on_progress: &mut dyn FnMut(SyncProgress),
    ) -> Result<SyncOutcome, EngineError> {
        self.drive(
            connection,
            stream,
            source,
            dest,
            opts,
            on_progress,
            None,
            None,
        )
    }

    /// Resident execution ([[WEIR-I-0035]] F1): hold the configured `source`
    /// instance open and drain its `read` stream **indefinitely**, committing on
    /// each source-driven `Checkpoint`, until `stop` fires (clean shutdown, `Ok`)
    /// or the stream errors / ends (`Err`, so the worker requeues and resumes from
    /// the last committed checkpoint). Identical transactional checkpoint/outbox
    /// semantics to [`sync_with`](Self::sync_with); the only difference is the loop
    /// does not terminate on stream drain and honors `stop`.
    pub fn run_resident(
        &self,
        connection: &str,
        stream: &ConfiguredStream,
        source: &ConnectorHandle,
        dest: &ConnectorHandle,
        opts: &SyncOptions,
        on_progress: &mut dyn FnMut(SyncProgress),
        stop: StopToken,
        cadence: Option<std::time::Duration>,
    ) -> Result<SyncOutcome, EngineError> {
        self.drive(
            connection,
            stream,
            source,
            dest,
            opts,
            on_progress,
            Some(stop),
            cadence,
        )
    }

    /// The shared source→map→destination drive loop for both run-once
    /// (`stop = None`) and resident (`stop = Some`) execution. Run-once returns
    /// when the stream drains; resident loops until `stop` fires (clean `Ok`) or
    /// the stream ends/errors (`Err`). Checkpoint + outbox + dead-letters commit in
    /// one transaction per source `Checkpoint` ([[WEIR-A-0011]]) in both modes.
    fn drive(
        &self,
        connection: &str,
        stream: &ConfiguredStream,
        source: &ConnectorHandle,
        dest: &ConnectorHandle,
        opts: &SyncOptions,
        on_progress: &mut dyn FnMut(SyncProgress),
        mut stop: Option<StopToken>,
        cadence: Option<std::time::Duration>,
    ) -> Result<SyncOutcome, EngineError> {
        let state_key = opts
            .state_key
            .clone()
            .unwrap_or_else(|| stream.stream.clone());
        // NB: a resident run is perpetual, so it must NOT hold a pooled connection across
        // the loop (the `read`/cadence waits) — else N residents exhaust the pool and
        // everything (starts, heartbeats, other runs) times out ([[WEIR-I-0035]] F1.6 fix).
        // Acquire a connection only where the DB is touched (initial state read, Log insert,
        // per-Checkpoint commit) and drop it between; run-once is unaffected.
        let mut state = {
            let mut conn = self
                .store
                .pool
                .get()
                .map_err(|e| EngineError::Pool(e.to_string()))?;
            let row: Option<(Option<String>, Bytes)> = stream_state::table
                .filter(
                    stream_state::connection
                        .eq(connection)
                        .and(stream_state::stream.eq(&state_key)),
                )
                .select((stream_state::cursor, stream_state::opaque))
                .first(&mut conn)
                .optional()?;
            row.map(|(cursor, opaque)| StreamState {
                cursor,
                opaque: opaque.0,
            })
            .unwrap_or_else(|| StreamState {
                cursor: opts.seed_cursor.clone(),
                opaque: Vec::new(),
            })
        };
        let mut chunks: u64 = 0;
        let mut rows_written: u64 = 0;
        let mut dead_lettered: u64 = 0;

        // Drive the connector's async streaming read with `futures::executor::block_on`
        // (NOT a tokio runtime); DB commits stay synchronous. Records are buffered and
        // written as a client-stream per `Checkpoint`, where `stream_state` + outbox +
        // pending dead-letters commit atomically (the connector owns checkpoint
        // granularity, [[WEIR-A-0029]]).
        //
        // Why `futures::executor` and not tokio: a WASM source's `wasi:http` is served
        // by wasmtime-wasi, which does its own `block_on` — that panics ("runtime within
        // a runtime") if this thread has an *entered* tokio runtime. Driving the stream
        // off-tokio leaves wasmtime-wasi free to enter its own. The dylib + non-http
        // wasm paths drive identically.
        let ctx = ReadContext {
            stream: stream.clone(),
            partition: opts.partition.clone().unwrap_or_else(|| Partition {
                id: "p0".to_string(),
                bounds: None,
            }),
            state: state.clone(),
        };
        let read_stream = futures::executor::block_on(source.read(&ctx))?;
        futures::pin_mut!(read_stream);
        tracing::debug!(target: "weir_engine", connection, resident = stop.is_some(), "engine run: draining read stream");

        let mut pending_dead: Vec<DeadLetter> = Vec::new();
        // Batches accumulated since the last checkpoint; written to the destination
        // as one client-stream per checkpoint segment (the dest pulls + applies
        // them at its own pace, [[WEIR-A-0029]]), keeping per-checkpoint atomicity.
        let mut batch_buffer: Vec<RecordBatch> = Vec::new();
        // Sample for schema capture ([[WEIR-T-0118]]) — the **post-mapping** shape (what the dest
        // receives), so a later run enforces the same shape.
        let mut schema_sample: Vec<String> = Vec::new();
        // The stored schema to enforce ([[WEIR-T-0119]]); None on the first run (captured at the end).
        let enforce_schema = self
            .store
            .stream_schema(connection, &stream.stream)
            .unwrap_or(None);
        let no_map = weir_connector::MappingSpec::default();
        loop {
            // Run-once drains the stream; resident races each poll against the stop
            // signal so a stop (send OR sender-drop = drop-to-cancel) shuts the loop
            // down promptly at a poll boundary ([[WEIR-A-0029]]).
            let item = match stop.as_mut() {
                None => futures::executor::block_on(read_stream.next()),
                Some(tok) => {
                    // Poll the stop token FIRST: `futures::select` is biased to its first
                    // argument, and a resident stream can be *always ready* (an unbounded
                    // hot source with a row waiting every poll), which would starve the stop
                    // signal and hang the run on drop-to-cancel. Stop-first makes a fired
                    // stop win over an always-ready stream ([[WEIR-I-0035]] F1.6 fix).
                    match futures::executor::block_on(select(&mut tok.rx, read_stream.next())) {
                        Either::Left((_, _)) => break, // stop signaled → clean shutdown
                        Either::Right((it, _)) => it,
                    }
                }
            };
            let Some(item) = item else {
                // Stream drained. Run-once: normal completion. Resident: abnormal —
                // return transient so the worker requeues from the last checkpoint.
                if stop.is_some() {
                    tracing::warn!(target: "weir_engine", connection, "resident stream ended unexpectedly → transient (worker requeues, resumes from last checkpoint)");
                    return Err(EngineError::ResidentStreamEnded);
                }
                break;
            };
            match item? {
                // Apply the in-flight mapping (WEIR-A-0026 / WEIR-T-0052) — select/drop/rename/cast/
                // filter/compute — then enforce the typed schema ([[WEIR-T-0119]]), then buffer for the
                // segment write. Filtered rows are dropped; failed casts + schema violations dead-letter.
                ReadMessage::Records(batch) => {
                    // Map first (no enforce) and sample the incoming post-mapping shape, so evolution
                    // ([[WEIR-T-0120]]) still sees a drift even when enforcement then rejects it.
                    let mapped = map_batch(&stream.mapping, None, batch, &mut pending_dead);
                    collect_records(&mapped, &mut schema_sample, SCHEMA_SAMPLE);
                    let enforced =
                        map_batch(&no_map, enforce_schema.as_ref(), mapped, &mut pending_dead);
                    batch_buffer.push(enforced);
                }
                ReadMessage::DeadLettered(dl) => pending_dead.push(dl),
                // Persist connector logs immediately (not buffered to checkpoint) so
                // they're visible mid-run and survive a `Fatal`. Log-write failures
                // never fail the run.
                ReadMessage::Log(entry) => {
                    if let Ok(mut conn) = self.store.pool.get() {
                        let _ = diesel::insert_into(run_logs::table)
                            .values((
                                run_logs::id.eq(new_id()),
                                run_logs::connection.eq(connection),
                                run_logs::stream.eq(&state_key),
                                run_logs::level.eq(level_str(&entry.level)),
                                run_logs::message.eq(&entry.message),
                                run_logs::ts.eq(now_ms()),
                            ))
                            .execute(&mut conn);
                    }
                }
                ReadMessage::Fatal(e) => {
                    tracing::error!(target: "weir_engine", connection, error = %e.message, "connector reported fatal error");
                    return Err(EngineError::Connector(e));
                }
                ReadMessage::Checkpoint(next) => {
                    tracing::debug!(target: "weir_engine", connection, cursor = ?next.cursor, batches = batch_buffer.len(), "checkpoint commit");
                    // Acquire a pooled connection ONLY for this commit and release it at the
                    // end of the arm — a resident run must not hold one across read()/cadence
                    // waits ([[WEIR-I-0035]] F1.6 fix).
                    let mut conn = self
                        .store
                        .pool
                        .get()
                        .map_err(|e| EngineError::Pool(e.to_string()))?;
                    // Client-streaming write of the segment's batches, then commit.
                    if !batch_buffer.is_empty() {
                        let wctx = WriteContext {
                            stream: stream.clone(),
                        };
                        let wout = dest
                            .write(&wctx, std::mem::take(&mut batch_buffer))
                            .map_err(|e| EngineError::Rt(e.to_string()))?;
                        for d in &wout.diagnostics {
                            let _ = diesel::insert_into(run_logs::table)
                                .values((
                                    run_logs::id.eq(new_id()),
                                    run_logs::connection.eq(connection),
                                    run_logs::stream.eq(&state_key),
                                    run_logs::level.eq(level_str(&d.level)),
                                    run_logs::message.eq(&d.message),
                                    run_logs::ts.eq(now_ms()),
                                ))
                                .execute(&mut conn);
                        }
                        pending_dead.extend(wout.dead_letters);
                        match wout.result {
                            WriteResult::Ok(receipt) => rows_written += receipt.accepted,
                            WriteResult::Err(e) => return Err(EngineError::Connector(e)),
                        }
                    }
                    // Atomic checkpoint: advance stream_state, record the outbox
                    // entry, and flush dead-letters accumulated since the last
                    // checkpoint — all in ONE transaction.
                    let conn_s = connection.to_string();
                    let stream_s = state_key.clone();
                    let seq = chunks as i64;
                    let dead = std::mem::take(&mut pending_dead);
                    retry_locked_db(|| {
                        conn.transaction::<_, diesel::result::Error, _>(|conn| {
                            for dl in &dead {
                                diesel::insert_into(dead_letters::table)
                                    .values((
                                        dead_letters::id.eq(new_id()),
                                        dead_letters::connection.eq(&conn_s),
                                        dead_letters::stream.eq(&stream_s),
                                        dead_letters::record.eq(&dl.record),
                                        dead_letters::reason.eq(&dl.reason),
                                        dead_letters::ts.eq(now_ms()),
                                    ))
                                    .execute(conn)?;
                            }
                            // Portable upsert: MultiBackend doesn't carry `on_conflict`,
                            // so update-then-insert-if-absent. Safe — checkpoints for a
                            // (connection, stream) are written by a single claimed worker,
                            // so there's no concurrent upsert of the same key.
                            let updated = diesel::update(
                                stream_state::table.filter(
                                    stream_state::connection
                                        .eq(&conn_s)
                                        .and(stream_state::stream.eq(&stream_s)),
                                ),
                            )
                            .set((
                                stream_state::cursor.eq(next.cursor.as_deref()),
                                stream_state::opaque.eq(Bytes(next.opaque.clone())),
                            ))
                            .execute(conn)?;
                            if updated == 0 {
                                diesel::insert_into(stream_state::table)
                                    .values((
                                        stream_state::connection.eq(&conn_s),
                                        stream_state::stream.eq(&stream_s),
                                        stream_state::cursor.eq(next.cursor.as_deref()),
                                        stream_state::opaque.eq(Bytes(next.opaque.clone())),
                                    ))
                                    .execute(conn)?;
                            }
                            diesel::insert_into(outbox::table)
                                .values((
                                    outbox::id.eq(new_id()),
                                    outbox::connection.eq(&conn_s),
                                    outbox::stream.eq(&stream_s),
                                    outbox::seq.eq(seq),
                                    outbox::processed.eq(1),
                                ))
                                .execute(conn)?;
                            Ok(())
                        })
                    })?;
                    dead_lettered += dead.len() as u64;
                    state = next;
                    chunks += 1;
                    // Report cumulative progress so the orchestrator can update the
                    // in-flight work unit (live feed) per committed checkpoint.
                    on_progress(SyncProgress {
                        rows_written,
                        dead_lettered,
                        chunks,
                    });
                    // Resident polling cadence ([[WEIR-I-0035]] F1.9): the host sleeps the declared
                    // cadence BETWEEN polls — once per committed Checkpoint (the poll boundary), NOT
                    // per Records batch. A poll's bounded read-to-end may span several `Records`; it
                    // still waits exactly one cadence. Run-once + event-readers (tail/ws) pass `None`
                    // → no delay. `thread::sleep` is fine (off-tokio, spawn_blocking).
                    if let Some(c) = cadence {
                        std::thread::sleep(c);
                    }
                }
            }
        }

        // Capture ([[WEIR-T-0118]]) or evolve ([[WEIR-T-0120]]) the stream's typed schema from the
        // sampled (post-mapping) records: first run captures; later runs merge additive drift + flag
        // breaking. Keyed by the logical stream, best-effort.
        let _ = self
            .store
            .evolve_schema(connection, &stream.stream, &schema_sample);

        Ok(SyncOutcome {
            chunks,
            rows_written,
            dead_lettered,
            final_state: state,
        })
    }
}
