//! WEIR-T-0014: configure a connection and run a real pipeline end-to-end on a
//! single node (embedded SQLite, no broker) through the `weir_app::App`.

use weir_app::{App, Connection, DEFAULT_TENANT, connector_ref};

fn use_wasm_connectors() {
    unsafe {
        std::env::set_var("WEIR_CONNECTORS_DIR", weir_wasm_testkit::connectors_dir());
    }
}

#[tokio::test]
async fn connection_modes_round_trip_and_validate() {
    // [[WEIR-I-0028]]: sync/write modes + business keys persist and reload on a connection, and an
    // incomplete upsert is rejected. (The engine executes the derived CDC stream — proven live by the
    // T-0117 delete-propagation harness; work_spec's mapping is unit-tested in weir-app.)
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).expect("open store");

    app.add_connection(
        DEFAULT_TENANT,
        &Connection {
            name: "cdc".to_string(),
            source: connector_ref("postgres"),
            dest: connector_ref("rest-dest"),
            stream: "orders".to_string(),
            source_config: "{}".to_string(),
            dest_config: "{}".to_string(),
            every_secs: None,
            cron: None,
            sync_mode: "cdc".to_string(),
            write_mode: "upsert".to_string(),
            business_keys: vec!["id".to_string()],
            cursor_field: None,
            execution_mode: "run_once".into(),
        },
    )
    .expect("add cdc/upsert connection");

    let got = app.get_connection(DEFAULT_TENANT, "cdc").expect("reload");
    assert_eq!(got.sync_mode, "cdc");
    assert_eq!(got.write_mode, "upsert");
    assert_eq!(got.business_keys, vec!["id".to_string()]);
    assert!(got.cursor_field.is_none());

    // An upsert with no business keys is rejected.
    let bad = Connection {
        name: "bad".to_string(),
        source: connector_ref("slow"),
        dest: connector_ref("postgres"),
        stream: "s".to_string(),
        source_config: "{}".to_string(),
        dest_config: "{}".to_string(),
        every_secs: None,
        cron: None,
        sync_mode: "full_refresh".to_string(),
        write_mode: "upsert".to_string(),
        business_keys: vec![],
        cursor_field: None,
        execution_mode: "run_once".into(),
    };
    assert!(
        app.add_connection(DEFAULT_TENANT, &bad).is_err(),
        "upsert needs business_keys"
    );
}

#[tokio::test]
async fn resident_start_is_enqueue_once_and_stop_cancels() {
    // [[WEIR-I-0035]] F1.5: a resident source is launched by an explicit enqueue-once `start`
    // (the scheduler won't fire it), is idempotent, and `stop` durably ends its restart loop.
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).expect("open store");

    app.add_connection(
        DEFAULT_TENANT,
        &Connection {
            name: "live".to_string(),
            source: connector_ref("slow"),
            dest: connector_ref("rest-dest"),
            stream: "s".to_string(),
            source_config: "{}".to_string(),
            dest_config: "{}".to_string(),
            every_secs: None,
            cron: None,
            sync_mode: "full_refresh".to_string(),
            write_mode: "append".to_string(),
            business_keys: vec![],
            cursor_field: None,
            execution_mode: "resident".into(),
        },
    )
    .expect("add resident connection");

    // First start enqueues exactly one unit.
    let first = app.start(DEFAULT_TENANT, "live").expect("start");
    assert!(first.is_some(), "first start enqueues a unit");
    assert!(app.relay().has_active("live").unwrap(), "unit is active");

    // Enqueue-once: a second start is a no-op while one is active.
    let second = app.start(DEFAULT_TENANT, "live").expect("start again");
    assert!(
        second.is_none(),
        "second start is idempotent (no double-start)"
    );

    // Stop durably cancels the active unit → restart loop ends.
    let stopped = app.stop(DEFAULT_TENANT, "live").expect("stop");
    assert!(stopped >= 1, "stop cancelled the active unit");
    assert!(
        !app.relay().has_active("live").unwrap(),
        "no active unit after stop"
    );

    // A run-once connection cannot be `start`ed (use `run`).
    app.add_connection(
        DEFAULT_TENANT,
        &Connection {
            name: "batch".to_string(),
            source: connector_ref("slow"),
            dest: connector_ref("rest-dest"),
            stream: "s".to_string(),
            source_config: "{}".to_string(),
            dest_config: "{}".to_string(),
            every_secs: None,
            cron: None,
            sync_mode: "full_refresh".to_string(),
            write_mode: "append".to_string(),
            business_keys: vec![],
            cursor_field: None,
            execution_mode: "run_once".into(),
        },
    )
    .expect("add run_once connection");
    assert!(
        app.start(DEFAULT_TENANT, "batch").is_err(),
        "run_once is not startable as resident"
    );
}

