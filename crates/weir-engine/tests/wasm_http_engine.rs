//! Follow-up to [[WEIR-T-0034]]: a **WASM source doing live `wasi:http`** driven
//! **through the Sync Engine** (not just the runtime seam). This is the case the
//! engine's off-tokio read loop exists for — wasmtime-wasi serves the guest's
//! `wasi:http` with its own `block_on`, which would panic ("runtime within a
//! runtime") if the engine drove the read on an entered tokio runtime. Proves the
//! whole pipeline (wasm http source → engine → native Arrow dest) works.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::thread;

use weir_connector::{
    CompareOp, Config, ConfiguredStream, MappingOp, MappingSpec, SyncMode, WriteMode,
};
use weir_engine::{Engine, Store};
use weir_runtime::{ConnectorHandle, Credential, HostAllowList};

/// Build the `rest` wasm guest and stage it as a fidius package with the
/// `http` capability under `root` — via the shared, cached build/stage seam
/// ([[WEIR-I-0011]] S1).
fn build_and_stage(root: &Path) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/connectors/rest");
    weir_wasm_testkit::stage(
        &weir_wasm_testkit::WasmPackage {
            fixture_dir: &fixture,
            wasm_file: "weir_rest_wasm.wasm",
            pkg_name: "weir-rest-pkg",
            capabilities: &["http"],
        },
        root,
    );
}

/// One-shot mock returning `body` (JSON). Returns the base URL (no trailing slash).
/// Read a full HTTP/1.1 request (request line + headers + any `content-length` body) from a
/// single-shot mock connection, so the mock responds only **after** the client has finished
/// sending. Responding + closing mid-send (the old single-`read` mocks did) races the client's
/// write and resets the connection — which made the OAuth grant (`ureq::post().send_form()`)
/// flakily fail with `HttpRequestDenied`. A read timeout bounds a malformed request.
fn read_full_request(stream: &mut std::net::TcpStream) -> String {
    stream
        .set_read_timeout(Some(std::time::Duration::from_millis(500)))
        .ok();
    let mut data = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&buf[..n]);
                if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&data[..end]);
                    let clen = head
                        .lines()
                        .find_map(|l| {
                            l.split_once(':')
                                .filter(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
                                .map(|(_, v)| v)
                        })
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if data.len() >= end + 4 + clen {
                        break;
                    }
                }
            }
            Err(_) => break, // timeout — take what we have
        }
    }
    String::from_utf8_lossy(&data).into_owned()
}

/// Write an HTTP 200 with `body`, then close the connection gracefully (FIN, not RST).
fn respond_ok(stream: &mut std::net::TcpStream, body: &str) {
    let resp = format!(
        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.flush();
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

fn mock_http_once(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{addr}");
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = read_full_request(&mut stream);
            respond_ok(&mut stream, body);
        }
    });
    url
}

/// Like `mock_http_once` but also hands back the raw request bytes, so a test can
/// assert on the **outgoing** headers (e.g. that the connector sent `Authorization`).
fn mock_http_capture(body: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{addr}");
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = tx.send(read_full_request(&mut stream));
            respond_ok(&mut stream, body);
        }
    });
    (url, rx)
}

/// Serves `/items?offset=N&limit=2` as pages of 2, 2, 1 records (ids 1..=5), then stops.
/// Lets a test prove the guest walks **offset** pagination (advancing by page_size) and
/// terminates on the short page.
fn mock_http_offset() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let offset: usize = req
                .split("offset=")
                .nth(1)
                .and_then(|s| s.split(['&', ' ']).next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let body = match offset {
                0 => r#"[{"id":1},{"id":2}]"#,
                2 => r#"[{"id":3},{"id":4}]"#,
                4 => r#"[{"id":5}]"#,
                _ => r#"[]"#,
            };
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
            if offset >= 4 {
                break; // short page served → guest will stop
            }
        }
    });
    format!("http://{addr}")
}

/// Serves opaque-cursor pages: no cursor → ids 1,2 (next "c2"); `cursor=c2` → 3,4 (next
/// "c3"); `cursor=c3` → 5 (next ""), then stops. Records under `data`, next token under
/// `meta.next`. Lets a test prove the guest walks a response-token cursor to exhaustion.
fn mock_http_cursor() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let cursor = req
                .split("cursor=")
                .nth(1)
                .and_then(|s| s.split(['&', ' ']).next())
                .unwrap_or("");
            let body = match cursor {
                "" => r#"{"data":[{"id":1},{"id":2}],"meta":{"next":"c2"}}"#,
                "c2" => r#"{"data":[{"id":3},{"id":4}],"meta":{"next":"c3"}}"#,
                _ => r#"{"data":[{"id":5}],"meta":{"next":""}}"#,
            };
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
            if !cursor.is_empty() && cursor != "c2" {
                break; // c3 page (next="") served → guest stops
            }
        }
    });
    format!("http://{addr}")
}

