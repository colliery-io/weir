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
    CompareOp, ComputeExpr, Config, ConfiguredStream, MappingOp, MappingSpec, SyncMode, WriteMode,
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
                    // A chunked body (the guest's `wasi:http` POSTs, [[WEIR-T-0154]]) has no
                    // content-length — it's complete at the zero-size terminator chunk.
                    if head
                        .to_ascii_lowercase()
                        .contains("transfer-encoding: chunked")
                    {
                        if data.ends_with(b"0\r\n\r\n") {
                            break;
                        }
                        continue;
                    }
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

/// Serves one `body` per request, in order (then `[]`), capturing each request
/// line — for multi-run tests where each engine run gets the next response.
fn mock_http_sequence(
    bodies: &'static [&'static str],
    requests: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        let mut i = 0usize;
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let req = read_full_request(&mut stream);
            requests
                .lock()
                .unwrap()
                .push(req.lines().next().unwrap_or_default().to_string());
            let body = bodies.get(i).copied().unwrap_or("[]");
            i += 1;
            respond_ok(&mut stream, body);
        }
    });
    format!("http://{addr}")
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

/// Serves Stripe-shaped pages ([[WEIR-T-0168]]): the next-page token is the LAST record's
/// `id` (`?starting_after=ch_N`) and `has_more:false` marks the final page — the server
/// thread exits after serving it, so any extra request the guest wrongly issued would fail
/// the test. Pages: (ch_1,ch_2) → (ch_3,ch_4) → (ch_5, has_more:false).
fn mock_http_stripe() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let after = req
                .split("starting_after=")
                .nth(1)
                .and_then(|s| s.split(['&', ' ']).next())
                .unwrap_or("");
            let body = match after {
                "" => r#"{"object":"list","has_more":true,"data":[{"id":"ch_1"},{"id":"ch_2"}]}"#,
                "ch_2" => {
                    r#"{"object":"list","has_more":true,"data":[{"id":"ch_3"},{"id":"ch_4"}]}"#
                }
                _ => r#"{"object":"list","has_more":false,"data":[{"id":"ch_5"}]}"#,
            };
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
            if !after.is_empty() && after != "ch_2" {
                break; // has_more:false page served → the guest must stop here
            }
        }
    });
    format!("http://{addr}")
}

