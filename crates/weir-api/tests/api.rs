//! WEIR-T-0016: the control-plane HTTP API, exercised via `oneshot` (no port).

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::ServiceExt; // oneshot
use weir_app::App;

// Run the api tests on wasm connectors (WEIR-I-0011): stage guests + point the app at them.
fn use_wasm_connectors() {
    unsafe {
        std::env::set_var("WEIR_CONNECTORS_DIR", weir_wasm_testkit::connectors_dir());
    }
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ` — RFC 3339 UTC with exactly millisecond precision,
/// the shape every run timestamp in the API uses ([[WEIR-T-0224]]).
fn is_rfc3339_ms_utc(s: &str) -> bool {
    s.len() == 24
        && s.ends_with('Z')
        && s.as_bytes()[19] == b'.'
        && chrono::DateTime::parse_from_rfc3339(s).is_ok()
}

async fn json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn connection_crud_run_and_history() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());

    // Create a connection.
    let body = serde_json::json!({
        "name": "demo", "source": "Echo", "dest": "ArrowSink", "stream": "echo", "config": {}
    })
    .to_string();
    let resp = router
        .clone()
        .oneshot(
            Request::post("/connections")
                .header("authorization", token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // List → one connection, source echoed back as a plugin name.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let list = json(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    // WASM-always: a connector name normalizes to its kebab package name (Echo → echo).
    assert_eq!(list[0]["source"], "echo");
    assert_eq!(list[0]["stream"], "echo");

    // Run it → enqueued (async); the API returns pending immediately.
    let resp = router
        .clone()
        .oneshot(
            Request::post("/connections/demo/run")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json(resp).await["state"], "pending");

    // [[WEIR-T-0224]]: a queued run's feed row carries both timestamps as null.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/runs")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let queued = json(resp).await;
    assert_eq!(queued[0]["state"], "pending");
    assert!(queued[0]["started_at"].is_null(), "unset while queued");
    assert!(queued[0]["finished_at"].is_null(), "unset while queued");

    // Drain the relay (a background worker does this in `serve`) → done.
    app.drain().await.unwrap();

    // History → one work unit, done.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections/demo/runs")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let runs = json(resp).await;
    assert_eq!(runs.as_array().unwrap().len(), 1);
    assert_eq!(runs[0]["state"], "done");

    // Missing connection → 404.
    let resp = router
        .oneshot(
            Request::get("/connections/nope")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// WEIR-T-0035: a failed run surfaces *why* (the stored error), and dead-lettered
/// records are listable (the *what + why* behind the count), not just counted.
#[tokio::test]
async fn failed_run_surfaces_error_and_dead_letters() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());

    // One connection that fails fatally; one that dead-letters a record.
    for (name, config) in [
        (
            "boom",
            serde_json::json!({"fail": "fatal", "token": "t-boom"}),
        ),
        ("dlq", serde_json::json!({"dead_letter": true})),
    ] {
        let body = serde_json::json!({
            "name": name, "source": "Faulty", "dest": "ArrowSink", "stream": "faulty", "config": config
        })
        .to_string();
        let resp = router
            .clone()
            .oneshot(
                Request::post("/connections")
                    .header("authorization", token.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        router
            .clone()
            .oneshot(
                Request::post(format!("/connections/{name}/run"))
                    .header("authorization", token.as_str())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    }
    app.drain().await.unwrap();

    // /runs surfaces the failure reason on the fatal run.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/runs")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let runs = json(resp).await;
    let boom = runs
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["connection"] == "boom")
        .expect("a boom run");
    assert_eq!(boom["state"], "failed");
    assert!(
        boom["error"]
            .as_str()
            .unwrap_or("")
            .contains("simulated fatal failure"),
        "expected a failure reason, got {:?}",
        boom["error"]
    );

    // [[WEIR-T-0224]]: feed rows carry started_at / finished_at as RFC 3339 UTC —
    // on /runs and on the admin tenant-scoped mirror alike — and GET /runs/{id}
    // (and its tenant-scoped mirror) give the very same strings.
    let rfc = |v: &serde_json::Value| -> i64 {
        chrono::DateTime::parse_from_rfc3339(v.as_str().expect("an RFC 3339 string"))
            .expect("parses as RFC 3339")
            .timestamp_millis()
    };
    let (feed_started, feed_finished) = (rfc(&boom["started_at"]), rfc(&boom["finished_at"]));
    assert!(boom["started_at"].as_str().unwrap().ends_with('Z'), "UTC");
    assert!(feed_finished >= feed_started);
    let resp = router
        .clone()
        .oneshot(
            Request::get("/tenants/default/runs")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let scoped = json(resp).await;
    let scoped_boom = scoped
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == boom["id"])
        .expect("the boom run on the tenant-scoped feed");
    assert_eq!(scoped_boom["started_at"], boom["started_at"]);
    assert_eq!(scoped_boom["finished_at"], boom["finished_at"]);

    // GET /runs/{id} ([[WEIR-T-0189]]): full detail for a real run, 404 for an
    // unknown id; ?limit caps the feed page.
    let boom_id = boom["id"].as_i64().unwrap();
    let resp = router
        .clone()
        .oneshot(
            Request::get(format!("/runs/{boom_id}"))
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let detail = json(resp).await;
    assert_eq!(detail["id"].as_i64(), Some(boom_id));
    assert_eq!(detail["state"], "failed");
    assert_eq!(detail["connection"], "boom");
    assert!(detail["logs"].is_array(), "detail carries a run-log tail");
    assert_eq!(
        detail["started_at"], boom["started_at"],
        "same string as the feed row"
    );
    assert_eq!(
        detail["finished_at"], boom["finished_at"],
        "same string as the feed row"
    );
    assert_eq!(rfc(&detail["started_at"]), feed_started);
    assert_eq!(rfc(&detail["finished_at"]), feed_finished);
    assert!(
        is_rfc3339_ms_utc(detail["started_at"].as_str().unwrap()),
        "{}",
        detail["started_at"]
    );
    assert!(
        is_rfc3339_ms_utc(detail["finished_at"].as_str().unwrap()),
        "{}",
        detail["finished_at"]
    );
    let resp = router
        .clone()
        .oneshot(
            Request::get(format!("/tenants/default/runs/{boom_id}"))
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let scoped_detail = json(resp).await;
    assert_eq!(scoped_detail["started_at"], boom["started_at"]);
    assert_eq!(scoped_detail["finished_at"], boom["finished_at"]);
    let resp = router
        .clone()
        .oneshot(
            Request::get("/runs/999999999")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = router
        .clone()
        .oneshot(
            Request::get("/runs?limit=1")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let one = json(resp).await;
    assert_eq!(
        one.as_array().map(Vec::len),
        Some(1),
        "?limit=1 caps the feed page"
    );

    // The dead-letter endpoint returns the rejected record + reason.
    let resp = router
        .oneshot(
            Request::get("/connections/dlq/dead-letters")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let dls = json(resp).await;
    let arr = dls.as_array().unwrap();
    assert!(!arr.is_empty(), "expected dead-letter records");
    assert!(
        arr[0]["reason"]
            .as_str()
            .unwrap_or("")
            .contains("rejection"),
        "expected a dead-letter reason, got {:?}",
        arr[0]["reason"]
    );
}

/// WEIR-T-0036: connector logs emitted during a run are captured + listable
/// (the engine used to drop them on the floor).
#[tokio::test]
async fn run_captures_connector_logs() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());

    // Slow emits an Info log each read; sleep_ms:0 keeps the test fast.
    let body = serde_json::json!({
        "name": "noisy", "source": "Slow", "dest": "ArrowSink", "stream": "slow",
        "config": {"sleep_ms": 0, "rows": 2}
    })
    .to_string();
    router
        .clone()
        .oneshot(
            Request::post("/connections")
                .header("authorization", token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    router
        .clone()
        .oneshot(
            Request::post("/connections/noisy/run")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    app.drain().await.unwrap();

    let resp = router
        .oneshot(
            Request::get("/connections/noisy/logs")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let logs = json(resp).await;
    let arr = logs.as_array().unwrap();
    assert!(!arr.is_empty(), "expected captured logs");
    assert_eq!(arr[0]["level"], "info");
    assert!(
        arr[0]["message"]
            .as_str()
            .unwrap_or("")
            .contains("slow source"),
        "got {:?}",
        arr[0]["message"]
    );
}

#[tokio::test]
async fn serves_ui_shell_at_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());

    let resp = router
        .oneshot(
            Request::get("/")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(ct.contains("text/html"), "content-type was {ct}");
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("weir"));
}

#[tokio::test]
async fn catalog_endpoints_respond() {
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());

    // Registered catalog starts empty.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/catalog")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json(resp).await, serde_json::json!([]));

    // Folder-scan availability responds.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/catalog/available")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Import with neither path nor package → 400.
    let resp = router
        .clone()
        .oneshot(
            Request::post("/catalog/import")
                .header("authorization", token.as_str())
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Unregister of an absent entry is a no-op → 204.
    let resp = router
        .oneshot(
            Request::delete("/catalog/foo/1.0.0")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

/// WEIR-T-0056: onboard a low-code manifest via the API — instant register (no
/// compile), kind=manifest; bad manifest → 400.
#[tokio::test]
async fn import_manifest_via_api() {
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());

    let manifest = r#"
type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: coins
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.coinpaprika.com/v1"
        path: "/coins"
      record_selector:
        type: RecordSelector
        extractor:
          type: DpathExtractor
          field_path: []
    schema_loader:
      type: InlineSchemaLoader
      schema:
        type: object
        properties:
          id: { type: string }
"#;
    let body = serde_json::json!({ "manifest": manifest, "name": "coinpaprika" }).to_string();
    let resp = router
        .clone()
        .oneshot(
            Request::post("/catalog/import")
                .header("authorization", token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let entry = json(resp).await;
    assert_eq!(entry["name"], "coinpaprika");
    assert_eq!(entry["kind"], "manifest");

    // It lists in the catalog as a manifest-kind connector.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/catalog")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cat = json(resp).await;
    assert!(
        cat.as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "coinpaprika" && c["kind"] == "manifest")
    );

    // A bad manifest is a client error (4xx), with the reason surfaced.
    let body = serde_json::json!({ "manifest": "garbage: [" }).to_string();
    let resp = router
        .oneshot(
            Request::post("/catalog/import")
                .header("authorization", token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// WEIR-T-0084: the API requires a valid bearer key; `/health` stays public.
#[tokio::test]
async fn auth_gate_health_open_and_key_required() {
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());
    let router = weir_api::router(Arc::clone(&app));

    // /health is public.
    let resp = router
        .clone()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // An API route with no key → 401.
    let resp = router
        .clone()
        .oneshot(Request::get("/connections").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // A bogus key → 401.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections")
                .header("authorization", "Bearer weirk_nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // The valid key → 200.
    let resp = router
        .oneshot(
            Request::get("/connections")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

/// WEIR-T-0085: authz — a read-only key may GET but not POST (mutations need write); a denied
/// write is audited.
#[tokio::test]
async fn authz_read_key_denied_write_and_audited() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let read_key = format!(
        "Bearer {}",
        app.create_api_key("reader", "read", None, false).unwrap()
    );
    let router = weir_api::router(Arc::clone(&app));

    // GET allowed for a read key.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections")
                .header("authorization", read_key.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // POST (write) → 403.
    let body = serde_json::json!({
        "name": "x", "source": "Echo", "dest": "ArrowSink", "stream": "echo", "config": {}
    })
    .to_string();
    let resp = router
        .oneshot(
            Request::post("/connections")
                .header("authorization", read_key.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // The denied mutation is in the audit trail.
    let audit = app.recent_audit(10).unwrap();
    assert!(
        audit.iter().any(|(actor, action, _res, _ts, outcome)| {
            actor == "key:reader" && action.starts_with("POST") && outcome == "denied"
        }),
        "expected a denied audit row, got {audit:?}"
    );
}

/// WEIR-T-0086: /auth/me returns the principal; the `weir_session` cookie is a 2nd door;
/// /auth/login reports not-configured until the OIDC flow lands.
#[tokio::test]
async fn auth_me_and_session_cookie_door() {
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let key = app.bootstrap_admin_key().unwrap().unwrap();
    let router = weir_api::router(Arc::clone(&app));

    // /auth/me via bearer → identity.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/auth/me")
                .header("authorization", format!("Bearer {key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let me = json(resp).await;
    assert_eq!(me["name"], "admin");
    assert!(me["is_admin"].as_bool().unwrap());

    // The session cookie authenticates an API route (2nd door).
    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections")
                .header("cookie", format!("weir_session={key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // /auth/login is public + reports not-configured.
    let resp = router
        .oneshot(Request::get("/auth/login").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn cross_tenant_isolation() {
    // [[WEIR-T-0090]]: two tenant-scoped keys can't see each other's data, and a non-admin
    // tenant key is denied the platform-admin /tenants surface.
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let acme = format!(
        "Bearer {}",
        app.create_api_key("acme", "write", Some("acme"), false)
            .unwrap()
    );
    let globex = format!(
        "Bearer {}",
        app.create_api_key("globex", "write", Some("globex"), false)
            .unwrap()
    );

    // acme creates a connection.
    let body = serde_json::json!({"name":"shared","source":"Echo","dest":"ArrowSink","stream":"echo","config":{}}).to_string();
    let resp = router
        .clone()
        .oneshot(
            Request::post("/connections")
                .header("authorization", acme.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // globex can't see it — 404 (not 403: no existence leak).
    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections/shared")
                .header("authorization", globex.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    // ...and globex's list is empty.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections")
                .header("authorization", globex.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json(resp).await.as_array().unwrap().len(), 0);

    // acme sees its own.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections/shared")
                .header("authorization", acme.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // A non-admin tenant key is denied the platform-admin tenants surface.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/tenants")
                .header("authorization", globex.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn two_tenant_secret_and_audit_isolation() {
    // [[WEIR-T-0093]]: a tenant's secret rides its (tenant-scoped) connection config, so a cross-tenant
    // read can't reach it; and mutations are audited to the acting tenant's key.
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let acme = format!(
        "Bearer {}",
        app.create_api_key("acme-key", "write", Some("acme"), false)
            .unwrap()
    );
    let globex = format!(
        "Bearer {}",
        app.create_api_key("globex-key", "write", Some("globex"), false)
            .unwrap()
    );

    // acme creates a connection whose config carries a secret.
    let body = serde_json::json!({
        "name":"sync","source":"Echo","dest":"ArrowSink","stream":"echo",
        "config": {"api_key":"acme-super-secret"}
    })
    .to_string();
    let resp = router
        .clone()
        .oneshot(
            Request::post("/connections")
                .header("authorization", acme.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // globex can't see the connection (404) → can't reach the secret; its list is empty + secret-free.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections/sync")
                .header("authorization", globex.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections")
                .header("authorization", globex.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list = json(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 0);
    assert!(
        !serde_json::to_string(&list)
            .unwrap()
            .contains("acme-super-secret"),
        "secret never leaks cross-tenant"
    );

    // acme reads its own connection.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections/sync")
                .header("authorization", acme.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Audit: acme's create is attributed to acme's key; no globex-attributed mutation exists.
    let audit = app.recent_audit(50).unwrap();
    let created = audit.iter().find(|(_a, action, resource, _t, _o)| {
        action.contains("POST") && resource.contains("/connections")
    });
    let (actor, _a, _r, _t, outcome) = created.expect("connection create audited");
    assert_eq!(actor, "key:acme-key");
    assert_eq!(outcome, "ok");
    assert!(
        !audit
            .iter()
            .any(|(actor, _, _, _, _)| actor == "key:globex-key"),
        "no globex mutation"
    );
}

#[tokio::test]
async fn admin_cross_tenant_browse() {
    // [[WEIR-T-0094]]: a platform-admin browses any tenant via /tenants/{id}/...; a non-admin is 403.
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let admin = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());
    let acme = format!(
        "Bearer {}",
        app.create_api_key("acme", "write", Some("acme"), false)
            .unwrap()
    );

    // acme creates a connection under tenant acme.
    let body = serde_json::json!({"name":"c1","source":"Echo","dest":"ArrowSink","stream":"echo","config":{}}).to_string();
    let resp = router
        .clone()
        .oneshot(
            Request::post("/connections")
                .header("authorization", acme.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // admin browses tenant acme via /tenants/acme/connections → sees c1.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/tenants/acme/connections")
                .header("authorization", admin.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let list = json(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["name"], "c1");

    // admin's own implicit list (default tenant) is empty — acme's data isn't leaked into it.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections")
                .header("authorization", admin.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json(resp).await.as_array().unwrap().len(), 0);

    // a non-admin key is DENIED the cross-tenant admin route (403), even for its own tenant.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/tenants/acme/connections")
                .header("authorization", acme.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn metrics_endpoint_public_and_records_runs() {
    // [[WEIR-T-0099]]: /metrics is public (no auth) + records run metrics after a run.
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app)); // installs the recorder
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());

    // Public — no auth header → 200.
    let resp = router
        .clone()
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Create + run a connection, drain → the run path emits metrics.
    let body = serde_json::json!({"name":"demo","source":"Echo","dest":"ArrowSink","stream":"echo","config":{}}).to_string();
    router
        .clone()
        .oneshot(
            Request::post("/connections")
                .header("authorization", token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    router
        .clone()
        .oneshot(
            Request::post("/connections/demo/run")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    app.drain().await.unwrap();

    let resp = router
        .clone()
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let text = String::from_utf8(
        to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        text.contains("weir_runs_total"),
        "expected weir_runs_total in:\n{text}"
    );
    assert!(
        text.contains("weir_rows_written_total"),
        "expected weir_rows_written_total"
    );
}

/// [[WEIR-T-0110]]: the ops-health endpoints — `/overview` is tenant-scoped (own connections only);
/// `/platform/health` is platform-admin only (a non-admin key is 403).
#[tokio::test]
async fn health_overview_scoped_and_platform_gated() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let admin = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());
    let reader = format!(
        "Bearer {}",
        app.create_api_key("reader", "read", None, false).unwrap()
    );

    // A connection in the default tenant.
    let body = serde_json::json!({
        "name": "demo", "source": "Echo", "dest": "ArrowSink", "stream": "echo", "config": {}
    })
    .to_string();
    let resp = router
        .clone()
        .oneshot(
            Request::post("/connections")
                .header("authorization", admin.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // GET /overview (own tenant) → the connection appears; no runs yet → "unknown".
    let resp = router
        .clone()
        .oneshot(
            Request::get("/overview")
                .header("authorization", admin.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let health = json(resp).await;
    assert_eq!(health.as_array().unwrap().len(), 1);
    assert_eq!(health[0]["connection"], "demo");
    assert_eq!(health[0]["status"], "unknown");

    // GET /platform/health as a NON-admin → 403 (the gate).
    let resp = router
        .clone()
        .oneshot(
            Request::get("/platform/health")
                .header("authorization", reader.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // GET /platform/health as admin → 200, includes the default tenant rollup.
    let resp = router
        .oneshot(
            Request::get("/platform/health")
                .header("authorization", admin.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let plat = json(resp).await;
    assert!(
        plat["tenants"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["tenant"] == "default")
    );
}

#[tokio::test]
async fn schema_endpoint_returns_captured_schema() {
    // [[WEIR-T-0121]]: after a run captures the schema, GET /connections/{name}/schema serves the
    // typed fields + a null drift flag (healthy).
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());

    let body = serde_json::json!({
        "name": "sc", "source": "Slow", "dest": "ArrowSink", "stream": "sc",
        "config": {"rows": 3, "batch": true, "sleep_ms": 0}
    })
    .to_string();
    router
        .clone()
        .oneshot(
            Request::post("/connections")
                .header("authorization", token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    router
        .clone()
        .oneshot(
            Request::post("/connections/sc/run")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    app.drain().await.unwrap();

    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections/sc/schema")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let sv = json(resp).await;
    assert!(sv["broken"].is_null(), "healthy schema → no drift flag");
    let fields = sv["fields"].as_array().expect("fields array");
    let n = fields
        .iter()
        .find(|f| f["name"] == "n")
        .expect("field n captured");
    assert_eq!(n["type"], "integer", "Slow's n inferred as integer");
}

#[tokio::test]
async fn create_rejects_unknown_connector_and_missing_required_config() {
    // [[WEIR-T-0166]]: creation-time validation — a typo'd connector 404s with a catalog
    // pointer, and config missing the connector's declared requireds 400s naming the fields.
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());

    // Unknown source connector → 404 naming it, never 201-then-fail-at-run.
    let body = serde_json::json!({
        "name": "typo", "source": "Ecoh", "dest": "ArrowSink", "stream": "s", "config": {}
    })
    .to_string();
    let resp = router
        .clone()
        .oneshot(
            Request::post("/connections")
                .header("authorization", token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let err = json(resp).await["error"].as_str().unwrap().to_string();
    assert!(
        err.to_lowercase().contains("ecoh"),
        "names the connector: {err}"
    );
    assert!(err.contains("/catalog"), "points at the catalog: {err}");

    // `rest` declares required base_url + path → an empty config 400s listing them.
    let body = serde_json::json!({
        "name": "restless", "source": "rest", "dest": "ArrowSink", "stream": "s", "config": {}
    })
    .to_string();
    let resp = router
        .clone()
        .oneshot(
            Request::post("/connections")
                .header("authorization", token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err = json(resp).await["error"].as_str().unwrap().to_string();
    assert!(
        err.contains("base_url") && err.contains("path"),
        "lists the missing required fields: {err}"
    );

    // Neither rejected create persisted anything.
    let resp = router
        .clone()
        .oneshot(
            Request::get("/connections")
                .header("authorization", token.as_str())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json(resp).await.as_array().unwrap().len(), 0);
}

// ---- [[WEIR-T-0201]]: secret fields are redacted on read and kept on write ----

const SENTINEL: &str = "__weir_secret_unchanged__";

async fn send(
    router: &axum::Router,
    token: &str,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", token)
        .header("content-type", "application/json");
    let req = match body {
        Some(b) => req.body(Body::from(b.to_string())).unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, v)
}

#[tokio::test]
async fn every_connection_route_redacts_secrets() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());

    // `conv`: the `config` convenience block carries a baked-pattern secret (`api_key`) to both sides.
    let (s, _) = send(
        &router,
        &token,
        "POST",
        "/connections",
        Some(serde_json::json!({
            "name":"conv","source":"Echo","dest":"ArrowSink","stream":"echo",
            "config":{"api_key":"conv-secret-1","note":"visible"}
        })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    // `split`: per-side blocks — source holds baked basic-auth (the key named by
    // `basic_password_key`); dest is postgres, whose schema marks `password` `format: password`.
    let (s, body) = send(&router, &token, "POST", "/connections", Some(serde_json::json!({
        "name":"split","source":"Echo","dest":"postgres","stream":"echo",
        "source_config":{"auth_scheme":"basic","basic_password_key":"pw","pw":"src-secret-2","user":"u"},
        "dest_config":{"host":"db","password":"dest-secret-3","table":"t"}
    }))).await;
    assert_eq!(s, StatusCode::CREATED, "{body}");
    // `ms`: an mssql dest, whose schema marks `password` secret ([[WEIR-T-0222]]).
    let (s, body) = send(&router, &token, "POST", "/connections", Some(serde_json::json!({
        "name":"ms","source":"Echo","dest":"mssql","stream":"echo",
        "source_config":{},
        "dest_config":{"host":"sql","database":"d","user":"sa","password":"mssql-secret-4","table":"t"}
    }))).await;
    assert_eq!(s, StatusCode::CREATED, "{body}");

    let routes = [
        "/connections",
        "/connections/conv",
        "/connections/split",
        "/connections/ms",
        "/tenants/default/connections",
        "/tenants/default/connections/conv",
        "/tenants/default/connections/split",
        "/tenants/default/connections/ms",
    ];
    for uri in routes {
        let (s, v) = send(&router, &token, "GET", uri, None).await;
        assert_eq!(s, StatusCode::OK, "{uri}");
        let text = v.to_string();
        for secret in [
            "conv-secret-1",
            "src-secret-2",
            "dest-secret-3",
            "mssql-secret-4",
        ] {
            assert!(!text.contains(secret), "{uri} leaks {secret}: {text}");
        }
        let conns: Vec<serde_json::Value> = match v {
            serde_json::Value::Array(a) => a,
            one => vec![one],
        };
        for c in conns {
            match c["name"].as_str().unwrap() {
                "conv" => {
                    assert_eq!(c["source_config"]["api_key"], SENTINEL, "{uri}");
                    assert_eq!(c["dest_config"]["api_key"], SENTINEL, "{uri}");
                    assert_eq!(c["source_config"]["note"], "visible", "{uri}");
                }
                "split" => {
                    assert_eq!(c["source_config"]["pw"], SENTINEL, "{uri}");
                    assert_eq!(c["source_config"]["user"], "u", "{uri}");
                    assert_eq!(c["dest_config"]["password"], SENTINEL, "{uri}");
                    assert_eq!(c["dest_config"]["host"], "db", "{uri}");
                }
                "ms" => {
                    assert_eq!(c["dest_config"]["password"], SENTINEL, "{uri}");
                    assert_eq!(c["dest_config"]["user"], "sa", "{uri}");
                }
                other => panic!("unexpected connection {other}"),
            }
        }
    }
    // The store still holds the real values (redaction is read-side only).
    let stored = app.get_connection("default", "split").unwrap();
    assert!(stored.dest_config.contains("dest-secret-3"));
}

// ---- [[WEIR-T-0202]]: `env:` / `file:` references in secret fields ----

#[tokio::test]
async fn secret_references_are_stored_as_text_and_refused_outside_secret_fields() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());

    // A reference in a field that is not secret is refused at create, naming the field.
    let (s, body) = send(
        &router,
        &token,
        "POST",
        "/connections",
        Some(serde_json::json!({
            "name":"badref","source":"Echo","dest":"postgres","stream":"echo",
            "source_config":{},
            "dest_config":{"host":"env:WEIR_T0202_DB_HOST","password":"pw","table":"t"}
        })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{body}");
    let err = body["error"].as_str().unwrap();
    assert!(err.contains("dest config field `host`"), "{err}");
    assert!(err.contains("`env:WEIR_T0202_DB_HOST`"), "{err}");
    assert!(
        app.get_connection("default", "badref").is_err(),
        "nothing persisted"
    );

    // A reference in a secret field (schema-marked and baked-auth) is accepted, stored as
    // the reference text, and a read returns the text: the API never resolves it.
    // SAFETY: a unique name; the API must not read it, so its value must never appear.
    unsafe { std::env::set_var("WEIR_T0202_API_PW", "resolved-must-not-leak") };
    let (s, body) = send(
        &router,
        &token,
        "POST",
        "/connections",
        Some(serde_json::json!({
            "name":"refs","source":"Echo","dest":"postgres","stream":"echo",
            "source_config":{"api_key":"file:/run/secrets/weir-t-0202"},
            "dest_config":{"host":"db","password":"env:WEIR_T0202_API_PW","table":"t"}
        })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{body}");
    for uri in [
        "/connections",
        "/connections/refs",
        "/tenants/default/connections/refs",
    ] {
        let (s, v) = send(&router, &token, "GET", uri, None).await;
        assert_eq!(s, StatusCode::OK, "{uri}");
        assert!(
            !v.to_string().contains("resolved-must-not-leak"),
            "{uri}: {v}"
        );
        let c = match v {
            serde_json::Value::Array(a) => a.into_iter().find(|c| c["name"] == "refs").unwrap(),
            one => one,
        };
        assert_eq!(
            c["dest_config"]["password"], "env:WEIR_T0202_API_PW",
            "{uri}"
        );
        assert_eq!(
            c["source_config"]["api_key"], "file:/run/secrets/weir-t-0202",
            "{uri}"
        );
    }
    let stored = app.get_connection("default", "refs").unwrap();
    assert!(stored.dest_config.contains("env:WEIR_T0202_API_PW"));
    assert!(!stored.dest_config.contains("resolved-must-not-leak"));
}

#[tokio::test]
async fn write_keeps_replaces_and_clears_secrets() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());
    let conn = |src: serde_json::Value, dst: serde_json::Value| {
        serde_json::json!({
            "name":"pg","source":"Echo","dest":"postgres","stream":"echo",
            "source_config":src,"dest_config":dst
        })
    };
    let stored = |app: &App| {
        let c = app.get_connection("default", "pg").unwrap();
        (
            serde_json::from_str::<serde_json::Value>(&c.source_config).unwrap(),
            serde_json::from_str::<serde_json::Value>(&c.dest_config).unwrap(),
        )
    };

    // A new connection that sends the sentinel is refused: nothing to keep.
    let (s, body) = send(
        &router,
        &token,
        "POST",
        "/connections",
        Some(conn(
            serde_json::json!({"api_key":SENTINEL}),
            serde_json::json!({"password":"pw"}),
        )),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{body}");
    assert!(body["error"].as_str().unwrap().contains(SENTINEL), "{body}");
    assert!(
        app.get_connection("default", "pg").is_err(),
        "nothing persisted"
    );

    let (s, _) = send(
        &router,
        &token,
        "POST",
        "/connections",
        Some(conn(
            serde_json::json!({"api_key":"k1"}),
            serde_json::json!({"host":"db","password":"pw1"}),
        )),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);

    // 1. Keep: the sentinel (source) and an omitted secret (dest) keep the stored values,
    //    while a non-secret field changes.
    let (s, body) = send(
        &router,
        &token,
        "POST",
        "/connections",
        Some(conn(
            serde_json::json!({"api_key":SENTINEL}),
            serde_json::json!({"host":"db2"}),
        )),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{body}");
    let (src, dst) = stored(&app);
    assert_eq!(src["api_key"], "k1");
    assert_eq!(dst["password"], "pw1");
    assert_eq!(dst["host"], "db2");

    // The round trip of a GET body is a no-op on the secrets.
    let (_, got) = send(&router, &token, "GET", "/connections/pg", None).await;
    let (s, body) = send(&router, &token, "POST", "/connections", Some(got)).await;
    assert_eq!(s, StatusCode::CREATED, "{body}");
    let (src, dst) = stored(&app);
    assert_eq!(src["api_key"], "k1");
    assert_eq!(dst["password"], "pw1");

    // 2. Replace: a new value replaces the stored one.
    let (s, _) = send(
        &router,
        &token,
        "POST",
        "/connections",
        Some(conn(
            serde_json::json!({"api_key":"k2"}),
            serde_json::json!({"host":"db2","password":"pw2"}),
        )),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let (src, dst) = stored(&app);
    assert_eq!(src["api_key"], "k2");
    assert_eq!(dst["password"], "pw2");

    // 3. Clear: an empty string removes the stored secret.
    let (s, _) = send(
        &router,
        &token,
        "POST",
        "/connections",
        Some(conn(
            serde_json::json!({"api_key":""}),
            serde_json::json!({"host":"db2","password":""}),
        )),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let (src, dst) = stored(&app);
    assert!(src.get("api_key").is_none(), "{src}");
    assert!(dst.get("password").is_none(), "{dst}");

    // With nothing stored, the sentinel on an existing connection is refused too.
    let (s, body) = send(
        &router,
        &token,
        "POST",
        "/connections",
        Some(conn(
            serde_json::json!({"api_key":SENTINEL}),
            serde_json::json!({"host":"db2"}),
        )),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn tenant_delete_cascades_and_its_key_401s_at_once() {
    // WEIR-T-0210: DELETE /tenants/{id} cascades, the tenant's key is refused on the next request
    // (the key cache is cleared, no TTL wait), and another tenant's same-named connection stays.
    // A tenant id of this test only: the delete removes <connectors_dir>/<tenant>/, which is the
    // shared testkit dir here.
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let admin = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());
    let acme = format!(
        "Bearer {}",
        app.create_api_key("t0210-doomed", "write", Some("t0210-doomed"), false)
            .unwrap()
    );
    let globex = format!(
        "Bearer {}",
        app.create_api_key("globex", "write", Some("globex"), false)
            .unwrap()
    );
    let call = |method: &str, uri: &str, key: &str, body: Option<String>| {
        let mut b = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", key);
        if body.is_some() {
            b = b.header("content-type", "application/json");
        }
        let req = b
            .body(body.map(Body::from).unwrap_or_else(Body::empty))
            .unwrap();
        router.clone().oneshot(req)
    };

    // Both tenants create a connection with the SAME name.
    let body = serde_json::json!({"name":"c1","source":"Echo","dest":"ArrowSink","stream":"echo","config":{}}).to_string();
    for key in [&acme, &globex] {
        let resp = call("POST", "/connections", key, Some(body.clone()))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }
    // The acme key is now validated (and cached).
    let resp = call("GET", "/connections", &acme, None).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = call("DELETE", "/tenants/t0210-doomed", &admin, None)
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // The cached acme key is refused at once.
    let resp = call("GET", "/connections", &acme, None).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    // acme's rows are gone; globex's same-named connection is untouched.
    assert!(app.list_connections("t0210-doomed").unwrap().is_empty());
    assert!(!app.tenant_exists("t0210-doomed").unwrap());
    let resp = call("GET", "/connections", &globex, None).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let list = json(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["name"], "c1");

    // The default tenant cannot be deleted.
    let resp = call("DELETE", "/tenants/default", &admin, None)
        .await
        .unwrap();
    assert_ne!(resp.status(), StatusCode::NO_CONTENT);
    assert!(app.tenant_exists("default").unwrap());
}

fn post(uri: &str, auth: &str, body: Option<String>) -> Request<Body> {
    let mut r = Request::post(uri).header("authorization", auth);
    if body.is_some() {
        r = r.header("content-type", "application/json");
    }
    r.body(body.map(Body::from).unwrap_or_else(Body::empty))
        .unwrap()
}

/// [[WEIR-T-0209]]: a Write key of tenant A stopping a connection named like one of tenant B
/// stops only A's own; B's run keeps going. A's key is denied B's tenant-scoped stop route.
#[tokio::test]
async fn cross_tenant_stop_is_scoped_by_tenant_and_name() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let key = |t: &str| {
        format!(
            "Bearer {}",
            app.create_api_key(t, "write", Some(t), false).unwrap()
        )
    };
    let (acme, globex) = (key("acme"), key("globex"));

    // Both tenants create and start a resident connection with the same name.
    for k in [&acme, &globex] {
        let body = serde_json::json!({
            "name": "live", "source": "slow", "dest": "rest-dest", "stream": "s",
            "config": {}, "execution_mode": "resident",
        })
        .to_string();
        let resp = router
            .clone()
            .oneshot(post("/connections", k, Some(body)))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let resp = router
            .clone()
            .oneshot(post("/connections/live/start", k, None))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(json(resp).await["started"], true);
    }
    assert!(app.relay().has_active("acme", "live").unwrap());
    assert!(app.relay().has_active("globex", "live").unwrap());

    // acme stops `live`: only acme's unit is cancelled; globex's run keeps going.
    let resp = router
        .clone()
        .oneshot(post("/connections/live/stop", &acme, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json(resp).await["cancelled"], 1);
    assert!(!app.relay().has_active("acme", "live").unwrap());
    assert!(
        app.relay().has_active("globex", "live").unwrap(),
        "acme's stop must not touch globex's same-named run"
    );

    // A second stop by acme is a no-op — it never reaches globex's unit.
    let resp = router
        .clone()
        .oneshot(post("/connections/live/stop", &acme, None))
        .await
        .unwrap();
    assert_eq!(json(resp).await["cancelled"], 0);
    assert!(app.relay().has_active("globex", "live").unwrap());

    // acme's (non-admin) key is denied globex's tenant-scoped stop route.
    let resp = router
        .clone()
        .oneshot(post("/tenants/globex/connections/live/stop", &acme, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(app.relay().has_active("globex", "live").unwrap());
}

/// [[WEIR-T-0209]]: tenant ids are safe slugs (`^[a-z0-9][a-z0-9-]{0,62}$`); `../x` and other bad
/// ids are refused at creation with a 400 and a clear message.
#[tokio::test]
async fn create_tenant_refuses_bad_ids() {
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let token = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());
    let create = |id: &str| {
        post(
            "/tenants",
            &token,
            Some(serde_json::json!({ "id": id }).to_string()),
        )
    };
    let too_long = "a".repeat(64);
    for bad in [
        "../x", "..", "a/b", "/etc", "Acme", "-x", "a_b", "a.b", "", &too_long,
    ] {
        let resp = router.clone().oneshot(create(bad)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "id `{bad}`");
        let err = json(resp).await["error"].as_str().unwrap().to_string();
        assert!(err.contains("invalid tenant id"), "id `{bad}`: {err}");
    }
    assert!(
        app.list_tenants()
            .unwrap()
            .iter()
            .all(|t| t.id == "default"),
        "no bad tenant was stored"
    );
    let resp = router.clone().oneshot(create("acme-2")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

// ---- [[WEIR-T-0217]]: the server mirrors every route the UI re-scopes to `/tenants/{id}/…` ----

/// The UI's list of re-scoped calls (`METHOD /path`), shared with the UI's own drift test.
const UI_TENANT_ROUTES: &str = include_str!("../../../weir-ui/src/tenant_routes.txt");

fn ui_tenant_routes() -> Vec<(String, String)> {
    UI_TENANT_ROUTES
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (m, p) = l.split_once(' ').expect("METHOD PATH");
            (m.to_string(), p.trim().to_string())
        })
        .collect()
}

/// Fill each `{param}` segment with `nope`.
fn concrete(path: &str) -> String {
    path.split('/')
        .map(|seg| if seg.starts_with('{') { "nope" } else { seg })
        .collect::<Vec<_>>()
        .join("/")
}

#[tokio::test]
async fn every_ui_tenant_route_is_served_and_gated() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let admin = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());
    let acme = format!(
        "Bearer {}",
        app.create_api_key("acme", "admin", Some("acme"), false)
            .unwrap()
    );
    let routes = ui_tenant_routes();
    assert!(routes.len() > 5, "the shared list is too short: {routes:?}");
    for (method, path) in routes {
        let uri = format!("/tenants/globex{}", concrete(&path));
        let req = |token: &str| {
            Request::builder()
                .method(method.as_str())
                .uri(uri.as_str())
                .header("authorization", token)
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap()
        };
        // Platform admin acting on tenant globex: the route matches (no 405, no SPA fallback)
        // and authz lets it through (no 403).
        let resp = router.clone().oneshot(req(&admin)).await.unwrap();
        let status = resp.status();
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert_ne!(status, StatusCode::METHOD_NOT_ALLOWED, "{method} {uri}");
        assert_ne!(status, StatusCode::FORBIDDEN, "{method} {uri}");
        assert!(
            !ct.contains("text/html"),
            "{method} {uri} fell through to the SPA fallback (status {status})"
        );
        // A key of tenant acme is denied tenant globex.
        let resp = router.clone().oneshot(req(&acme)).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "acme key: {method} {uri}"
        );
    }
}

#[tokio::test]
async fn switched_admin_sets_up_another_tenant() {
    use_wasm_connectors();
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let admin = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());
    let acme = format!(
        "Bearer {}",
        app.create_api_key("acme", "write", Some("acme"), false)
            .unwrap()
    );
    let globex = format!(
        "Bearer {}",
        app.create_api_key("globex", "admin", Some("globex"), false)
            .unwrap()
    );

    // Connector spec + discover + catalog availability + preview answer under the tenant.
    let (s, spec) = send(
        &router,
        &admin,
        "GET",
        "/tenants/acme/connectors/Echo/spec",
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{spec}");
    let (s, _) = send(
        &router,
        &admin,
        "GET",
        "/tenants/acme/catalog/available",
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, streams) = send(
        &router,
        &admin,
        "POST",
        "/tenants/acme/connectors/Echo/discover",
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{streams}");
    let manifest = r#"
type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: coins
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://example.invalid"
        path: "/coins"
      record_selector:
        type: RecordSelector
        extractor:
          type: DpathExtractor
          field_path: []
    schema_loader:
      type: InlineSchemaLoader
      schema:
        type: object
        properties:
          id: { type: string }
"#;
    let (s, report) = send(
        &router,
        &admin,
        "POST",
        "/tenants/acme/catalog/preview",
        Some(serde_json::json!({ "manifest": manifest })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{report}");

    // Import lands in tenant acme's catalog, not the admin's own (default) one.
    let (s, entry) = send(
        &router,
        &admin,
        "POST",
        "/tenants/acme/catalog/import",
        Some(serde_json::json!({ "manifest": manifest, "name": "coins-src" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{entry}");
    let (_, cat) = send(&router, &acme, "GET", "/catalog", None).await;
    assert!(cat.to_string().contains("coins-src"), "{cat}");
    let (_, cat) = send(&router, &admin, "GET", "/catalog", None).await;
    assert!(!cat.to_string().contains("coins-src"), "{cat}");

    // Create in tenant acme: acme sees it, redacted; the store keeps the secret.
    let conn = |key: &str| {
        serde_json::json!({
            "name":"c1","source":"Echo","dest":"ArrowSink","stream":"echo",
            "config":{"api_key":key,"note":"v1"}
        })
    };
    let (s, body) = send(
        &router,
        &admin,
        "POST",
        "/tenants/acme/connections",
        Some(conn("s3cret")),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{body}");
    let (_, list) = send(&router, &acme, "GET", "/connections", None).await;
    assert_eq!(list[0]["name"], "c1");
    assert_eq!(list[0]["source_config"]["api_key"], SENTINEL);
    assert!(!list.to_string().contains("s3cret"));
    let (_, own) = send(&router, &admin, "GET", "/connections", None).await;
    assert_eq!(
        own.as_array().unwrap().len(),
        0,
        "nothing leaks into default"
    );

    // An edit through the tenant route that sends the sentinel keeps the stored secret.
    let mut edit = conn(SENTINEL);
    edit["config"]["note"] = "v2".into();
    let (s, body) = send(
        &router,
        &admin,
        "POST",
        "/tenants/acme/connections",
        Some(edit),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{body}");
    let stored = app.get_connection("acme", "c1").unwrap();
    assert!(stored.source_config.contains("s3cret"));
    assert!(stored.source_config.contains("v2"));

    // Schema view + accept answer for the tenant's connection.
    let (s, sv) = send(
        &router,
        &admin,
        "GET",
        "/tenants/acme/connections/c1/schema",
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{sv}");
    let (s, body) = send(
        &router,
        &admin,
        "POST",
        "/tenants/acme/connections/c1/schema/accept",
        None,
    )
    .await;
    assert!(s.is_success(), "{s} {body}");

    // A key of another tenant is denied each write.
    for (m, uri) in [
        ("POST", "/tenants/acme/connections"),
        ("DELETE", "/tenants/acme/connections/c1"),
        ("POST", "/tenants/acme/catalog/import"),
    ] {
        let (s, _) = send(&router, &globex, m, uri, Some(conn("x"))).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{m} {uri}");
    }

    // Delete in tenant acme, and unregister its connector.
    let (s, _) = send(
        &router,
        &admin,
        "DELETE",
        "/tenants/acme/connections/c1",
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (_, list) = send(&router, &acme, "GET", "/connections", None).await;
    assert_eq!(list.as_array().unwrap().len(), 0);
    let version = entry["version"].as_str().unwrap().to_string();
    let (s, _) = send(
        &router,
        &admin,
        "DELETE",
        &format!("/tenants/acme/catalog/coins-src/{version}"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (_, cat) = send(&router, &acme, "GET", "/catalog", None).await;
    assert!(!cat.to_string().contains("coins-src"), "{cat}");
}

#[tokio::test]
async fn tenant_key_revoke_is_tenant_scoped_and_purges_the_cache() {
    // WEIR-T-0221: DELETE /tenants/{id}/keys/{kid} revokes only the path tenant's key (by name or
    // id), and the revoked key is refused on the next request (cache purged, no TTL wait).
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let router = weir_api::router(Arc::clone(&app));
    let admin = format!("Bearer {}", app.bootstrap_admin_key().unwrap().unwrap());
    let a = app
        .create_api_key("ci", "write", Some("acme"), false)
        .unwrap();
    let b = app
        .create_api_key("ci", "write", Some("globex"), false)
        .unwrap();
    let (acme, globex) = (format!("Bearer {a}"), format!("Bearer {b}"));

    // Both keys work, and are now cached.
    for key in [&acme, &globex] {
        let (s, _) = send(&router, key, "GET", "/connections", None).await;
        assert_eq!(s, StatusCode::OK);
    }

    // Revoke acme's `ci` by name.
    let (s, _) = send(&router, &admin, "DELETE", "/tenants/acme/keys/ci", None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = send(&router, &acme, "GET", "/connections", None).await;
    assert_eq!(
        s,
        StatusCode::UNAUTHORIZED,
        "revoked key is refused at once"
    );
    let (s, _) = send(&router, &globex, "GET", "/connections", None).await;
    assert_eq!(
        s,
        StatusCode::OK,
        "same-named key of another tenant stays valid"
    );

    // globex's key id under the acme path matches nothing.
    let gid = app.validate_api_key(&b).unwrap().unwrap().key_id;
    let (s, _) = send(
        &router,
        &admin,
        "DELETE",
        &format!("/tenants/acme/keys/{gid}"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = send(&router, &globex, "GET", "/connections", None).await;
    assert_eq!(s, StatusCode::OK);

    // Revoke by id under its own tenant.
    let (s, _) = send(
        &router,
        &admin,
        "DELETE",
        &format!("/tenants/globex/keys/{gid}"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = send(&router, &globex, "GET", "/connections", None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

/// WEIR-T-0220: a credential that cannot be checked (a store error, e.g. sqlite "database is
/// locked") is a 503 with the reason, not a 401: a 401 signs the UI out. An unknown key is
/// still a 401.
#[tokio::test]
async fn auth_store_error_is_503_not_401() {
    use diesel::RunQueryDsl;
    let tmp = tempfile::TempDir::new().unwrap();
    let app = Arc::new(App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap());
    let key = format!(
        "Bearer {}",
        app.create_api_key("ops", "write", None, false).unwrap()
    );
    let router = weir_api::router(Arc::clone(&app));
    let get = |auth: String| {
        Request::get("/connections")
            .header("authorization", auth)
            .body(Body::empty())
            .unwrap()
    };

    let unknown = router
        .clone()
        .oneshot(get("Bearer weirk_not-a-key".into()))
        .await
        .unwrap();
    assert_eq!(unknown.status(), StatusCode::UNAUTHORIZED);

    // Break the key lookup. The key was never presented, so it is not cached: the store is asked.
    let mut conn = app.store().pool().get().unwrap();
    diesel::sql_query("ALTER TABLE api_keys RENAME TO api_keys_gone")
        .execute(&mut conn)
        .unwrap();
    drop(conn);
    let resp = router.oneshot(get(key)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = json(resp).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("could not check the credential"),
        "{body}"
    );
}