fn items_stream() -> ConfiguredStream {
    ConfiguredStream {
        stream: "items".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: Some(vec!["id".to_string()]),
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    }
}

fn posts_stream() -> ConfiguredStream {
    ConfiguredStream {
        stream: "posts".to_string(),
        sync_mode: SyncMode::Incremental,
        cursor_field: Some("updated_at".to_string()),
        primary_key: Some(vec!["id".to_string()]),
        write_mode: WriteMode::Append,
        mapping: MappingSpec::default(),
    }
}

/// Build a wasm `rest` source the way the orchestrator does ([[WEIR-A-0033]]): split the
/// secret out of `cfg_json` into a host-side [`Credential`] and hand the guest only the
/// sanitized config, so the credential is injected host-side and never enters the sandbox.
/// `allowed_hosts` is pinned to loopback (the mock servers).
fn host_auth_source(pkg_root: &Path, cfg_json: &str) -> ConnectorHandle {
    let (credential, guest_json) = Credential::from_auth_config(cfg_json);
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential,
    };
    ConnectorHandle::from_wasm_package(
        pkg_root,
        "weir-rest-pkg",
        &Config { json: guest_json },
        policy,
        &[],
    )
    .expect("load wasm rest http source")
}

#[test]
fn engine_drives_wasm_http_source_to_arrow_dest() {
    // One record (< page size 2) so the paginating guest stops after one request.
    let base = mock_http_once(r#"[{"id":1,"title":"hello","updated_at":"2026-01-01T00:00:00Z"}]"#);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    // Source: the wasm rest, base_url bound at construction + egress allowed.
    let src_cfg = Config {
        json: format!("{{\"base_url\":\"{base}\",\"path\":\"/posts\"}}"),
    };
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let source =
        ConnectorHandle::from_wasm_package(pkg_root.path(), "weir-rest-pkg", &src_cfg, policy, &[])
            .expect("load wasm rest http source");

    let dest_cfg = Config {
        json: "{}".to_string(),
    };
    let dest = weir_wasm_testkit::load("ArrowSink", &dest_cfg).unwrap();

    // The engine drives the wasm source's streaming read (incl. wasi:http) off-tokio.
    let out = engine
        .sync("wasm-http-conn", &posts_stream(), &source, &dest)
        .expect("engine sync over wasm http source");

    assert_eq!(
        out.rows_written, 1,
        "the record fetched over wasi:http flowed through the engine to the Arrow sink"
    );
    assert_eq!(store.outbox_count("wasm-http-conn").unwrap(), 1);
}

/// Bearer auth is injected **host-side** ([[WEIR-A-0033]]): the config carries
/// `auth_scheme=bearer` + `api_key`, the host strips the secret from the guest config and
/// the egress policy adds `Authorization: Bearer <key>`. Proven over the real wasi:http
/// wire — and the guest never receives the token.
#[test]
fn wasm_http_source_sends_bearer_auth_header() {
    let (base, req_rx) =
        mock_http_capture(r#"[{"id":1,"title":"hi","updated_at":"2026-01-01T00:00:00Z"}]"#);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg_json = format!(
        "{{\"base_url\":\"{base}\",\"path\":\"/posts\",\"auth_scheme\":\"bearer\",\"api_key\":\"s3cr3t-token\"}}"
    );
    let source = host_auth_source(pkg_root.path(), &cfg_json);
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("auth-conn", &posts_stream(), &source, &dest)
        .expect("engine sync over authed wasm http source");
    assert_eq!(
        out.rows_written, 1,
        "the authed request returned the record"
    );

    let req = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("captured outbound request");
    assert!(
        req.to_ascii_lowercase()
            .contains("authorization: bearer s3cr3t-token"),
        "outbound request must carry the host-injected bearer header; got:\n{req}"
    );
}

/// Query-param api-key ([[WEIR-A-0033]]): with `auth_scheme=query` the host rewrites the
/// outbound URI to append `?apikey=<key>` (the NASA/Ticketmaster shape) — host-side, so
/// the key never reaches the guest config.
#[test]
fn wasm_http_source_sends_query_param_key() {
    let (base, req_rx) =
        mock_http_capture(r#"[{"id":1,"title":"hi","updated_at":"2026-01-01T00:00:00Z"}]"#);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg_json = serde_json::json!({
        "base_url": base, "path": "/posts",
        "auth_scheme": "query", "auth_name": "apikey", "api_key": "qk-123",
    })
    .to_string();
    let source = host_auth_source(pkg_root.path(), &cfg_json);
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("qk-conn", &posts_stream(), &source, &dest)
        .expect("engine sync over query-key wasm http source");
    assert_eq!(out.rows_written, 1);

    let req = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("captured outbound request");
    let line = req.lines().next().unwrap_or_default();
    assert!(
        line.contains("apikey=qk-123"),
        "request line must carry the host-injected query-param key; got: {line}"
    );
}

