//! WEIR-T-0009: the scheduler fires runs on an interval by calling the relay's
//! `plan()`, evaluated against a deterministic clock (no sleeping), and does not
//! double-start a connection that already has an in-flight unit.

mod common;
use common::wasm_ref;

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::Store;
use weir_orchestrator::{
    Clock, InProcessExecutor, Relay, Scheduler, WorkSpec, Worker, WorkerConfig,
};

/// A clock the test drives by hand.
#[derive(Clone)]
struct ManualClock(Arc<AtomicI64>);
impl ManualClock {
    fn new(t: i64) -> Self {
        Self(Arc::new(AtomicI64::new(t)))
    }
    fn advance(&self, ms: i64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}
impl Clock for ManualClock {
    fn now_ms(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

fn spec() -> WorkSpec {
    WorkSpec {
        connection: "sched-conn".to_string(),
        tenant: "default".to_string(),
        stream: ConfiguredStream {
            stream: "echo".to_string(),
            sync_mode: SyncMode::FullRefresh,
            cursor_field: None,
            primary_key: None,
            write_mode: WriteMode::Append,
            mapping: MappingSpec::default(),
        },
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

async fn drain(relay: &Relay, store: &Arc<Store>) {
    let worker = Worker::new(
        relay.clone(),
        InProcessExecutor::new(Arc::clone(store), relay.clone()),
        WorkerConfig::default(),
    );
    worker.run_until_idle().await.unwrap();
}

#[tokio::test]
async fn fires_due_schedule_then_drains() {
    let (_tmp, store, relay) = setup();
    let clock = ManualClock::new(1_000);
    let scheduler = Scheduler::new(relay.clone(), clock.clone()).unwrap();
    scheduler
        .add("sched-conn", &spec(), Duration::from_millis(1_000))
        .unwrap();

    assert_eq!(scheduler.tick().unwrap(), 1, "due on first tick");
    drain(&relay, &store).await;
    // A drained connection has no in-flight unit.
    assert!(!relay.has_active("sched-conn").unwrap());
}

/// WEIR-I-0035 F1.3: the scheduler does not interval/cron-fire a **resident** spec —
/// resident sources are enqueued once (via `Relay::plan`) and kept alive by the perpetual
/// lease; firing them here would resurrect a deliberately-stopped source.
#[tokio::test]
async fn skips_resident_specs() {
    let (_tmp, _store, relay) = setup();
    let clock = ManualClock::new(1_000);
    let scheduler = Scheduler::new(relay.clone(), clock.clone()).unwrap();
    let mut s = spec();
    s.connection = "resident-conn".to_string();
    s.execution_mode = weir_orchestrator::ExecutionMode::Resident {
        cadence_ms: None,
        restart_backoff_ms: 1_000,
    };
    scheduler
        .add("resident-conn", &s, Duration::from_millis(1_000))
        .unwrap();

    assert_eq!(
        scheduler.tick().unwrap(),
        0,
        "resident specs are not interval-fired"
    );
    assert!(!relay.has_active("resident-conn").unwrap());
}

#[tokio::test]
async fn does_not_fire_before_interval_elapses() {
    let (_tmp, store, relay) = setup();
    let clock = ManualClock::new(1_000);
    let scheduler = Scheduler::new(relay.clone(), clock.clone()).unwrap();
    scheduler
        .add("sched-conn", &spec(), Duration::from_millis(1_000))
        .unwrap();

    assert_eq!(scheduler.tick().unwrap(), 1); // fires at t=1000, next_due=2000
    drain(&relay, &store).await;

    clock.advance(500); // t=1500 < 2000
    assert_eq!(scheduler.tick().unwrap(), 0, "not due yet");

    clock.advance(500); // t=2000
    assert_eq!(scheduler.tick().unwrap(), 1, "due again after the interval");
}

#[tokio::test]
async fn does_not_double_start_an_active_connection() {
    let (_tmp, _store, relay) = setup();
    let clock = ManualClock::new(0);
    let scheduler = Scheduler::new(relay.clone(), clock.clone()).unwrap();
    scheduler
        .add("sched-conn", &spec(), Duration::from_millis(1_000))
        .unwrap();

    assert_eq!(scheduler.tick().unwrap(), 1); // plans a unit (left un-drained → in-flight)
    clock.advance(1_000); // t=1000, due again
    assert_eq!(
        scheduler.tick().unwrap(),
        0,
        "skipped — the connection still has an in-flight unit"
    );
}

#[tokio::test]
async fn cron_fires_at_the_expected_second() {
    let (_tmp, store, relay) = setup();
    let clock = ManualClock::new(0); // 1970-01-01T00:00:00Z
    let scheduler = Scheduler::new(relay.clone(), clock.clone()).unwrap();
    // Second 30 of every minute (6-field, seconds-first cron).
    scheduler
        .add_cron("sched-conn", &spec(), "30 * * * * *")
        .unwrap();

    assert_eq!(scheduler.tick().unwrap(), 0, "not due before 00:00:30");

    clock.0.store(30_000, Ordering::SeqCst); // 00:00:30
    assert_eq!(scheduler.tick().unwrap(), 1, "fires at 00:00:30");
    drain(&relay, &store).await;

    clock.0.store(60_000, Ordering::SeqCst); // 00:01:00 — next is 00:01:30
    assert_eq!(scheduler.tick().unwrap(), 0, "not due until 00:01:30");

    clock.0.store(90_000, Ordering::SeqCst); // 00:01:30
    assert_eq!(scheduler.tick().unwrap(), 1, "fires again the next minute");
}

#[tokio::test]
async fn cron_rejects_an_invalid_expression() {
    let (_tmp, _store, relay) = setup();
    let scheduler = Scheduler::new(relay.clone(), ManualClock::new(0)).unwrap();
    assert!(scheduler.add_cron("c", &spec(), "not a cron").is_err());
}

/// WEIR-I-0035 F1.4: the resident claim-headroom gate returns a claimed-but-unstarted resident
/// unit to Pending (deferred, attempt undone) so a runner with headroom takes it — it is not lost.
#[tokio::test]
async fn resident_release_returns_unit_to_pending_deferred() {
    let (_tmp, _store, relay) = setup();
    let mut s = spec();
    s.connection = "resident-gate".to_string();
    s.execution_mode = weir_orchestrator::ExecutionMode::Resident {
        cadence_ms: None,
        restart_backoff_ms: 1_000,
    };
    let id = relay.plan(&s).unwrap();

    // Claim (leases it, attempt → 1); then release — the gate's action on a saturated runner.
    let ev = relay
        .claim("owner", "default", Duration::from_secs(30))
        .unwrap()
        .unwrap();
    assert_eq!(ev.work_unit_id, id);
    relay.release(id, Duration::from_millis(500)).unwrap();

    // Deferred: not instantly re-claimable, but still present (pending) — not dropped.
    assert!(
        relay
            .claim("owner", "default", Duration::from_secs(30))
            .unwrap()
            .is_none(),
        "released unit is deferred, not instantly re-claimable"
    );
    assert!(
        relay.has_active("resident-gate").unwrap(),
        "released resident unit is still pending (not lost)"
    );
}

/// WEIR-I-0035 F1.4 (through the worker, not just the relay): a `Worker` configured with a
/// `high_water` and a probe reading over it must, on `tick`, claim the resident unit, GATE it
/// (release back to pending) and NOT invoke the executor — proving the measured-usage wiring, not
/// just `Relay::release` in isolation.
#[tokio::test]
async fn worker_tick_gates_new_resident_when_over_high_water() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let (_tmp, _store, relay) = setup();
    let mut s = spec();
    s.connection = "resident-hot".to_string();
    s.execution_mode = weir_orchestrator::ExecutionMode::Resident {
        cadence_ms: None,
        restart_backoff_ms: 0,
    };
    relay.plan(&s).unwrap();

    let calls = Arc::new(AtomicUsize::new(0));
    struct FakeExec(Arc<AtomicUsize>);
    #[async_trait::async_trait]
    impl weir_orchestrator::WorkExecutor for FakeExec {
        fn name(&self) -> &str {
            "fake"
        }
        async fn execute(
            &self,
            _e: weir_orchestrator::WorkReadyEvent,
        ) -> Result<weir_orchestrator::ExecutionResult, weir_orchestrator::ExecutorError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(weir_orchestrator::ExecutionResult::Completed {
                rows_written: 0,
                dead_lettered: 0,
            })
        }
    }

    let probe = weir_orchestrator::StaticUsageProbe(weir_orchestrator::Usage {
        mem_fraction: 0.95,
        cpu_fraction: 0.0,
    });
    let worker = Worker::new(
        relay.clone(),
        FakeExec(calls.clone()),
        WorkerConfig::default(),
    )
    .with_headroom(Arc::new(probe), 0.8);

    let ran = worker.tick().await.unwrap();
    assert!(
        !ran,
        "gated tick claims then releases → reports no work run"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "the executor is NOT invoked for a gated resident unit"
    );
    assert!(
        relay.has_active("resident-hot").unwrap(),
        "the gated resident unit remains pending (released, not lost)"
    );
}

/// [[WEIR-T-0146]]: a **perpetual** resident must NOT wedge `run_until_idle`. Co-schedule a
/// resident (whose `execute` never completes) with run-once units; `run_until_idle` must RETURN
/// and the run-once units must drain to `done` WHILE the resident keeps running. Pre-fix this hangs
/// (the resident's tick future never resolves), so the timeout guard turns the regression into a
/// failure rather than a hung suite.
/// [[WEIR-T-0149]] cross-process mid-stream stop: a durable cancel (as from `App::stop` on ANY
/// process) must halt the *actual* resident run, not just its DB state. The worker never touches the
/// in-process `StopHandle` here — the run stops only because the heartbeat observes the lost lease
/// and fires `WorkExecutor::stop`.
#[tokio::test]
async fn resident_cancel_stops_the_running_task_cross_process() {
    let (_tmp, _store, relay) = setup();

    let mut r = spec();
    r.connection = "resident".to_string();
    r.execution_mode = weir_orchestrator::ExecutionMode::Resident {
        cadence_ms: Some(20),
        restart_backoff_ms: 0,
    };
    relay.plan(&r).unwrap();

    // A resident whose run loops until its stop token fires; `stop()` (the trait hook the heartbeat
    // calls on lease-loss) flips the flag the run polls. No StopHandle is touched directly.
    #[derive(Clone)]
    struct Stoppable {
        stopped: Arc<std::sync::atomic::AtomicBool>,
        running: Arc<std::sync::atomic::AtomicBool>,
    }
    #[async_trait::async_trait]
    impl weir_orchestrator::WorkExecutor for Stoppable {
        fn name(&self) -> &str {
            "stoppable"
        }
        async fn execute(
            &self,
            _e: weir_orchestrator::WorkReadyEvent,
        ) -> Result<weir_orchestrator::ExecutionResult, weir_orchestrator::ExecutorError> {
            self.running.store(true, Ordering::SeqCst);
            while !self.stopped.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Ok(weir_orchestrator::ExecutionResult::Completed {
                rows_written: 0,
                dead_lettered: 0,
            })
        }
        fn stop(&self, _id: i64) {
            self.stopped.store(true, Ordering::SeqCst);
        }
    }
    let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let running = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker = Worker::new(
        relay.clone(),
        Stoppable {
            stopped: stopped.clone(),
            running: running.clone(),
        },
        WorkerConfig {
            heartbeat: Duration::from_millis(50),
            ..WorkerConfig::default()
        },
    );

    // Detach the resident (run_until_idle returns; the run keeps going in a spawned task).
    worker.run_until_idle().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !running.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("resident run should start");
    assert!(!stopped.load(Ordering::SeqCst), "not stopped before cancel");

    // Durable cancel — no direct StopHandle, as a control-plane `stop` on another process would do.
    relay.cancel("resident").unwrap();

    // The heartbeat observes the lost lease and fires stop → the run ends on its own.
    tokio::time::timeout(Duration::from_secs(5), async {
        while !stopped.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("a durable cancel must stop the ACTUAL resident run (cross-process), not just the DB");
}

#[tokio::test]
async fn resident_does_not_wedge_run_until_idle() {
    let (_tmp, _store, relay) = setup();

    // One perpetual resident...
    let mut r = spec();
    r.connection = "resident".to_string();
    r.execution_mode = weir_orchestrator::ExecutionMode::Resident {
        cadence_ms: None,
        restart_backoff_ms: 0,
    };
    relay.plan(&r).unwrap();

    // ...co-scheduled with three run-once units.
    for i in 0..3 {
        let mut s = spec();
        s.connection = format!("once-{i}");
        relay.plan(&s).unwrap();
    }

    // A resident's `execute` never completes; run-once completes immediately. Classify by loading
    // the spec (the detach decision itself is made in `tick`, before `execute` is called).
    struct FakeExec {
        relay: Relay,
    }
    #[async_trait::async_trait]
    impl weir_orchestrator::WorkExecutor for FakeExec {
        fn name(&self) -> &str {
            "fake"
        }
        async fn execute(
            &self,
            e: weir_orchestrator::WorkReadyEvent,
        ) -> Result<weir_orchestrator::ExecutionResult, weir_orchestrator::ExecutorError> {
            let resident = self
                .relay
                .load(e.work_unit_id)
                .ok()
                .flatten()
                .map(|s| s.execution_mode.is_resident())
                .unwrap_or(false);
            if resident {
                std::future::pending::<()>().await; // never completes (a live resident)
            }
            Ok(weir_orchestrator::ExecutionResult::Completed {
                rows_written: 1,
                dead_lettered: 0,
            })
        }
    }

    let worker = Worker::new(
        relay.clone(),
        FakeExec {
            relay: relay.clone(),
        },
        WorkerConfig {
            heartbeat: Duration::ZERO,
            ..WorkerConfig::default()
        },
    );

    // Must RETURN despite the perpetual resident in flight (pre-fix: hangs → timeout → test fails).
    tokio::time::timeout(Duration::from_secs(10), worker.run_until_idle())
        .await
        .expect("run_until_idle must return even with a perpetual resident in flight")
        .unwrap();

    // Run-once units drained to done...
    for i in 0..3 {
        assert!(
            !relay.has_active(&format!("once-{i}")).unwrap(),
            "once-{i} should have drained while the resident keeps running"
        );
    }
    // ...and the resident is still active (leased, running in its detached task).
    assert!(
        relay.has_active("resident").unwrap(),
        "the resident stays active/leased (detached, still running)"
    );
}

/// [[WEIR-T-0152]]: TWO concurrent runners claiming from ONE shared store co-exist safely —
/// run-once units all drain, each executed **exactly once** (the atomic lease/claim prevents
/// double-run), WHILE a perpetual resident runs on one runner. This is the claim/lease mechanism
/// that underpins a multi-runner fleet; true multi-node (k8s pods + autoscaler scaling real
/// replicas) is beyond in-process, but this proves the safety guarantee it relies on (and ties to
/// F4 fan-out-at-scale).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_runners_share_one_store_no_double_run() {
    let (_tmp, _store, relay) = setup();

    // One perpetual resident + six run-once, all in ONE store.
    let mut r = spec();
    r.connection = "resident".to_string();
    r.execution_mode = weir_orchestrator::ExecutionMode::Resident {
        cadence_ms: None,
        restart_backoff_ms: 0,
    };
    relay.plan(&r).unwrap();
    let ro_ids: Vec<i64> = (0..6)
        .map(|i| {
            let mut s = spec();
            s.connection = format!("once-{i}");
            relay.plan(&s).unwrap()
        })
        .collect();

    // Records every execute(id) so we can prove no unit runs twice across the two runners.
    #[derive(Clone)]
    struct RecExec {
        relay: Relay,
        seen: Arc<std::sync::Mutex<Vec<i64>>>,
    }
    #[async_trait::async_trait]
    impl weir_orchestrator::WorkExecutor for RecExec {
        fn name(&self) -> &str {
            "rec"
        }
        async fn execute(
            &self,
            e: weir_orchestrator::WorkReadyEvent,
        ) -> Result<weir_orchestrator::ExecutionResult, weir_orchestrator::ExecutorError> {
            self.seen.lock().unwrap().push(e.work_unit_id);
            let resident = self
                .relay
                .load(e.work_unit_id)
                .ok()
                .flatten()
                .map(|s| s.execution_mode.is_resident())
                .unwrap_or(false);
            if resident {
                std::future::pending::<()>().await; // a live resident: never completes
            }
            Ok(weir_orchestrator::ExecutionResult::Completed {
                rows_written: 1,
                dead_lettered: 0,
            })
        }
    }

    let seen = Arc::new(std::sync::Mutex::new(Vec::<i64>::new()));
    let mk = |owner: &str| {
        Worker::new(
            relay.clone(),
            RecExec {
                relay: relay.clone(),
                seen: seen.clone(),
            },
            WorkerConfig {
                owner: owner.to_string(),
                // Heartbeat holds the resident's lease so the OTHER runner never reclaims it.
                heartbeat: Duration::from_millis(50),
                lease: Duration::from_secs(5),
                ..WorkerConfig::default()
            },
        )
    };

    async fn drive(w: Worker<RecExec>, relay: Relay, ids: Vec<i64>) {
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                w.run_until_idle().await.unwrap();
                if ids
                    .iter()
                    .all(|id| relay.state(*id).unwrap().as_deref() == Some("done"))
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("runner drained within 20s");
    }

    // Two runners drive the ONE store concurrently.
    let (w1, w2) = (mk("runner-1"), mk("runner-2"));
    tokio::join!(
        drive(w1, relay.clone(), ro_ids.clone()),
        drive(w2, relay.clone(), ro_ids.clone()),
    );

    // All run-once drained...
    for id in &ro_ids {
        assert_eq!(
            relay.state(*id).unwrap().as_deref(),
            Some("done"),
            "run-once unit {id} did not drain"
        );
    }
    // ...each executed EXACTLY ONCE across BOTH runners (atomic claim → no double-run)...
    let seen = seen.lock().unwrap();
    for id in &ro_ids {
        let n = seen.iter().filter(|x| **x == *id).count();
        assert_eq!(
            n, 1,
            "unit {id} ran {n}× across 2 runners (must be exactly 1)"
        );
    }
    // ...and the resident is still alive on whichever runner claimed it.
    assert!(
        relay.has_active("resident").unwrap(),
        "resident should still be running under one of the runners"
    );
}
