//! [[WEIR-T-0092]] regression: a connection referencing a tenant-PRIVATE compiled
//! artifact (staged only under `<connectors_dir>/<tenant>/<pkg>`, the layout
//! `POST /catalog/import` produces) must pass creation-time validation
//! ([[WEIR-T-0166]]) — the required-config check loads the spec via the
//! tenant-SCOPED ref, mirroring how execution resolves it.
//!
//! Own test binary on purpose: it points WEIR_CONNECTORS_DIR at a private temp
//! layout, which would race the other suites' shared testkit setting.

use weir_app::{App, Connection, connector_ref};

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).unwrap();
        }
    }
}

fn conn(name: &str) -> Connection {
    Connection {
        name: name.to_string(),
        source: connector_ref("echo"),
        dest: connector_ref("arrow-sink"),
        stream: "s".to_string(),
        source_config: "{}".to_string(),
        dest_config: "{}".to_string(),
        every_secs: None,
        cron: None,
        sync_mode: "full_refresh".to_string(),
        write_mode: "append".to_string(),
        business_keys: vec![],
        cursor_field: None,
        execution_mode: "run_once".into(),
    }
}

#[tokio::test]
async fn tenant_private_connector_passes_creation_validation() {
    // Stage acme-private copies of the testkit packages; the shared root stays EMPTY,
    // so only the tenant-scoped resolution path can find them.
    let staged = weir_wasm_testkit::connectors_dir();
    let tmp = tempfile::TempDir::new().unwrap();
    let private = tmp.path().join("connectors");
    for pkg in ["weir-echo-pkg", "weir-arrow-sink-pkg"] {
        copy_dir(
            &std::path::Path::new(&staged).join(pkg),
            &private.join("acme").join(pkg),
        );
    }
    unsafe {
        std::env::set_var("WEIR_CONNECTORS_DIR", private.to_str().unwrap());
    }

    let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).expect("open store");

    app.add_connection("acme", &conn("private-ok"))
        .expect("tenant-private artifacts must pass creation-time validation");

    // Control: a tenant WITHOUT the private artifact (and an empty shared root) is
    // refused at creation — the resolution check still bites.
    assert!(
        app.add_connection("globex", &conn("no-artifact")).is_err(),
        "a tenant without the artifact must be refused"
    );
}
