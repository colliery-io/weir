//! WEIR-I-0011 S4 + parity (T-0046): the `postgres` connector as a wasm guest doing
//! real TCP to a live Postgres — source (FullRefresh / Incremental) + destination
//! (Append / Upsert), driven through the engine. `#[ignore]`: needs the integration
//! Postgres (`docker compose up -d --wait`). Run with
//! `cargo test -p weir-engine --test wasm_postgres_engine -- --ignored --test-threads=1`.

use std::net::SocketAddr;
use std::path::Path;

use weir_connector::fidius::{EgressDenied, EgressPolicy};
use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::{Engine, Store};
use weir_runtime::ConnectorHandle;

/// The Postgres URL under test — `WEIR_TEST_PG_URL` (so the harness follows weir's compose stack when
/// its host port is remapped off 5432), else the integration default. `sslmode=disable` because the
/// integration Postgres is plaintext; the connector's default is `require` ([[WEIR-A-0041]]) and the
/// TLS wire gates live in the postgres-tls harness ([[WEIR-T-0178]]).
fn pg_url() -> String {
    std::env::var("WEIR_TEST_PG_URL")
        .unwrap_or_else(|_| "postgres://weir:weir@localhost:5432/weir?sslmode=disable".to_string())
}

/// The URL for the harness's RAW `postgres::Client` (seed/verify): query stripped,
/// because the raw client runs `NoTls` regardless of the connector's sslmode —
/// letting the suite run unchanged against both plaintext and TLS servers.
fn raw_pg_url() -> String {
    pg_url().split('?').next().unwrap().to_string()
}

/// The port from `pg_url()` (for the egress allow-list).
fn pg_port() -> u16 {
    pg_url()
        .rsplit(':')
        .next()
        .and_then(|tail| tail.split('/').next())
        .and_then(|p| p.parse().ok())
        .unwrap_or(5432)
}

struct PgEgress(u16);
impl EgressPolicy for PgEgress {
    fn authorize(
        &self,
        _parts: &mut weir_connector::fidius::http_types::request::Parts,
    ) -> Result<(), EgressDenied> {
        Err(EgressDenied::new(
            "postgres connector does not use http egress",
        ))
    }
    fn authorize_tcp(&self, addr: &SocketAddr) -> Result<(), EgressDenied> {
        if addr.port() == self.0 {
            Ok(())
        } else {
            Err(EgressDenied::new(format!("tcp to {addr} not allowed")))
        }
    }
}

fn pg(root: &Path) -> ConnectorHandle {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/connectors/postgres");
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &fixture,
            wasm_file: "weir_postgres_wasm.wasm",
            pkg_name: "weir-postgres-pkg",
            capabilities: &["tcp"],
        },
        root,
    );
    // `WEIR_TEST_PG_SSLROOTCERT` (a PEM file path) feeds the connector's inline-PEM
    // `sslrootcert` — the TLS harness ([[WEIR-T-0178]]) uses it for verify-full runs.
    let sslrootcert = std::env::var("WEIR_TEST_PG_SSLROOTCERT")
        .ok()
        .map(|p| std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {p}: {e}")));
    let json = match sslrootcert {
        Some(pem) => format!(
            "{{\"url\":\"{}\",\"sslrootcert\":{}}}",
            pg_url(),
            serde_json::to_string(&pem).unwrap()
        ),
        None => format!("{{\"url\":\"{}\"}}", pg_url()),
    };
    let cfg = Config { json };
    ConnectorHandle::from_wasm_package(root, "weir-postgres-pkg", &cfg, PgEgress(pg_port()), &[])
        .expect("load postgres wasm")
}

/// A `slow` source emitting `{ "n": 1.. }` in one batch — the controllable input.
fn slow(rows: u64) -> ConnectorHandle {
    let cfg = Config {
        json: format!("{{\"sleep_ms\":0,\"rows\":{rows},\"batch\":true}}"),
    };
    weir_wasm_testkit::load("Slow", &cfg).expect("slow")
}

fn arrow() -> ConnectorHandle {
    weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .expect("arrow")
}

fn cstream(name: &str, sync: SyncMode, write: WriteMode, cursor: Option<&str>) -> ConfiguredStream {
    ConfiguredStream {
        stream: name.to_string(),
        sync_mode: sync,
        cursor_field: cursor.map(str::to_string),
        primary_key: None,
        write_mode: write,
        mapping: MappingSpec::default(),
    }
}

/// Drop leftover destination tables (raw client) so layout changes between runs
/// ([[WEIR-T-0182]] typed columns vs legacy JSONB) can't poison assertions.
fn clean_tables(tables: &[&str]) {
    let mut sql = postgres::Client::connect(&raw_pg_url(), postgres::NoTls).expect("raw pg");
    for t in tables {
        let _ = sql.batch_execute(&format!("DROP TABLE IF EXISTS {t}"));
    }
}

fn store(tmp: &Path, name: &str) -> Store {
    Store::open(tmp.join(format!("{name}.db")).to_str().unwrap()).expect("store")
}

