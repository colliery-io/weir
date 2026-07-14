//! The generated control-plane schema + the dispatched `migrate` runner round-trip
//! on **both** backends ([[WEIR-I-0013]]) — this is the dual-backend proof for the
//! whole portable-store pattern (UUID keys, `Bytes`, `ts`-ordering, the portable
//! update-then-insert upsert). SQLite runs in-memory; Postgres runs only when
//! `DUALDB_PG_URL` is set (e.g. `angreal integration up`).

use diesel::prelude::*;
use diesel_dualdb::DualConnection;
use diesel_dualdb::types::{Bytes, Uuid as DbUuid};
use weir_schema::{
    connections, connectors, dead_letters, outbox, run_logs, stream_state, work_units,
};

fn fresh_id() -> DbUuid {
    DbUuid(uuid::Uuid::new_v4())
}

#[diesel_dualdb::test(pg, sqlite)]
fn control_plane_schema_round_trips(conn: &mut DualConnection) {
    weir_schema::migrate(conn).expect("migrate");
    // Idempotent: a second migrate is a no-op (sentinel).
    weir_schema::migrate(conn).expect("migrate is idempotent");

    // Postgres persists across runs; start from a clean slate for this test's keys.
    diesel::delete(stream_state::table.filter(stream_state::connection.eq("c1")))
        .execute(conn)
        .ok();
    diesel::delete(dead_letters::table.filter(dead_letters::connection.eq("c1")))
        .execute(conn)
        .ok();
    diesel::delete(outbox::table.filter(outbox::connection.eq("c1")))
        .execute(conn)
        .ok();

    // stream_state: insert → portable update-then-insert upsert → Bytes round-trip.
    diesel::insert_into(stream_state::table)
        .values((
            stream_state::connection.eq("c1"),
            stream_state::stream.eq("s1"),
            stream_state::cursor.eq(Some("cur-1")),
            stream_state::opaque.eq(Bytes(vec![0u8, 255, 7])),
        ))
        .execute(conn)
        .expect("insert stream_state");
    let updated = diesel::update(
        stream_state::table.filter(
            stream_state::connection
                .eq("c1")
                .and(stream_state::stream.eq("s1")),
        ),
    )
    .set((
        stream_state::cursor.eq(Some("cur-2")),
        stream_state::opaque.eq(Bytes(vec![1u8, 2, 3])),
    ))
    .execute(conn)
    .expect("update stream_state");
    assert_eq!(updated, 1, "upsert update path hits the existing row");
    let (cur, opa): (Option<String>, Bytes) = stream_state::table
        .select((stream_state::cursor, stream_state::opaque))
        .filter(
            stream_state::connection
                .eq("c1")
                .and(stream_state::stream.eq("s1")),
        )
        .first(conn)
        .expect("select stream_state");
    assert_eq!(cur.as_deref(), Some("cur-2"));
    assert_eq!(opa.0, vec![1u8, 2, 3], "opaque BYTEA/BLOB round-trips");

    // dead_letters: UUID PK + newest-first by ts (the old ORDER BY id DESC).
    for (i, rec) in ["r0", "r1", "r2"].iter().enumerate() {
        diesel::insert_into(dead_letters::table)
            .values((
                dead_letters::id.eq(fresh_id()),
                dead_letters::connection.eq("c1"),
                dead_letters::stream.eq("s1"),
                dead_letters::record.eq(*rec),
                dead_letters::reason.eq("why"),
                dead_letters::ts.eq(i as i64),
            ))
            .execute(conn)
            .expect("insert dead_letter");
    }
    let recent: Vec<String> = dead_letters::table
        .filter(dead_letters::connection.eq("c1"))
        .order(dead_letters::ts.desc())
        .limit(2)
        .select(dead_letters::record)
        .load(conn)
        .expect("recent dead_letters");
    assert_eq!(
        recent,
        vec!["r2".to_string(), "r1".to_string()],
        "newest-first by ts"
    );

    // outbox: UUID PK + the processed count.
    diesel::insert_into(outbox::table)
        .values((
            outbox::id.eq(fresh_id()),
            outbox::connection.eq("c1"),
            outbox::stream.eq("s1"),
            outbox::seq.eq(0i64),
            outbox::processed.eq(1),
        ))
        .execute(conn)
        .expect("insert outbox");
    let n: i64 = outbox::table
        .filter(outbox::connection.eq("c1").and(outbox::processed.eq(1)))
        .count()
        .get_result(conn)
        .expect("outbox count");
    assert_eq!(n, 1);

    // run_logs: UUID PK insert + ts-ordered read.
    diesel::insert_into(run_logs::table)
        .values((
            run_logs::id.eq(fresh_id()),
            run_logs::connection.eq("c1"),
            run_logs::stream.eq("s1"),
            run_logs::level.eq("info"),
            run_logs::message.eq("hello"),
            run_logs::ts.eq(1i64),
        ))
        .execute(conn)
        .expect("insert run_log");
    let msg: String = run_logs::table
        .filter(run_logs::connection.eq("c1"))
        .order(run_logs::ts.desc())
        .select(run_logs::message)
        .first(conn)
        .expect("select run_log");
    assert_eq!(msg, "hello");
}