/// Serves `/items?page=N&limit=2`: pages 1..=3 of 2 records + a short page 4
/// (ids 1..=7), then `[]`. While `healthy` is false, page 3 answers HTTP
/// `sick_status` — letting a test kill a run mid-pagination ([[WEIR-T-0184]] /
/// [[WEIR-T-0186]]). Every served page number is pushed to `requests`; serves
/// until the process exits.
fn mock_http_paged(
    healthy: std::sync::Arc<std::sync::atomic::AtomicBool>,
    requests: std::sync::Arc<std::sync::Mutex<Vec<u64>>>,
    sick_status: u16,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let page: u64 = req
                .split("page=")
                .nth(1)
                .and_then(|s| s.split(['&', ' ']).next())
                .and_then(|s| s.parse().ok())
                .unwrap_or(1);
            requests.lock().unwrap().push(page);
            let resp = if page == 3 && !healthy.load(std::sync::atomic::Ordering::SeqCst) {
                format!(
                    "HTTP/1.1 {sick_status} Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                )
            } else {
                let body = match page {
                    1 => r#"[{"id":1},{"id":2}]"#,
                    2 => r#"[{"id":3},{"id":4}]"#,
                    3 => r#"[{"id":5},{"id":6}]"#,
                    4 => r#"[{"id":7}]"#,
                    _ => "[]",
                };
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

/// Kill-at-page-N resume proof ([[WEIR-T-0184]]): the rest runtime checkpoints per
/// page, so a run that dies at page 3 keeps pages 1-2 committed and the NEXT run
/// resumes at page 3 from `StreamState.opaque` — no re-read, no gap. A third run
/// (after the clean finish cleared `opaque`) starts back at page 1.
#[test]
fn wasm_http_source_resumes_pagination_after_mid_run_failure() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let healthy = std::sync::Arc::new(AtomicBool::new(false));
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let base = mock_http_paged(healthy.clone(), requests.clone(), 500);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base,
            "path": "/items",
            "page_param": "page",
            "page_size_param": "limit",
            "page_size": 2,
            "max_retries": 0, // the 500 fails immediately instead of retrying
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

    // Run 1 dies at page 3 (the mock answers 500) — pages 1-2 are already committed.
    let err = engine.sync("resume-conn", &items_stream(), &source, &dest);
    assert!(err.is_err(), "run 1 must fail at the 500 page");
    assert_eq!(
        store.outbox_count("resume-conn").unwrap(),
        2,
        "two per-page checkpoints committed before the failure"
    );

    // Run 2 resumes at page 3 and finishes the read.
    healthy.store(true, Ordering::SeqCst);
    let out = engine
        .sync("resume-conn", &items_stream(), &source, &dest)
        .expect("resumed run completes");
    assert_eq!(
        out.rows_written, 3,
        "resume delivers pages 3-4 only (ids 5,6,7)"
    );
    {
        let reqs = requests.lock().unwrap();
        assert_eq!(
            &*reqs,
            &[1, 2, 3, 3, 4],
            "run 1 = pages 1,2,3(500); run 2 resumes at 3 — pages 1-2 never re-fetched"
        );
    }

    // Run 3: the clean finish cleared the resume state — a fresh read starts at page 1.
    let out = engine
        .sync("resume-conn", &items_stream(), &source, &dest)
        .expect("fresh run after clean finish");
    assert_eq!(
        out.rows_written, 7,
        "a fresh full-refresh re-reads everything"
    );
    assert_eq!(
        requests.lock().unwrap()[5],
        1,
        "run 3 starts back at page 1 (opaque was cleared by the clean finish)"
    );
}

/// max_pages ([[WEIR-T-0184]]): hitting the per-run page cap is LOUD (a Warn in
/// run_logs) and resumable (checkpoint committed) — never a silent truncation. The
/// next run continues exactly where the capped run stopped.
#[test]
fn wasm_http_source_max_pages_warns_and_resumes() {
    let healthy = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let base = mock_http_paged(healthy, requests.clone(), 500);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base,
            "path": "/items",
            "page_param": "page",
            "page_size_param": "limit",
            "page_size": 2,
            "max_pages": 2,
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

    // Run 1 hits the cap after 2 pages: SUCCEEDS, warns, checkpoints resumably.
    let out = engine
        .sync("cap-conn", &items_stream(), &source, &dest)
        .expect("capped run succeeds");
    assert_eq!(out.rows_written, 4, "pages 1-2 delivered under the cap");
    let logs = store.logs("cap-conn", 50).unwrap();
    assert!(
        logs.iter()
            .any(|l| l.level == "warn" && l.message.contains("max_pages")),
        "hitting the cap must leave a Warn naming max_pages in run_logs; got {logs:?}"
    );

    // Run 2 resumes past the cap and finishes.
    let out = engine
        .sync("cap-conn", &items_stream(), &source, &dest)
        .expect("run 2 resumes past the cap");
    assert_eq!(
        out.rows_written, 3,
        "pages 3-4 delivered by the resumed run"
    );
    assert_eq!(
        &*requests.lock().unwrap(),
        &[1, 2, 3, 4],
        "no page fetched twice across the capped runs"
    );
}

/// Status-aware end-of-data ([[WEIR-T-0186]]): a 401 mid-pagination FAILS the run —
/// never a silent partial "success" — with the status + page in the error; the
/// checkpoint through the last good page is already committed ([[WEIR-T-0184]]) so
/// the healed re-run resumes past it, and a 2xx page with an empty record array is
/// still the legitimate clean end (no `page_size` here, so the read ends by probing
/// the empty page 5).
#[test]
fn wasm_http_source_fails_run_on_mid_pagination_error_page() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let healthy = std::sync::Arc::new(AtomicBool::new(false));
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let base = mock_http_paged(healthy.clone(), requests.clone(), 401);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base,
            "path": "/items",
            "page_param": "page",
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

    // Run 1: the 401 at page 3 FAILS the run, naming status + page.
    let err = engine
        .sync("err-page-conn", &items_stream(), &source, &dest)
        .expect_err("a mid-pagination 401 must fail the run, not end it cleanly");
    let msg = format!("{err}");
    assert!(
        msg.contains("401") && msg.contains("page 3"),
        "the error must carry the status and page position; got: {msg}"
    );
    assert_eq!(
        store.outbox_count("err-page-conn").unwrap(),
        2,
        "pages 1-2 were committed before the failure"
    );

    // Run 2 (credential "healed"): resumes at page 3, ends cleanly on the
    // 2xx-empty page 5.
    healthy.store(true, Ordering::SeqCst);
    let out = engine
        .sync("err-page-conn", &items_stream(), &source, &dest)
        .expect("healed run completes");
    assert_eq!(out.rows_written, 3, "pages 3-4 delivered (ids 5,6,7)");
    assert_eq!(
        &*requests.lock().unwrap(),
        &[1, 2, 3, 3, 4, 5],
        "run 2 resumed at page 3 and stopped after the empty page 5"
    );
}

/// The sanctioned non-2xx stop ([[WEIR-T-0186]]): a 404 PAST page 1 is how
/// page-probing APIs signal one-past-the-end — still a clean end, since the same
/// URL shape already succeeded so it cannot be a config error.
#[test]
fn wasm_http_source_treats_404_past_last_page_as_end() {
    let healthy = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let base = mock_http_paged(healthy, requests.clone(), 404); // page 3 404s forever

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base,
            "path": "/items",
            "page_param": "page",
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
        .sync("probe-404-conn", &items_stream(), &source, &dest)
        .expect("the 404 one page past the end is a clean stop");
    assert_eq!(
        out.rows_written, 4,
        "pages 1-2 delivered; the 404 page ended the read"
    );
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

/// Stripe-shaped pagination ([[WEIR-T-0168]]): the guest takes the next-page token from the
/// LAST record's `id` (`?starting_after=…`) and stops when `has_more` is false — collecting
/// 2+2+1 = 5 records across 3 pages with no wasted empty-page request (the mock's server
/// thread exits after the final page, so an extra request would fail the run).
#[test]
fn wasm_http_source_walks_stripe_last_record_cursor() {
    let base = mock_http_stripe();

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base, "path": "/v1/charges",
            "record_path": "data",
            "page_cursor_record_field": "id",
            "page_cursor_param": "starting_after",
            "page_stop_on_false_path": "has_more",
            "page_size_param": "limit", "page_size": 2,
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
        .sync("stripe-conn", &items_stream(), &source, &dest)
        .expect("engine sync over stripe-shaped wasm http source");
    assert_eq!(out.rows_written, 5, "all 3 starting_after pages collected");
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
    // Query values are percent-encoded exactly once at build time ([[WEIR-T-0185]]).
    assert!(
        line.contains("since=2020-01-01T00%3A00%3A00Z")
            && line.contains("until=2026-12-31T00%3A00%3A00Z"),
        "request must carry the start + end datetime bounds, percent-encoded; got: {line}"
    );
}

/// Query values are percent-encoded exactly once at build time ([[WEIR-T-0185]]):
/// an ISO cursor with a `+02:00` offset must reach the wire as `%2B` (a raw `+`
/// decodes as a space server-side), and a host-injected query credential with
/// reserved characters must not corrupt the query string.
#[test]
fn wasm_http_source_percent_encodes_query_values() {
    let (base, req_rx) =
        mock_http_capture(r#"[{"id":1,"title":"hi","updated_at":"2026-01-01T00:00:00+02:00"}]"#);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    // Host-side query credential ([[WEIR-A-0033]]) with every reserved char that
    // corrupts a query, plus a `+`-bearing datetime lower bound from the guest.
    let cfg_json = serde_json::json!({
        "base_url": base, "path": "/posts",
        "auth_scheme": "query", "auth_name": "apikey", "api_key": "k+&=#z",
        "cursor_field": "updated_at", "cursor_param": "since",
        "cursor_start": "2020-01-01T00:00:00+02:00",
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
        .sync("enc-conn", &posts_stream(), &source, &dest)
        .expect("engine sync over percent-encoded query values");
    assert_eq!(out.rows_written, 1);

    let req = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("captured outbound request");
    let line = req.lines().next().unwrap_or_default();
    assert!(
        line.contains("since=2020-01-01T00%3A00%3A00%2B02%3A00"),
        "the datetime cursor must reach the wire with `+` as %2B, encoded once; got: {line}"
    );
    assert!(
        line.contains("apikey=k%2B%26%3D%23z"),
        "the query credential must be percent-encoded once host-side; got: {line}"
    );
    assert!(
        !line.contains("%25"),
        "nothing may be double-encoded (no %25 anywhere); got: {line}"
    );
}

/// Numeric-aware cursor advance ([[WEIR-T-0187]]): once a numeric cursor crosses a
/// digit boundary, a lexicographic max sticks at the old value (`"9" > "12"`) and
/// re-delivers those rows on every later run. Two incremental runs across the
/// 9 → 12 rollover must commit `12`.
#[test]
fn wasm_http_source_numeric_cursor_survives_digit_rollover() {
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    static BODIES: &[&str] = &[
        r#"[{"id":8},{"id":9}]"#,
        r#"[{"id":10},{"id":11},{"id":12}]"#,
    ];
    let base = mock_http_sequence(BODIES, requests.clone());

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base, "path": "/items",
            "cursor_field": "id", "cursor_param": "since",
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

    // Run 1: ids 8,9 → cursor "9".
    engine
        .sync("num-cursor-conn", &items_stream(), &source, &dest)
        .expect("run 1");
    assert_eq!(
        store.cursor("num-cursor-conn", "items").unwrap().as_deref(),
        Some("9")
    );

    // Run 2: ids 10,11,12 — the numeric max advances past the digit rollover
    // (a lexicographic compare would keep "9" and re-deliver 10-12 forever).
    engine
        .sync("num-cursor-conn", &items_stream(), &source, &dest)
        .expect("run 2");
    assert_eq!(
        store.cursor("num-cursor-conn", "items").unwrap().as_deref(),
        Some("12"),
        "the committed cursor must advance numerically past the rollover"
    );
    assert!(
        requests.lock().unwrap()[1].contains("since=9"),
        "run 2 asked the API for rows past the committed cursor"
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

/// A captured request's body: content-length bodies pass through; chunked bodies (the
/// guest's `wasi:http` POSTs, [[WEIR-T-0154]]) have their chunk framing stripped.
fn request_body(req: &str) -> String {
    let Some((head, raw)) = req.split_once("\r\n\r\n") else {
        return String::new();
    };
    if !head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        return raw.to_string();
    }
    // De-chunk: `<hex-size>\r\n<data>\r\n` … terminated by a zero-size chunk.
    let mut out = String::new();
    let mut rest = raw;
    loop {
        let Some((size_line, tail)) = rest.split_once("\r\n") else {
            break;
        };
        let Ok(size) = usize::from_str_radix(size_line.trim(), 16) else {
            break;
        };
        if size == 0 || tail.len() < size {
            break;
        }
        out.push_str(&tail[..size]);
        rest = tail.get(size + 2..).unwrap_or(""); // skip data + trailing CRLF
    }
    out
}

/// Serves Notion-style POST body-cursor pages ([[WEIR-T-0154]]): the opaque cursor
/// arrives as `start_cursor` **in the JSON body** (never the query string), the next
/// token in the response `next_cursor`. A page is served only when the request is a
/// POST whose static `filter` (from `request_body`) survived the per-page rebuild.
fn mock_http_body_cursor() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let req = read_full_request(&mut stream);
            let body_json: serde_json::Value =
                serde_json::from_str(&request_body(&req)).unwrap_or(serde_json::Value::Null);
            let well_formed = req.starts_with("POST")
                && !req.lines().next().unwrap_or("").contains("start_cursor=")
                && body_json["filter"]["value"] == "page";
            let cursor = body_json["start_cursor"].as_str().unwrap_or("");
            let body = match (well_formed, cursor) {
                (false, _) => r#"{"results":[],"next_cursor":null}"#,
                (true, "") => r#"{"results":[{"id":1},{"id":2}],"next_cursor":"c2"}"#,
                (true, "c2") => r#"{"results":[{"id":3},{"id":4}],"next_cursor":"c3"}"#,
                (true, _) => r#"{"results":[{"id":5}],"next_cursor":null}"#,
            };
            respond_ok(&mut stream, body);
            if !well_formed || (!cursor.is_empty() && cursor != "c2") {
                break; // last page (next_cursor null) served → guest stops
            }
        }
    });
    format!("http://{addr}")
}

/// Body-injected opaque-cursor pagination ([[WEIR-T-0154]], the Notion shape): POST with a
/// static body filter, the cursor advancing **inside the body**, all 3 pages collected.
#[test]
fn wasm_http_source_walks_body_cursor_pagination() {
    let base = mock_http_body_cursor();

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base, "path": "/v1/search",
            "http_method": "POST",
            "request_body": r#"{"filter":{"property":"object","value":"page"}}"#,
            "record_path": "results",
            "page_cursor_path": "next_cursor",
            "page_cursor_param": "start_cursor",
            "page_inject_into": "body",
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
        .sync("body-cursor-conn", &items_stream(), &source, &dest)
        .expect("engine sync over body-cursor-paginated wasm http source");
    assert_eq!(
        out.rows_written, 5,
        "all 3 body-cursor pages collected (2+2+1)"
    );
}

/// Serves GA4-style offset pagination **in the POST body** ([[WEIR-T-0154]]): `offset` +
/// `limit` are read from the JSON body, pages of 2, 2, 1 records (ids 1..=5).
fn mock_http_body_offset() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let req = read_full_request(&mut stream);
            let body_json: serde_json::Value =
                serde_json::from_str(&request_body(&req)).unwrap_or(serde_json::Value::Null);
            // Both params must ride the body as JSON numbers; the query must stay clean.
            let well_formed = req.starts_with("POST")
                && !req.lines().next().unwrap_or("").contains("offset=")
                && body_json["limit"].as_u64() == Some(2);
            let offset = body_json["offset"].as_u64().unwrap_or(0);
            let body = match (well_formed, offset) {
                (false, _) => r#"[]"#,
                (true, 0) => r#"[{"id":1},{"id":2}]"#,
                (true, 2) => r#"[{"id":3},{"id":4}]"#,
                (true, 4) => r#"[{"id":5}]"#,
                _ => r#"[]"#,
            };
            respond_ok(&mut stream, body);
            if !well_formed || offset >= 4 {
                break; // short page served → guest will stop
            }
        }
    });
    format!("http://{addr}")
}

