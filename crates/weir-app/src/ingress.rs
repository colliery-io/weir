//! Connector **ingress** ([[WEIR-I-0010]] S2 / [[WEIR-A-0018]] Publication Contract):
//! the shared pipeline that turns a connector *source* into a registered, usable
//! wasm package — **fetch → compile (`wasm32-wasip2`) → snapshot `spec()` → upsert
//! the catalog row**. Registration is always an explicit act; this is the seam the
//! parity arc ([[WEIR-I-0008]]) and the UI's "import" both terminate in.
//!
//! MVP implements the **local-crate** + **folder** sources; crates.io `(name,ver)`
//! and git URL are stubbed (the pipeline is identical, only the fetch differs).

use std::path::{Path, PathBuf};
use std::process::Command;

use weir_connector::{ConnectorRole, SyncMode};
use weir_orchestrator::{ConnectorRef, Origin};

use crate::{App, AppError, CatalogEntry, connectors_dir, kebab};

/// The shared declarative-runtime package a low-code (`manifest`) connector binds
/// to ([[WEIR-A-0032]] interpreter model). Today: the generalized `rest`.
pub(crate) const DECLARATIVE_RUNTIME_PKG: &str = "weir-rest-pkg";
/// The shared **destination** runtime package ([[WEIR-A-0034]] / [[WEIR-I-0015]]).
pub(crate) const DEST_RUNTIME_PKG: &str = "weir-rest-dest-pkg";

/// Where a connector's source comes from ([[WEIR-A-0018]] ingress paths).
pub enum Source {
    /// A local crate directory (the fidius guest source) — compiled here. MVP.
    LocalCrate(PathBuf),
    /// An already-built package directory `<connectors_dir>/<pkg>` — no compile,
    /// just snapshot + register. The [[WEIR-I-0010]] folder-scan case.
    Folder { package: String },
    /// A **low-code declarative manifest** ([[WEIR-A-0032]] / [[WEIR-I-0012]]):
    /// registered as data + run on the shared declarative runtime — **no compile**.
    Manifest { yaml: String, name: Option<String> },
    // Future ([[WEIR-A-0018]]): CratesIo { name, version }, Git { url, rev }.
}

impl App {
    /// Import a connector from `source` into `tenant`'s catalog, recording it with
    /// `origin`. Idempotent: re-importing the same `(name, version)` re-stages + upserts.
    pub fn import(
        &self,
        tenant: &str,
        source: Source,
        origin: Origin,
    ) -> Result<CatalogEntry, AppError> {
        let dir = connectors_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::Config(format!("connectors dir: {e}")))?;

        // Low-code: a manifest is data run on the shared declarative runtime — no
        // compile, no staged package ([[WEIR-A-0032]] interpreter model).
        if let Source::Manifest { yaml, name } = source {
            return self.import_manifest(tenant, yaml, name, origin);
        }

        // Compiled **private** crates get a per-tenant staging namespace `<dir>/<tenant>/<pkg>`
        // ([[WEIR-A-0036]] d5 / [[WEIR-T-0092]]) so two tenants' same-named crates never collide.
        // Pre-staged **shared** packages (`Folder` — the generic runtimes/guests) stay in `<dir>`.
        let (package, version, search_path) = match source {
            Source::LocalCrate(path) => {
                let sp = Path::new(&dir).join(tenant);
                std::fs::create_dir_all(&sp)
                    .map_err(|e| AppError::Config(format!("tenant connectors dir: {e}")))?;
                let (p, v) = compile_and_stage(&path, &sp)?;
                (p, v, sp.to_string_lossy().into_owned())
            }
            Source::Folder { package } => {
                // Already staged: prefer the tenant's namespace (`<dir>/<tenant>/<pkg>`, e.g. a
                // previously-compiled private crate), else the shared dir for the generic guests.
                let tdir = Path::new(&dir).join(tenant);
                let sp = if tdir.join(&package).exists() {
                    tdir.to_string_lossy().into_owned()
                } else {
                    dir.clone()
                };
                (package, String::new(), sp)
            }
            Source::Manifest { .. } => unreachable!("handled above"),
        };