/// weir-app's tables ([[WEIR-T-0060]]) round-trip on both backends — connections
/// (`every_secs` REAL) + the connector catalog.
#[diesel_dualdb::test(pg, sqlite)]
fn app_tables_round_trip(conn: &mut DualConnection) {
    weir_schema::migrate(conn).expect("migrate");
    diesel::delete(connections::table.filter(connections::name.eq("conn-a")))
        .execute(conn)
        .ok();
    diesel::delete(connectors::table.filter(connectors::name.eq("cat-a")))
        .execute(conn)
        .ok();

    diesel::insert_into(connections::table)
        .values((
            connections::name.eq("conn-a"),
            connections::source_ref.eq("{}"),
            connections::dest_ref.eq("{}"),
            connections::stream.eq("s"),
            connections::source_config.eq("{}"),
            connections::dest_config.eq("{}"),
            connections::every_secs.eq(Some(60.0f32)),
            connections::cron.eq(None::<&str>),
        ))
        .execute(conn)
        .expect("insert connection");
    let every: Option<f32> = connections::table
        .select(connections::every_secs)
        .filter(connections::name.eq("conn-a"))
        .first(conn)
        .expect("select every_secs");
    assert_eq!(every, Some(60.0f32), "every_secs REAL round-trips");

    diesel::insert_into(connectors::table)
        .values((
            connectors::name.eq("cat-a"),
            connectors::version.eq("1.0.0"),
            connectors::roles.eq("[]"),
            connectors::config_schema.eq("{}"),
            connectors::contract_version.eq(1i64),
            connectors::supported_sync_modes.eq("[]"),
            connectors::origin.eq("\"FirstParty\""),
            connectors::status.eq("ready"),
            connectors::location.eq("pkg"),
            connectors::kind.eq("wasm"),
            connectors::manifest.eq(None::<&str>),
            connectors::created_at.eq(1i64),
            connectors::updated_at.eq(1i64),
        ))
        .execute(conn)
        .expect("insert connector");
    let kind: String = connectors::table
        .select(connectors::kind)
        .filter(
            connectors::name
                .eq("cat-a")
                .and(connectors::version.eq("1.0.0")),
        )
        .first(conn)
        .expect("select connector kind");
    assert_eq!(kind, "wasm");
}

/// weir-orchestrator's `work_units` ([[WEIR-T-0061]]): a subset insert (defaults
/// fill the rest) + the portable select-then-guarded-UPDATE claim, on both backends.
#[diesel_dualdb::test(pg, sqlite)]
fn work_units_claim_round_trips(conn: &mut DualConnection) {
    weir_schema::migrate(conn).expect("migrate");
    diesel::delete(work_units::table.filter(work_units::connection.eq("wq")))
        .execute(conn)
        .ok();

    let id = 42_000_001i64;
    diesel::insert_into(work_units::table)
        .values((
            work_units::id.eq(id),
            work_units::connection.eq("wq"),
            work_units::stream.eq("{}"),
            work_units::source_ref.eq("{}"),
            work_units::dest_ref.eq("{}"),
            work_units::source_config.eq("{}"),
            work_units::dest_config.eq("{}"),
            work_units::state.eq("pending"),
        ))
        .execute(conn)
        .expect("insert work_unit (defaults fill attempt/next_attempt_at/...)");

    // Claim: lowest-id due pending → guarded UPDATE to leased (the atomic lock).
    let cand: Option<(i64, i64)> = work_units::table
        .filter(
            work_units::state
                .eq("pending")
                .and(work_units::next_attempt_at.le(0i64)),
        )
        .order(work_units::id.asc())
        .select((work_units::id, work_units::attempt))
        .first(conn)
        .optional()
        .expect("candidate");
    let (cid, _) = cand.expect("a pending candidate");
    let claimed = diesel::update(
        work_units::table.filter(work_units::id.eq(cid).and(work_units::state.eq("pending"))),
    )
    .set((
        work_units::state.eq("leased"),
        work_units::attempt.eq(work_units::attempt + 1),
    ))
    .execute(conn)
    .expect("claim update");
    assert_eq!(claimed, 1, "guarded UPDATE claims exactly one");

    let again: Option<i64> = work_units::table
        .filter(work_units::state.eq("pending"))
        .select(work_units::id)
        .first(conn)
        .optional()
        .expect("second claim");
    assert_eq!(again, None, "nothing left to claim");
    let st: String = work_units::table
        .filter(work_units::id.eq(id))
        .select(work_units::state)
        .first(conn)
        .expect("state");
    assert_eq!(st, "leased");
}

