//! Integration harness for the vendored manifest corpus (`manifests/`, the vendored
//! connectors [[WEIR-T-0058]]). Every `*.yaml` must onboard onto the shared declarative
//! runtime and register in the catalog — so a broken or regressed manifest fails CI, and
//! the tier report tracks runtime coverage. A live, network-gated smoke
//! (`WEIR_LIVE_MANIFEST_TESTS=1`) runs a no-auth connector end-to-end against its API.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use weir_app::{App, Connection, DEFAULT_TENANT, Origin, connector_ref};

fn manifests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../manifests")
}

fn corpus_slugs(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .expect("read manifests/")
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
            (ext == "yaml" || ext == "yml")
                .then(|| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .flatten()
        })
        .collect();
    v.sort();
    v
}

/// Every vendored manifest onboards (kind=manifest, stored, ≥1 stream) and registers.
#[test]
fn every_vendored_manifest_onboards() {
    let dir = manifests_dir();
    unsafe { std::env::set_var("WEIR_MANIFESTS_DIR", &dir) };
    let slugs = corpus_slugs(&dir);
    assert!(
        slugs.len() >= 30,
        "expected the full vendored corpus, found {} ({slugs:?})",
        slugs.len()
    );

    let tmp = tempfile::TempDir::new().unwrap();
    let app = App::open(tmp.path().join("corpus.db").to_str().unwrap()).unwrap();

    let mut failures: Vec<String> = Vec::new();
    let mut tiers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for slug in &slugs {
        let yaml = std::fs::read_to_string(dir.join(format!("{slug}.yaml"))).unwrap();
        let report = app.preview_manifest(&yaml);
        tiers
            .entry(report.tier.clone())
            .or_default()
            .push(slug.clone());

        match app.import_vendored_manifest(DEFAULT_TENANT, slug, Origin::Community) {
            Ok(entry) => {
                if entry.kind != "manifest" {
                    failures.push(format!("{slug}: kind={} (expected manifest)", entry.kind));
                }
                if entry.manifest.as_deref().unwrap_or("").trim().is_empty() {
                    failures.push(format!("{slug}: no stored manifest"));
                }
                if report.streams.is_empty() {
                    failures.push(format!("{slug}: zero streams"));
                }
            }
            Err(e) => failures.push(format!("{slug}: onboard failed — {e}")),
        }
    }

    eprintln!(
        "\n=== vendored manifest corpus: {} connectors ===",
        slugs.len()
    );
    for (tier, names) in &tiers {
        eprintln!("  tier {tier}  ({}): {}", names.len(), names.join(", "));
    }

    let registered = app.list_connectors(DEFAULT_TENANT).unwrap().len();
    assert_eq!(
        registered,
        slugs.len(),
        "every manifest should register exactly once"
    );
    assert!(
        failures.is_empty(),
        "vendored manifest corpus has {} failure(s):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// Stage rest (http) + the standard guests once, and point the env at them + the
/// repo's `manifests/`. Returns the connectors dir.
fn stage_live_runtime() -> std::path::PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let conns = weir_wasm_testkit::connectors_dir(); // incl. arrow-sink (the dest)
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &root.join("crates/connectors/rest"),
            wasm_file: "weir_rest_wasm.wasm",
            pkg_name: "weir-rest-pkg",
            capabilities: &["http"],
        },
        &conns,
    );
    unsafe {
        std::env::set_var("WEIR_CONNECTORS_DIR", &conns);
        std::env::set_var("WEIR_MANIFESTS_DIR", root.join("manifests"));
    }
    conns
}