/// HTTP Basic ([[WEIR-A-0033]]): the host injects `Authorization: Basic base64(user:pass)`;
/// the username/password never reach the guest config.
#[test]
fn wasm_http_source_sends_basic_auth_header() {
    let (base, req_rx) =
        mock_http_capture(r#"[{"id":1,"title":"hi","updated_at":"2026-01-01T00:00:00Z"}]"#);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg_json = serde_json::json!({
        "base_url": base, "path": "/posts",
        "auth_scheme": "basic",
        "basic_username_key": "username", "basic_password_key": "password",
        "username": "alice", "password": "s3cr3t",
    })
    .to_string();
    let source = host_auth_source(pkg_root.path(), &cfg_json);
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("basic-conn", &posts_stream(), &source, &dest)
        .expect("engine sync over basic-auth wasm http source");
    assert_eq!(out.rows_written, 1);

    let req = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("captured outbound request");
    // base64("alice:s3cr3t") = YWxpY2U6czNjcjN0 (base64 is case-sensitive, so match exactly).
    assert!(
        req.to_ascii_lowercase().contains("authorization: basic")
            && req.contains("YWxpY2U6czNjcjN0"),
        "host must inject Basic auth (base64 of user:pass); got:\n{req}"
    );
}

/// OAuth2 end-to-end ([[WEIR-A-0033]]): the **host** performs the token grant (a real
/// `ureq` POST to the mock token endpoint), caches it, and injects
/// `Authorization: Bearer <minted token>` on the guest's request. The client secret never
/// enters the sandbox. Uses the client-credentials grant (no refresh token needed).
#[test]
fn wasm_http_source_oauth_host_mints_and_injects_bearer() {
    // The host hits this to mint a token (not subject to the guest egress policy).
    let token_url = mock_http_once(r#"{"access_token":"tok-xyz","expires_in":3600}"#);
    // The guest hits this; we capture the host-injected Authorization header.
    let (base, req_rx) =
        mock_http_capture(r#"[{"id":1,"title":"hi","updated_at":"2026-01-01T00:00:00Z"}]"#);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg_json = serde_json::json!({
        "base_url": base, "path": "/posts",
        "auth_scheme": "oauth2",
        "oauth_token_url": token_url,
        "oauth_grant": "client_credentials",
        "oauth_client_id_key": "client_id",
        "oauth_client_secret_key": "client_secret",
        "client_id": "cid", "client_secret": "csec",
    })
    .to_string();
    let source = host_auth_source(pkg_root.path(), &cfg_json);
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("oauth-conn", &posts_stream(), &source, &dest)
        .expect("engine sync over oauth wasm http source");
    assert_eq!(
        out.rows_written, 1,
        "the host-authed request returned the record"
    );

    let req = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("captured outbound request");
    assert!(
        req.to_ascii_lowercase()
            .contains("authorization: bearer tok-xyz"),
        "host must mint + inject the oauth bearer; got:\n{req}"
    );
}

/// Templated `url_base` ([[WEIR-I-0008]]): a `{{ config['…'] }}` host (tenant subdomain /
/// account id) is substituted from the per-connection config before the request goes out,
/// so the guest reaches the configured host and the record flows through.
#[test]
fn wasm_http_source_renders_templated_url_base() {
    let (base, _rx) =
        mock_http_capture(r#"[{"id":1,"title":"hi","updated_at":"2026-01-01T00:00:00Z"}]"#);
    let hostport = base.strip_prefix("http://").unwrap();

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    // base_url is a template; the per-connection config carries the host it resolves to.
    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": "http://{{ config['mock_host'] }}",
            "path": "/posts",
            "mock_host": hostport,
        })
        .to_string(),
    };
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let source =
        ConnectorHandle::from_wasm_package(pkg_root.path(), "weir-rest-pkg", &src_cfg, policy, &[])
            .expect("load wasm rest http source");
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("tmpl-conn", &posts_stream(), &source, &dest)
        .expect("engine sync over templated url_base");
    assert_eq!(
        out.rows_written, 1,
        "templated host resolved → record fetched"
    );
}