/// The derive-based connection store ([[WEIR-I-0030]]) round-trips on **both** backends:
/// `Insertable`, `Selectable`/`as_select`, and `AsChangeset` (the portable update-then-insert
/// upsert's update arm) all compose over `DualConnection` — not just SQLite. Mirrors weir-app's
/// (crate-private) `ConnectionRow` with a test-local twin so the pattern is proven on Postgres.
#[derive(Queryable, Selectable, Insertable, AsChangeset, Identifiable, PartialEq, Debug)]
#[diesel(table_name = connections, primary_key(tenant_id, name), treat_none_as_null = true)]
struct ConnRow {
    tenant_id: String,
    name: String,
    source_ref: String,
    dest_ref: String,
    stream: String,
    source_config: String,
    dest_config: String,
    every_secs: Option<f32>,
    cron: Option<String>,
    sync_mode: String,
    write_mode: String,
    business_keys: Option<String>,
    cursor_field: Option<String>,
}

#[diesel_dualdb::test(pg, sqlite)]
fn connection_row_derives_round_trip(conn: &mut DualConnection) {
    weir_schema::migrate(conn).expect("migrate");
    // Postgres persists across runs; clear this test's tenant first.
    diesel::delete(connections::table.filter(connections::tenant_id.eq("t-i0030")))
        .execute(conn)
        .ok();

    let mut row = ConnRow {
        tenant_id: "t-i0030".into(),
        name: "c".into(),
        source_ref: "{}".into(),
        dest_ref: "{}".into(),
        stream: "s".into(),
        source_config: "{\"a\":1}".into(),
        dest_config: "{}".into(),
        every_secs: Some(1.5),
        cron: None,
        sync_mode: "full_refresh".into(),
        write_mode: "append".into(),
        business_keys: None,
        cursor_field: None,
    };

    // Insert via `Insertable`, read back via `Selectable`/`as_select` — full round-trip.
    diesel::insert_into(connections::table)
        .values(&row)
        .execute(conn)
        .expect("insert");
    let got: ConnRow = connections::table
        .filter(
            connections::tenant_id
                .eq("t-i0030")
                .and(connections::name.eq("c")),
        )
        .select(ConnRow::as_select())
        .first(conn)
        .expect("select");
    assert_eq!(got, row, "row round-trips unchanged");

    // Update a non-key column via `AsChangeset` (the upsert's update arm); the key is skipped.
    row.dest_config = "{\"b\":2}".into();
    let n = diesel::update(
        connections::table.filter(
            connections::tenant_id
                .eq("t-i0030")
                .and(connections::name.eq("c")),
        ),
    )
    .set(&row)
    .execute(conn)
    .expect("update");
    assert_eq!(n, 1, "AsChangeset updates exactly the keyed row");
    let got2: ConnRow = connections::table
        .filter(
            connections::tenant_id
                .eq("t-i0030")
                .and(connections::name.eq("c")),
        )
        .select(ConnRow::as_select())
        .first(conn)
        .expect("reselect");
    assert_eq!(
        got2.dest_config, "{\"b\":2}",
        "AsChangeset wrote the non-key column"
    );
}

/// The orchestrator's derive-based work-unit store ([[WEIR-I-0030]] / [[WEIR-T-0134]]) selects the
/// `WorkSpec` subset via `Selectable`/`as_select` over `DualConnection` — proven on both backends.
/// Mirrors the (crate-private) `WorkSpecRow` with a test-local twin.
#[derive(Queryable, Selectable, PartialEq, Debug)]
#[diesel(table_name = work_units)]
struct SpecRow {
    tenant_id: String,
    connection: String,
    stream: String,
    source_ref: String,
    dest_ref: String,
    source_config: String,
    dest_config: String,
    state_key: Option<String>,
    seed_cursor: Option<String>,
    partition: String,
}

#[diesel_dualdb::test(pg, sqlite)]
fn work_unit_spec_row_round_trips(conn: &mut DualConnection) {
    weir_schema::migrate(conn).expect("migrate");
    let id = 300_030_i64;
    diesel::delete(work_units::table.filter(work_units::id.eq(id)))
        .execute(conn)
        .ok();

    diesel::insert_into(work_units::table)
        .values((
            work_units::id.eq(id),
            work_units::tenant_id.eq("t-i0030"),
            work_units::connection.eq("cx"),
            work_units::stream.eq("{\"s\":1}"),
            work_units::source_ref.eq("{\"src\":1}"),
            work_units::dest_ref.eq("{\"dst\":1}"),
            work_units::source_config.eq("{\"a\":1}"),
            work_units::dest_config.eq("{\"b\":2}"),
            work_units::state.eq("pending"),
            work_units::state_key.eq(Some("k")),
            work_units::seed_cursor.eq(None::<&str>),
            work_units::partition.eq("null"),
        ))
        .execute(conn)
        .expect("insert work unit");

    let got: SpecRow = work_units::table
        .filter(work_units::id.eq(id))
        .select(SpecRow::as_select())
        .first(conn)
        .expect("select spec row");
    assert_eq!(got.tenant_id, "t-i0030");
    assert_eq!(got.source_config, "{\"a\":1}");
    assert_eq!(got.dest_config, "{\"b\":2}");
    assert_eq!(got.state_key.as_deref(), Some("k"));
    assert_eq!(got.seed_cursor, None);
    assert_eq!(got.partition, "null");
}