/// Slugs to skip in this environment (`WEIR_LIVE_SKIP=nasa,coinpaprika`). Used by CI, where a
/// shared-IP rate limit (nasa's public `DEMO_KEY`) or a heavy multi-page fetch (coinpaprika's
/// ~60k rows) is unreliable on the runner. Empty by default, so everything runs in full locally.
fn live_skip() -> std::collections::HashSet<String> {
    std::env::var("WEIR_LIVE_SKIP")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Onboard `slug`, wire a connection on the shared runtime → arrow-sink, run it, and
/// return the rows written — or a readable failure (state + connector logs).
async fn run_manifest_live(
    app: &App,
    name: &str,
    slug: &str,
    stream: &str,
    config: &str,
) -> Result<i64, String> {
    app.import_vendored_manifest(DEFAULT_TENANT, slug, Origin::Community)
        .map_err(|e| format!("onboard: {e}"))?;
    app.add_connection(
        DEFAULT_TENANT,
        &Connection {
            name: name.to_string(),
            source: connector_ref(slug),
            dest: connector_ref("arrow-sink"),
            stream: stream.to_string(),
            source_config: config.to_string(),
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
    .map_err(|e| format!("add_connection: {e}"))?;

    // Per-run cap — configurable so a slower CI runner can give heavy connectors more room
    // (`WEIR_LIVE_TIMEOUT_SECS`); default 25s locally.
    let secs = std::env::var("WEIR_LIVE_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(25);
    let run = tokio::time::timeout(std::time::Duration::from_secs(secs), app.run(name)).await;
    let outcome = match run {
        Err(_) => {
            return Err(format!(
                "timed out after {secs}s (connector hung — likely non-terminating pagination)"
            ));
        }
        Ok(r) => r,
    };
    match outcome {
        Ok(rep) if rep.state == "done" => {
            let rows = app
                .recent_runs(DEFAULT_TENANT, 64)
                .ok()
                .and_then(|rs| rs.into_iter().find(|r| r.connection == name))
                .map(|r| r.rows_written)
                .unwrap_or(0);
            Ok(rows)
        }
        Ok(rep) => {
            // The connector's fatal error is on the work unit, not run_logs.
            let err = app
                .history(DEFAULT_TENANT, name)
                .ok()
                .and_then(|h| h.into_iter().rev().find_map(|s| s.error))
                .unwrap_or_default();
            let detail = if err.is_empty() {
                "(no error captured)".into()
            } else {
                err
            };
            Err(format!("state={} :: {}", rep.state, detail))
        }
        Err(e) => Err(format!("run: {e}")),
    }
}

/// Record a live pass in the vendored verification ledger ([[WEIR-T-0183]],
/// `manifests/verified.json` — surfaces as `verified_at` on catalog rows at
/// registration). Opt-in via WEIR_WRITE_VERIFIED=1 (nightly/local); the updated
/// file is committed like any vendored change.
fn record_verified(slug: &str, reference: &str) {
    if std::env::var("WEIR_WRITE_VERIFIED").as_deref() != Ok("1") {
        return;
    }
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../manifests/verified.json");
    let ledger: serde_json::Map<String, serde_json::Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut sorted: std::collections::BTreeMap<String, serde_json::Value> =
        ledger.into_iter().collect();
    sorted.insert(
        slug.to_string(),
        serde_json::json!({"verified_at": today, "ref": reference}),
    );
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&sorted).expect("ledger json") + "\n",
    )
    .expect("write verified.json");
}

/// Live integration: every **no-auth** vendored connector actually functions against
/// its real API on the shared rest runtime → arrow-sink. `#[ignore]` (network);
/// run explicitly: `cargo test -p weir-app --test manifest_corpus -- --ignored`
/// (or `angreal test manifests --live`).
#[tokio::test]
#[ignore = "live: hits real public APIs over the network"]
async fn no_auth_manifests_run_live() {
    stage_live_runtime();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = App::open(tmp.path().join("live.db").to_str().unwrap()).unwrap();

    // These function today on the shared rest runtime — asserted (rows > 0).
    let functional = [
        ("coinpaprika", "coins"),
        ("jsonplaceholder", "posts"),
        ("rickandmorty", "character"),
        ("frankfurter", "latest"),
        ("xkcd", "comics"), // single-object response, now supported (I-0008)
    ];
    // Known runtime/config gaps ([[WEIR-I-0008]]) — run + reported (not asserted) so the
    // gap stays visible without breaking CI. The note says why each can't function yet.
    let gaps: [(&str, &str, &str); 3] = [
        (
            "pokeapi",
            "pokemon",
            "offset pagination now supported (I-0008); large multi-page catalog",
        ),
        (
            "openlibrary",
            "search",
            "needs a query param — 0 rows without config",
        ),
        (
            "mediawiki",
            "search",
            "templated url_base now supported; needs wiki_base_url config",
        ),
    ];

    let mut report = String::new();
    let mut failed: Vec<&str> = Vec::new();
    let skip = live_skip();
    for (i, (slug, stream)) in functional.iter().enumerate() {
        if skip.contains(*slug) {
            report.push_str(&format!("  ⤍ {slug}/{stream}: skipped (WEIR_LIVE_SKIP)\n"));
            continue;
        }
        let name = format!("f{i}-{slug}");
        match run_manifest_live(&app, &name, slug, stream, "{}").await {
            Ok(rows) if rows > 0 => {
                record_verified(slug, &format!("live read {stream}: {rows} rows"));
                report.push_str(&format!("  ✓ {slug}/{stream}: {rows} rows\n"))
            }
            Ok(rows) => {
                report.push_str(&format!(
                    "  ✗ {slug}/{stream}: {rows} rows (expected > 0)\n"
                ));
                failed.push(slug);
            }
            Err(e) => {
                report.push_str(&format!("  ✗ {slug}/{stream}: {e}\n"));
                failed.push(slug);
            }
        }
    }
    report.push_str("  -- known gaps (I-0008 runtime/config; reported, not asserted) --\n");
    for (i, (slug, stream, why)) in gaps.iter().enumerate() {
        if skip.contains(*slug) {
            report.push_str(&format!(
                "  ~ {slug}/{stream}: {why}  [skipped (WEIR_LIVE_SKIP)]\n"
            ));
            continue;
        }
        let name = format!("g{i}-{slug}");
        let got = match run_manifest_live(&app, &name, slug, stream, "{}").await {
            Ok(rows) => format!("{rows} rows"),
            Err(e) => e,
        };
        report.push_str(&format!("  ~ {slug}/{stream}: {why}  [{got}]\n"));
    }
    eprintln!("\n=== no-auth manifest live runs ===\n{report}");
    assert!(
        failed.is_empty(),
        "manifests expected to function but didn't: {failed:?}"
    );
}

/// Live integration for **authed** connectors (the auth path, [[WEIR-I-0008]]). Each
/// `secrets/<slug>.json` (gitignored — you drop real keys in) is the per-connection
/// config overlay (`api_key` + any required params; optional `__stream` picks the
/// stream, else the manifest's first). Every present secret is run end-to-end on the
/// shared rest runtime and asserted (rows > 0). Connectors **without** a secret file are
/// skipped — so this harness *grows* as keys land, and an empty `secrets/` is a no-op.
/// `#[ignore]` (network + secrets). Run:
/// `cargo test -p weir-app --test manifest_corpus keyed_manifests_run_live -- --ignored`
#[tokio::test]
#[ignore = "live + keyed: needs secrets/<slug>.json with real API keys"]
async fn keyed_manifests_run_live() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    stage_live_runtime();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = App::open(tmp.path().join("keyed.db").to_str().unwrap()).unwrap();

    // Collect <slug>.json from the secrets dir (skipping the committed *.example.json
    // templates). `WEIR_SECRETS_DIR` overrides the default `secrets/` so the angreal
    // secrets-decrypt task ([[WEIR-T-0065]]) can point this at a temp dir of
    // sops-decrypted bundles without touching the repo's gitignored plaintext.
    let secrets_dir = std::env::var_os("WEIR_SECRETS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("secrets"));
    let mut files: Vec<PathBuf> = std::fs::read_dir(&secrets_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|x| x.to_str()) == Some("json")
                && !p.to_string_lossy().ends_with(".example.json")
        })
        .collect();
    files.sort();

    let mut report = String::new();
    let mut failed: Vec<String> = Vec::new();
    let skip = live_skip();
    for path in &files {
        let slug = path.file_stem().unwrap().to_string_lossy().into_owned();
        if skip.contains(&slug) {
            report.push_str(&format!("  ⤍ {slug}: skipped (WEIR_LIVE_SKIP)\n"));
            continue;
        }
        let raw = std::fs::read_to_string(path).unwrap();
        let cfg: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("secrets/{slug}.json is not valid JSON: {e}"));
        let yaml = std::fs::read_to_string(root.join(format!("manifests/{slug}.yaml")))
            .unwrap_or_else(|_| panic!("keyed secret '{slug}' has no manifests/{slug}.yaml"));
        let stream = cfg
            .get("__stream")
            .and_then(|s| s.as_str())
            .map(str::to_string)
            .or_else(|| app.preview_manifest(&yaml).streams.first().cloned())
            .unwrap_or_default();

        let name = format!("k-{slug}");
        match run_manifest_live(&app, &name, &slug, &stream, &raw).await {
            Ok(rows) if rows > 0 => {
                record_verified(&slug, &format!("live read {stream}: {rows} rows"));
                report.push_str(&format!("  ✓ {slug}/{stream}: {rows} rows\n"))
            }
            Ok(rows) => {
                report.push_str(&format!(
                    "  ✗ {slug}/{stream}: {rows} rows (expected > 0)\n"
                ));
                failed.push(slug);
            }
            Err(e) => {
                report.push_str(&format!("  ✗ {slug}/{stream}: {e}\n"));
                failed.push(slug);
            }
        }
    }

    if files.is_empty() {
        eprintln!(
            "\n=== keyed manifest live runs: no secrets/<slug>.json present — nothing to run ==="
        );
        return;
    }
    eprintln!(
        "\n=== keyed manifest live runs ({}) ===\n{report}",
        files.len()
    );
    assert!(
        failed.is_empty(),
        "keyed connectors expected to function but didn't: {failed:?}"
    );
}

