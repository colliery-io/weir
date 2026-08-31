//! [[WEIR-T-0190]]: orchestrator sharp edges — owner-guarded transitions, the
//! T-0066 per-run isolation (an executor error fails only its run), the drain
//! containment cap, and per-schedule tick isolation.

mod common;
use common::wasm_ref;

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use diesel::prelude::*;
use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::Store;
use weir_orchestrator::{
    Clock, ExecutionResult, ExecutorError, Relay, Scheduler, WorkExecutor, WorkReadyEvent,
    WorkSpec, Worker, WorkerConfig,
};
use weir_schema::schedules;

fn stream(name: &str) -> ConfiguredStream {
    ConfiguredStream {
        stream: name.to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: None,
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    }
}

fn spec(connection: &str) -> WorkSpec {
    WorkSpec {
        connection: connection.to_string(),
        tenant: "default".to_string(),
        stream: stream("s"),
        source: wasm_ref("Echo"),
        dest: wasm_ref("ArrowSink"),
        source_config: Config {
            json: "{}".to_string(),
        },
        dest_config: Config {
            json: "{}".to_string(),
        },
        state_key: None,
        seed_cursor: None,
        partition: None,
        execution_mode: Default::default(),
    }
}

fn setup() -> (tempfile::TempDir, Arc<Store>, Relay) {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Arc::new(Store::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let relay = Relay::new(Arc::clone(&store)).unwrap();
    (tmp, store, relay)
}

/// Owner-guarded transitions: after a lease expires and the unit is re-claimed
/// by another worker, the ORIGINAL claimant's late `mark_done`/`mark_failed`
/// must be a no-op — never a clobber of the new holder's state.
#[test]
fn stale_owner_cannot_transition_a_reclaimed_unit() {
    let (_tmp, _store, relay) = setup();
    let id = relay.plan(&spec("c1")).unwrap();

    // w1 claims with a zero lease (instantly expired), the reclaimer returns the
    // unit to pending, and w2 claims it.
    let ev = relay
        .claim("w1", "default", Duration::ZERO)
        .unwrap()
        .expect("w1 claims");
    assert_eq!(ev.work_unit_id, id);
    assert_eq!(relay.reclaim_expired().unwrap(), 1);
    relay
        .claim("w2", "default", Duration::from_secs(60))
        .unwrap()
        .expect("w2 claims");

    // The ghost's late transitions are void.
    relay.mark_done(id, "w1", 5, 0).unwrap();
    assert_eq!(relay.state(id).unwrap().as_deref(), Some("leased"));
    relay.mark_failed(id, "w1", "late failure").unwrap();
    assert_eq!(relay.state(id).unwrap().as_deref(), Some("leased"));
    relay.requeue(id, "w1", 0).unwrap();
    assert_eq!(relay.state(id).unwrap().as_deref(), Some("leased"));

    // The live holder's transition applies.
    relay.mark_done(id, "w2", 5, 0).unwrap();
    assert_eq!(relay.state(id).unwrap().as_deref(), Some("done"));
}

/// A cancelled unit must stay cancelled: the (still-matching-owner) worker's
/// exit requeue must not resurrect it — the guard also requires an in-flight
/// state.
#[test]
fn stale_requeue_cannot_resurrect_a_cancelled_unit() {
    let (_tmp, _store, relay) = setup();
    let id = relay.plan(&spec("c1")).unwrap();
    relay
        .claim("w1", "default", Duration::from_secs(60))
        .unwrap()
        .expect("claimed");
    assert_eq!(relay.cancel("c1").unwrap(), 1);
    assert_eq!(relay.state(id).unwrap().as_deref(), Some("done"));

    relay.requeue(id, "w1", 0).unwrap();
    assert_eq!(
        relay.state(id).unwrap().as_deref(),
        Some("done"),
        "a cancelled unit must not be resurrected by the worker's exit requeue"
    );
}

/// The [[WEIR-T-0066]] wedge: an executor error (e.g. the JoinError a panicking
/// wasm run surfaces as) fails only ITS unit — the drain continues, the next
/// unit runs, and the failed unit lands terminal with the error recorded
/// (previously the whole pass aborted and the unit stayed leased-to-a-ghost).
struct FirstCallErrors(AtomicI64);
#[async_trait::async_trait]
impl WorkExecutor for FirstCallErrors {
    async fn execute(&self, _e: WorkReadyEvent) -> Result<ExecutionResult, ExecutorError> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            Err(ExecutorError::Join("task panicked".to_string()))
        } else {
            Ok(ExecutionResult::Completed {
                rows_written: 1,
                dead_lettered: 0,
            })
        }
    }
    fn name(&self) -> &str {
        "first-call-errors"
    }
}

