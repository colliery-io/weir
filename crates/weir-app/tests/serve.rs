//! WEIR-T-0015: the serve path — schedules registered from connections fire
//! through the scheduler and run end-to-end on one node. Driven against an
//! injected clock (no sleeping).

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use weir_app::{App, Connection, DEFAULT_TENANT, connector_ref};

fn use_wasm_connectors() {
    unsafe {
        std::env::set_var("WEIR_CONNECTORS_DIR", weir_wasm_testkit::connectors_dir());
    }
}
use weir_orchestrator::{Clock, InProcessExecutor, Scheduler, Worker, WorkerConfig};

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

#[tokio::test]
async fn serve_runs_a_scheduled_connection() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap();
    app.add_connection(
        DEFAULT_TENANT,
        &Connection {
            name: "sched".to_string(),
            source: connector_ref("Echo"),
            dest: connector_ref("ArrowSink"),
            stream: "echo".to_string(),
            source_config: "{}".to_string(),
            dest_config: "{}".to_string(),
            every_secs: Some(1.0),
            cron: None,
            sync_mode: "full_refresh".into(),
            write_mode: "append".into(),
            business_keys: vec![],
            cursor_field: None,
            execution_mode: "run_once".into(),
        },
    )
    .unwrap();

    // The serve loop's building blocks, driven by a hand-clock.
    let clock = ManualClock::new(0);
    let scheduler = Scheduler::new(app.relay().clone(), clock.clone()).unwrap();
    assert_eq!(
        app.register_schedules(DEFAULT_TENANT, &scheduler).unwrap(),
        1
    );
    let worker = Worker::new(
        app.relay().clone(),
        InProcessExecutor::new(app.store().clone(), app.relay().clone()),
        WorkerConfig::default(),
    );

    // Due at t=0 → fires; draining runs the pipeline end-to-end (committed outbox).
    assert_eq!(scheduler.tick().unwrap(), 1);
    worker.run_until_idle().await.unwrap();
    assert_eq!(
        app.store().outbox_count("sched").unwrap(),
        1,
        "scheduled connection ran end-to-end on one node"
    );

    // Not due again until the interval elapses.
    clock.advance(500);
    assert_eq!(scheduler.tick().unwrap(), 0);
}

#[tokio::test]
async fn sync_picks_up_live_connection_at_subsecond_interval() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap();

    let clock = ManualClock::new(0);
    let scheduler = Scheduler::new(app.relay().clone(), clock.clone()).unwrap();
    let worker = Worker::new(
        app.relay().clone(),
        InProcessExecutor::new(app.store().clone(), app.relay().clone()),
        WorkerConfig::default(),
    );

    // No connections yet → reconcile registers nothing.
    assert_eq!(app.sync_schedules(&scheduler).unwrap(), 0);

    // Add a connection LIVE with a 0.1s (100ms) interval.
    app.add_connection(
        DEFAULT_TENANT,
        &Connection {
            name: "live".to_string(),
            source: connector_ref("Echo"),
            dest: connector_ref("ArrowSink"),
            stream: "echo".to_string(),
            source_config: "{}".to_string(),
            dest_config: "{}".to_string(),
            every_secs: Some(0.1),
            cron: None,
            sync_mode: "full_refresh".into(),
            write_mode: "append".into(),
            business_keys: vec![],
            cursor_field: None,
            execution_mode: "run_once".into(),
        },
    )
    .unwrap();

    // Reconcile picks it up (1 change) without a restart → fires at t=0.
    assert_eq!(app.sync_schedules(&scheduler).unwrap(), 1);
    assert_eq!(scheduler.tick().unwrap(), 1);
    worker.run_until_idle().await.unwrap();

    // Sub-second granularity: NOT due at +50ms, due again at +100ms (proves
    // every_ms=100, not the old whole-second 1000).
    clock.advance(50);
    assert_eq!(scheduler.tick().unwrap(), 0);
    clock.advance(50);
    assert_eq!(scheduler.tick().unwrap(), 1);

    // Idempotent reconcile: no spurious changes when nothing changed.
    assert_eq!(app.sync_schedules(&scheduler).unwrap(), 0);
}
