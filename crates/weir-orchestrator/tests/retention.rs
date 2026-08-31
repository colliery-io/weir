//! [[WEIR-T-0188]]: retention pruning — terminal work_units / run_logs /
//! dead_letters are bounded by an age cap and a per-tenant count cap; in-flight
//! units are never touched; disabled knobs prune nothing. Exercised at the
//! relay level (the serve loop calls `prune_retention` on the leader tick).

use std::sync::Arc;

use diesel::prelude::*;
use weir_engine::Store;
use weir_orchestrator::{Relay, RetentionConfig};
use weir_schema::{dead_letters, run_logs, work_units};

fn relay() -> (tempfile::TempDir, Arc<Store>, Relay) {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Arc::new(Store::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let relay = Relay::new(Arc::clone(&store)).unwrap();
    (tmp, store, relay)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

const DAY_MS: i64 = 86_400_000;

fn insert_unit(store: &Store, id: i64, tenant: &str, state: &str, finished_at: Option<i64>) {
    let mut c = store.pool().get().unwrap();
    diesel::insert_into(work_units::table)
        .values((
            work_units::id.eq(id),
            work_units::tenant_id.eq(tenant),
            work_units::connection.eq("conn"),
            work_units::stream.eq("s"),
            work_units::source_ref.eq("{}"),
            work_units::dest_ref.eq("{}"),
            work_units::state.eq(state),
            work_units::finished_at.eq(finished_at),
        ))
        .execute(&mut c)
        .unwrap();
}

fn db_uuid() -> diesel_dualdb::types::Uuid {
    diesel_dualdb::types::Uuid(uuid::Uuid::new_v4())
}

fn insert_log(store: &Store, _id: &str, tenant: &str, ts: i64) {
    let mut c = store.pool().get().unwrap();
    diesel::insert_into(run_logs::table)
        .values((
            run_logs::id.eq(db_uuid()),
            run_logs::tenant_id.eq(tenant),
            run_logs::connection.eq("conn"),
            run_logs::stream.eq("s"),
            run_logs::level.eq("info"),
            run_logs::message.eq("m"),
            run_logs::ts.eq(ts),
        ))
        .execute(&mut c)
        .unwrap();
}

fn insert_dl(store: &Store, _id: &str, tenant: &str, ts: i64) {
    let mut c = store.pool().get().unwrap();
    diesel::insert_into(dead_letters::table)
        .values((
            dead_letters::id.eq(db_uuid()),
            dead_letters::tenant_id.eq(tenant),
            dead_letters::connection.eq("conn"),
            dead_letters::stream.eq("s"),
            dead_letters::record.eq("{}"),
            dead_letters::reason.eq("r"),
            dead_letters::ts.eq(ts),
        ))
        .execute(&mut c)
        .unwrap();
}

fn unit_ids(store: &Store) -> Vec<i64> {
    let mut c = store.pool().get().unwrap();
    let mut ids: Vec<i64> = work_units::table
        .select(work_units::id)
        .load(&mut c)
        .unwrap();
    ids.sort();
    ids
}

fn log_ts(store: &Store) -> Vec<i64> {
    let mut c = store.pool().get().unwrap();
    let mut ts: Vec<i64> = run_logs::table.select(run_logs::ts).load(&mut c).unwrap();
    ts.sort();
    ts
}

fn dl_ts(store: &Store) -> Vec<i64> {
    let mut c = store.pool().get().unwrap();
    let mut ts: Vec<i64> = dead_letters::table
        .select(dead_letters::ts)
        .load(&mut c)
        .unwrap();
    ts.sort();
    ts
}

#[test]
fn age_cap_prunes_aged_terminal_rows_but_never_in_flight() {
    let (_tmp, store, relay) = relay();
    let now = now_ms();
    let old = now - 40 * DAY_MS;

    insert_unit(&store, 1, "default", "done", Some(old)); // aged terminal → pruned
    insert_unit(&store, 2, "default", "failed", Some(old)); // aged terminal → pruned
    insert_unit(&store, 3, "default", "done", Some(now)); // fresh terminal → kept
    insert_unit(&store, 4, "default", "pending", None); // in-flight → kept
    insert_unit(&store, 5, "default", "leased", None); // in-flight → kept
    insert_log(&store, "l1", "default", old);
    insert_log(&store, "l2", "default", now);
    insert_dl(&store, "d1", "default", old);
    insert_dl(&store, "d2", "default", now);

    let cfg = RetentionConfig {
        max_age_ms: Some(30 * DAY_MS),
        max_rows: None,
    };
    let deleted = relay.prune_retention(&cfg).unwrap();
    assert_eq!(
        deleted, 4,
        "two aged units + one aged log + one aged dead letter"
    );
    assert_eq!(unit_ids(&store), vec![3, 4, 5]);
    assert_eq!(log_ts(&store), vec![now]);
    assert_eq!(dl_ts(&store), vec![now]);
}

#[test]
fn count_cap_keeps_newest_per_tenant() {
    let (_tmp, store, relay) = relay();
    let now = now_ms();

    // Tenant A: 5 terminal units; tenant B: 3 — each capped to 2 independently,
    // oldest-first eviction (ids are monotonic).
    for id in 1..=5 {
        insert_unit(&store, id, "a", "done", Some(now));
    }
    for id in 101..=103 {
        insert_unit(&store, id, "b", "failed", Some(now));
    }
    // An in-flight unit with the LOWEST id must survive the cap.
    insert_unit(&store, 0, "a", "running", None);
    // run_logs for tenant a: 4 entries, cap 2 → the two newest ts survive.
    for (i, ts) in [(1, 10i64), (2, 20), (3, 30), (4, 40)] {
        insert_log(&store, &format!("l{i}"), "a", ts);
    }
    for (i, ts) in [(1, 10i64), (2, 20), (3, 30)] {
        insert_dl(&store, &format!("d{i}"), "a", ts);
    }

    let cfg = RetentionConfig {
        max_age_ms: None,
        max_rows: Some(2),
    };
    relay.prune_retention(&cfg).unwrap();
    assert_eq!(
        unit_ids(&store),
        vec![0, 4, 5, 102, 103],
        "each tenant keeps its 2 newest terminal units; the in-flight unit survives"
    );
    assert_eq!(log_ts(&store), vec![30, 40]);
    assert_eq!(dl_ts(&store), vec![20, 30]);
}

#[test]
fn disabled_knobs_prune_nothing() {
    let (_tmp, store, relay) = relay();
    let old = now_ms() - 400 * DAY_MS;
    for id in 1..=20 {
        insert_unit(&store, id, "default", "done", Some(old));
    }
    insert_log(&store, "l1", "default", old);
    insert_dl(&store, "d1", "default", old);

    let cfg = RetentionConfig {
        max_age_ms: None,
        max_rows: None,
    };
    assert_eq!(relay.prune_retention(&cfg).unwrap(), 0);
    assert_eq!(unit_ids(&store).len(), 20);
    assert_eq!(log_ts(&store).len(), 1);
    assert_eq!(dl_ts(&store).len(), 1);
}

#[test]
fn under_cap_tables_are_untouched() {
    let (_tmp, store, relay) = relay();
    let now = now_ms();
    insert_unit(&store, 1, "default", "done", Some(now));
    insert_log(&store, "l1", "default", now);
    insert_dl(&store, "d1", "default", now);

    let cfg = RetentionConfig {
        max_age_ms: Some(30 * DAY_MS),
        max_rows: Some(10),
    };
    assert_eq!(relay.prune_retention(&cfg).unwrap(), 0);
    assert_eq!(unit_ids(&store), vec![1]);
}