#[test]
#[ignore = "needs the integration Postgres (docker compose up)"]
fn pg_wasm_append_then_fullrefresh_roundtrip() {
    clean_tables(&["rt"]);
    let tmp = tempfile::TempDir::new().unwrap();
    let pg = pg(tmp.path());

    // Write 3 rows (Append) into table "rt".
    let st = cstream("rt", SyncMode::FullRefresh, WriteMode::Append, None);
    Engine::new(&store(tmp.path(), "w"))
        .sync("w", &st, &slow(3), &pg)
        .expect("append write");

    // Read them back (FullRefresh) → arrow-sink counts 3.
    let out = Engine::new(&store(tmp.path(), "r"))
        .sync("r", &st, &pg, &arrow())
        .expect("fullrefresh read");
    assert_eq!(
        out.rows_written, 3,
        "3 rows round-tripped through postgres-wasm"
    );
}

#[test]
#[ignore = "needs the integration Postgres (docker compose up)"]
fn pg_wasm_upsert_is_idempotent() {
    clean_tables(&["up"]);
    let tmp = tempfile::TempDir::new().unwrap();
    let pg = pg(tmp.path());
    let up = cstream(
        "up",
        SyncMode::FullRefresh,
        WriteMode::Upsert {
            business_keys: vec!["n".to_string()],
        },
        None,
    );

    // Upsert the same 3 rows twice — idempotent under at-least-once re-delivery.
    for tag in ["a", "b"] {
        Engine::new(&store(tmp.path(), tag))
            .sync(tag, &up, &slow(3), &pg)
            .expect("upsert write");
    }
    let out = Engine::new(&store(tmp.path(), "r"))
        .sync("r", &up, &pg, &arrow())
        .expect("read");
    assert_eq!(
        out.rows_written, 3,
        "upsert keeps 3 rows (not 6) — idempotent"
    );
}

#[test]
#[ignore = "needs the integration Postgres (docker compose up)"]
fn pg_wasm_incremental_advances_cursor() {
    clean_tables(&["inc"]);
    let tmp = tempfile::TempDir::new().unwrap();
    let pg = pg(tmp.path());

    // Seed a table that has a top-level `n` column (Upsert creates `n TEXT`).
    let up = cstream(
        "inc",
        SyncMode::FullRefresh,
        WriteMode::Upsert {
            business_keys: vec!["n".to_string()],
        },
        None,
    );
    Engine::new(&store(tmp.path(), "seed"))
        .sync("seed", &up, &slow(3), &pg)
        .expect("seed");

    // Incremental read by `n` → 3 rows, cursor advances to "3".
    let inc = cstream("inc", SyncMode::Incremental, WriteMode::Append, Some("n"));
    let out = Engine::new(&store(tmp.path(), "inc"))
        .sync("inc", &inc, &pg, &arrow())
        .expect("incremental read");
    assert_eq!(out.rows_written, 3);
    assert_eq!(
        out.final_state.cursor.as_deref(),
        Some("3"),
        "cursor advanced to max n"
    );
}

#[test]
#[ignore = "needs the integration Postgres (docker compose up; wal_level=logical)"]
fn pg_wasm_cdc_captures_inserts() {
    clean_tables(&["cdc_t"]);
    let tmp = tempfile::TempDir::new().unwrap();
    let pg = pg(tmp.path());
    let cdc = cstream("cdc_t", SyncMode::Cdc, WriteMode::Append, None);
    let st = store(tmp.path(), "cdc"); // one store: run 2 resumes run 1's slot

    // Run 1: creates the logical-replication slot (captures WAL from here on).
    Engine::new(&st)
        .sync("cdc", &cdc, &pg, &arrow())
        .expect("cdc init");

    // Insert 2 rows into cdc_t (Append) — WAL the slot now captures.
    let ap = cstream("cdc_t", SyncMode::FullRefresh, WriteMode::Append, None);
    Engine::new(&store(tmp.path(), "seed"))
        .sync("seed", &ap, &slow(2), &pg)
        .expect("seed inserts");

    // Run 2: resume the slot → see the captured INSERT changes.
    let out = Engine::new(&st)
        .sync("cdc", &cdc, &pg, &arrow())
        .expect("cdc read");
    assert!(
        out.rows_written > 0,
        "cdc captured the inserts (got {})",
        out.rows_written
    );
}

#[test]
#[ignore = "needs the integration Postgres (angreal integration up)"]
fn pg_wasm_incremental_resume_delta() {
    clean_tables(&["incd"]);
    // [[WEIR-T-0107]] fidelity: a resumed incremental sync returns ONLY the new rows — no
    // re-emission of what the persisted cursor already covered.
    let tmp = tempfile::TempDir::new().unwrap();
    let pg = pg(tmp.path());
    let seed = cstream(
        "incd",
        SyncMode::FullRefresh,
        WriteMode::Upsert {
            business_keys: vec!["n".to_string()],
        },
        None,
    );
    // Seed {1,2,3}.
    Engine::new(&store(tmp.path(), "seed"))
        .sync("seed", &seed, &slow(3), &pg)
        .expect("seed 3");

    // Incremental read #1 on a persistent store → 3 rows, cursor "3".
    let rd = store(tmp.path(), "rd");
    let inc = cstream("incd", SyncMode::Incremental, WriteMode::Append, Some("n"));
    let o1 = Engine::new(&rd)
        .sync("rd", &inc, &pg, &arrow())
        .expect("read 1");
    assert_eq!(o1.rows_written, 3);
    assert_eq!(o1.final_state.cursor.as_deref(), Some("3"));

    // Grow the table to {1..5} (upsert is idempotent on the first 3).
    Engine::new(&store(tmp.path(), "grow"))
        .sync("grow", &seed, &slow(5), &pg)
        .expect("grow to 5");

    // Incremental read #2 on the SAME store (cursor persisted at "3") → only {4,5}.
    let o2 = Engine::new(&rd)
        .sync("rd", &inc, &pg, &arrow())
        .expect("read 2");
    assert_eq!(
        o2.rows_written, 2,
        "only the 2 new rows — no re-emission of 1..3"
    );
    assert_eq!(
        o2.final_state.cursor.as_deref(),
        Some("5"),
        "cursor advanced to 5"
    );
}