/// Live Snowflake **destination** ([[WEIR-T-0157]]): write a fixture batch through the
/// `snowflake-dest` wasm guest into the trial account (key-pair JWT injected host-side,
/// [[WEIR-T-0156]]), **replay it** (MERGE idempotency), then read the rows back over the
/// SQL API through the `rest` runtime (POST-with-body, [[WEIR-T-0154]]). Skips when the
/// `snowflake` bundle is absent. Bundle shape ([[WEIR-S-0018]] §2): `account`, `user`,
/// `private_key`, `database`, `schema`, `warehouse`.
#[tokio::test]
#[ignore = "live + keyed: needs secrets/snowflake.json (WEIR-S-0018 §2)"]
async fn snowflake_dest_writes_and_reads_back_live() {
    use futures::StreamExt;
    use weir_connector::{
        Config, ConfiguredStream, MappingSpec, Partition, ReadContext, ReadMessage, RecordBatch,
        StreamState, SyncMode, WriteContext, WriteMode, WriteResult,
    };
    use weir_runtime::{ConnectorHandle, Credential, HostAllowList};

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let secrets_dir = std::env::var_os("WEIR_SECRETS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("secrets"));
    let Ok(raw) = std::fs::read_to_string(secrets_dir.join("snowflake.json")) else {
        eprintln!("\n=== snowflake dest live: no secrets/snowflake.json — skipped ===");
        return;
    };
    let bundle: serde_json::Value =
        serde_json::from_str(&raw).expect("secrets/snowflake.json is not valid JSON");
    let field = |k: &str| {
        bundle
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("snowflake bundle missing `{k}`"))
            .to_string()
    };

    let conns = weir_wasm_testkit::connectors_dir();
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &root.join("crates/connectors/snowflake"),
            wasm_file: "weir_snowflake_wasm.wasm",
            pkg_name: "weir-snowflake-pkg",
            capabilities: &["http"],
        },
        &conns,
    );
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &root.join("crates/connectors/rest"),
            wasm_file: "weir_rest_wasm.wasm",
            pkg_name: "weir-rest-pkg",
            capabilities: &["http"],
        },
        &conns,
    );

    const TABLE: &str = "weir_live_t0157";
    let auth_cfg = |extra: serde_json::Value| {
        let mut cfg = bundle.clone();
        cfg["auth_scheme"] = "snowflake_keypair_jwt".into();
        if let (Some(c), Some(e)) = (cfg.as_object_mut(), extra.as_object()) {
            for (k, v) in e {
                c.insert(k.clone(), v.clone());
            }
        }
        cfg.to_string()
    };

    // ── Write, then replay (MERGE keyed on email → idempotent). ─────────────────
    let dest_json = auth_cfg(serde_json::json!({ "table": TABLE }));
    let (cred, guest_json) = Credential::from_auth_config(&dest_json);
    assert!(
        !guest_json.contains("PRIVATE KEY"),
        "key material must never reach the guest config"
    );
    let policy = HostAllowList {
        allowed_hosts: Vec::new(),
        inject_headers: Vec::new(),
        credential: cred,
    };
    let dest = ConnectorHandle::from_wasm_package(
        &conns,
        "weir-snowflake-pkg",
        &Config { json: guest_json },
        policy,
        &[],
    )
    .expect("load snowflake-dest");
    let ctx = WriteContext {
        stream: ConfiguredStream {
            stream: TABLE.to_string(),
            sync_mode: SyncMode::FullRefresh,
            cursor_field: None,
            primary_key: Some(vec!["email".to_string()]),
            write_mode: WriteMode::Upsert {
                business_keys: vec!["email".to_string()],
            },
            mapping: MappingSpec::default(),
        },
    };
    let batch = RecordBatch::Rows(vec![
        r#"{"email":"ada@weir.test","name":"Ada","score":1}"#.to_string(),
        r#"{"email":"cy@weir.test","name":"Cy","score":2}"#.to_string(),
    ]);
    for attempt in ["write", "replay"] {
        let out = dest
            .write(&ctx, vec![batch.clone()])
            .unwrap_or_else(|e| panic!("{attempt}: {e}"));
        match out.result {
            WriteResult::Ok(r) => assert_eq!(r.accepted, 2, "{attempt} accepted both rows"),
            WriteResult::Err(e) => panic!("{attempt} failed: {e:?}"),
        }
    }

    // ── Read back through the rest runtime (SQL API SELECT, POST body). ─────────
    let fq = format!(
        "\"{}\".\"{}\".\"{}\"",
        field("database").to_uppercase(),
        field("schema").to_uppercase(),
        TABLE.to_uppercase()
    );
    let statement = serde_json::json!({
        "statement": format!("SELECT \"EMAIL\" FROM {fq}"),
        "database": field("database").to_uppercase(),
        "schema": field("schema").to_uppercase(),
        "warehouse": field("warehouse").to_uppercase(),
    });
    let src_json = auth_cfg(serde_json::json!({
        "base_url": format!("https://{}.snowflakecomputing.com", field("account").to_lowercase()),
        "path": "/api/v2/statements",
        "http_method": "POST",
        "request_body": statement.to_string(),
        "record_path": "data",
    }));
    let (cred, guest_json) = Credential::from_auth_config(&src_json);
    let policy = HostAllowList {
        allowed_hosts: Vec::new(),
        inject_headers: Vec::new(),
        credential: cred,
    };
    let source = ConnectorHandle::from_wasm_package(
        &conns,
        "weir-rest-pkg",
        &Config { json: guest_json },
        policy,
        &[],
    )
    .expect("load rest source for read-back");
    let rctx = ReadContext {
        stream: ConfiguredStream {
            stream: "records".to_string(),
            sync_mode: SyncMode::FullRefresh,
            cursor_field: None,
            primary_key: None,
            write_mode: WriteMode::Append,
            mapping: MappingSpec::default(),
        },
        partition: Partition {
            id: String::new(),
            bounds: None,
        },
        state: StreamState {
            cursor: None,
            opaque: Vec::new(),
        },
    };
    let mut rows = 0usize;
    let mut stream = source.read(&rctx).await.expect("read-back stream");
    while let Some(msg) = stream.next().await {
        if let ReadMessage::Records(RecordBatch::Rows(r)) = msg.expect("read-back message") {
            rows += r.len();
        }
    }
    assert_eq!(
        rows, 2,
        "write + replay must land exactly the 2 keyed rows (MERGE is idempotent)"
    );
    eprintln!("\n=== snowflake dest live: 2 rows written, replayed, read back — no dupes ===");
}