#[tokio::test]
async fn configure_and_run_single_node_pipeline() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let db = tmp.path().join("weir.db");
    let app = App::open(db.to_str().unwrap()).expect("open store");

    app.add_connection(
        DEFAULT_TENANT,
        &Connection {
            name: "demo".to_string(),
            source: connector_ref("Echo"),
            dest: connector_ref("ArrowSink"),
            stream: "echo".to_string(),
            source_config: "{}".to_string(),
            dest_config: "{}".to_string(),
            every_secs: None,
            cron: None,
            sync_mode: "full_refresh".into(),
            write_mode: "append".into(),
            business_keys: vec![],
            cursor_field: None,
            execution_mode: "run_once".into(),
        },
    )
    .expect("add connection");

    // Persisted + reloadable.
    assert_eq!(app.list_connections(DEFAULT_TENANT).unwrap().len(), 1);
    assert_eq!(
        app.get_connection(DEFAULT_TENANT, "demo").unwrap().source,
        connector_ref("Echo")
    );

    // Run it: a real source→engine→destination pipeline on one node.
    let report = app.run("demo").await.expect("run");
    assert_eq!(
        report.state, "done",
        "single-node pipeline ran to completion"
    );
}

#[tokio::test]
async fn run_unknown_connection_errors() {
    let tmp = tempfile::TempDir::new().unwrap();
    let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap();
    assert!(app.run("nope").await.is_err());
}