#[test]
#[ignore = "needs the integration Postgres (angreal integration up)"]
fn pg_wasm_incremental_no_dupes_no_gaps() {
    clean_tables(&["ndg"]);
    // [[WEIR-T-0107]] fidelity: across interleaved grow+resume rounds, each distinct source row is
    // delivered EXACTLY once — the sum over resumes equals the distinct count (dupes → more,
    // gaps → fewer).
    let tmp = tempfile::TempDir::new().unwrap();
    let pg = pg(tmp.path());
    let seed = cstream(
        "ndg",
        SyncMode::FullRefresh,
        WriteMode::Upsert {
            business_keys: vec!["n".to_string()],
        },
        None,
    );
    let rd = store(tmp.path(), "rd");
    let inc = cstream("ndg", SyncMode::Incremental, WriteMode::Append, Some("n"));

    let mut delivered = 0u64;
    // Round 1: {1..4} → read.
    Engine::new(&store(tmp.path(), "w1"))
        .sync("w1", &seed, &slow(4), &pg)
        .expect("w1");
    delivered += Engine::new(&rd)
        .sync("rd", &inc, &pg, &arrow())
        .expect("r1")
        .rows_written;
    // Round 2: grow to {1..7} → read (only 5,6,7).
    Engine::new(&store(tmp.path(), "w2"))
        .sync("w2", &seed, &slow(7), &pg)
        .expect("w2");
    delivered += Engine::new(&rd)
        .sync("rd", &inc, &pg, &arrow())
        .expect("r2")
        .rows_written;
    // Round 3: no growth → read (nothing new).
    let o3 = Engine::new(&rd)
        .sync("rd", &inc, &pg, &arrow())
        .expect("r3");
    delivered += o3.rows_written;

    assert_eq!(
        delivered, 7,
        "7 distinct rows delivered exactly once across 3 resumes"
    );
    assert_eq!(o3.final_state.cursor.as_deref(), Some("7"));
}

#[test]
#[ignore = "needs the integration Postgres (angreal integration up; wal_level=logical)"]
fn pg_wasm_cdc_insert_update_delete_advances_in_order() {
    clean_tables(&["cdc_iud"]);
    // [[WEIR-T-0107]] fidelity: the logical-slot reader captures INSERT→UPDATE→DELETE in commit
    // order and ADVANCES the slot each read — so a replay double-delivers nothing. A raw client
    // drives one mutation at a time; each CDC read must consume exactly that change (and not
    // re-see the earlier ones), which proves both ordering and slot advance without a payload sink.
    let tmp = tempfile::TempDir::new().unwrap();
    let pg = pg(tmp.path());
    let mut sql = postgres::Client::connect(&raw_pg_url(), postgres::NoTls).expect("raw pg client");
    sql.batch_execute(
        "DROP TABLE IF EXISTS cdc_iud; \
         CREATE TABLE cdc_iud (id INT PRIMARY KEY, v TEXT); \
         ALTER TABLE cdc_iud REPLICA IDENTITY FULL;",
    )
    .expect("setup table");

    let cdc = cstream("cdc_iud", SyncMode::Cdc, WriteMode::Append, None);
    let st = store(tmp.path(), "cdc"); // one store → the slot (in opaque state) persists across reads

    // Create the slot before any change (captures WAL from here on).
    Engine::new(&st)
        .sync("cdc", &cdc, &pg, &arrow())
        .expect("cdc slot init");

    // INSERT → the next read captures it.
    sql.execute("INSERT INTO cdc_iud VALUES (1, 'a')", &[])
        .expect("insert");
    let ins = Engine::new(&st)
        .sync("cdc", &cdc, &pg, &arrow())
        .expect("read insert")
        .rows_written;
    assert!(ins > 0, "INSERT captured (got {ins})");

    // UPDATE → captured next; the INSERT is NOT re-delivered (slot already advanced past it).
    sql.execute("UPDATE cdc_iud SET v = 'b' WHERE id = 1", &[])
        .expect("update");
    let upd = Engine::new(&st)
        .sync("cdc", &cdc, &pg, &arrow())
        .expect("read update")
        .rows_written;
    assert!(
        upd > 0,
        "UPDATE captured after the INSERT was already consumed (got {upd})"
    );

    // DELETE → captured next.
    sql.execute("DELETE FROM cdc_iud WHERE id = 1", &[])
        .expect("delete");
    let del = Engine::new(&st)
        .sync("cdc", &cdc, &pg, &arrow())
        .expect("read delete")
        .rows_written;
    assert!(del > 0, "DELETE captured (got {del})");

    // Replay with no new changes → the slot fully advanced; nothing double-delivered.
    let replay = Engine::new(&st)
        .sync("cdc", &cdc, &pg, &arrow())
        .expect("replay")
        .rows_written;
    assert_eq!(replay, 0, "slot advanced — replay double-delivers nothing");
}

