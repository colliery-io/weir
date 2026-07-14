//! WEIR-T-0018: the lease heartbeat. Tested deterministically at the relay
//! level (no wall-clock races): a unit claimed with a zero-length lease sits at
//! the expiry boundary, so `reclaim_expired` would take it — unless a heartbeat
//! has extended the lease. This is exactly the in-flight situation `Worker::tick`
//! creates while a sync longer than the lease executes.

mod common;
use common::wasm_ref;

use std::sync::Arc;
use std::time::Duration;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::Store;
use weir_orchestrator::{Relay, WorkSpec};

fn spec() -> WorkSpec {
    WorkSpec {
        connection: "hb".to_string(),
        tenant: "default".to_string(),
        stream: ConfiguredStream {
            stream: "s".to_string(),
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

fn relay() -> (tempfile::TempDir, Relay) {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Arc::new(Store::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let relay = Relay::new(Arc::clone(&store)).unwrap();
    (tmp, relay)
}

#[test]
fn heartbeat_extends_lease_so_it_is_not_reclaimed() {
    let (_tmp, relay) = relay();
    relay.plan(&spec()).unwrap();
    // Zero-length lease → the unit is immediately at its expiry boundary.
    let ev = relay
        .claim("w", "default", Duration::ZERO)
        .unwrap()
        .expect("claimed");

    // Heartbeat pushes the expiry well into the future.
    assert!(
        relay
            .heartbeat(ev.work_unit_id, "w", Duration::from_secs(60))
            .unwrap()
    );
    assert_eq!(
        relay.reclaim_expired().unwrap(),
        0,
        "a heartbeated lease must not be reclaimed"
    );
}

#[test]
fn without_a_heartbeat_the_expired_lease_is_reclaimed() {
    let (_tmp, relay) = relay();
    relay.plan(&spec()).unwrap();
    relay
        .claim("w", "default", Duration::ZERO)
        .unwrap()
        .expect("claimed");

    // No heartbeat → the expired lease is reclaimable (the gap this closes).
    assert_eq!(relay.reclaim_expired().unwrap(), 1);
}

#[test]
fn heartbeat_is_a_noop_once_the_lease_is_lost() {
    let (_tmp, relay) = relay();
    let id = relay.plan(&spec()).unwrap();
    relay
        .claim("w", "default", Duration::ZERO)
        .unwrap()
        .expect("claimed");
    assert_eq!(
        relay.reclaim_expired().unwrap(),
        1,
        "reclaimed back to pending"
    );

    // The original owner's heartbeat must not resurrect a lease it no longer holds.
    assert!(!relay.heartbeat(id, "w", Duration::from_secs(60)).unwrap());
}
