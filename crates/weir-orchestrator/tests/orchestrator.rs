//! WEIR-T-0011: the orchestration relay + in-process executor over the sync
//! engine. plan → claim(lease) → execute(spawn_blocking) → state transition,
//! with retry-as-transition and lease reclaim.

mod common;
use common::wasm_ref;

use std::sync::Arc;
use std::time::Duration;

use weir_connector::{Config, ConfiguredStream, MappingSpec, PartitionScheme, SyncMode, WriteMode};
use weir_engine::Store;
use weir_orchestrator::{
    ExecutionMode, InProcessExecutor, Relay, WorkSpec, Worker, WorkerConfig, materialize_partitions,
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

fn setup() -> (tempfile::TempDir, Arc<Store>, Relay) {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Arc::new(Store::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let relay = Relay::new(Arc::clone(&store)).unwrap();
    (tmp, store, relay)
}

fn spec(source_plugin: &str, name: &str, cfg: &str) -> WorkSpec {
    WorkSpec {
        connection: "c1".to_string(),
        tenant: "default".to_string(),
        stream: stream(name),
        source: wasm_ref(source_plugin),
        dest: wasm_ref("ArrowSink"),
        source_config: Config {
            json: cfg.to_string(),
        },
        dest_config: Config {
            json: cfg.to_string(),
        },
        state_key: None,
        seed_cursor: None,
        partition: None,
        execution_mode: Default::default(),
    }
}

fn worker(relay: &Relay, store: &Arc<Store>, max_attempts: u32) -> Worker<InProcessExecutor> {
    let executor = InProcessExecutor::new(Arc::clone(store), relay.clone());
    Worker::new(
        relay.clone(),
        executor,
        WorkerConfig {
            owner: "test".to_string(),
            max_attempts,
            base_delay: Duration::ZERO,
            lease: Duration::from_secs(30),
            heartbeat: Duration::ZERO,
            ..Default::default()
        },
    )
}

#[tokio::test]
async fn plan_then_drain_completes() {
    let (_tmp, store, relay) = setup();
    let id = relay.plan(&spec("Echo", "echo", "{}")).unwrap();
    assert_eq!(relay.state(id).unwrap().as_deref(), Some("pending"));

    worker(&relay, &store, 1).run_until_idle().await.unwrap();
    assert_eq!(relay.state(id).unwrap().as_deref(), Some("done"));
}

// NOTE (WEIR-I-0011 / WEIR-A-0030): the former `transient_blip_recovers_across_ticks`
// test relied on `faulty`'s `until` counter — a process-global static that native
// in-process connectors shared across retries. Under WASM-always, each retry resolves
// a FRESH, isolated wasm instance, so a connector cannot self-recover via internal
// state (which is the correct sandbox property). Deterministic "fail-then-succeed"
// recovery isn't expressible connector-side on wasm; the relay's retry/backoff +
// exhaustion path is covered by `persistent_transient_exhausts_to_failed` below.

#[tokio::test]
async fn persistent_transient_exhausts_to_failed() {
    let (_tmp, store, relay) = setup();
    let id = relay
        .plan(&spec(
            "Faulty",
            "faulty",
            r#"{"fail":"transient","token":"orch-persist"}"#,
        ))
        .unwrap();

    worker(&relay, &store, 3).run_until_idle().await.unwrap();
    assert_eq!(relay.state(id).unwrap().as_deref(), Some("failed"));
}

#[tokio::test]
async fn fatal_fails_without_retry() {
    let (_tmp, store, relay) = setup();
    let id = relay
        .plan(&spec(
            "Faulty",
            "faulty",
            r#"{"fail":"fatal","token":"orch-fatal"}"#,
        ))
        .unwrap();

    worker(&relay, &store, 5).run_until_idle().await.unwrap();
    assert_eq!(relay.state(id).unwrap().as_deref(), Some("failed"));
}

#[tokio::test]
async fn expired_lease_is_reclaimed() {
    let (_tmp, _store, relay) = setup();
    let id = relay.plan(&spec("Echo", "echo", "{}")).unwrap();

    // Claim with a zero lease → immediately expired.
    let event = relay
        .claim("worker-a", "default", Duration::ZERO)
        .unwrap()
        .unwrap();
    assert_eq!(event.work_unit_id, id);
    assert_eq!(relay.state(id).unwrap().as_deref(), Some("leased"));

    let reclaimed = relay.reclaim_expired().unwrap();
    assert_eq!(reclaimed, 1);
    assert_eq!(relay.state(id).unwrap().as_deref(), Some("pending"));
}

#[tokio::test]
async fn claim_does_not_double_issue() {
    let (_tmp, _store, relay) = setup();
    relay.plan(&spec("Echo", "echo", "{}")).unwrap();

    let first = relay
        .claim("worker-a", "default", Duration::from_secs(30))
        .unwrap();
    let second = relay
        .claim("worker-b", "default", Duration::from_secs(30))
        .unwrap();
    assert!(first.is_some(), "first claim wins the unit");
    assert!(second.is_none(), "no second claimant gets the same unit");
}

#[tokio::test]
async fn partitioned_plan_fans_out_with_per_partition_state_keys() {
    let (_tmp, store, relay) = setup();
    let parts = materialize_partitions(&PartitionScheme::ByKeyShards {
        key: "id".to_string(),
        count: 3,
    });
    assert_eq!(parts.len(), 3, "ByKeyShards count=3 → 3 partitions");

    let ids = relay
        .plan_partitioned(&spec("Echo", "echo", "{}"), &parts)
        .unwrap();
    assert_eq!(ids.len(), 3, "one work unit per partition");

    // Each unit carries its Partition + a distinct per-partition checkpoint key.
    let mut keys = std::collections::BTreeSet::new();
    for id in &ids {
        let s = relay.load(*id).unwrap().unwrap();
        assert!(s.partition.is_some(), "unit carries its Partition");
        keys.insert(s.state_key.clone().unwrap());
    }
    assert_eq!(keys.len(), 3, "independent per-partition state_keys");

    // All partitions drain to done.
    worker(&relay, &store, 1).run_until_idle().await.unwrap();
    for id in &ids {
        assert_eq!(relay.state(*id).unwrap().as_deref(), Some("done"));
    }
}

#[test]
fn claim_is_tenant_isolated() {
    // [[WEIR-T-0091]]: a runner claims ONLY its own tenant's work; active_tenants lists both.
    let (_tmp, _store, relay) = setup();
    let mut a = spec("echo", "sa", "{}");
    a.tenant = "acme".to_string();
    a.connection = "ca".to_string();
    let mut b = spec("echo", "sb", "{}");
    b.tenant = "globex".to_string();
    b.connection = "cb".to_string();
    let id_a = relay.plan(&a).unwrap();
    let id_b = relay.plan(&b).unwrap();

    let mut ts = relay.active_tenants().unwrap();
    ts.sort();
    assert_eq!(ts, vec!["acme".to_string(), "globex".to_string()]);

    // acme's runner gets acme's unit only...
    let ev = relay
        .claim("w", "acme", Duration::from_secs(30))
        .unwrap()
        .expect("acme has work");
    assert_eq!(ev.work_unit_id, id_a);
    // ...and then nothing (globex's unit is never visible to it).
    assert!(
        relay
            .claim("w", "acme", Duration::from_secs(30))
            .unwrap()
            .is_none()
    );
    // globex's runner gets globex's unit.
    let ev = relay
        .claim("w", "globex", Duration::from_secs(30))
        .unwrap()
        .expect("globex has work");
    assert_eq!(ev.work_unit_id, id_b);
}

/// [[WEIR-I-0035]] F1 **Goal 2**: killing the source's runner does not kill the source —
/// its lease expires, `reclaim_expired` returns it to Pending, and a **different** runner
/// re-claims the **same** unit (no pinning) with an incremented attempt (supervised restart).
/// The unit's spec/state_key is unchanged across the reclaim, so the re-run resumes from the
/// last committed checkpoint (checkpoint-intact-on-stop is proven in weir-engine's
/// `run_resident_errs_on_stream_end_leaving_checkpoint_intact`).
#[test]
fn resident_reclaims_on_runner_death_by_a_different_runner() {
    let (_tmp, _store, relay) = setup();
    let mut s = spec("Echo", "echo", "{}");
    s.execution_mode = ExecutionMode::Resident {
        cadence_ms: Some(50),
        restart_backoff_ms: 100,
    };
    let id = relay.plan(&s).unwrap();

    // Runner A claims with a short lease (it is about to die).
    let a = relay
        .claim("runner-a", "default", Duration::from_millis(30))
        .unwrap()
        .expect("runner-a claims the resident unit");
    assert_eq!(a.work_unit_id, id);
    assert_eq!(a.attempt, 1);
    assert_eq!(relay.state(id).unwrap().as_deref(), Some("leased"));

    // While A holds a live lease, no one else can steal it.
    assert!(
        relay
            .claim("runner-b", "default", Duration::from_secs(30))
            .unwrap()
            .is_none(),
        "a live lease is not claimable by another runner"
    );

    // Runner A dies; its lease expires and the sweep returns the unit to Pending.
    std::thread::sleep(Duration::from_millis(45));
    assert_eq!(
        relay.reclaim_expired().unwrap(),
        1,
        "expired lease reclaimed"
    );
    assert_eq!(relay.state(id).unwrap().as_deref(), Some("pending"));

    // A DIFFERENT runner re-claims the SAME unit — supervised restart, not a dead source.
    let b = relay
        .claim("runner-b", "default", Duration::from_secs(30))
        .unwrap()
        .expect("runner-b re-claims the reclaimed resident unit");
    assert_eq!(b.work_unit_id, id, "same unit re-claimed (no pinning)");
    assert_eq!(b.attempt, 2, "attempt incremented on supervised restart");
}
