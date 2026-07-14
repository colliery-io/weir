//! Closes the WEIR-T-0002 signing gap (criterion 4 / [[WEIR-A-0015]]): a WASM
//! package signed with a trusted Ed25519 key loads under `require_signature`,
//! and an unsigned package is rejected.

use std::path::Path;
use std::process::Command;

use ed25519_dalek::{Signer, SigningKey};
use weir_connector::Config;
use weir_runtime::ConnectorHandle;

/// Build the wasm echo guest once and return its component bytes.
fn echo_wasm_bytes() -> Vec<u8> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wasm-fixtures/echo");
    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-wasip2", "--release"])
        .current_dir(&fixture)
        .status()
        .expect("cargo build wasm echo");
    assert!(status.success(), "wasm echo build failed");
    std::fs::read(fixture.join("target/wasm32-wasip2/release/weir_echo_wasm.wasm")).unwrap()
}

/// Stage a `weir-echo-pkg` package under `root`; return its dir.
fn stage(root: &Path, bytes: &[u8]) -> std::path::PathBuf {
    let dir = root.join("weir-echo-pkg");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("package.toml"),
        r#"
[package]
name = "weir-echo-pkg"
version = "0.1.0"
interface = "connector"
interface_version = 1
runtime = "wasm"

[metadata]
category = "test"

[wasm]
component = "weir_echo_wasm.wasm"
"#,
    )
    .unwrap();
    std::fs::write(dir.join("weir_echo_wasm.wasm"), bytes).unwrap();
    dir
}

/// Sign the staged package with a fixed key; return the trusted public key bytes.
fn sign(dir: &Path) -> [u8; 32] {
    let sk = SigningKey::from_bytes(&[7u8; 32]);
    let digest = fidius_core::package::package_digest(dir).expect("package digest");
    let sig = sk.sign(&digest);
    std::fs::write(dir.join("package.sig"), sig.to_bytes()).unwrap();
    sk.verifying_key().to_bytes()
}

#[test]
fn signed_package_loads_with_trusted_key() {
    let bytes = echo_wasm_bytes();
    let root = tempfile::TempDir::new().unwrap();
    let dir = stage(root.path(), &bytes);
    let trusted = sign(&dir);

    let conn = ConnectorHandle::from_wasm_package(
        root.path(),
        "weir-echo-pkg",
        &Config {
            json: "{}".to_string(),
        },
        weir_runtime::HostAllowList::allow_all(),
        &[trusted],
    )
    .expect("signed package should load with a trusted key");
    assert_eq!(conn.spec().unwrap().name, "echo");
}

#[test]
fn unsigned_package_is_rejected() {
    let bytes = echo_wasm_bytes();
    let root = tempfile::TempDir::new().unwrap();
    stage(root.path(), &bytes); // no signature written

    let trusted = SigningKey::from_bytes(&[9u8; 32])
        .verifying_key()
        .to_bytes();
    let result = ConnectorHandle::from_wasm_package(
        root.path(),
        "weir-echo-pkg",
        &Config {
            json: "{}".to_string(),
        },
        weir_runtime::HostAllowList::allow_all(),
        &[trusted],
    );
    assert!(
        result.is_err(),
        "unsigned package must be rejected under require_signature"
    );
}
