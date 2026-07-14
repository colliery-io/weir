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