/// Body-injected offset pagination ([[WEIR-T-0154]], the GA4 `runReport` shape): offset +
/// limit advance inside the POST body as JSON numbers; the guest walks to the short page.
#[test]
fn wasm_http_source_walks_offset_pagination_in_body() {
    let base = mock_http_body_offset();

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base, "path": "/v1beta/properties/123:runReport",
            "http_method": "POST",
            "offset_param": "offset",
            "page_size_param": "limit",
            "page_size": 2,
            "page_inject_into": "body",
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
        .sync("body-offset-conn", &items_stream(), &source, &dest)
        .expect("engine sync over body-offset-paginated wasm http source");
    assert_eq!(
        out.rows_written, 5,
        "all 3 body-offset pages collected (2+2+1)"
    );
}

/// Body-injected datetime cursor ([[WEIR-T-0154]]): the incremental lower bound lands at a
/// nested **dot-path** of the POST body (`filter.timestamp_after`), merged into the static
/// `request_body` without clobbering its sibling fields.
#[test]
fn wasm_http_source_sends_datetime_cursor_in_body() {
    let (base, rx) = mock_http_capture(r#"[{"id":1,"updated_at":"2026-02-01T00:00:00Z"}]"#);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base, "path": "/query",
            "http_method": "POST",
            "request_body": r#"{"filter":{"scope":"all"}}"#,
            "cursor_field": "updated_at",
            "cursor_param": "filter.timestamp_after",
            "cursor_start": "2026-01-01T00:00:00Z",
            "cursor_inject_into": "body",
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
        .sync("body-datetime-conn", &posts_stream(), &source, &dest)
        .expect("engine sync with a body-injected datetime cursor");
    assert_eq!(out.rows_written, 1);

    let req = rx.recv().expect("captured request");
    assert!(req.starts_with("POST"), "cursor-in-body implies POST");
    let body_json: serde_json::Value = serde_json::from_str(&request_body(&req))
        .unwrap_or_else(|_| panic!("JSON request body; raw request: {req:?}"));
    assert_eq!(
        body_json["filter"]["timestamp_after"], "2026-01-01T00:00:00Z",
        "cursor start injected at the nested dot-path"
    );
    assert_eq!(
        body_json["filter"]["scope"], "all",
        "static request_body fields survive the overlay"
    );
    assert!(
        !req.lines().next().unwrap_or("").contains("timestamp_after"),
        "the cursor must not leak into the query string"
    );
}

/// Serves page-increment pagination **in the POST body** ([[WEIR-T-0154]]): `page` +
/// `size` read from the JSON body, pages of 2, 2, 1 records (ids 1..=5).
fn mock_http_body_page() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let req = read_full_request(&mut stream);
            let body_json: serde_json::Value =
                serde_json::from_str(&request_body(&req)).unwrap_or(serde_json::Value::Null);
            let well_formed = req.starts_with("POST")
                && !req.lines().next().unwrap_or("").contains("page=")
                && body_json["size"].as_u64() == Some(2);
            let page = body_json["page"].as_u64().unwrap_or(0);
            let body = match (well_formed, page) {
                (false, _) => r#"[]"#,
                (true, 1) => r#"[{"id":1},{"id":2}]"#,
                (true, 2) => r#"[{"id":3},{"id":4}]"#,
                (true, 3) => r#"[{"id":5}]"#,
                _ => r#"[]"#,
            };
            respond_ok(&mut stream, body);
            if !well_formed || page >= 3 {
                break; // short page served → guest will stop
            }
        }
    });
    format!("http://{addr}")
}