        // Snapshot the spec by loading the staged package (post-compile authority).
        let probe = ConnectorRef::Wasm {
            search_path: search_path.clone(),
            package: package.clone(),
            version: version.clone(),
            origin,
        };
        let spec = probe
            .spec()
            .map_err(|e| AppError::Config(format!("snapshot spec for `{package}`: {e}")))?;

        // Version: a compiled crate carries its Cargo.toml version; a pre-built folder
        // package falls back to the connector's own `connector_version`.
        let version = if version.is_empty() {
            spec.connector_version.clone()
        } else {
            version
        };

        let entry = CatalogEntry {
            name: spec.name.clone(),
            version,
            roles: spec.roles.clone(),
            config_schema: spec.config_schema.clone(),
            contract_version: spec.contract_version,
            supported_sync_modes: spec.supported_sync_modes.clone(),
            origin,
            status: "ready".to_string(),
            location: package,
            kind: "wasm".to_string(),
            manifest: None,
            verified_at: None,
        };
        self.register_connector(tenant, &entry)?;
        // Re-import = new wasm at the same package path → drop any stale cached
        // handle so the next run loads the freshly-built component ([[WEIR-T-0051]]).
        ConnectorRef::invalidate_cache(&search_path, &entry.location);
        Ok(entry)
    }

    /// **Preview** a low-code manifest ([[WEIR-T-0055]]): tier / confidence /
    /// streams / unsupported-features, **without** running or registering anything.
    pub fn preview_manifest(&self, yaml: &str) -> crate::ImportReport {
        weir_importer::analyze(yaml)
    }

    /// Onboard a **vendored** manifest by name (`manifests/<name>.yaml`) — the
    /// "discover & select" path for low-code connectors ([[WEIR-T-0057]]).
    pub fn import_vendored_manifest(
        &self,
        tenant: &str,
        name: &str,
        origin: Origin,
    ) -> Result<CatalogEntry, AppError> {
        let path = Path::new(&crate::manifests_dir()).join(format!("{name}.yaml"));
        let yaml = std::fs::read_to_string(&path)
            .map_err(|e| AppError::Config(format!("read manifest `{name}`: {e}")))?;
        self.import(
            tenant,
            Source::Manifest {
                yaml,
                name: Some(name.to_string()),
            },
            origin,
        )
    }

    /// Register a **low-code manifest** as a named catalog connector that runs on
    /// the shared declarative runtime ([[WEIR-A-0032]] / [[WEIR-T-0054]]). Validates
    /// the manifest via `weir-importer`; **no codegen, no compile**.
    fn import_manifest(
        &self,
        tenant: &str,
        yaml: String,
        name: Option<String>,
        origin: Origin,
    ) -> Result<CatalogEntry, AppError> {
        let probe = name.clone().unwrap_or_else(|| "manifest".to_string());
        let manifest = weir_importer::import_yaml(&probe, &yaml)
            .map_err(|e| AppError::Config(format!("invalid manifest: {e}")))?;
        // Store the **canonical weir manifest** (importer output), not the input
        // Airbyte yaml — resolution parses it back via `Manifest::from_yaml`.
        let canonical = serde_yaml::to_string(&manifest)
            .map_err(|e| AppError::Config(format!("serialize manifest: {e}")))?;
        let has_incremental = manifest.streams.iter().any(|s| s.incremental.is_some());
        let modes = if has_incremental {
            vec![SyncMode::FullRefresh, SyncMode::Incremental]
        } else {
            vec![SyncMode::FullRefresh]
        };
        let entry = CatalogEntry {
            name: name.unwrap_or_else(|| manifest.spec.name.clone()),
            version: manifest.spec.version.clone(),
            roles: vec![ConnectorRole::Source],
            // The manifest carries base_url/path/etc.; per-connection config (e.g.
            // credentials) is layered at connection time. No extra schema for now.
            config_schema: "{}".to_string(),
            contract_version: 1,
            supported_sync_modes: modes,
            origin,
            status: "ready".to_string(),
            location: DECLARATIVE_RUNTIME_PKG.to_string(),
            kind: "manifest".to_string(),
            manifest: Some(canonical),
            verified_at: None,
        };
        self.register_connector(tenant, &entry)?;
        Ok(entry)
    }

    /// Onboard a **vendored destination** manifest by name (`dest-manifests/<name>.yaml`) —
    /// the reverse-ETL analogue of `import_vendored_manifest` ([[WEIR-I-0015]]).
    pub fn import_vendored_dest_manifest(
        &self,
        tenant: &str,
        name: &str,
        origin: Origin,
    ) -> Result<CatalogEntry, AppError> {
        let path = Path::new(&crate::dest_manifests_dir()).join(format!("{name}.yaml"));
        let yaml = std::fs::read_to_string(&path)
            .map_err(|e| AppError::Config(format!("read dest manifest `{name}`: {e}")))?;
        self.import_dest_manifest(tenant, yaml, Some(name.to_string()), origin)
    }

    /// Register a **destination manifest** as a named catalog connector that runs on the shared
    /// `rest-dest` runtime ([[WEIR-A-0034]]). Validated via `weir-manifest`; no codegen.
    pub fn import_dest_manifest(
        &self,
        tenant: &str,
        yaml: String,
        name: Option<String>,
        origin: Origin,
    ) -> Result<CatalogEntry, AppError> {
        let manifest = weir_manifest::DestinationManifest::from_yaml(&yaml)
            .map_err(|e| AppError::Config(format!("invalid dest manifest: {e}")))?;
        let canonical = serde_yaml::to_string(&manifest)
            .map_err(|e| AppError::Config(format!("serialize dest manifest: {e}")))?;
        let entry = CatalogEntry {
            name: name.unwrap_or_else(|| manifest.spec.name.clone()),
            version: manifest.spec.version.clone(),
            roles: vec![ConnectorRole::Destination],
            config_schema: "{}".to_string(),
            contract_version: 1,
            supported_sync_modes: vec![SyncMode::FullRefresh, SyncMode::Incremental],
            origin,
            status: "ready".to_string(),
            location: DEST_RUNTIME_PKG.to_string(),
            kind: "dest-manifest".to_string(),
            manifest: Some(canonical),
            verified_at: None,
        };
        self.register_connector(tenant, &entry)?;
        Ok(entry)
    }
}