#[test]
#[ignore = "needs the integration Postgres (angreal integration up)"]
fn pg_wasm_partition_checkpoints_are_isolated() {
    clean_tables(&["parts"]);
    // [[WEIR-T-0107]] fidelity: key-shard partitions read DISJOINT slices and checkpoint
    // INDEPENDENTLY — the 2 shards together cover every row exactly once, and advancing one shard's
    // cursor (its own state_key) leaves the other's untouched.
    let tmp = tempfile::TempDir::new().unwrap();
    let pg = pg(tmp.path());

    // Seed {1..12} into "parts".
    let seed = cstream(
        "parts",
        SyncMode::FullRefresh,
        WriteMode::Upsert {
            business_keys: vec!["n".to_string()],
        },
        None,
    );
    Engine::new(&store(tmp.path(), "seed"))
        .sync("seed", &seed, &slow(12), &pg)
        .expect("seed 12");

    // Two hash shards over key "n" (the bounds shape `materialize_partitions` emits).
    let shard = |i: u32| weir_connector::Partition {
        id: format!("shard-{i}"),
        bounds: Some(format!("{{\"key\":\"n\",\"shard\":{i},\"of\":2}}")),
    };
    let inc = cstream("parts", SyncMode::Incremental, WriteMode::Append, Some("n"));
    let st = store(tmp.path(), "rd"); // one store; distinct state_key per shard isolates checkpoints
    let read_shard = |part: weir_connector::Partition, key: &str| -> u64 {
        let opts = weir_engine::SyncOptions {
            state_key: Some(key.to_string()),
            partition: Some(part),
            ..Default::default()
        };
        Engine::new(&st)
            .sync_with("rd", &inc, &pg, &arrow(), &opts, &mut |_| {})
            .expect("shard read")
            .rows_written
    };

    // First pass: the shards partition all 12 rows with no overlap and no gaps.
    let s0 = read_shard(shard(0), "part-0");
    let s1 = read_shard(shard(1), "part-1");
    assert_eq!(
        s0 + s1,
        12,
        "2 shards cover all 12 rows exactly once (s0={s0}, s1={s1})"
    );
    assert!(s0 > 0 && s1 > 0, "both shards non-empty (s0={s0}, s1={s1})");

    // Isolation: each shard advanced its OWN checkpoint → a re-read yields nothing new, independently.
    assert_eq!(
        read_shard(shard(0), "part-0"),
        0,
        "shard 0 checkpoint advanced independently"
    );
    assert_eq!(
        read_shard(shard(1), "part-1"),
        0,
        "shard 1 checkpoint advanced independently"
    );
}

/// A postgres handle with extra config beyond `url` (e.g. `"table":"x","on_delete":"hard"`).
fn pg_with(root: &Path, extra: &str) -> ConnectorHandle {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/connectors/postgres");
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &fixture,
            wasm_file: "weir_postgres_wasm.wasm",
            pkg_name: "weir-postgres-pkg",
            capabilities: &["tcp"],
        },
        root,
    );
    let cfg = Config {
        json: format!("{{\"url\":\"{}\",{extra}}}", pg_url()),
    };
    ConnectorHandle::from_wasm_package(root, "weir-postgres-pkg", &cfg, PgEgress(pg_port()), &[])
        .expect("load pg")
}

fn dst_count(sql: &mut postgres::Client, table: &str, id: &str) -> i64 {
    sql.query_one(
        &format!("SELECT count(*) FROM {table} WHERE id = $1"),
        &[&id],
    )
    .unwrap()
    .get(0)
}

#[test]
#[ignore = "needs the integration Postgres (angreal integration up; wal_level=logical)"]
fn cdc_hard_delete_propagates_pg_to_pg() {
    // [[WEIR-T-0117]]: CDC a source row's INSERT→UPDATE→DELETE → the dest row appears, updates, then
    // is HARD-deleted; a replay changes nothing (slot advanced). Source filters to its own table so
    // the dest's writes don't feed back.
    let tmp = tempfile::TempDir::new().unwrap();
    let mut sql = postgres::Client::connect(&raw_pg_url(), postgres::NoTls).expect("raw pg");
    sql.batch_execute(
        "DROP TABLE IF EXISTS cdc_hsrc; DROP TABLE IF EXISTS cdc_hdst; \
         CREATE TABLE cdc_hsrc (id INT PRIMARY KEY, v TEXT); ALTER TABLE cdc_hsrc REPLICA IDENTITY FULL;",
    )
    .expect("setup");

    let src = pg_with(tmp.path(), "\"table\":\"cdc_hsrc\"");
    let dst = pg_with(tmp.path(), "\"table\":\"cdc_hdst\",\"on_delete\":\"hard\"");
    let cdc = cstream(
        "cdc_hsrc",
        SyncMode::Cdc,
        WriteMode::Upsert {
            business_keys: vec!["id".to_string()],
        },
        None,
    );
    let st = store(tmp.path(), "cdc"); // persists the slot across syncs

    let sync = |st: &Store| {
        Engine::new(st)
            .sync("cdc", &cdc, &src, &dst)
            .expect("cdc sync")
    };
    sync(&st); // create the slot before any change

    sql.execute("INSERT INTO cdc_hsrc VALUES (1, 'a')", &[])
        .unwrap();
    sync(&st);
    assert_eq!(dst_count(&mut sql, "cdc_hdst", "1"), 1, "insert propagated");

    sql.execute("UPDATE cdc_hsrc SET v = 'b' WHERE id = 1", &[])
        .unwrap();
    sync(&st);
    // Typed destination ([[WEIR-T-0182]]): `v` is a real column, not data->>'v'.
    let v: String = sql
        .query_one("SELECT v::text FROM cdc_hdst WHERE id::text = '1'", &[])
        .unwrap()
        .get(0);
    assert_eq!(v, "b", "update propagated");

    sql.execute("DELETE FROM cdc_hsrc WHERE id = 1", &[])
        .unwrap();
    sync(&st);
    assert_eq!(
        dst_count(&mut sql, "cdc_hdst", "1"),
        0,
        "delete propagated — row gone"
    );

    sync(&st); // replay: no new changes
    assert_eq!(
        dst_count(&mut sql, "cdc_hdst", "1"),
        0,
        "replay double-applies nothing"
    );
}

