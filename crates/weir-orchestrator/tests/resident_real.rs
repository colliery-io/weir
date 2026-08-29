//! WEIR-T-0146 (REAL path): a perpetual resident must not block run-once execution
//! through the real `InProcessExecutor`/`Fleet` + real wasm. The earlier unit test used a
//! mock `future::pending` executor and missed the regression this reproduces.

mod common;
use common::wasm_ref;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::Store;
use weir_orchestrator::{
    ConnectorRef, ExecutionMode, Fleet, InProcessExecutor, Origin, Relay, WorkSpec, WorkerConfig,
};

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

/// The unified resident connector (`wasm-fixtures/resident`), staged into the shared dir.
fn resident_ref() -> ConnectorRef {
    let dir = weir_wasm_testkit::connectors_dir();
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../wasm-fixtures/resident"),
            wasm_file: "weir_resident_wasm.wasm",
            pkg_name: "weir-resident-pkg",
            capabilities: &[],
        },
        &dir,
    );
    ConnectorRef::Wasm {
        search_path: dir.display().to_string(),
        package: "weir-resident-pkg".to_string(),
        version: "0.0.0".to_string(),
        origin: Origin::FirstParty,
    }
}

fn runonce_spec(i: usize) -> WorkSpec {
    WorkSpec {
        connection: format!("ro-{i}"),
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
        execution_mode: ExecutionMode::RunOnce,
    }
}

fn resident_spec() -> WorkSpec {
    WorkSpec {
        connection: "res".to_string(),
        tenant: "default".to_string(),
        stream: stream("res"),
        source: resident_ref(),
        dest: wasm_ref("ArrowSink"),
        source_config: Config {
            json: r#"{"mode":"poll","rows_per_poll":1}"#.to_string(),
        },
        dest_config: Config {
            json: "{}".to_string(),
        },
        state_key: None,
        seed_cursor: None,
        partition: None,
        execution_mode: ExecutionMode::Resident {
            cadence_ms: Some(50),
            restart_backoff_ms: 1_000,
        },
    }
}

#[tokio::test]
async fn resident_does_not_block_runonce_real_executor() {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Arc::new(Store::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let relay = Relay::new(Arc::clone(&store)).unwrap();

    let res_id = relay.plan(&resident_spec()).unwrap();
    let ro: Vec<i64> = (0..3)
        .map(|i| relay.plan(&runonce_spec(i)).unwrap())
        .collect();

    let fleet = {
        let s = Arc::clone(&store);
        let r = relay.clone();
        Fleet::new(
            relay.clone(),
            move || InProcessExecutor::new(Arc::clone(&s), r.clone()),
            WorkerConfig {
                concurrency: 8,
                ..WorkerConfig::default()
            },
        )
    };

    // Drive like `serve`: run_until_idle each poll. The run-once units must reach `done`
    // WHILE the resident keeps running. A wedge/regression makes this hang → timeout → fail.
    let drained = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            fleet.run_until_idle().await.unwrap();
            let all_done = ro
                .iter()
                .all(|id| relay.state(*id).unwrap().as_deref() == Some("done"));
            if all_done {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;

    assert!(
        drained.is_ok(),
        "run-once did not drain while a resident runs (T-0146 wedge/regression)"
    );
    assert_eq!(
        relay.state(res_id).unwrap().as_deref(),
        Some("leased"),
        "resident should still be running (leased), not completed/failed"
    );

    // Shutdown ([[WEIR-T-0170]]): fire the resident's stop token — exactly what the
    // serve/runner daemons now do on ctrl-c — so the blocking run ends at its next poll
    // boundary and runtime teardown doesn't wait on the blocking pool forever (the wedge
    // this test used to reproduce).
    assert_eq!(
        relay.stop_all_residents(),
        1,
        "exactly one live resident stopped at shutdown"
    );
}

/// A tail (event-reader) unit: the unified `resident` fixture in `mode:tail` with a finite
/// config-supplied arrival burst. Run-once, so it reads the arrivals to end via the real path.
fn tail_spec() -> WorkSpec {
    WorkSpec {
        connection: "tail".to_string(),
        tenant: "default".to_string(),
        stream: stream("tail"),
        source: resident_ref(),
        dest: wasm_ref("ArrowSink"),
        source_config: Config {
            json: r#"{"mode":"tail","events":["a","b","c","d"]}"#.to_string(),
        },
        dest_config: Config {
            json: "{}".to_string(),
        },
        state_key: None,
        seed_cursor: None,
        partition: None,
        execution_mode: ExecutionMode::RunOnce,
    }
}

/// [[WEIR-T-0151]]: a tail/event-reader connector drains through the REAL `Fleet`/
/// `InProcessExecutor` + wasm — the arrival-driven emission path flows through the worker/executor,
/// not just the engine unit test. (The exact K arrivals → K records count is proven at the engine
/// level by `wasm_resident_engine::resident_tail_emits_per_arrival`; here we prove it completes
/// through the real drain path.)
#[tokio::test]
async fn tail_arrivals_drain_through_real_fleet() {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Arc::new(Store::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let relay = Relay::new(Arc::clone(&store)).unwrap();
    let id = relay.plan(&tail_spec()).unwrap();

    let fleet = {
        let s = Arc::clone(&store);
        let r = relay.clone();
        Fleet::new(
            relay.clone(),
            move || InProcessExecutor::new(Arc::clone(&s), r.clone()),
            WorkerConfig {
                concurrency: 4,
                ..WorkerConfig::default()
            },
        )
    };
    let done = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            fleet.run_until_idle().await.unwrap();
            if relay.state(id).unwrap().as_deref() == Some("done") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(done.is_ok(), "tail did not drain through the real Fleet");
    assert_eq!(
        relay.state(id).unwrap().as_deref(),
        Some("done"),
        "tail run should complete through the real InProcessExecutor"
    );
}