/// `cargo build --target wasm32-wasip2 --release` the crate at `crate_dir`, then
/// stage it as a fidius package under `dest_root`. Returns `(package_name, version)`.
fn compile_and_stage(crate_dir: &Path, dest_root: &Path) -> Result<(String, String), AppError> {
    let (crate_name, version, caps) = read_manifest(crate_dir)?;
    let wasm_file = format!("{}.wasm", crate_name.replace('-', "_"));
    // Connector/package name: strip the `weir-` prefix + `-wasm`/`-connector` infix
    // to the logical connector name, then the canonical `weir-<name>-pkg`.
    let connector = kebab(
        crate_name
            .strip_prefix("weir-")
            .unwrap_or(&crate_name)
            .strip_suffix("-wasm")
            .unwrap_or(&crate_name),
    );
    let package = format!("weir-{connector}-pkg");

    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-wasip2", "--release"])
        .current_dir(crate_dir)
        .status()
        .map_err(|e| AppError::Config(format!("spawn cargo for {}: {e}", crate_dir.display())))?;
    if !status.success() {
        return Err(AppError::Config(format!(
            "compile failed for `{crate_name}` (cargo exited {status})"
        )));
    }

    let artifact = crate_dir
        .join("target/wasm32-wasip2/release")
        .join(&wasm_file);
    let bytes = std::fs::read(&artifact)
        .map_err(|e| AppError::Config(format!("read {}: {e}", artifact.display())))?;

    let pkg_dir = dest_root.join(&package);
    std::fs::create_dir_all(&pkg_dir).map_err(|e| AppError::Config(format!("stage dir: {e}")))?;
    let caps_toml = caps
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    // Full fidius manifest: `find_wasm_package` matches `[package].name` +
    // `runtime = "wasm"`, so both are required (not just the `[wasm]` block).
    let toml = format!(
        "[package]\nname = \"{package}\"\nversion = \"{version}\"\ninterface = \"connector\"\n\
         interface_version = 1\nruntime = \"wasm\"\n\n[metadata]\ncategory = \"connector\"\n\n\
         [wasm]\ncomponent = \"{wasm_file}\"\ncapabilities = [{caps_toml}]\n"
    );
    std::fs::write(pkg_dir.join("package.toml"), toml)
        .map_err(|e| AppError::Config(format!("write package.toml: {e}")))?;
    std::fs::write(pkg_dir.join(&wasm_file), bytes)
        .map_err(|e| AppError::Config(format!("write wasm: {e}")))?;

    Ok((package, version))
}