/// Live rETL ([[WEIR-T-0158]]): **seeded Snowflake table → weir → HubSpot contact
/// upsert** — the reverse-ETL pipeline end-to-end. Seeds the table through the
/// `snowflake` guest's write side ([[WEIR-T-0157]]), then engine-syncs the guest's
/// **source** side into `rest-dest` baked from the vendored `hubspot-dest.yaml`
/// (private-app token injected host-side). Runs twice: the PATCH upsert is idempotent.
/// Skips unless BOTH bundles are present ([[WEIR-S-0018]] §2 + §3).
#[tokio::test]
#[ignore = "live + keyed: needs secrets/snowflake.json + secrets/hubspot.json"]
async fn snowflake_to_hubspot_retl_live() {
    use weir_connector::{
        Config, ConfiguredStream, MappingSpec, RecordBatch, SyncMode, WriteContext, WriteMode,
        WriteResult,
    };
    use weir_engine::{Engine, Store};
    use weir_manifest::DestinationManifest;
    use weir_runtime::{ConnectorHandle, Credential, HostAllowList};

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let secrets_dir = std::env::var_os("WEIR_SECRETS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("secrets"));
    let (Ok(sf_raw), Ok(hs_raw)) = (
        std::fs::read_to_string(secrets_dir.join("snowflake.json")),
        std::fs::read_to_string(secrets_dir.join("hubspot.json")),
    ) else {
        eprintln!("\n=== snowflake→hubspot rETL live: needs both bundles — skipped ===");
        return;
    };
    let sf: serde_json::Value = serde_json::from_str(&sf_raw).expect("snowflake.json JSON");
    let hs: serde_json::Value = serde_json::from_str(&hs_raw).expect("hubspot.json JSON");
    let hs_token = hs
        .get("api_key")
        .or_else(|| hs.get("token"))
        .and_then(|t| t.as_str())
        .expect("hubspot bundle needs `api_key` (private-app token)")
        .to_string();

    let conns = weir_wasm_testkit::connectors_dir();
    for (dir, wasm, pkg) in [
        (
            "crates/connectors/snowflake",
            "weir_snowflake_wasm.wasm",
            "weir-snowflake-pkg",
        ),
        (
            "crates/connectors/rest-dest",
            "weir_rest_dest_wasm.wasm",
            "weir-rest-dest-pkg",
        ),
    ] {
        weir_wasm_testkit::stage(
            &weir_wasm_testkit::WasmPackage {
                fixture_dir: &root.join(dir),
                wasm_file: wasm,
                pkg_name: pkg,
                capabilities: &["http"],
            },
            &conns,
        );
    }

    const TABLE: &str = "weir_live_t0158";
    let sf_cfg = |extra: serde_json::Value| {
        let mut cfg = sf.clone();
        cfg["auth_scheme"] = "snowflake_keypair_jwt".into();
        if let (Some(c), Some(e)) = (cfg.as_object_mut(), extra.as_object()) {
            for (k, v) in e {
                c.insert(k.clone(), v.clone());
            }
        }
        cfg.to_string()
    };
    let sf_handle = |json: &str| {
        let (cred, guest_json) = Credential::from_auth_config(json);
        let policy = HostAllowList {
            allowed_hosts: Vec::new(),
            inject_headers: Vec::new(),
            credential: cred,
        };
        ConnectorHandle::from_wasm_package(
            &conns,
            "weir-snowflake-pkg",
            &Config { json: guest_json },
            policy,
            &[],
        )
        .expect("load snowflake guest")
    };

    // ── Seed the warehouse table (upsert by email → stable across reruns). ─────
    let seeder = sf_handle(&sf_cfg(serde_json::json!({ "table": TABLE })));
    let wctx = WriteContext {
        stream: ConfiguredStream {
            stream: TABLE.to_string(),
            sync_mode: SyncMode::FullRefresh,
            cursor_field: None,
            primary_key: Some(vec!["email".to_string()]),
            write_mode: WriteMode::Upsert {
                business_keys: vec!["email".to_string()],
            },
            mapping: MappingSpec::default(),
        },
    };
    let out = seeder
        .write(
            &wctx,
            vec![RecordBatch::Rows(vec![
                r#"{"email":"retl-ada@weir.test","first_name":"Ada","last_name":"Weir","updated_at":"2026-01-01T00:00:00Z"}"#.to_string(),
                r#"{"email":"retl-cy@weir.test","first_name":"Cy","last_name":"Weir","updated_at":"2026-01-02T00:00:00Z"}"#.to_string(),
            ])],
        )
        .expect("seed write");
    match out.result {
        WriteResult::Ok(r) => assert_eq!(r.accepted, 2, "seeded both rows"),
        WriteResult::Err(e) => panic!("seed failed: {e:?}"),
    }

    // ── rETL: snowflake source → rest-dest (hubspot-dest.yaml), token host-side. ──
    let source = sf_handle(&sf_cfg(serde_json::json!({ "table": TABLE })));
    let m = DestinationManifest::from_yaml(
        &std::fs::read_to_string(root.join("dest-manifests/hubspot-dest.yaml")).unwrap(),
    )
    .expect("parse hubspot dest manifest");
    let mut dest_cfg = weir_app::dest_object_to_config(&m, "contacts");
    dest_cfg["api_key"] = serde_json::Value::String(hs_token);
    let (cred, guest_json) = Credential::from_auth_config(&dest_cfg.to_string());
    assert!(
        !guest_json.contains("pat-") && !guest_json.contains("api_key"),
        "the HubSpot token must be stripped from the guest config"
    );
    let policy = HostAllowList {
        allowed_hosts: Vec::new(),
        inject_headers: Vec::new(),
        credential: cred,
    };
    let dest = ConnectorHandle::from_wasm_package(
        &conns,
        "weir-rest-dest-pkg",
        &Config { json: guest_json },
        policy,
        &[],
    )
    .expect("load hubspot dest");

    let tmp = tempfile::TempDir::new().unwrap();
    let store = Store::open(tmp.path().join("retl.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);
    let cs = ConfiguredStream {
        stream: "contacts".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: Some(vec!["email".to_string()]),
        write_mode: WriteMode::Upsert {
            business_keys: vec!["email".to_string()],
        },
        mapping: MappingSpec::default(),
    };
    for run in 1..=2 {
        let out = engine
            .sync("retl-live", &cs, &source, &dest)
            .unwrap_or_else(|e| panic!("rETL run {run}: {e:?}"));
        assert_eq!(
            out.rows_written, 2,
            "run {run}: both warehouse rows upserted into HubSpot (idempotent PATCH)"
        );
    }
    eprintln!(
        "\n=== snowflake→hubspot rETL live: 2 contacts upserted from {TABLE}, twice, no failures ==="
    );
}