/// Body-injected page-increment pagination ([[WEIR-T-0154]]): `page`/`size` advance inside
/// the POST body as JSON numbers; the guest walks to the short page.
#[test]
fn wasm_http_source_walks_page_pagination_in_body() {
    let base = mock_http_body_page();

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base, "path": "/list",
            "http_method": "POST",
            "page_param": "page",
            "page_size_param": "size",
            "page_size": 2,
            "page_inject_into": "body",
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
        .sync("body-page-conn", &items_stream(), &source, &dest)
        .expect("engine sync over body-page-paginated wasm http source");
    assert_eq!(
        out.rows_written, 5,
        "all 3 body-page pages collected (2+2+1)"
    );
}

/// Google service-account auth end-to-end ([[WEIR-T-0155]] / [[WEIR-A-0033]]): the **host**
/// signs the RS256 JWT-bearer assertion, exchanges it at the mock token endpoint, and
/// injects `Authorization: Bearer <minted token>` on the guest's request. The SA key
/// never enters the sandbox — asserted against the sanitized guest config.
#[test]
fn wasm_http_source_google_sa_host_mints_and_injects_bearer() {
    // A throwaway test key — never a real credential.
    const TEST_KEY_PEM: &str = include_str!("fixtures/google_sa_test_key.pem");
    // The host hits this to exchange the assertion (not subject to the guest egress policy).
    let (token_url, grant_rx) = mock_http_capture(r#"{"access_token":"tok-sa","expires_in":3600}"#);
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
        "auth_scheme": "google_service_account",
        "google_sa_key_key": "service_account_key",
        "google_scopes": ["https://www.googleapis.com/auth/analytics.readonly"],
        "service_account_key": {
            "client_email": "engine-test@x.iam.gserviceaccount.com",
            "private_key": TEST_KEY_PEM,
            "token_uri": token_url,
        },
    })
    .to_string();
    // The credential split must leave no key material in what reaches the guest.
    let (_, guest_json) = Credential::from_auth_config(&cfg_json);
    assert!(
        !guest_json.contains("PRIVATE KEY") && !guest_json.contains("client_email"),
        "SA key material must never reach the guest config; got: {guest_json}"
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
        .sync("google-sa-conn", &posts_stream(), &source, &dest)
        .expect("engine sync over google-sa wasm http source");
    assert_eq!(
        out.rows_written, 1,
        "the host-authed request returned the record"
    );

    let grant = grant_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("captured token-grant request");
    assert!(
        grant.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer")
            && grant.contains("assertion=eyJ"),
        "host must run the signed JWT-bearer grant; got:\n{grant}"
    );
    let req = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("captured outbound request");
    assert!(
        req.to_ascii_lowercase()
            .contains("authorization: bearer tok-sa"),
        "host must mint + inject the google bearer; got:\n{req}"
    );
}