/// Offset pagination ([[WEIR-I-0008]]): with `offset_param` + `page_size`, the guest walks
/// `?offset=0,2,4…` advancing by page_size, collecting every page and stopping on the
/// short one — here 2+2+1 = 5 records across 3 requests.
#[test]
fn wasm_http_source_walks_offset_pagination() {
    let base = mock_http_offset();

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base,
            "path": "/items",
            "offset_param": "offset",
            "page_size_param": "limit",
            "page_size": 2,
        })
        .to_string(),
    };
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let source =
        ConnectorHandle::from_wasm_package(pkg_root.path(), "weir-rest-pkg", &src_cfg, policy, &[])
            .expect("load wasm rest http source");
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("offset-conn", &items_stream(), &source, &dest)
        .expect("engine sync over offset-paginated wasm http source");
    assert_eq!(out.rows_written, 5, "all 3 offset pages collected (2+2+1)");
}

/// Opaque cursor/token pagination ([[WEIR-I-0008]]): the guest reads the next-page token
/// from the response (`meta.next`), sends it as `?cursor=<token>`, and stops when the token
/// is empty — collecting 2+2+1 = 5 records across 3 pages. The slack/square/intercom shape.
#[test]
fn wasm_http_source_walks_cursor_pagination() {
    let base = mock_http_cursor();

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base, "path": "/items",
            "record_path": "data",
            "page_cursor_path": "meta.next",
            "page_cursor_param": "cursor",
        })
        .to_string(),
    };
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let source =
        ConnectorHandle::from_wasm_package(pkg_root.path(), "weir-rest-pkg", &src_cfg, policy, &[])
            .expect("load wasm rest http source");
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("cursor-conn", &items_stream(), &source, &dest)
        .expect("engine sync over cursor-paginated wasm http source");
    assert_eq!(out.rows_written, 5, "all 3 cursor pages collected (2+2+1)");
}

/// Single-object response ([[WEIR-I-0008]]): an endpoint returning a bare JSON object (not
/// an array) — xkcd's `/info.0.json` — is emitted as exactly one record.
#[test]
fn wasm_http_source_emits_single_object() {
    let base = mock_http_once(r#"{"num":2960,"title":"weir","safe_title":"weir"}"#);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({ "base_url": base, "path": "/info.0.json" }).to_string(),
    };
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let source =
        ConnectorHandle::from_wasm_package(pkg_root.path(), "weir-rest-pkg", &src_cfg, policy, &[])
            .expect("load wasm rest http source");
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("obj-conn", &items_stream(), &source, &dest)
        .expect("engine sync over single-object wasm http source");
    assert_eq!(out.rows_written, 1, "the single object is one record");
}

/// Substream mock ([[WEIR-T-0064]]): `/posts` lists two parent ids; each
/// `/posts/<id>/comments` returns that post's comments. Multi-connection (one per slice).
fn mock_http_substream() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let line = String::from_utf8_lossy(&buf[..n])
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            // Specific child paths first (they contain "/posts").
            let body = if line.contains("/posts/1/comments") {
                r#"[{"id":11,"post":1}]"#
            } else if line.contains("/posts/2/comments") {
                r#"[{"id":21,"post":2},{"id":22,"post":2}]"#
            } else if line.contains("/posts") {
                r#"[{"id":1},{"id":2}]"#
            } else {
                r#"[]"#
            };
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    format!("http://{addr}")
}

/// List-router mock ([[WEIR-T-0064]]): `/cat/<value>/items` returns one record per call.
fn mock_http_list() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let body = r#"[{"id":1}]"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    format!("http://{addr}")
}

