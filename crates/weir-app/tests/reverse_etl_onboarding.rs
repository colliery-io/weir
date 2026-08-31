//! [[WEIR-T-0076]] (reverse-ETL productionization A): a **destination manifest** onboards
//! through the normal path — discover → register → a connection whose dest is the manifest,
//! baked onto the shared `rest-dest` runtime — exactly like a source manifest ([[WEIR-I-0015]]).

use std::path::Path;

use weir_app::{App, Connection, DEFAULT_TENANT, Origin, connector_ref, plugin_name};

#[test]
fn dest_manifest_discovers_registers_and_bakes_into_a_connection() {
    let dest_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dest-manifests");
    // SAFETY: set before the app reads the dirs; scopes discovery to the repo's dest
    // manifests, and stages the wasm fixtures so `connector_ref("slow")` passes the
    // creation-time existence gate ([[WEIR-T-0166]]) regardless of ambient env.
    unsafe {
        std::env::set_var("WEIR_DEST_MANIFESTS_DIR", &dest_dir);
        std::env::set_var("WEIR_CONNECTORS_DIR", weir_wasm_testkit::connectors_dir());
    }

    let tmp = tempfile::TempDir::new().unwrap();
    let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap();

    // Discover — the HubSpot destination manifest widens the discover list, marked as a dest.
    let pkgs = app.available_packages();
    let hs = pkgs
        .iter()
        .find(|p| p.name == "hubspot-dest")
        .expect("hubspot dest discovered");
    assert_eq!(hs.kind, "dest-manifest", "marked as a destination package");

    // Register — onboarding stores a dest-manifest catalog entry with the Destination role.
    app.import_vendored_dest_manifest(DEFAULT_TENANT, "hubspot-dest", Origin::Community)
        .expect("onboard dest manifest");
    let entry = app
        .list_connectors(DEFAULT_TENANT)
        .unwrap()
        .into_iter()
        .find(|e| e.name == "hubspot-dest")
        .expect("registered in the catalog");
    assert_eq!(entry.kind, "dest-manifest");
    assert!(
        format!("{:?}", entry.roles).contains("Destination"),
        "roles = [Destination]: {:?}",
        entry.roles
    );

    // Resolve — a connection whose dest is the manifest bakes onto rest-dest (source is any
    // non-manifest warehouse stand-in, so the single config doesn't collide on base_url).
    app.add_connection(
        DEFAULT_TENANT,
        &Connection {
            name: "activate".into(),
            source: connector_ref("slow"),
            dest: connector_ref("hubspot-dest"),
            stream: "contacts".into(),
            source_config: "{}".into(),
            dest_config: "{}".into(),
            every_secs: None,
            cron: None,
            sync_mode: "full_refresh".into(),
            write_mode: "append".into(),
            business_keys: vec![],
            cursor_field: None,
            execution_mode: "run_once".into(),
        },
    )
    .expect("add_connection with a manifest dest");

    let conn = app.get_connection(DEFAULT_TENANT, "activate").unwrap();
    assert_eq!(
        plugin_name(&conn.dest),
        "rest-dest",
        "dest rewritten to the shared rest-dest runtime"
    );
    assert!(
        conn.dest_config.contains("\"method\":\"PATCH\""),
        "baked HubSpot method: {}",
        conn.dest_config
    );
    assert!(
        conn.dest_config.contains("properties"),
        "baked body_wrap: {}",
        conn.dest_config
    );
    assert!(
        conn.dest_config.contains("bearer"),
        "baked auth_scheme: {}",
        conn.dest_config
    );
    assert!(
        conn.dest_config.contains("/crm/v3/objects/contacts"),
        "baked path: {}",
        conn.dest_config
    );
}