/// Snowflake key-pair JWT end-to-end ([[WEIR-T-0156]] / [[WEIR-A-0033]]): the **host**
/// signs the self-issued RS256 JWT (no token endpoint — the JWT is the bearer) and
/// injects it plus `X-Snowflake-Authorization-Token-Type: KEYPAIR_JWT` on the guest's
/// request. The private key never enters the sandbox; account/user stay in the guest
/// config for `{{ config['account'] }}` templating.
#[test]
fn wasm_http_source_snowflake_keypair_host_signs_and_injects() {
    const TEST_KEY_PEM: &str = include_str!("fixtures/google_sa_test_key.pem");
    let (base, req_rx) =
        mock_http_capture(r#"[{"id":1,"title":"hi","updated_at":"2026-01-01T00:00:00Z"}]"#);

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let cfg_json = serde_json::json!({
        "base_url": base, "path": "/api/v2/statements",
        "auth_scheme": "snowflake_keypair_jwt",
        "account": "myorg-acct1",
        "user": "weir_demo",
        "private_key": TEST_KEY_PEM,
    })
    .to_string();
    // The split strips ONLY the private key — account/user remain for URL templating.
    let (_, guest_json) = Credential::from_auth_config(&cfg_json);
    assert!(
        !guest_json.contains("PRIVATE KEY"),
        "key material must never reach the guest config; got: {guest_json}"
    );
    assert!(
        guest_json.contains("\"account\":\"myorg-acct1\""),
        "account stays templatable in the guest config"
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
        .sync("snowflake-kp-conn", &posts_stream(), &source, &dest)
        .expect("engine sync over snowflake-keypair wasm http source");
    assert_eq!(out.rows_written, 1);

    let req = req_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("captured outbound request");
    let lower = req.to_ascii_lowercase();
    assert!(
        lower.contains("authorization: bearer eyj"),
        "host must inject the self-signed JWT bearer; got:\n{req}"
    );
    assert!(
        lower.contains("x-snowflake-authorization-token-type: keypair_jwt"),
        "host must inject the token-type header alongside the bearer; got:\n{req}"
    );
}

/// Serves GA4-style **columnar** pages ([[WEIR-T-0159]]): body-injected offset/limit
/// pagination, rows as `{dimensionValues:[{value},…], metricValues:[{value},…]}`.
/// Asserts the static `dateRanges` window survives the per-page body rebuild and no
/// cursor is injected anywhere (the cursor is track-only).
fn mock_http_ga4() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let req = read_full_request(&mut stream);
            let body_json: serde_json::Value =
                serde_json::from_str(&request_body(&req)).unwrap_or(serde_json::Value::Null);
            let well_formed = req.starts_with("POST")
                && body_json["limit"].as_u64() == Some(2)
                && body_json["dateRanges"][0]["startDate"] == "30daysAgo"
                && body_json.get("date").is_none(); // track-only: never injected
            let offset = body_json["offset"].as_u64().unwrap_or(0);
            let body = match (well_formed, offset) {
                (false, _) => r#"{"rows":[]}"#,
                (true, 0) => {
                    r#"{"rows":[
                        {"dimensionValues":[{"value":"20260701"},{"value":"Direct"}],"metricValues":[{"value":"12"}]},
                        {"dimensionValues":[{"value":"20260702"},{"value":"Organic Search"}],"metricValues":[{"value":"9"}]}
                    ],"rowCount":3}"#
                }
                (true, 2) => {
                    r#"{"rows":[
                        {"dimensionValues":[{"value":"20260703"},{"value":"Referral"}],"metricValues":[{"value":"4"}]}
                    ],"rowCount":3}"#
                }
                _ => r#"{"rows":[]}"#,
            };
            respond_ok(&mut stream, body);
            if !well_formed || offset >= 2 {
                break; // short page served → guest will stop
            }
        }
    });
    format!("http://{addr}")
}