#[test]
#[ignore = "needs the integration Postgres (angreal integration up; wal_level=logical)"]
fn cdc_tombstone_delete_propagates_pg_to_pg() {
    // [[WEIR-T-0117]]: with on_delete=tombstone, a delete leaves the row + stamps the tombstone column.
    let tmp = tempfile::TempDir::new().unwrap();
    let mut sql = postgres::Client::connect(&raw_pg_url(), postgres::NoTls).expect("raw pg");
    sql.batch_execute(
        "DROP TABLE IF EXISTS cdc_tsrc; DROP TABLE IF EXISTS cdc_tdst; \
         CREATE TABLE cdc_tsrc (id INT PRIMARY KEY, v TEXT); ALTER TABLE cdc_tsrc REPLICA IDENTITY FULL;",
    )
    .expect("setup");

    let src = pg_with(tmp.path(), "\"table\":\"cdc_tsrc\"");
    let dst = pg_with(
        tmp.path(),
        "\"table\":\"cdc_tdst\",\"on_delete\":\"tombstone\"",
    );
    let cdc = cstream(
        "cdc_tsrc",
        SyncMode::Cdc,
        WriteMode::Upsert {
            business_keys: vec!["id".to_string()],
        },
        None,
    );
    let st = store(tmp.path(), "cdc");
    let sync = |st: &Store| {
        Engine::new(st)
            .sync("cdc", &cdc, &src, &dst)
            .expect("cdc sync")
    };
    sync(&st);

    sql.execute("INSERT INTO cdc_tsrc VALUES (1, 'a')", &[])
        .unwrap();
    sync(&st);
    sql.execute("DELETE FROM cdc_tsrc WHERE id = 1", &[])
        .unwrap();
    sync(&st);

    // Row remains, tombstone stamped.
    assert_eq!(
        dst_count(&mut sql, "cdc_tdst", "1"),
        1,
        "tombstone keeps the row"
    );
    let tomb: Option<String> = sql
        .query_one("SELECT _deleted_at FROM cdc_tdst WHERE id = '1'", &[])
        .unwrap()
        .get(0);
    assert!(tomb.is_some(), "tombstone column stamped");
}

/// A minimal HTTP stub: 200s everything, records each request's (method, path). Returns its base URL.
fn start_stub() -> (
    String,
    std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let log2 = std::sync::Arc::clone(&log);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut buf = [0u8; 2048];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            if let Some(line) = req.lines().next() {
                let mut p = line.split_whitespace();
                let method = p.next().unwrap_or("").to_string();
                let path = p.next().unwrap_or("").to_string();
                log2.lock().unwrap().push((method, path));
            }
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
        }
    });
    (format!("http://127.0.0.1:{port}"), log)
}

fn rest_dest(root: &Path, base_url: &str) -> ConnectorHandle {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/connectors/rest-dest");
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &fixture,
            wasm_file: "weir_rest_dest_wasm.wasm",
            pkg_name: "weir-rest-dest-pkg",
            capabilities: &["http"],
        },
        root,
    );
    let path = "/items/{{ record.id }}";
    let cfg = Config {
        json: format!(
            r#"{{"base_url":"{base_url}","path":"{path}","delete_path":"{path}","method":"POST"}}"#
        ),
    };
    ConnectorHandle::from_wasm_package(
        root,
        "weir-rest-dest-pkg",
        &cfg,
        weir_runtime::HostAllowList::allow_all(),
        &[],
    )
    .expect("load rest-dest")
}

#[test]
#[ignore = "needs the integration Postgres (angreal integration up; wal_level=logical)"]
fn cdc_delete_propagates_to_rest_dest() {
    // [[WEIR-T-0117]]: a CDC delete → the rest-dest issues an HTTP DELETE to the key-templated URL.
    let tmp = tempfile::TempDir::new().unwrap();
    let mut sql = postgres::Client::connect(&raw_pg_url(), postgres::NoTls).expect("raw pg");
    sql.batch_execute(
        "DROP TABLE IF EXISTS cdc_rsrc; \
         CREATE TABLE cdc_rsrc (id INT PRIMARY KEY, v TEXT); ALTER TABLE cdc_rsrc REPLICA IDENTITY FULL;",
    )
    .expect("setup");

    let (base_url, log) = start_stub();
    let src = pg_with(tmp.path(), "\"table\":\"cdc_rsrc\"");
    let dst = rest_dest(tmp.path(), &base_url);
    let cdc = cstream(
        "cdc_rsrc",
        SyncMode::Cdc,
        WriteMode::Upsert {
            business_keys: vec!["id".to_string()],
        },
        None,
    );
    let st = store(tmp.path(), "cdc");
    let sync = |st: &Store| {
        Engine::new(st)
            .sync("cdc", &cdc, &src, &dst)
            .expect("cdc sync")
    };
    sync(&st); // init slot

    sql.execute("INSERT INTO cdc_rsrc VALUES (1, 'a')", &[])
        .unwrap();
    sync(&st); // insert → POST /items/1
    sql.execute("DELETE FROM cdc_rsrc WHERE id = 1", &[])
        .unwrap();
    sync(&st); // delete → DELETE /items/1

    std::thread::sleep(std::time::Duration::from_millis(150));
    let reqs = log.lock().unwrap().clone();
    assert!(
        reqs.iter().any(|(m, p)| m == "DELETE" && p == "/items/1"),
        "rest-dest issued DELETE /items/1; saw {reqs:?}"
    );
}

