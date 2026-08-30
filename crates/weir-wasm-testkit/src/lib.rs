//! Test helper for [[WEIR-I-0011]] (WASM-always): build a connector **guest crate**
//! to `wasm32-wasip2` and stage it as a fidius package on disk, so a test can load
//! it via `ConnectorHandle::from_wasm_package`.
//!
//! Replaces the per-test inlined `build_and_stage`. Adds a **freshness cache**: the
//! `cargo build` is skipped when the artifact is already newer than the crate's
//! sources, and runs at most once per fixture per test process. The build/stage
//! seam the migrated connectors (S2+) reuse.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use weir_connector::Config;
use weir_runtime::ConnectorHandle;

/// The standard no-capability guests: `(name, fixture-dir, wasm-file, package)`.
/// `name` matches what tests used to pass to `native_in_process` (case/`-`/`_`
/// insensitive). Connectors needing host capabilities (http) are staged bespoke.
const GUESTS: &[(&str, &str, &str, &str)] = &[
    ("echo", "echo", "weir_echo_wasm.wasm", "weir-echo-pkg"),
    ("slow", "slow", "weir_slow_wasm.wasm", "weir-slow-pkg"),
    (
        "faulty",
        "faulty",
        "weir_faulty_wasm.wasm",
        "weir-faulty-pkg",
    ),
    (
        "arrowsink",
        "arrow-sink",
        "weir_arrow_sink_wasm.wasm",
        "weir-arrow-sink-pkg",
    ),
];

fn norm(name: &str) -> String {
    name.to_ascii_lowercase().replace(['-', '_'], "")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

/// Stage **all** standard guests into one stable, shared dir (built once per process;
/// the build itself is freshness-cached → compile-once-use-many). Returns the dir,
/// which doubles as `WEIR_CONNECTORS_DIR` for app/api/orchestrator tests.
pub fn connectors_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let root = workspace_root();
        let dest = root.join("target/weir-staged-connectors");
        std::fs::create_dir_all(&dest).expect("create staged dir");
        for (_, kebab, wasm_file, pkg) in GUESTS {
            stage(
                &WasmPackage {
                    fixture_dir: &root.join("wasm-fixtures").join(kebab),
                    wasm_file,
                    pkg_name: pkg,
                    capabilities: &[],
                },
                &dest,
            );
        }
        // The production connectors, mirroring scripts/stage-connectors.sh: creation-time
        // validation ([[WEIR-T-0166]]) rightly demands a connection's connectors resolve, so
        // the test staging carries the same set a real deployment stages (manifest-backed
        // connections rewrite onto `rest`; CDC/reverse-ETL tests reference postgres/rest-dest).
        for (dir, wasm_file, pkg, cap) in [
            (
                "crates/connectors/rest",
                "weir_rest_wasm.wasm",
                "weir-rest-pkg",
                "http",
            ),
            (
                "crates/connectors/rest-dest",
                "weir_rest_dest_wasm.wasm",
                "weir-rest-dest-pkg",
                "http",
            ),
            (
                "crates/connectors/s3",
                "weir_s3_wasm.wasm",
                "weir-s3-pkg",
                "http",
            ),
            (
                "crates/connectors/snowflake",
                "weir_snowflake_wasm.wasm",
                "weir-snowflake-pkg",
                "http",
            ),
            (
                "crates/connectors/postgres",
                "weir_postgres_wasm.wasm",
                "weir-postgres-pkg",
                "tcp",
            ),
            (
                "crates/connectors/mssql",
                "weir_mssql_wasm.wasm",
                "weir-mssql-pkg",
                "tcp",
            ),
        ] {
            stage(
                &WasmPackage {
                    fixture_dir: &root.join(dir),
                    wasm_file,
                    pkg_name: pkg,
                    capabilities: &[cap],
                },
                &dest,
            );
        }
        dest
    })
    .clone()
}

/// Load a standard guest as a configured `ConnectorHandle` — the wasm replacement
/// for `ConnectorHandle::native_in_process(name, cfg)` in tests (returns `Result`
/// so call sites keep their `.expect(...)`).
pub fn load(name: &str, config: &Config) -> Result<ConnectorHandle, weir_runtime::RuntimeError> {
    let n = norm(name);
    let (_, _, _, pkg) = GUESTS
        .iter()
        .find(|(nm, ..)| norm(nm) == n)
        .unwrap_or_else(|| panic!("no wasm guest for connector `{name}`"));
    ConnectorHandle::from_wasm_package(
        &connectors_dir(),
        pkg,
        config,
        weir_runtime::HostAllowList::allow_all(),
        &[],
    )
}