/// GA4 end-to-end shape ([[WEIR-T-0159]]): POST-body offset pagination over a columnar
/// response, flattened to schema fields by dot-path Compute mappings, with a
/// **track-only** cursor extracted from `dimensionValues.0.value` and checkpointed.
#[test]
fn wasm_http_source_flattens_ga4_columnar_and_checkpoints_track_only_cursor() {
    let base = mock_http_ga4();

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base, "path": "/v1beta/properties/123:runReport",
            "http_method": "POST",
            "request_body": r#"{"dimensions":[{"name":"date"}],"dateRanges":[{"startDate":"30daysAgo","endDate":"today"}]}"#,
            "record_path": "rows",
            "offset_param": "offset",
            "page_size_param": "limit",
            "page_size": 2,
            "page_inject_into": "body",
            "cursor_field": "date",
            "cursor_value_path": "dimensionValues.0.value",
            // no cursor_param → track-only: checkpoint advances, nothing injected
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

    // Flatten the columnar rows to the schema fields with dot-path Computes.
    let stream = ConfiguredStream {
        stream: "traffic".to_string(),
        sync_mode: SyncMode::Incremental,
        cursor_field: Some("date".to_string()),
        primary_key: Some(vec!["date".to_string(), "channel".to_string()]),
        write_mode: WriteMode::Append,
        mapping: MappingSpec {
            ops: vec![
                MappingOp::Compute {
                    field: "date".to_string(),
                    value: ComputeExpr::Field("dimensionValues.0.value".to_string()),
                },
                MappingOp::Compute {
                    field: "channel".to_string(),
                    value: ComputeExpr::Field("dimensionValues.1.value".to_string()),
                },
                MappingOp::Compute {
                    field: "sessions".to_string(),
                    value: ComputeExpr::Field("metricValues.0.value".to_string()),
                },
                MappingOp::Drop {
                    fields: vec!["dimensionValues".to_string(), "metricValues".to_string()],
                },
            ],
        },
    };

    let out = engine
        .sync("ga4-conn", &stream, &source, &dest)
        .expect("engine sync over the GA4-shaped wasm http source");
    assert_eq!(
        out.rows_written, 3,
        "both body-offset pages collected (2+1)"
    );
    assert_eq!(
        store.cursor("ga4-conn", "traffic").expect("state"),
        Some("20260703".to_string()),
        "track-only cursor checkpointed from dimensionValues.0.value"
    );
}