#[tokio::test]
async fn runner_drains_work_planned_by_another_app() {
    // [[WEIR-T-0102]]: a standalone `weir runner` (run_workers) drains work planned by a *separate*
    // App against the same store — the multi-claimant, separable-runner property.
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let db = tmp.path().join("weir.db");

    // Control plane: plan a run (no worker running here).
    let cp = App::open(db.to_str().unwrap()).unwrap();
    cp.add_connection(
        DEFAULT_TENANT,
        &Connection {
            name: "demo".to_string(),
            source: connector_ref("Echo"),
            dest: connector_ref("ArrowSink"),
            stream: "echo".to_string(),
            source_config: "{}".to_string(),
            dest_config: "{}".to_string(),
            every_secs: None,
            cron: None,
            sync_mode: "full_refresh".into(),
            write_mode: "append".into(),
            business_keys: vec![],
            cursor_field: None,
            execution_mode: "run_once".into(),
        },
    )
    .unwrap();
    let id = cp.plan_run(DEFAULT_TENANT, "demo").unwrap();

    // A separate runner App on the SAME store drains it.
    let runner = App::open(db.to_str().unwrap()).unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let handle = tokio::spawn(async move {
        let _ = runner
            .run_workers(std::time::Duration::from_millis(50), 1, None, async {
                let _ = rx.await;
            })
            .await;
    });

    // The work reaches `done`, executed by the runner (not the control plane).
    let mut done = false;
    for _ in 0..120 {
        if cp.relay().state(id).unwrap().as_deref() == Some("done") {
            done = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let _ = tx.send(());
    let _ = handle.await;
    assert!(
        done,
        "runner drained the control plane's planned work to done"
    );
}

#[tokio::test]
async fn only_the_leader_schedules() {
    // [[WEIR-T-0106]]: two control-plane replicas on one store — only the lease leader schedules,
    // so a due connection is enqueued exactly once, not once-per-replica.
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let db = tmp.path().join("weir.db");
    let a = App::open(db.to_str().unwrap()).unwrap();
    a.add_connection(
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
    let b = App::open(db.to_str().unwrap()).unwrap();

    // One guarded scheduling cycle per replica (mirrors serve's leader guard).
    let ttl = std::time::Duration::from_secs(30);
    let led = |app: &App, owner: &str| -> bool {
        let sched = weir_app::Scheduler::new(app.relay().clone(), weir_app::SystemClock).unwrap();
        if app
            .relay()
            .try_acquire_lease("scheduler", owner, ttl)
            .unwrap()
        {
            app.sync_schedules(&sched).unwrap();
            sched.tick().unwrap();
            true
        } else {
            false
        }
    };
    let led_a = led(&a, "replica-A");
    let led_b = led(&b, "replica-B");
    // Exactly one replica scheduled this window — without the lease both would.
    assert_eq!(
        led_a as i32 + led_b as i32,
        1,
        "exactly one leader (A={led_a}, B={led_b})"
    );
    assert!(led_a && !led_b, "the first acquirer leads");
}

#[tokio::test]
async fn run_captures_stream_schema() {
    // [[WEIR-T-0118]]: a run infers + persists the stream's typed schema from the source records.
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap();
    app.add_connection(
        DEFAULT_TENANT,
        &Connection {
            name: "sc".to_string(),
            source: connector_ref("Slow"),
            dest: connector_ref("ArrowSink"),
            stream: "sc".to_string(),
            source_config: "{\"rows\":3,\"batch\":true,\"sleep_ms\":0}".to_string(),
            dest_config: "{}".to_string(),
            every_secs: None,
            cron: None,
            sync_mode: "full_refresh".into(),
            write_mode: "append".into(),
            business_keys: vec![],
            cursor_field: None,
            execution_mode: "run_once".into(),
        },
    )
    .unwrap();
    app.plan_run(DEFAULT_TENANT, "sc").unwrap();
    app.drain().await.unwrap();

    // Slow emits {"n": i} → the captured schema has field `n` of type integer.
    let schema = app
        .get_stream_schema(DEFAULT_TENANT, "sc", "sc")
        .unwrap()
        .expect("schema captured on run");
    let n = schema
        .fields
        .iter()
        .find(|f| f.name == "n")
        .expect("field n");
    assert_eq!(n.field_type, weir_app::FieldType::Integer);
}

#[tokio::test]
async fn per_side_config_round_trips() {
    // [[WEIR-I-0029]]: a connection's source and destination configs persist + reload independently.
    use_wasm_connectors(); // creation-time validation ([[WEIR-T-0166]]) resolves the refs
    let tmp = tempfile::TempDir::new().unwrap();
    let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).expect("open store");
    app.add_connection(
        DEFAULT_TENANT,
        &Connection {
            name: "split".to_string(),
            source: connector_ref("postgres"),
            dest: connector_ref("postgres"),
            stream: "orders".to_string(),
            source_config: r#"{"table":"orders_src"}"#.to_string(),
            dest_config: r#"{"table":"orders_dst"}"#.to_string(),
            every_secs: None,
            cron: None,
            sync_mode: "full_refresh".to_string(),
            write_mode: "append".to_string(),
            business_keys: vec![],
            cursor_field: None,
            execution_mode: "run_once".into(),
        },
    )
    .expect("add split-config connection");

    let got = app.get_connection(DEFAULT_TENANT, "split").expect("reload");
    assert!(
        got.source_config.contains("orders_src"),
        "source config: {}",
        got.source_config
    );
    assert!(
        got.dest_config.contains("orders_dst"),
        "dest config: {}",
        got.dest_config
    );
    assert_ne!(
        got.source_config, got.dest_config,
        "the two sides are independent"
    );
}