/// Read `[package] name`/`version` + optional `[package.metadata.weir] capabilities`
/// from the crate's `Cargo.toml`. (Roles + contract come from `spec()` post-compile.)
fn read_manifest(crate_dir: &Path) -> Result<(String, String, Vec<String>), AppError> {
    let text = std::fs::read_to_string(crate_dir.join("Cargo.toml"))
        .map_err(|e| AppError::Config(format!("read Cargo.toml: {e}")))?;
    let doc: toml::Value =
        toml::from_str(&text).map_err(|e| AppError::Config(format!("parse Cargo.toml: {e}")))?;
    let pkg = doc
        .get("package")
        .ok_or_else(|| AppError::Config("no [package]".into()))?;
    let name = pkg
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Config("no package.name".into()))?
        .to_string();
    let version = pkg
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Config("no package.version".into()))?
        .to_string();
    let caps = pkg
        .get("metadata")
        .and_then(|m| m.get("weir"))
        .and_then(|w| w.get("capabilities"))
        .and_then(|c| c.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Ok((name, version, caps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Connection, DEFAULT_TENANT, connector_ref, plugin_name};

    /// [[WEIR-T-0054]] backend rails: a low-code manifest onboards as a named
    /// `Manifest` connector (no compile), and a connection on it resolves to the
    /// shared declarative runtime (`rest`) with the manifest baked into config.
    #[test]
    fn manifest_onboards_and_resolves_to_runtime() {
        let tmp = tempfile::TempDir::new().unwrap();
        let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap();

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
          name: { type: ["null", "string"] }
"#;
        let entry = app
            .import(
                DEFAULT_TENANT,
                Source::Manifest {
                    yaml: manifest.to_string(),
                    name: Some("coinpaprika".into()),
                },
                Origin::Community,
            )
            .expect("import manifest");
        assert_eq!(entry.name, "coinpaprika");
        assert_eq!(entry.kind, "manifest");
        assert_eq!(entry.location, "weir-rest-pkg"); // the shared runtime
        assert!(entry.manifest.is_some());
        assert!(
            app.get_connector(DEFAULT_TENANT, "coinpaprika", &entry.version)
                .unwrap()
                .is_some()
        );

        // A connection on the manifest connector resolves to rest + baked config.
        // Creation-time existence validation ([[WEIR-T-0166]]) needs real staged packages.
        unsafe {
            std::env::set_var("WEIR_CONNECTORS_DIR", weir_wasm_testkit::connectors_dir());
        }
        app.add_connection(
            DEFAULT_TENANT,
            &Connection {
                name: "c".into(),
                source: connector_ref("coinpaprika"),
                dest: connector_ref("arrow-sink"),
                stream: "coins".into(),
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
        .expect("add connection");

        let c = app.get_connection(DEFAULT_TENANT, "c").unwrap();
        assert_eq!(
            plugin_name(&c.source),
            "rest",
            "resolved to the shared runtime"
        );
        let cfg: serde_json::Value = serde_json::from_str(&c.source_config).unwrap();
        assert_eq!(cfg["base_url"], "https://api.coinpaprika.com/v1");
        assert_eq!(cfg["path"], "/coins");
    }

    /// Full S2 pipeline against a real crate: compile `slow` → stage → snapshot
    /// spec() → register, then confirm it's cataloged + loadable (spec round-trips).
    #[test]
    fn import_local_crate_compiles_and_registers() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Stage into an isolated connectors dir (also where connector_ref resolves).
        unsafe { std::env::set_var("WEIR_CONNECTORS_DIR", tmp.path().join("connectors")) };
        let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap();

        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wasm-fixtures/slow");
        let entry = app
            .import(
                DEFAULT_TENANT,
                Source::LocalCrate(crate_dir),
                Origin::FirstParty,
            )
            .expect("import slow");

        assert_eq!(entry.name, "slow");
        assert_eq!(entry.version, "0.1.0"); // wasm-fixtures/slow Cargo.toml
        assert_eq!(entry.status, "ready");
        assert_eq!(entry.contract_version, 1);
        assert!(entry.roles.contains(&weir_connector::ConnectorRole::Source));
        assert_eq!(entry.origin, Origin::FirstParty);

        // Cataloged at its pinned version + the tenant-staged package loads (spec round-trips).
        // Compiled private crates land under `<dir>/<tenant>` ([[WEIR-T-0092]]), so scope the ref.
        assert!(
            app.get_connector(DEFAULT_TENANT, "slow", "0.1.0")
                .unwrap()
                .is_some()
        );
        let scoped = crate::scope_wasm_to_tenant(crate::connector_ref("slow"), DEFAULT_TENANT);
        assert_eq!(scoped.spec().unwrap().name, "slow");

        // Folder source: re-import the now-staged package (no compile) → upserts.
        let folder = app
            .import(
                DEFAULT_TENANT,
                Source::Folder {
                    package: "weir-slow-pkg".to_string(),
                },
                Origin::Private,
            )
            .expect("folder import");
        assert_eq!(folder.name, "slow");
        assert_eq!(folder.origin, Origin::Private);
    }

    /// [[WEIR-T-0092]]: two tenants importing the same private crate get DISTINCT, tenant-namespaced
    /// on-disk artifacts — no cross-tenant reuse.
    #[test]
    fn compile_isolation_two_tenants_distinct_artifacts() {
        let tmp = tempfile::TempDir::new().unwrap();
        let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap();
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wasm-fixtures/slow");
        app.import(
            "acme",
            Source::LocalCrate(crate_dir.clone()),
            Origin::Private,
        )
        .expect("acme import");
        app.import("globex", Source::LocalCrate(crate_dir), Origin::Private)
            .expect("globex import");

        // Distinct per-tenant artifacts on disk.
        let dir = Path::new(&crate::connectors_dir()).to_path_buf();
        assert!(
            dir.join("acme").join("weir-slow-pkg").exists(),
            "acme artifact is tenant-namespaced"
        );
        assert!(
            dir.join("globex").join("weir-slow-pkg").exists(),
            "globex artifact is tenant-namespaced"
        );
        // Each tenant's run-time ref resolves its OWN namespace (distinct → no reuse).
        let acme_ref = crate::scope_wasm_to_tenant(crate::connector_ref("slow"), "acme");
        let globex_ref = crate::scope_wasm_to_tenant(crate::connector_ref("slow"), "globex");
        assert_ne!(acme_ref, globex_ref);
        // Cataloged per tenant.
        assert!(
            app.get_connector("acme", "slow", "0.1.0")
                .unwrap()
                .is_some()
        );
        assert!(
            app.get_connector("globex", "slow", "0.1.0")
                .unwrap()
                .is_some()
        );
    }

    /// [[WEIR-T-0058]]: the vendored `manifests/` corpus is surfaced by "discover &
    /// select" (kind=manifest) and each entry validates + onboards.
    #[test]
    fn vendored_manifests_list_and_onboard() {
        let repo_manifests = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../manifests");
        unsafe { std::env::set_var("WEIR_MANIFESTS_DIR", &repo_manifests) };
        let tmp = tempfile::TempDir::new().unwrap();
        let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap();

        let avail = app.available_packages();
        for name in ["coinpaprika", "rickandmorty"] {
            assert!(
                avail.iter().any(|a| a.name == name && a.kind == "manifest"),
                "available is missing manifest `{name}`: {avail:?}"
            );
            let yaml =
                std::fs::read_to_string(repo_manifests.join(format!("{name}.yaml"))).unwrap();
            let entry = app
                .import(
                    DEFAULT_TENANT,
                    Source::Manifest {
                        yaml,
                        name: Some(name.to_string()),
                    },
                    Origin::Community,
                )
                .unwrap_or_else(|e| panic!("onboard `{name}`: {e}"));
            assert_eq!(entry.kind, "manifest");
        }
    }
}