// ---- TLS wire gates ([[WEIR-T-0178]] / [[WEIR-A-0041]]) — against the
// `postgres-tls` compose service: ssl=on with a hostssl-ONLY pg_hba, so a
// plaintext connection is REJECTED and a passing sync PROVES TLS engaged.
// Certs are ephemeral (`scripts/gen-test-tls.sh`; `angreal integration up`
// runs it). ----

/// The TLS-only Postgres host port (`WEIR_PG_TLS_HOST_PORT`, default 5434).
fn tls_port() -> u16 {
    std::env::var("WEIR_PG_TLS_HOST_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5434)
}

/// The harness CA (inline PEM for `sslrootcert`).
fn tls_ca() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weir-tls-certs/ca.crt");
    std::fs::read_to_string(&p).unwrap_or_else(|e| {
        panic!(
            "read {} ({e}) — run scripts/gen-test-tls.sh first",
            p.display()
        )
    })
}

/// Load the postgres guest against an explicit URL (+ optional inline-PEM root).
fn pg_at(root: &Path, url: &str, sslrootcert: Option<&str>) -> ConnectorHandle {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/connectors/postgres");
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &fixture,
            wasm_file: "weir_postgres_wasm.wasm",
            pkg_name: "weir-postgres-pkg",
            capabilities: &["tcp"],
        },
        root,
    );
    let json = match sslrootcert {
        Some(pem) => format!(
            "{{\"url\":\"{url}\",\"sslrootcert\":{}}}",
            serde_json::to_string(pem).unwrap()
        ),
        None => format!("{{\"url\":\"{url}\"}}"),
    };
    ConnectorHandle::from_wasm_package(
        root,
        "weir-postgres-pkg",
        &Config { json },
        PgEgress(tls_port()),
        &[],
    )
    .expect("load postgres wasm")
}

/// `sslmode=require` syncs against the hostssl-ONLY server — since plaintext is
/// rejected outright there, a passing roundtrip PROVES the TLS upgrade engaged.
#[test]
#[ignore = "needs the postgres-tls compose service (integration profile)"]
fn tls_require_syncs_against_hostssl_only_server() {
    let tmp = tempfile::TempDir::new().unwrap();
    let url = format!(
        "postgres://weir:weir@localhost:{}/weir?sslmode=require",
        tls_port()
    );
    let pg = pg_at(tmp.path(), &url, None);
    let st = cstream("tls_rt", SyncMode::FullRefresh, WriteMode::Overwrite, None);
    Engine::new(&store(tmp.path(), "w"))
        .sync("w", &st, &slow(3), &pg)
        .expect("write over TLS");
    let out = Engine::new(&store(tmp.path(), "r"))
        .sync("r", &st, &pg, &arrow())
        .expect("read over TLS");
    assert_eq!(out.rows_written, 3, "3 rows through the TLS-only server");
}

/// The engagement proof's other half: `sslmode=disable` against the same server
/// is REJECTED by the hostssl-only pg_hba — plaintext cannot silently pass.
#[test]
#[ignore = "needs the postgres-tls compose service (integration profile)"]
fn tls_plaintext_is_rejected_by_hostssl_only_server() {
    let tmp = tempfile::TempDir::new().unwrap();
    let url = format!(
        "postgres://weir:weir@localhost:{}/weir?sslmode=disable",
        tls_port()
    );
    let pg = pg_at(tmp.path(), &url, None);
    let st = cstream("tls_plain", SyncMode::FullRefresh, WriteMode::Append, None);
    let err = Engine::new(&store(tmp.path(), "w"))
        .sync("w", &st, &slow(1), &pg)
        .expect_err("plaintext must be rejected by the hostssl-only server");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("pg_hba") || msg.contains("no encryption") || msg.contains("SSL"),
        "rejection names the hba/ssl policy: {msg}"
    );
}

/// `verify-full` with the harness CA as inline-PEM `sslrootcert` verifies the
/// chain + hostname and syncs.
#[test]
#[ignore = "needs the postgres-tls compose service (integration profile)"]
fn tls_verify_full_with_ca_syncs() {
    let tmp = tempfile::TempDir::new().unwrap();
    let url = format!(
        "postgres://weir:weir@localhost:{}/weir?sslmode=verify-full",
        tls_port()
    );
    let pg = pg_at(tmp.path(), &url, Some(&tls_ca()));
    let st = cstream("tls_vf", SyncMode::FullRefresh, WriteMode::Overwrite, None);
    Engine::new(&store(tmp.path(), "w"))
        .sync("w", &st, &slow(2), &pg)
        .expect("verify-full write");
    let out = Engine::new(&store(tmp.path(), "r"))
        .sync("r", &st, &pg, &arrow())
        .expect("verify-full read");
    assert_eq!(out.rows_written, 2);
}

