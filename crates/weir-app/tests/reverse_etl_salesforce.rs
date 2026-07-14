//! [[WEIR-T-0075]] (reverse-ETL S5): **Salesforce as a manifest** — the headline
//! differentiator — on the shared `rest-dest` runtime, with the one genuinely-new piece:
//! **host-side OAuth token refresh**. Bakes `dest-manifests/salesforce.yaml`, mints an access
//! token host-side via the OAuth2 provider (reused from the source arc, [[WEIR-A-0033]]),
//! upserts sObjects by External Id (`PATCH …/Contact/ExternalId__c/{id}`), and — with a short
//! `expires_in` — **re-mints the token per request** (expiry-driven refresh). The client secret
//! never reaches the guest; a rejected record dead-letters; replay is idempotent.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};
use weir_engine::{Engine, Store};
use weir_manifest::DestinationManifest;
use weir_runtime::{ConnectorHandle, Credential, HostAllowList};

fn stage_both(root: &Path) {
    let connectors = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/connectors");
    for (dir, wasm, pkg) in [
        ("rest", "weir_rest_wasm.wasm", "weir-rest-pkg"),
        (
            "rest-dest",
            "weir_rest_dest_wasm.wasm",
            "weir-rest-dest-pkg",
        ),
    ] {
        weir_wasm_testkit::stage(
            &weir_wasm_testkit::WasmPackage {
                fixture_dir: &connectors.join(dir),
                wasm_file: wasm,
                pkg_name: pkg,
                capabilities: &["http"],
            },
            root,
        );
    }
}

fn mock_source(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
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

struct SfState {
    mints: usize,
    upserts: HashMap<String, String>,
}

/// Mock Salesforce: the OAuth token endpoint (short-lived tokens → forces refresh) + the
/// sObject upsert API (keyed by External-Id path; rejects `reject` with 400).
fn mock_salesforce() -> (String, Arc<Mutex<SfState>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let state = Arc::new(Mutex::new(SfState {
        mints: 0,
        upserts: HashMap::new(),
    }));
    let st = state.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).into_owned();
            let path = req.split_whitespace().nth(1).unwrap_or("").to_string();
            let (status, body) = if path.starts_with("/services/oauth2/token") {
                let mut g = st.lock().unwrap();
                g.mints += 1;
                let n = g.mints;
                (
                    "200 OK".to_string(),
                    format!(r#"{{"access_token":"sf-{n}","token_type":"Bearer","expires_in":1}}"#),
                )
            } else if req.contains("/reject") {
                (
                    "400 Bad Request".to_string(),
                    r#"{"error":"bad external id"}"#.to_string(),
                )
            } else {
                st.lock().unwrap().upserts.insert(path.clone(), req.clone());
                (
                    "200 OK".to_string(),
                    r#"{"id":"003","success":true}"#.to_string(),
                )
            };
            let resp = format!(
                "HTTP/1.1 {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                status,
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    (format!("http://{addr}"), state)
}

fn contacts_stream() -> ConfiguredStream {
    ConfiguredStream {
        stream: "contacts".to_string(),
        sync_mode: SyncMode::FullRefresh,
        cursor_field: None,
        primary_key: Some(vec!["ext_id".to_string()]),
        write_mode: WriteMode::Upsert {
            business_keys: vec!["ext_id".to_string()],
        },
        mapping: MappingSpec::default(),
    }
}

#[test]
fn salesforce_manifest_upserts_with_oauth_refresh() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    stage_both(root);

    let (sf, state) = mock_salesforce();

    // Bake the vendored Salesforce manifest, then point token + API at the mock and supply the
    // per-connection secrets (client_id/secret) that the host uses to mint — never the guest.
    let manifest_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dest-manifests/salesforce-dest.yaml");
    let m = DestinationManifest::from_yaml(&std::fs::read_to_string(&manifest_path).unwrap())
        .expect("parse salesforce dest manifest");
    let mut dest_cfg = weir_app::dest_object_to_config(&m, "contacts");
    assert_eq!(dest_cfg["auth_scheme"], "oauth2");
    assert_eq!(dest_cfg["oauth_grant"], "client_credentials");
    dest_cfg["base_url"] = serde_json::Value::String(sf.clone());
    dest_cfg["oauth_token_url"] = serde_json::Value::String(format!("{sf}/services/oauth2/token"));
    dest_cfg["client_id"] = serde_json::Value::String("cid".into());
    dest_cfg["client_secret"] = serde_json::Value::String("shhh-secret".into());

    // Host-side split: the OAuth2 credential is built + the secrets stripped from the guest cfg.
    let (credential, guest_cfg) = Credential::from_auth_config(&dest_cfg.to_string());
    assert!(credential.is_some(), "OAuth2 credential built host-side");
    assert!(
        !guest_cfg.contains("shhh-secret"),
        "client secret stripped from the guest config"
    );

    let db = Store::open(root.join("weir.db").to_str().unwrap()).unwrap();
    let engine = Engine::new(&db);

    let src = mock_source(
        r#"[{"ext_id":"e1","first_name":"Ada","last_name":"L","email":"a@x.com"},{"ext_id":"reject","first_name":"No","last_name":"Pe","email":"r@x.com"},{"ext_id":"e3","first_name":"Cy","last_name":"B","email":"c@x.com"}]"#,
    );
    let source = ConnectorHandle::from_wasm_package(
        root,
        "weir-rest-pkg",
        &Config {
            json: serde_json::json!({ "base_url": src, "path": "/contacts" }).to_string(),
        },
        HostAllowList::allow_all(),
        &[],
    )
    .expect("load source");

    let policy = HostAllowList {
        allowed_hosts: vec!["127.0.0.1".to_string()],
        inject_headers: vec![],
        credential,
    };
    let dest = ConnectorHandle::from_wasm_package(
        root,
        "weir-rest-dest-pkg",
        &Config { json: guest_cfg },
        policy,
        &[],
    )
    .expect("load salesforce dest");

    for run in 1..=2 {
        let out = engine
            .sync("sf", &contacts_stream(), &source, &dest)
            .unwrap_or_else(|e| panic!("run {run}: {e:?}"));
        assert_eq!(
            out.rows_written, 2,
            "run {run}: two contacts upserted (one rejected)"
        );
    }

    let st = state.lock().unwrap();
    assert_eq!(
        st.upserts.len(),
        2,
        "idempotent: replay upserted the same 2 External-Id keys"
    );
    assert!(
        st.mints >= 2,
        "OAuth token re-minted on expiry (refresh); mints={}",
        st.mints
    );
    assert_eq!(
        db.dead_letter_count("sf").unwrap(),
        2,
        "the rejected sObject dead-lettered each run"
    );

    let ada = st
        .upserts
        .iter()
        .find(|(k, _)| k.contains("/e1"))
        .map(|(_, v)| v.clone())
        .expect("e1 upserted");
    assert!(
        ada.contains("PATCH /services/data/v59.0/sobjects/Contact/ExternalId__c/e1"),
        "External-Id upsert URL; got:\n{ada}"
    );
    assert!(
        ada.to_ascii_lowercase()
            .contains("authorization: bearer sf-"),
        "host-minted bearer injected; got:\n{ada}"
    );
    assert!(
        ada.contains("\"FirstName\":\"Ada\""),
        "field map (first_name → FirstName); got:\n{ada}"
    );
}