/// Header-row zip ([[WEIR-T-0160]], the Google Sheets shape): `values` is row-arrays
/// whose first row is the header. The guest zips data rows into objects — snake_cased
/// header cells as field names, **ragged rows padded with null**, a `_row` index — so
/// a mapping can filter on a zipped field name.
#[test]
fn wasm_http_source_zips_header_row_values() {
    let base = mock_http_once(
        r#"{"range":"Sheet1!A1:C4","majorDimension":"ROWS","values":[
            ["Email Address","Full Name","Score"],
            ["a@x.com","Ada","10"],
            ["drop@x.com","Dee","3"],
            ["c@x.com","Cy"]
        ]}"#,
    );

    let pkg_root = tempfile::TempDir::new().unwrap();
    build_and_stage(pkg_root.path());
    let db_dir = tempfile::TempDir::new().unwrap();
    let store = Store::open(db_dir.path().join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&store);

    let src_cfg = Config {
        json: serde_json::json!({
            "base_url": base,
            "path": "/v4/spreadsheets/1AbC/values/Sheet1",
            "record_path": "values",
            "header_row": true,
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

    // Filtering on the snake_cased header name proves the zip produced named fields;
    // the ragged row (no Score cell) must still arrive, padded — not error.
    let stream = ConfiguredStream {
        stream: "rows".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: Some(vec!["_row".to_string()]),
        write_mode: WriteMode::Append,
        mapping: MappingSpec {
            ops: vec![MappingOp::Filter {
                field: "email_address".to_string(),
                op: CompareOp::Ne,
                value: "drop@x.com".to_string(),
            }],
        },
    };

    let out = engine
        .sync("sheets-conn", &stream, &source, &dest)
        .expect("engine sync over the header-row wasm http source");
    assert_eq!(
        out.rows_written, 2,
        "3 data rows (header consumed, ragged row padded) minus the filtered one"
    );
}