/// Negative SAN gate: the server cert is valid ONLY for `localhost`; dialing the
/// same server as `127.0.0.1` under `verify-full` must fail hostname
/// verification — proving verification is real, not `trust_cert` theater.
#[test]
#[ignore = "needs the postgres-tls compose service (integration profile)"]
fn tls_verify_full_wrong_name_fails() {
    let tmp = tempfile::TempDir::new().unwrap();
    let url = format!(
        "postgres://weir:weir@127.0.0.1:{}/weir?sslmode=verify-full",
        tls_port()
    );
    let pg = pg_at(tmp.path(), &url, Some(&tls_ca()));
    let st = cstream("tls_san", SyncMode::FullRefresh, WriteMode::Append, None);
    let err = Engine::new(&store(tmp.path(), "w"))
        .sync("w", &st, &slow(1), &pg)
        .expect_err("wrong-name dial must fail verify-full");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("not valid for name") || msg.contains("invalid peer certificate"),
        "failure is a certificate verification error: {msg}"
    );
}

// ---- Real discover ([[WEIR-T-0179]]): information_schema introspection. ----

/// Discover lists real base tables in the configured schema with their
/// source-defined primary keys — the hardcoded stub stream is gone.
#[test]
#[ignore = "needs the integration Postgres (docker compose up)"]
fn pg_discover_lists_real_tables_with_pks() {
    let mut sql = postgres::Client::connect(&raw_pg_url(), postgres::NoTls).expect("raw pg");
    sql.batch_execute(
        "CREATE TABLE IF NOT EXISTS disc_t (id int, tag text, PRIMARY KEY (id, tag));",
    )
    .expect("seed disc_t");

    let tmp = tempfile::TempDir::new().unwrap();
    let src = pg(tmp.path());
    let outcome = src
        .discover(&Config { json: "{}".into() })
        .expect("discover");
    let weir_connector::DiscoverOutcome::Catalog(cat) = outcome else {
        panic!("expected a catalog, got an error");
    };
    let disc = cat
        .streams
        .iter()
        .find(|s| s.name == "disc_t")
        .expect("disc_t discovered");
    assert_eq!(disc.namespace.as_deref(), Some("public"));
    assert_eq!(
        disc.source_defined_primary_key.as_deref(),
        Some(&["id".to_string(), "tag".to_string()][..]),
        "composite PK in ordinal order"
    );
    assert!(
        !cat.streams.iter().any(|s| s.name == "table"),
        "the stub stream is gone"
    );
}

/// An empty (unknown) schema is an empty catalog — not an error; a dead server
/// is an honest DiscoverOutcome::Error — not an empty catalog.
#[test]
#[ignore = "needs the integration Postgres (docker compose up)"]
fn pg_discover_empty_schema_and_error_cases() {
    let tmp = tempfile::TempDir::new().unwrap();

    let empty = pg_at(tmp.path(), &format!("{}", pg_url()), None);
    // Point discover at a schema with no tables via the constructor config.
    let cfg = Config {
        json: format!("{{\"url\":\"{}\",\"schema\":\"no_such_schema\"}}", pg_url()),
    };
    let src = ConnectorHandle::from_wasm_package(
        tmp.path(),
        "weir-postgres-pkg",
        &cfg,
        PgEgress(pg_port()),
        &[],
    )
    .expect("load");
    let outcome = src
        .discover(&Config { json: "{}".into() })
        .expect("discover");
    match outcome {
        weir_connector::DiscoverOutcome::Catalog(c) => {
            assert!(c.streams.is_empty(), "unknown schema → empty catalog");
        }
        _ => panic!("expected an empty catalog"),
    }
    drop(empty);

    // Dead server → DiscoverOutcome::Error.
    let cfg = Config {
        json: "{\"host\":\"127.0.0.1\",\"port\":1,\"user\":\"weir\",\"password\":\"weir\",\"dbname\":\"weir\",\"sslmode\":\"disable\"}".to_string(),
    };
    let src =
        ConnectorHandle::from_wasm_package(tmp.path(), "weir-postgres-pkg", &cfg, PgEgress(1), &[])
            .expect("load");
    let outcome = src
        .discover(&Config { json: "{}".into() })
        .expect("discover call");
    assert!(
        matches!(outcome, weir_connector::DiscoverOutcome::Error(_)),
        "dead server surfaces as a discover error"
    );
}

// ---- Typed relational columns ([[WEIR-T-0182]]): the destination lands
// typed columns inferred from the records (default), with `typed_columns:
// false` restoring the legacy single-`data JSONB` layout. ----

fn wctx(stream_name: &str, write: WriteMode) -> weir_connector::WriteContext {
    weir_connector::WriteContext {
        stream: ConfiguredStream {
            stream: stream_name.to_string(),
            sync_mode: SyncMode::FullRefresh,
            cursor_field: None,
            primary_key: None,
            write_mode: write,
            mapping: MappingSpec::default(),
        },
    }
}

fn rows(v: &[&str]) -> Vec<weir_connector::RecordBatch> {
    vec![weir_connector::RecordBatch::Rows(
        v.iter().map(|s| s.to_string()).collect(),
    )]
}