/// A connector guest crate to build + stage as a fidius wasm package.
pub struct WasmPackage<'a> {
    /// Path to the guest crate (e.g. `crates/connectors/rest`).
    pub fixture_dir: &'a Path,
    /// The produced wasm file name (e.g. `weir_rest_wasm.wasm`).
    pub wasm_file: &'a str,
    /// The fidius package name (the staged dir + `package.toml` name).
    pub pkg_name: &'a str,
    /// Capabilities to grant the package (e.g. `&["http"]`).
    pub capabilities: &'a [&'a str],
}

fn built_guests() -> &'static Mutex<HashSet<PathBuf>> {
    static S: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Build the guest crate for `wasm32-wasip2` (release), returning the `.wasm` path.
/// Skips the build when the artifact is already newer than the crate's sources, and
/// builds at most once per fixture per process. The lock is held across the build so
/// two tests never `cargo build` the same crate concurrently.
pub fn build(fixture_dir: &Path, wasm_file: &str) -> PathBuf {
    let art = fixture_dir
        .join("target/wasm32-wasip2/release")
        .join(wasm_file);

    let mut built = built_guests().lock().expect("testkit build lock");
    if built.contains(&art) || is_fresh(&art, fixture_dir) {
        built.insert(art.clone());
        return art;
    }
    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-wasip2", "--release"])
        .current_dir(fixture_dir)
        .status()
        .unwrap_or_else(|e| panic!("spawn cargo for {}: {e}", fixture_dir.display()));
    assert!(
        status.success(),
        "wasm guest build failed: {}",
        fixture_dir.display()
    );
    assert!(art.exists(), "expected wasm artifact at {}", art.display());
    built.insert(art.clone());
    art
}

/// Build (cached) + stage the package under `dest_root`; returns the staged package dir.
pub fn stage(pkg: &WasmPackage, dest_root: &Path) -> PathBuf {
    let art = build(pkg.fixture_dir, pkg.wasm_file);
    let bytes = std::fs::read(&art).unwrap_or_else(|e| panic!("read {}: {e}", art.display()));
    let dir = dest_root.join(pkg.pkg_name);
    std::fs::create_dir_all(&dir).expect("create package dir");
    let caps = pkg
        .capabilities
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let toml = format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\ninterface = \"connector\"\n\
         interface_version = 1\nruntime = \"wasm\"\n\n[metadata]\ncategory = \"test\"\n\n\
         [wasm]\ncomponent = \"{wasm}\"\ncapabilities = [{caps}]\n",
        name = pkg.pkg_name,
        wasm = pkg.wasm_file,
        caps = caps,
    );
    std::fs::write(dir.join("package.toml"), toml).expect("write package.toml");
    std::fs::write(dir.join(pkg.wasm_file), bytes).expect("write wasm component");
    dir
}

/// True if `art` exists and is newer than the crate's `Cargo.toml`, `build.rs`,
/// everything under `src/` + `wit/`, AND the vendored path deps under
/// `<workspace>/vendor` (weir-tiberius etc. — a fork edit must rebuild its
/// guest) — i.e. a rebuild would be a no-op.
fn is_fresh(art: &Path, fixture_dir: &Path) -> bool {
    let Ok(art_mtime) = modified(art) else {
        return false;
    };
    let mut newest = SystemTime::UNIX_EPOCH;
    for f in ["Cargo.toml", "build.rs"] {
        if let Ok(m) = modified(&fixture_dir.join(f)) {
            newest = newest.max(m);
        }
    }
    newest = newest.max(newest_under(&fixture_dir.join("src")));
    newest = newest.max(newest_under(&fixture_dir.join("wit")));
    for vendor in ["vendor"] {
        let dir = workspace_root().join(vendor);
        if dir.is_dir() {
            for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let crate_dir = entry.path();
                newest = newest.max(newest_under(&crate_dir.join("src")));
                if let Ok(m) = modified(&crate_dir.join("Cargo.toml")) {
                    newest = newest.max(m);
                }
            }
        }
    }
    art_mtime >= newest
}

fn newest_under(dir: &Path) -> SystemTime {
    let mut newest = SystemTime::UNIX_EPOCH;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                newest = newest.max(newest_under(&p));
            } else if let Ok(m) = modified(&p) {
                newest = newest.max(m);
            }
        }
    }
    newest
}

fn modified(p: &Path) -> std::io::Result<SystemTime> {
    std::fs::metadata(p)?.modified()
}