/// SubstreamPartitionRouter end-to-end ([[WEIR-T-0064]]): the runtime reads the parent
/// `/posts`, then one `/posts/<id>/comments` request per parent id, concatenating the
/// child records (1 for post 1 + 2 for post 2 = 3).
#[test]
fn wasm_http_source_walks_substream_partitions() {
    let base = mock_http_substream();
    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg = Config {
        json: serde_json::json!({
            "base_url": base,
            "path": "/posts/{{ stream_partition.post_id }}/comments",
            "partition_kind": "substream",
            "partition_field": "post_id",
            "parent_path": "/posts",
            "parent_key": "id",
        })
        .to_string(),
    };
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let source =
        ConnectorHandle::from_wasm_package(pkg_root.path(), "weir-rest-pkg", &cfg, policy, &[])
            .expect("load wasm rest http source");
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("substream-conn", &items_stream(), &source, &dest)
        .expect("engine sync over substream wasm http source");
    assert_eq!(out.rows_written, 3, "1 comment for post 1 + 2 for post 2");
}

/// ListPartitionRouter end-to-end ([[WEIR-T-0064]]): one request per static value, child
/// records concatenated (2 values → 2 records).
#[test]
fn wasm_http_source_walks_list_partitions() {
    let base = mock_http_list();
    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg = Config {
        json: serde_json::json!({
            "base_url": base,
            "path": "/cat/{{ stream_partition.category }}/items",
            "partition_kind": "list",
            "partition_field": "category",
            "partition_values": ["a", "b"],
        })
        .to_string(),
    };
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let source =
        ConnectorHandle::from_wasm_package(pkg_root.path(), "weir-rest-pkg", &cfg, policy, &[])
            .expect("load wasm rest http source");
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("list-conn", &items_stream(), &source, &dest)
        .expect("engine sync over list-partition wasm http source");
    assert_eq!(out.rows_written, 2, "one record per list value");
}

/// Link-header mock ([[WEIR-T-0070]]): page 1 (`/posts`) returns 2 records + a
/// `Link: <…/posts?page=2>; rel="next"` header; page 2 returns 1 record + no Link.
fn mock_http_link() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");
    let link_base = base.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let line = String::from_utf8_lossy(&buf[..n])
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            let (body, link) = if line.contains("page=2") {
                (r#"[{"id":3}]"#, None)
            } else {
                (
                    r#"[{"id":1},{"id":2}]"#,
                    Some(format!("{link_base}/posts?page=2")),
                )
            };
            let link_hdr = link
                .map(|u| format!("link: <{u}>; rel=\"next\"\r\n"))
                .unwrap_or_default();
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n{}connection: close\r\n\r\n{}",
                body.len(),
                link_hdr,
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    base
}

/// Link-header pagination end-to-end ([[WEIR-T-0070]]): the runtime reads the response
/// `Link` header's `rel="next"` URL and follows it (absolute) until absent — 2 + 1 = 3.
#[test]
fn wasm_http_source_walks_link_header_pagination() {
    let base = mock_http_link();
    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg = Config {
        json: serde_json::json!({
            "base_url": base, "path": "/posts", "page_link_header": true,
        })
        .to_string(),
    };
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let source =
        ConnectorHandle::from_wasm_package(pkg_root.path(), "weir-rest-pkg", &cfg, policy, &[])
            .expect("load wasm rest http source");
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("link-conn", &items_stream(), &source, &dest)
        .expect("engine sync over link-header wasm http source");
    assert_eq!(
        out.rows_written, 3,
        "2 on page 1 + 1 on page 2 followed via the Link header"
    );
}

/// Richer datetime cursor ([[WEIR-T-0070]]): the first run (no state) sends the configured
/// lower bound (`cursor_start`), and every request carries the upper bound
/// (`cursor_end_param`=`cursor_end`) — e.g. `?since=<start>&until=<end>`.
#[test]
fn wasm_http_source_sends_datetime_start_end_bounds() {
    let (base, req_rx) =
        mock_http_capture(r#"[{"id":1,"title":"hi","updated_at":"2026-01-01T00:00:00Z"}]"#);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg = Config {
        json: serde_json::json!({
            "base_url": base, "path": "/posts",
            "cursor_field": "updated_at", "cursor_param": "since",
            "cursor_start": "2020-01-01T00:00:00Z",
            "cursor_end_param": "until", "cursor_end": "2026-12-31T00:00:00Z",
        })
        .to_string(),
    };
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let source =
        ConnectorHandle::from_wasm_package(pkg_root.path(), "weir-rest-pkg", &cfg, policy, &[])
            .expect("load wasm rest http source");
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("dt-conn", &posts_stream(), &source, &dest)
        .expect("engine sync over datetime-bounded wasm http source");
    assert_eq!(out.rows_written, 1);

    let req = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("captured outbound request");
    let line = req.lines().next().unwrap_or_default();
    assert!(
        line.contains("since=2020-01-01T00:00:00Z") && line.contains("until=2026-12-31T00:00:00Z"),
        "request must carry the start + end datetime bounds; got: {line}"
    );
}