#[tokio::test]
async fn executor_error_fails_only_its_own_run() {
    let (_tmp, _store, relay) = setup();
    // FIFO by id: `bad` is claimed first and its execute errors.
    let bad = relay.plan(&spec("bad")).unwrap();
    let good = relay.plan(&spec("good")).unwrap();

    let worker = Worker::new(
        relay.clone(),
        FirstCallErrors(AtomicI64::new(0)),
        WorkerConfig {
            owner: "w".to_string(),
            max_attempts: 1,
            base_delay: Duration::ZERO,
            heartbeat: Duration::ZERO,
            concurrency: 1,
            ..Default::default()
        },
    );
    worker
        .run_until_idle()
        .await
        .expect("the drain must survive one run's executor error");

    assert_eq!(relay.state(bad).unwrap().as_deref(), Some("failed"));
    assert_eq!(relay.state(good).unwrap().as_deref(), Some("done"));
}

/// Drain containment: a unit that perpetually reports `Skipped` (requeue
/// due-now) must surface as [`ExecutorError::DrainStuck`] instead of spinning
/// `run_until_idle` forever.
struct AlwaysSkips;
#[async_trait::async_trait]
impl WorkExecutor for AlwaysSkips {
    async fn execute(&self, _e: WorkReadyEvent) -> Result<ExecutionResult, ExecutorError> {
        Ok(ExecutionResult::Skipped)
    }
    fn name(&self) -> &str {
        "always-skips"
    }
}

#[tokio::test]
async fn run_until_idle_fails_loudly_when_stuck() {
    let (_tmp, _store, relay) = setup();
    relay.plan(&spec("looper")).unwrap();

    let worker = Worker::new(
        relay.clone(),
        AlwaysSkips,
        WorkerConfig {
            owner: "w".to_string(),
            heartbeat: Duration::ZERO,
            concurrency: 1,
            max_drain_units: 5,
            ..Default::default()
        },
    );
    match worker.run_until_idle().await {
        Err(ExecutorError::DrainStuck(n)) => assert_eq!(n, 5),
        other => panic!("expected DrainStuck, got {other:?}"),
    }
}

struct FixedClock(i64);
impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.0
    }
}

/// Per-schedule tick isolation: a poisoned schedule (garbage spec JSON) must
/// not stop the remaining due schedules from firing in the same pass.
#[test]
fn poisoned_schedule_does_not_block_the_rest() {
    let (_tmp, store, relay) = setup();
    let scheduler = Scheduler::new(relay.clone(), FixedClock(1_000)).unwrap();

    // A well-formed schedule, due now.
    scheduler
        .add("good", &spec("good"), Duration::from_secs(60))
        .unwrap();
    // A poisoned row: unparseable spec, due now, and an id that sorts FIRST so a
    // fail-fast loop would abort before reaching `good`.
    {
        let mut c = store.pool().get().unwrap();
        diesel::insert_into(schedules::table)
            .values((
                schedules::id.eq(1i64),
                schedules::connection.eq("poisoned"),
                schedules::spec.eq("{not json"),
                schedules::every_ms.eq(60_000i64),
                schedules::next_due_at.eq(0i64),
            ))
            .execute(&mut c)
            .unwrap();
    }

    let fired = scheduler
        .tick()
        .expect("tick survives the poisoned schedule");
    assert_eq!(fired, 1, "the well-formed schedule still fired");
    assert!(
        relay.has_active_in("default", "good").unwrap(),
        "the good connection got its planned unit"
    );
}