/// Column name → data type from information_schema.
fn columns_of(sql: &mut postgres::Client, table: &str) -> Vec<(String, String)> {
    sql.query(
        "SELECT column_name, data_type FROM information_schema.columns \
         WHERE table_schema = 'public' AND table_name = $1 ORDER BY ordinal_position",
        &[&table],
    )
    .expect("columns")
    .iter()
    .map(|r| (r.get::<_, String>(0), r.get::<_, String>(1)))
    .collect()
}

/// Append lands typed columns matching the records, values round-trip, and a
/// new field in a later write is ALTERed in additively.
#[test]
#[ignore = "needs the integration Postgres (docker compose up)"]
fn pg_typed_columns_land_and_evolve() {
    let mut sql = postgres::Client::connect(&raw_pg_url(), postgres::NoTls).expect("raw pg");
    sql.batch_execute("DROP TABLE IF EXISTS typed_t")
        .expect("clean");

    let tmp = tempfile::TempDir::new().unwrap();
    let dest = pg(tmp.path());

    let out = dest
        .write(
            &wctx("typed_t", WriteMode::Append),
            rows(&[
                r#"{"n":1,"tag":"a","score":1.5,"ok":true}"#,
                r#"{"n":2,"tag":"b","score":2.5,"ok":false}"#,
            ]),
        )
        .expect("typed append");
    assert!(matches!(out.result, weir_connector::WriteResult::Ok(r) if r.accepted == 2));

    // Column order follows serde_json's (alphabetical) object iteration — assert
    // the SET of (name, type) pairs.
    let mut cols = columns_of(&mut sql, "typed_t");
    cols.sort();
    assert_eq!(
        cols,
        vec![
            ("n".to_string(), "bigint".to_string()),
            ("ok".to_string(), "boolean".to_string()),
            ("score".to_string(), "double precision".to_string()),
            ("tag".to_string(), "text".to_string()),
        ],
        "typed columns match the records"
    );
    let got: Vec<(i64, String)> = sql
        .query("SELECT n, tag FROM typed_t ORDER BY n", &[])
        .unwrap()
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    assert_eq!(
        got,
        vec![(1, "a".to_string()), (2, "b".to_string())],
        "values round-trip typed"
    );

    // Additive evolution: a later write carries a new field → column added.
    dest.write(
        &wctx("typed_t", WriteMode::Append),
        rows(&[r#"{"n":3,"tag":"c","score":3.5,"ok":true,"extra":"new"}"#]),
    )
    .expect("evolving append");
    let cols = columns_of(&mut sql, "typed_t");
    assert!(
        cols.contains(&("extra".to_string(), "text".to_string())),
        "additive column ALTERed in: {cols:?}"
    );
}

/// Typed upsert keeps key semantics (typed PK, idempotent update).
#[test]
#[ignore = "needs the integration Postgres (docker compose up)"]
fn pg_typed_upsert_by_typed_key() {
    let mut sql = postgres::Client::connect(&raw_pg_url(), postgres::NoTls).expect("raw pg");
    sql.batch_execute("DROP TABLE IF EXISTS typed_up")
        .expect("clean");

    let tmp = tempfile::TempDir::new().unwrap();
    let dest = pg(tmp.path());
    let up = WriteMode::Upsert {
        business_keys: vec!["id".to_string()],
    };
    dest.write(
        &wctx("typed_up", up.clone()),
        rows(&[r#"{"id":1,"v":"x"}"#]),
    )
    .expect("upsert 1");
    dest.write(
        &wctx("typed_up", up),
        rows(&[r#"{"id":1,"v":"y"}"#, r#"{"id":2,"v":"z"}"#]),
    )
    .expect("upsert 2");

    assert_eq!(
        columns_of(&mut sql, "typed_up"),
        vec![
            ("id".to_string(), "bigint".to_string()),
            ("v".to_string(), "text".to_string())
        ]
    );
    let got: Vec<(i64, String)> = sql
        .query("SELECT id, v FROM typed_up ORDER BY id", &[])
        .unwrap()
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    assert_eq!(
        got,
        vec![(1, "y".to_string()), (2, "z".to_string())],
        "typed upsert updated by key"
    );
}

/// `typed_columns: false` restores the legacy single-`data JSONB` layout.
#[test]
#[ignore = "needs the integration Postgres (docker compose up)"]
fn pg_typed_columns_false_keeps_jsonb_layout() {
    let mut sql = postgres::Client::connect(&raw_pg_url(), postgres::NoTls).expect("raw pg");
    sql.batch_execute("DROP TABLE IF EXISTS legacy_t")
        .expect("clean");

    let tmp = tempfile::TempDir::new().unwrap();
    let _staged = pg(tmp.path()); // stages weir-postgres-pkg into the root
    let cfg = Config {
        json: format!("{{\"url\":\"{}\",\"typed_columns\":false}}", pg_url()),
    };
    let dest = ConnectorHandle::from_wasm_package(
        tmp.path(),
        "weir-postgres-pkg",
        &cfg,
        PgEgress(pg_port()),
        &[],
    )
    .expect("load");
    dest.write(&wctx("legacy_t", WriteMode::Append), rows(&[r#"{"n":1}"#]))
        .expect("jsonb append");
    assert_eq!(
        columns_of(&mut sql, "legacy_t"),
        vec![("data".to_string(), "jsonb".to_string())],
        "legacy layout preserved behind the flag"
    );
}