/// Retry mock ([[WEIR-T-0069]]): the first request → 429 (Retry-After: 0); every
/// subsequent request → 200 with a record.
fn mock_http_retry() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let n = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let resp = if n == 0 {
                "HTTP/1.1 429 Too Many Requests\r\nretry-after: 0\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_string()
            } else {
                let body = r#"[{"id":1}]"#;
                format!(
                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
            };
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    format!("http://{addr}")
}

/// Transient-error retry end-to-end ([[WEIR-T-0069]]): the API returns 429 once (with
/// Retry-After: 0), the connector backs off + retries, and the second attempt succeeds.
#[test]
fn wasm_http_source_retries_on_429() {
    let base = mock_http_retry();
    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg = Config {
        json: serde_json::json!({ "base_url": base, "path": "/posts" }).to_string(),
    };
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let source =
        ConnectorHandle::from_wasm_package(pkg_root.path(), "weir-rest-pkg", &cfg, policy, &[])
            .expect("load wasm rest http source");
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("retry-conn", &items_stream(), &source, &dest)
        .expect("engine sync retries past the 429");
    assert_eq!(
        out.rows_written, 1,
        "the connector retried the 429 and got the record"
    );
}

/// POST request options end-to-end ([[WEIR-T-0068]]): the runtime issues a POST with the
/// configured JSON body and a static header (e.g. Notion-Version) — auth still host-side.
#[test]
fn wasm_http_source_posts_with_body_and_headers() {
    let (base, req_rx) = mock_http_capture(r#"{"results":[{"id":1}]}"#);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg = Config {
        json: serde_json::json!({
            "base_url": base, "path": "/query", "record_path": "results",
            "http_method": "POST",
            "request_body": "{\"page_size\":100}",
            "request_headers": { "Notion-Version": "2022-06-28" },
        })
        .to_string(),
    };
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let source =
        ConnectorHandle::from_wasm_package(pkg_root.path(), "weir-rest-pkg", &cfg, policy, &[])
            .expect("load wasm rest http source");
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let out = engine
        .sync("post-conn", &items_stream(), &source, &dest)
        .expect("engine sync over POST wasm http source");
    assert_eq!(out.rows_written, 1);

    let req = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("captured outbound request");
    let line = req.lines().next().unwrap_or_default();
    assert!(
        line.contains("POST") && line.contains("/query"),
        "expected POST /query; got: {line}"
    );
    assert!(
        req.to_ascii_lowercase()
            .contains("notion-version: 2022-06-28"),
        "expected the static header; got:\n{req}"
    );
    assert!(
        req.contains("{\"page_size\":100}"),
        "expected the POST body; got:\n{req}"
    );
}

/// The engine mapping stage ([[WEIR-T-0052]]) applied over a real wasm http source, driven by
/// a manifest transform ([[WEIR-T-0071]]): a `record_filter` → `MappingOp::Filter` keeps only
/// the matching records. 3 records in, 2 (`keep=yes`) written — proving transforms reshape the
/// stream end-to-end (source → engine mapping → dest), not just in the mapping unit tests.
#[test]
fn engine_applies_transform_mapping_over_wasm_source() {
    let base =
        mock_http_once(r#"[{"id":1,"keep":"yes"},{"id":2,"keep":"no"},{"id":3,"keep":"yes"}]"#);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg = Config {
        json: serde_json::json!({ "base_url": base, "path": "/items" }).to_string(),
    };
    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential: None,
    };
    let source =
        ConnectorHandle::from_wasm_package(pkg_root.path(), "weir-rest-pkg", &cfg, policy, &[])
            .expect("load wasm rest http source");
    let dest = weir_wasm_testkit::load(
        "ArrowSink",
        &Config {
            json: "{}".to_string(),
        },
    )
    .unwrap();

    let stream = ConfiguredStream {
        stream: "items".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: Some(vec!["id".to_string()]),
        write_mode: WriteMode::Append,
        mapping: MappingSpec {
            ops: vec![MappingOp::Filter {
                field: "keep".to_string(),
                op: CompareOp::Eq,
                value: "yes".to_string(),
            }],
        },
    };

    let out = engine
        .sync("map-conn", &stream, &source, &dest)
        .expect("engine sync applies the transform mapping over the wasm source");
    assert_eq!(
        out.rows_written, 2,
        "the filter kept only the keep=yes records"
    );
}
