//! WEIR-T-0051 benchmark: end-to-end **run throughput** through the relay + worker
//! at varying concurrency — measures runs/s for independent echo→arrow-sink units,
//! exposing per-run overhead (each run currently re-resolves the connector via
//! `from_wasm_package`). `#[ignore]`: run with
//! `cargo test -p weir-orchestrator --test throughput -- --ignored --nocapture`.

mod common;
use common::wasm_ref;

use std::sync::Arc;
use std::time::Instant;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::Store;
use weir_orchestrator::{InProcessExecutor, Relay, WorkSpec, Worker, WorkerConfig};

fn unit(i: usize) -> WorkSpec {
    WorkSpec {
        connection: format!("c{i}"), // distinct → independent stream_state rows
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

#[tokio::test(flavor = "multi_thread")]
#[ignore = "benchmark — run with --ignored --nocapture"]
async fn concurrent_run_throughput() {
    unsafe { std::env::set_var("WEIR_CONNECTORS_DIR", weir_wasm_testkit::connectors_dir()) };
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Arc::new(Store::open(tmp.path().join("bench.db").to_str().unwrap()).unwrap());
    let relay = Relay::new(Arc::clone(&store)).unwrap();

    const RUNS: usize = 100;
    eprintln!("\n==== run throughput (echo→arrow-sink, {RUNS} units) ====");
    for k in [1usize, 4, 16] {
        for i in 0..RUNS {
            relay.plan(&unit(i)).unwrap();
        }
        let worker = Worker::new(
            relay.clone(),
            InProcessExecutor::new(Arc::clone(&store), relay.clone()),
            WorkerConfig {
                concurrency: k,
                ..WorkerConfig::default()
            },
        );
        let start = Instant::now();
        worker.run_until_idle().await.unwrap();
        let secs = start.elapsed().as_secs_f64();
        eprintln!(
            "  concurrency {k:>2}: {RUNS} runs in {secs:6.2}s = {:7.1} runs/s  ({:.1} ms/run)",
            RUNS as f64 / secs,
            secs * 1000.0 / RUNS as f64,
        );
    }
    eprintln!();
}
