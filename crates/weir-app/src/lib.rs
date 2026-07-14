//! # weir-app
//!
//! The single-node application core: a minimal **connection config model**
//! persisted on the shared [`Store`], and the [`App`] command logic
//! (`open` / `add_connection` / `run` / `serve` …) over the orchestrator —
//! embedded SQLite, no broker ([[WEIR-S-0012]]). The `weir` binary ([`weir-cli`])
//! and the HTTP control plane ([`weir-api`]) are thin layers over this.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use weir_schema::{
    connections, connectors, dead_letters, outbox, run_logs, stream_schemas, stream_state,
    work_units,
};

pub use weir_connector::ConnectorSpec;
use weir_connector::{Config, ConfiguredStream, ConnectorRole, MappingSpec, SyncMode, WriteMode};
pub use weir_connector::{Field, FieldType, StreamSchema};
use weir_engine::Store;
pub use weir_engine::{DeadLetterRecord, LogRecord};
pub use weir_orchestrator::ScalePolicy;
use weir_orchestrator::{
    Clock, ConnectorRef, ExecutionMode, Fleet, InProcessExecutor, Relay, WorkSpec, Worker,
    WorkerConfig,
};
pub use weir_orchestrator::{Origin, RunRow, WorkUnitStatus};
pub use weir_orchestrator::{Scheduler, SystemClock};

pub mod health;
pub use health::{
    AttentionItem, ConnectionHealth, HealthStatus, HealthThresholds, PlatformHealth, TenantHealth,
};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Engine(#[from] weir_engine::EngineError),
    #[error(transparent)]
    Orchestrator(#[from] weir_orchestrator::ExecutorError),
    #[error("db: {0}")]
    Db(#[from] diesel::result::Error),
    #[error("config: {0}")]
    Config(String),
    #[error("connection `{0}` not found")]
    NotFound(String),
    #[error("contract: {0}")]
    Contract(String),
}

/// The connector contract version this engine build speaks ([[WEIR-A-0019]]). A
/// registered connector whose targeted `contract_version` differs is gated off at
/// dispatch. Single version today; widens to a range as the contract evolves.
pub const ENGINE_CONTRACT_VERSION: u32 = 1;

pub mod ingress;
pub use ingress::Source;
pub use weir_importer::ImportReport;

pub mod auth;
pub use auth::{ApiKeyInfo, AuthenticatedKey};
pub mod tenant;
pub use tenant::{DEFAULT_TENANT, Tenant};
mod store;

/// A persisted connection — the minimal config model the binary needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub name: String,
    pub source: ConnectorRef,
    pub dest: ConnectorRef,
    pub stream: String,
    /// Source connector config as a JSON string ([[WEIR-I-0029]]).
    pub source_config: String,
    /// Destination connector config as a JSON string — resolved independently of the source.
    pub dest_config: String,
    /// If set, the scheduler fires this connection on this interval, in **seconds**
    /// (fractional; as low as 0.1) — honored by `weir serve` ([[WEIR-T-0051]]).
    pub every_secs: Option<f64>,
    /// If set, a cron expression (6/7-field, seconds-first) — takes precedence
    /// over `every_secs`.
    pub cron: Option<String>,
    /// How the source reads ([[WEIR-I-0028]]): `full_refresh` | `incremental` | `cdc`.
    pub sync_mode: String,
    /// How the destination applies records: `append` | `upsert` | `overwrite`.
    pub write_mode: String,
    /// Business keys for `upsert` (the on-conflict key set).
    pub business_keys: Vec<String>,
    /// Cursor field for `incremental`.
    pub cursor_field: Option<String>,
    /// Execution mode ([[WEIR-I-0035]] F1): `run_once` (default; batch/CDC) | `resident`
    /// (long-lived source). Threaded into the `WorkSpec` for the runtime to branch on.
    pub execution_mode: String,
}

/// Resolve a connector name to a wasm [`ConnectorRef`] ([[WEIR-A-0030]] WASM-always;
/// native retired in [[WEIR-I-0011]]). The package is `weir-<kebab>-pkg` under
/// `WEIR_CONNECTORS_DIR` (default `connectors/`) — a directory-backed catalog
/// ([[WEIR-I-0010]]). The package must be staged (built) for the run to resolve.
pub fn connector_ref(name: &str) -> ConnectorRef {
    ConnectorRef::Wasm {
        search_path: connectors_dir(),
        package: format!("weir-{}-pkg", kebab(name)),
        // Bundled connectors are first-party; version stays the backfill default
        // until catalog-driven pinning (S3, [[WEIR-T-0049]]) selects a real one.
        version: "0.0.0".to_string(),
        origin: Origin::FirstParty,
    }
}

/// Scope a Wasm connector ref to a tenant's artifact namespace ([[WEIR-T-0092]]): a ref pointing at
/// the shared `connectors_dir()` is retargeted to `<dir>/<tenant>`, so a run loads the tenant's private
/// compiled artifact when present. The resolver (orchestrator `resolve`) falls back to the shared dir
/// for the generic runtimes/guests every tenant shares. Only the shared default is rewritten (idempotent).
fn scope_wasm_to_tenant(r: ConnectorRef, tenant: &str) -> ConnectorRef {
    match r {
        ConnectorRef::Wasm {
            search_path,
            package,
            version,
            origin,
        } if search_path == connectors_dir() => ConnectorRef::Wasm {
            search_path: std::path::Path::new(&search_path)
                .join(tenant)
                .to_string_lossy()
                .into_owned(),
            package,
            version,
            origin,
        },
        other => other,
    }
}

/// The directory holding staged wasm connector packages — `WEIR_CONNECTORS_DIR`
/// (default `connectors/`). Both `connector_ref` (resolve) and ingress (stage) use it.
pub fn connectors_dir() -> String {
    std::env::var("WEIR_CONNECTORS_DIR").unwrap_or_else(|_| "connectors".to_string())
}

/// The directory holding vendored declarative `*.yaml` manifests for "discover &
/// select" — `WEIR_MANIFESTS_DIR` (default `manifests/`). [[WEIR-T-0056]]/[[WEIR-T-0058]].
pub fn manifests_dir() -> String {
    std::env::var("WEIR_MANIFESTS_DIR").unwrap_or_else(|_| "manifests".to_string())
}

/// Vendored **destination** manifests for reverse-ETL discover & select —
/// `WEIR_DEST_MANIFESTS_DIR` (default `dest-manifests/`) ([[WEIR-A-0034]] / [[WEIR-I-0015]]).
pub fn dest_manifests_dir() -> String {
    std::env::var("WEIR_DEST_MANIFESTS_DIR").unwrap_or_else(|_| "dest-manifests".to_string())
}

/// An onboardable connector surfaced by "discover & select" ([[WEIR-S-0015]]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailablePackage {
    pub name: String,
    /// `crate` (staged wasm package) | `manifest` (vendored declarative yaml).
    pub kind: String,
    /// A one-line summary for the UI hover — e.g. `<base_url> · N streams` for a
    /// manifest ([[WEIR-T-0057]]). Empty for crates.
    #[serde(default)]
    pub summary: String,
}

/// `"ArrowSink"` → `"arrow-sink"`, `"Slow"` → `"slow"` — the wasm package/crate naming.
/// Idempotent on already-kebab input (`"arrow-sink"` → `"arrow-sink"`).
pub(crate) fn kebab(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push('-');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// Display/round-trippable connector name from a ref: `weir-slow-pkg` → `slow`.
pub fn plugin_name(r: &ConnectorRef) -> String {
    match r {
        ConnectorRef::Wasm { package, .. } => package
            .strip_prefix("weir-")
            .and_then(|p| p.strip_suffix("-pkg"))
            .unwrap_or(package)
            .to_string(),
    }
}

/// Inspectable per-connection state: committed resume point + progress counters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionState {
    /// Committed resume cursor (where the connection is caught up to), if any.
    pub cursor: Option<String>,
    /// Committed chunks (outbox rows) — checkpoints landed.
    pub chunks: i64,
    /// Total dead-lettered records for this connection.
    pub dead_lettered: i64,
}

/// Outcome of [`App::run`].
#[derive(Debug, Clone)]
pub struct RunReport {
    pub work_unit_id: i64,
    pub state: String,
}

/// The single-node application: a store + the orchestration relay + the
/// persisted connection config.
pub struct App {
    store: Arc<Store>,
    relay: Relay,
}

impl App {
    /// Open (or create) the local store at `db` and ensure all schema exists.
    pub fn open(db: &str) -> Result<Self, AppError> {
        let store = Arc::new(Store::open(db)?);
        let relay = Relay::new(Arc::clone(&store))?;
        // All schema (connections + the connector catalog included) is created by
        // Store::open → weir_schema::migrate ([[WEIR-T-0060]] / [[WEIR-I-0013]]).
        let app = Self { store, relay };
        // The implicit `default` tenant backs single-tenant deploys ([[WEIR-A-0036]] / [[WEIR-I-0018]]).
        app.ensure_default_tenant()?;
        Ok(app)
    }

    /// Persist a connection (upsert by `(tenant, name)`) under `tenant`.
    pub fn add_connection(&self, tenant: &str, c: &Connection) -> Result<(), AppError> {
        // Validate the sync/write modes before touching the store ([[WEIR-I-0028]]).
        validate_connection_modes(
            &c.sync_mode,
            &c.write_mode,
            &c.business_keys,
            &c.cursor_field,
            &c.execution_mode,
        )
        .map_err(AppError::Config)?;
        // If the source is a low-code (manifest) connector, rewrite it onto the shared declarative
        // runtime + bake the manifest→config ([[WEIR-T-0054]]) — into the **source** config only ([[WEIR-I-0029]]).
        let (source, source_config) =
            self.resolve_manifest_source(tenant, &c.source, &c.stream, &c.source_config)?;
        // Reverse-ETL ([[WEIR-I-0015]]): a dest-manifest bakes onto `rest-dest` in the **dest** config.
        let (dest, dest_config) =
            self.resolve_manifest_dest(tenant, &c.dest, &c.stream, &c.dest_config)?;
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        // The row↔column mapping and the portable update-then-insert upsert live in `store`
        // ([[WEIR-I-0030]]); here we only assemble the resolved row.
        let row = store::ConnectionRow::from_resolved(
            tenant,
            c,
            json(&source)?,
            source_config,
            json(&dest)?,
            dest_config,
        )?;
        store::upsert(&mut conn, &row)
    }

    /// Resolve a connection `source`: if it names a **manifest** catalog connector,
    /// return `(rest ref, baked config)` — the manifest mapped onto the shared
    /// runtime's config, with the user's connection config layered on top
    /// ([[WEIR-T-0054]]). Otherwise the source + config pass through unchanged.
    fn resolve_manifest_source(
        &self,
        tenant: &str,
        source: &ConnectorRef,
        stream: &str,
        user_config: &str,
    ) -> Result<(ConnectorRef, String), AppError> {
        let name = plugin_name(source);
        let entry = self
            .list_connectors(tenant)?
            .into_iter()
            .find(|e| e.name == name);
        if let Some(e) = entry
            && e.kind == "manifest"
            && let Some(yaml) = &e.manifest
        {
            let m = weir_manifest::Manifest::from_yaml(yaml).map_err(|err| {
                AppError::Config(format!("stored manifest for `{name}` invalid: {err}"))
            })?;
            let base = manifest_stream_to_config(&m, stream);
            let merged = merge_config(base, user_config)?;
            return Ok((connector_ref("rest"), merged));
        }
        Ok((source.clone(), user_config.to_string()))
    }

    /// Resolve a connection `dest`: if it names a **dest-manifest** catalog connector, bake its
    /// object → the `rest-dest` config and rewrite the ref to `rest-dest` ([[WEIR-I-0015]] /
    /// [[WEIR-A-0034]]). The destination "object" plays the role the source "stream" plays.
    /// Note: with a manifest *source* + manifest *dest* the single connection config collides on
    /// `base_url`; the real reverse-ETL case (warehouse source + SaaS dest) does not.
    fn resolve_manifest_dest(
        &self,
        tenant: &str,
        dest: &ConnectorRef,
        object: &str,
        config: &str,
    ) -> Result<(ConnectorRef, String), AppError> {
        let name = plugin_name(dest);
        let entry = self
            .list_connectors(tenant)?
            .into_iter()
            .find(|e| e.name == name);
        if let Some(e) = entry
            && e.kind == "dest-manifest"
            && let Some(yaml) = &e.manifest
        {
            let m = weir_manifest::DestinationManifest::from_yaml(yaml).map_err(|err| {
                AppError::Config(format!("stored dest manifest for `{name}` invalid: {err}"))
            })?;
            let base = dest_object_to_config(&m, object);
            let merged = merge_config(base, config)?;
            return Ok((connector_ref("rest-dest"), merged));
        }
        Ok((dest.clone(), config.to_string()))
    }

    /// All connections for `tenant`, by name.
    pub fn list_connections(&self, tenant: &str) -> Result<Vec<Connection>, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        store::list(&mut conn, tenant)?
            .into_iter()
            .map(store::ConnectionRow::into_connection)
            .collect()
    }

    /// One connection by name within `tenant`. A miss (including a row owned by a
    /// different tenant) returns [`AppError::NotFound`] — cross-tenant access is a 404.
    pub fn get_connection(&self, tenant: &str, name: &str) -> Result<Connection, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        store::get(&mut conn, tenant, name)?
            .ok_or_else(|| AppError::NotFound(name.to_string()))?
            .into_connection()
    }

    /// Plan a run (enqueue a `pending` work unit) without executing it — the
    /// async path used by the HTTP API; a background worker drains it.
    pub fn plan_run(&self, tenant: &str, name: &str) -> Result<i64, AppError> {
        let c = self.get_connection(tenant, name)?;
        // Contract gate ([[WEIR-A-0019]]): if the pinned connector is in the catalog,
        // its targeted contract_version must match this engine. Uncataloged refs pass
        // (legacy / pre-registration), so the gate engages as the catalog fills.
        self.check_contract(tenant, &c.source)?;
        self.check_contract(tenant, &c.dest)?;
        // The enqueued work_unit carries `tenant_id = tenant` (via `WorkSpec.tenant`)
        // so the runner stays isolated per tenant ([[WEIR-T-0090]]).
        Ok(self.relay.plan(&work_spec(tenant, &c))?)
    }

    /// **Start** a resident source ([[WEIR-I-0035]] F1.5). Resident connections are NOT fired by
    /// the scheduler ([[WEIR-T-0139]]), so they are launched by this explicit **enqueue-once**
    /// action: plan a single `pending` unit that the worker runs indefinitely under supervision.
    /// Idempotent — if the connection already has an active (pending/leased) unit, this is a no-op
    /// and returns `Ok(None)`. Errors for non-resident connections (use [`App::run`]/[`App::plan_run`]).
    pub fn start(&self, tenant: &str, name: &str) -> Result<Option<i64>, AppError> {
        let c = self.get_connection(tenant, name)?;
        if c.execution_mode != "resident" {
            return Err(AppError::Config(format!(
                "connection `{name}` is `{}`, not resident — use `run` for a one-shot",
                c.execution_mode
            )));
        }
        self.check_contract(tenant, &c.source)?;
        self.check_contract(tenant, &c.dest)?;
        // Enqueue-once: the perpetual lease + `has_active` keep it single ([[WEIR-T-0139]]).
        if self.relay.has_active(name)? {
            return Ok(None);
        }
        Ok(Some(self.relay.plan(&work_spec(tenant, &c))?))
    }

    /// **Stop** a resident source ([[WEIR-I-0035]] F1.5): durably end its supervised restart loop.
    /// Returns the number of active units stopped (0 if it wasn't running). See [`Relay::cancel`]
    /// for the cross-process mid-stream caveat.
    pub fn stop(&self, _tenant: &str, name: &str) -> Result<u64, AppError> {
        Ok(self.relay.cancel(name)?)
    }

    /// Refuse a run whose pinned connector is cataloged with an incompatible contract.
    fn check_contract(&self, tenant: &str, r: &ConnectorRef) -> Result<(), AppError> {
        let (name, version) = (plugin_name(r), ref_version(r));
        if let Some(e) = self.get_connector(tenant, &name, &version)?
            && e.contract_version != ENGINE_CONTRACT_VERSION
        {
            return Err(AppError::Contract(format!(
                "connector `{name}`@{version} targets contract v{} but this engine speaks v{ENGINE_CONTRACT_VERSION}",
                e.contract_version
            )));
        }
        Ok(())
    }

    /// Register (upsert) a connector into the catalog, keyed `(name, version)`. The
    /// ingress pipeline ([[WEIR-T-0048]]) calls this after compile + spec snapshot.
    pub fn register_connector(&self, tenant: &str, e: &CatalogEntry) -> Result<(), AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|err| AppError::Config(err.to_string()))?;
        let now = now_ms();
        let roles_j = json(&e.roles)?;
        let modes_j = json(&e.supported_sync_modes)?;
        let origin_j = json(&e.origin)?;
        // Portable upsert (MultiBackend has no `on_conflict`): update keeps the
        // original created_at; insert stamps both timestamps. Keyed `(tenant, name, version)`.
        let updated = diesel::update(
            connectors::table.filter(
                connectors::tenant_id
                    .eq(tenant)
                    .and(connectors::name.eq(&e.name))
                    .and(connectors::version.eq(&e.version)),
            ),
        )
        .set((
            connectors::roles.eq(&roles_j),
            connectors::config_schema.eq(&e.config_schema),
            connectors::contract_version.eq(e.contract_version as i64),
            connectors::supported_sync_modes.eq(&modes_j),
            connectors::origin.eq(&origin_j),
            connectors::status.eq(&e.status),
            connectors::location.eq(&e.location),
            connectors::kind.eq(&e.kind),
            connectors::manifest.eq(e.manifest.as_deref()),
            connectors::updated_at.eq(now),
        ))
        .execute(&mut conn)?;
        if updated == 0 {
            diesel::insert_into(connectors::table)
                .values((
                    connectors::tenant_id.eq(tenant),
                    connectors::name.eq(&e.name),
                    connectors::version.eq(&e.version),
                    connectors::roles.eq(&roles_j),
                    connectors::config_schema.eq(&e.config_schema),
                    connectors::contract_version.eq(e.contract_version as i64),
                    connectors::supported_sync_modes.eq(&modes_j),
                    connectors::origin.eq(&origin_j),
                    connectors::status.eq(&e.status),
                    connectors::location.eq(&e.location),
                    connectors::kind.eq(&e.kind),
                    connectors::manifest.eq(e.manifest.as_deref()),
                    connectors::created_at.eq(now),
                    connectors::updated_at.eq(now),
                ))
                .execute(&mut conn)?;
        }
        Ok(())
    }

    /// All registered connectors for `tenant`, ordered `(name, version)` — a table read.
    pub fn list_connectors(&self, tenant: &str) -> Result<Vec<CatalogEntry>, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|err| AppError::Config(err.to_string()))?;
        let rows: Vec<CatalogTuple> = connectors::table
            .filter(connectors::tenant_id.eq(tenant))
            .order((connectors::name.asc(), connectors::version.asc()))
            .select((
                connectors::name,
                connectors::version,
                connectors::roles,
                connectors::config_schema,
                connectors::contract_version,
                connectors::supported_sync_modes,
                connectors::origin,
                connectors::status,
                connectors::location,
                connectors::kind,
                connectors::manifest,
            ))
            .load(&mut conn)?;
        rows.into_iter().map(catalog_row_to_entry).collect()
    }

    /// One registered connector by `(name, version)` within `tenant`, or `None`.
    pub fn get_connector(
        &self,
        tenant: &str,
        name: &str,
        version: &str,
    ) -> Result<Option<CatalogEntry>, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|err| AppError::Config(err.to_string()))?;
        let row: Option<CatalogTuple> = connectors::table
            .filter(
                connectors::tenant_id
                    .eq(tenant)
                    .and(connectors::name.eq(name))
                    .and(connectors::version.eq(version)),
            )
            .select((
                connectors::name,
                connectors::version,
                connectors::roles,
                connectors::config_schema,
                connectors::contract_version,
                connectors::supported_sync_modes,
                connectors::origin,
                connectors::status,
                connectors::location,
                connectors::kind,
                connectors::manifest,
            ))
            .first(&mut conn)
            .optional()?;
        row.map(catalog_row_to_entry).transpose()
    }

    /// Unregister a connector `(name, version)` from the catalog (drops the row;
    /// the staged artifact on disk is left in place). No-op if absent.
    pub fn unregister_connector(
        &self,
        tenant: &str,
        name: &str,
        version: &str,
    ) -> Result<(), AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        diesel::delete(
            connectors::table.filter(
                connectors::tenant_id
                    .eq(tenant)
                    .and(connectors::name.eq(name))
                    .and(connectors::version.eq(version)),
            ),
        )
        .execute(&mut conn)?;
        // Drop any cached handle so a later re-register loads fresh ([[WEIR-T-0051]]).
        ConnectorRef::invalidate_cache(&connectors_dir(), &format!("weir-{}-pkg", kebab(name)));
        Ok(())
    }

    /// Staged package directories under `connectors_dir()` — the folder-scan
    /// **availability** source ([[WEIR-I-0010]] S2/S3): "what could be registered".
    pub fn available_packages(&self) -> Vec<AvailablePackage> {
        let mut out: Vec<AvailablePackage> = std::fs::read_dir(connectors_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().join("package.toml").exists())
            .filter_map(|e| e.file_name().into_string().ok())
            .map(|name| AvailablePackage {
                name,
                kind: "crate".to_string(),
                summary: String::new(),
            })
            .collect();
        // Vendored declarative manifests ([[WEIR-T-0056]]/[[WEIR-T-0058]]): the
        // "discover & select" source widens from crates to crates + manifests.
        if let Ok(rd) = std::fs::read_dir(manifests_dir()) {
            for e in rd.flatten() {
                let p = e.path();
                let is_yaml = p
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x == "yaml" || x == "yml")
                    .unwrap_or(false);
                if is_yaml && let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    // Best-effort summary for the UI hover: base URL · stream count.
                    let summary = std::fs::read_to_string(&p)
                        .ok()
                        .and_then(|y| weir_importer::import_yaml(stem, &y).ok())
                        .map(|m| {
                            let n = m.streams.len();
                            format!(
                                "{} · {} stream{}",
                                m.base_url,
                                n,
                                if n == 1 { "" } else { "s" }
                            )
                        })
                        .unwrap_or_default();
                    out.push(AvailablePackage {
                        name: stem.to_string(),
                        kind: "manifest".to_string(),
                        summary,
                    });
                }
            }
        }
        // Vendored declarative **destination** manifests ([[WEIR-A-0034]]/[[WEIR-I-0015]]):
        // reverse-ETL targets widen the same discover list, marked `dest-manifest`.
        if let Ok(rd) = std::fs::read_dir(dest_manifests_dir()) {
            for e in rd.flatten() {
                let p = e.path();
                let is_yaml = p
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x == "yaml" || x == "yml")
                    .unwrap_or(false);
                if is_yaml && let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    let summary = std::fs::read_to_string(&p)
                        .ok()
                        .and_then(|y| weir_manifest::DestinationManifest::from_yaml(&y).ok())
                        .map(|m| {
                            let n = m.objects.len();
                            format!(
                                "{} · {} object{}",
                                m.base_url,
                                n,
                                if n == 1 { "" } else { "s" }
                            )
                        })
                        .unwrap_or_default();
                    out.push(AvailablePackage {
                        name: stem.to_string(),
                        kind: "dest-manifest".to_string(),
                        summary,
                    });
                }
            }
        }
        out
    }

    /// Drain all currently-due work to a terminal state (one in-process agent).
    pub async fn drain(&self) -> Result<(), AppError> {
        // `drain` is the synchronous one-shot path (CLI `run`, tests): serial +
        // deterministic. Concurrency is a daemon concern — `serve` sets the cap.
        // The `Fleet` runs a per-tenant worker for each active tenant ([[WEIR-T-0091]]) so a
        // multi-tenant store drains every tenant (each in its own isolated worker).
        let store = Arc::clone(&self.store);
        let relay = self.relay.clone();
        let fleet = Fleet::new(
            self.relay.clone(),
            move || InProcessExecutor::new(Arc::clone(&store), relay.clone()),
            WorkerConfig {
                concurrency: 1,
                ..WorkerConfig::default()
            },
        );
        fleet.run_until_idle().await?;
        Ok(())
    }

    /// Run a connection once: plan + drain to completion (the synchronous CLI path).
    pub async fn run(&self, name: &str) -> Result<RunReport, AppError> {
        // The synchronous single-node CLI path runs under the implicit default tenant.
        let id = self.plan_run(DEFAULT_TENANT, name)?;
        self.drain().await?;
        let state = self
            .relay
            .state(id)?
            .unwrap_or_else(|| "unknown".to_string());
        Ok(RunReport {
            work_unit_id: id,
            state,
        })
    }

    /// Remove a connection within `tenant` (no-op if absent).
    pub fn delete_connection(&self, tenant: &str, name: &str) -> Result<(), AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        diesel::delete(
            connections::table.filter(
                connections::tenant_id
                    .eq(tenant)
                    .and(connections::name.eq(name)),
            ),
        )
        .execute(&mut conn)?;
        Ok(())
    }

    /// Work-unit history for a connection within `tenant` (id/state/attempt), oldest
    /// first. Scoped by `work_units.tenant_id` so runs never leak across tenants.
    pub fn history(&self, tenant: &str, connection: &str) -> Result<Vec<WorkUnitStatus>, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        let rows: Vec<(i64, String, i64, Option<String>)> = work_units::table
            .filter(
                work_units::tenant_id
                    .eq(tenant)
                    .and(work_units::connection.eq(connection)),
            )
            .order(work_units::id.asc())
            .select((
                work_units::id,
                work_units::state,
                work_units::attempt,
                work_units::error,
            ))
            .load(&mut conn)?;
        Ok(rows
            .into_iter()
            .map(|(id, state, attempt, error)| WorkUnitStatus {
                id,
                state,
                attempt,
                error,
            })
            .collect())
    }

    /// The captured typed **schema** for a stream ([[WEIR-T-0118]]), if any. Keyed by
    /// `(tenant, connection, stream)`; the engine captures under the default tenant, aligning with
    /// `stream_state`.
    pub fn get_stream_schema(
        &self,
        tenant: &str,
        connection: &str,
        stream: &str,
    ) -> Result<Option<weir_connector::StreamSchema>, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        let row: Option<String> = stream_schemas::table
            .filter(
                stream_schemas::tenant_id
                    .eq(tenant)
                    .and(stream_schemas::connection.eq(connection))
                    .and(stream_schemas::stream.eq(stream)),
            )
            .select(stream_schemas::schema)
            .first(&mut conn)
            .optional()?;
        Ok(row.and_then(|j| serde_json::from_str(&j).ok()))
    }

    /// The breaking-drift reason for a stream ([[WEIR-T-0120]]), if the schema is flagged broken.
    pub fn schema_broken(
        &self,
        tenant: &str,
        connection: &str,
        stream: &str,
    ) -> Result<Option<String>, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        let row: Option<Option<String>> = stream_schemas::table
            .filter(
                stream_schemas::tenant_id
                    .eq(tenant)
                    .and(stream_schemas::connection.eq(connection))
                    .and(stream_schemas::stream.eq(stream)),
            )
            .select(stream_schemas::broken)
            .first(&mut conn)
            .optional()?;
        Ok(row.flatten())
    }

    /// Accept a stream's evolved schema ([[WEIR-T-0120]]) — the operator's escape hatch for a breaking
    /// drift: forget the stored schema so the next run re-captures the current shape as the new
    /// baseline (clearing the breaking flag; records then enforce against the new types).
    pub fn accept_schema(
        &self,
        tenant: &str,
        connection: &str,
        stream: &str,
    ) -> Result<(), AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        diesel::delete(
            stream_schemas::table.filter(
                stream_schemas::tenant_id
                    .eq(tenant)
                    .and(stream_schemas::connection.eq(connection))
                    .and(stream_schemas::stream.eq(stream)),
            ),
        )
        .execute(&mut conn)?;
        Ok(())
    }

    /// Per-connection **health** for a tenant ([[WEIR-T-0110]]) — the store gathers recent runs +
    /// dead-letter counts + schedules; `health::compute` decides green/amber/red.
    pub fn tenant_health(
        &self,
        tenant: &str,
        now_ms: i64,
    ) -> Result<Vec<ConnectionHealth>, AppError> {
        let th = HealthThresholds::default();
        let connections = self.list_connections(tenant)?;
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;

        // Recent runs for the tenant, newest first; grouped per connection (cap 20 each).
        let recent: Vec<(String, String, Option<i64>, i64)> = work_units::table
            .filter(work_units::tenant_id.eq(tenant))
            .order(work_units::id.desc())
            .limit(2000)
            .select((
                work_units::connection,
                work_units::state,
                work_units::finished_at,
                work_units::rows_written,
            ))
            .load(&mut conn)?;
        let mut by_conn: std::collections::HashMap<String, Vec<health::HealthRun>> =
            std::collections::HashMap::new();
        for (c, state, finished_ms, rows) in recent {
            let v = by_conn.entry(c).or_default();
            if v.len() < 20 {
                v.push(health::HealthRun {
                    state,
                    finished_ms,
                    rows_written: rows,
                });
            }
        }

        // Dead-letter counts per connection (tallied in Rust — portable, no group_by).
        let dl_conns: Vec<String> = dead_letters::table
            .filter(dead_letters::tenant_id.eq(tenant))
            .select(dead_letters::connection)
            .load(&mut conn)?;
        let mut dl: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
        for c in dl_conns {
            *dl.entry(c).or_insert(0) += 1;
        }

        Ok(connections
            .into_iter()
            .map(|c| {
                let runs = by_conn.remove(&c.name).unwrap_or_default();
                let dead = dl.get(&c.name).copied().unwrap_or(0);
                let schedule_ms = c.every_secs.map(|s| (s * 1000.0) as i64);
                health::compute(&c.name, &runs, dead, schedule_ms, now_ms, &th)
            })
            .collect())
    }

    /// The platform-wide health rollup ([[WEIR-T-0110]], platform-admin) — per-tenant status + the
    /// cross-tenant "needs attention" list + fleet signals.
    pub fn platform_health(&self, now_ms: i64) -> Result<PlatformHealth, AppError> {
        let tenants = self.list_tenants()?;
        let mut rollups = Vec::new();
        let mut attention: Vec<AttentionItem> = Vec::new();
        let mut total_queue = 0i64;
        for t in &tenants {
            let conns = self.tenant_health(&t.id, now_ms)?;
            let status = health::worst(conns.iter().map(|c| c.status));
            let needs: Vec<&ConnectionHealth> = conns
                .iter()
                .filter(|c| matches!(c.status, HealthStatus::Amber | HealthStatus::Red))
                .collect();
            let dead_letters: u64 = conns.iter().map(|c| c.dead_letters).sum();
            let queue_depth = self.relay.pending_depth(&t.id).unwrap_or(0);
            total_queue += queue_depth;
            for c in &needs {
                attention.push(AttentionItem {
                    tenant: t.id.clone(),
                    connection: c.connection.clone(),
                    status: c.status,
                });
            }
            rollups.push(TenantHealth {
                tenant: t.id.clone(),
                status,
                connections: conns.len() as u32,
                needs_attention: needs.len() as u32,
                dead_letters,
                queue_depth,
            });
        }
        // Worst-first: red before amber.
        attention.sort_by_key(|a| match a.status {
            HealthStatus::Red => 0,
            HealthStatus::Amber => 1,
            _ => 2,
        });
        let active_tenants = self.relay.active_tenants().unwrap_or_default().len() as u32;
        Ok(PlatformHealth {
            tenants: rollups,
            needs_attention: attention,
            active_tenants,
            total_queue_depth: total_queue,
        })
    }

    /// Recent runs for `tenant`, newest first (the live feed) — scoped by
    /// `work_units.tenant_id`.
    pub fn recent_runs(&self, tenant: &str, limit: i64) -> Result<Vec<RunRow>, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        Ok(store::run_feed(&mut conn, tenant, limit)?
            .into_iter()
            .map(|r| RunRow {
                id: r.id,
                connection: r.connection,
                state: r.state,
                attempt: r.attempt,
                rows_written: r.rows_written,
                dead_lettered: r.dead_lettered,
                // duration is a view over the two timestamps, computed here (not stored).
                duration_ms: match (r.started_at, r.finished_at) {
                    (Some(s), Some(f)) => Some((f - s).max(0)),
                    _ => None,
                },
                error: r.error,
            })
            .collect())
    }

    /// Recent dead-lettered records for a connection within `tenant` (newest first) —
    /// the *what + why* behind the dead-letter count ([[WEIR-T-0035]]).
    pub fn dead_letters(
        &self,
        tenant: &str,
        connection: &str,
        limit: i64,
    ) -> Result<Vec<DeadLetterRecord>, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        let rows: Vec<(String, String, String)> = dead_letters::table
            .filter(
                dead_letters::tenant_id
                    .eq(tenant)
                    .and(dead_letters::connection.eq(connection)),
            )
            .order(dead_letters::ts.desc())
            .limit(limit)
            .select((
                dead_letters::stream,
                dead_letters::record,
                dead_letters::reason,
            ))
            .load(&mut conn)?;
        Ok(rows
            .into_iter()
            .map(|(stream, record, reason)| DeadLetterRecord {
                stream,
                record,
                reason,
            })
            .collect())
    }

    /// Recent run logs for a connection within `tenant` (newest first) — connector logs
    /// + write diagnostics captured during syncs ([[WEIR-T-0036]]).
    pub fn logs(
        &self,
        tenant: &str,
        connection: &str,
        limit: i64,
    ) -> Result<Vec<LogRecord>, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        let rows: Vec<(String, String, String, i64)> = run_logs::table
            .filter(
                run_logs::tenant_id
                    .eq(tenant)
                    .and(run_logs::connection.eq(connection)),
            )
            .order(run_logs::ts.desc())
            .limit(limit)
            .select((
                run_logs::stream,
                run_logs::level,
                run_logs::message,
                run_logs::ts,
            ))
            .load(&mut conn)?;
        Ok(rows
            .into_iter()
            .map(|(stream, level, message, ts)| LogRecord {
                stream,
                level,
                message,
                ts,
            })
            .collect())
    }

    /// A connector's static spec (roles + `config_schema`) by plugin name — loads
    /// it with empty config; for the UI's schema-driven config form ([[WEIR-T-0040]]).
    pub fn connector_spec(&self, plugin: &str) -> Result<ConnectorSpec, AppError> {
        // Resolve wasm-or-native (WEIR_CONNECTORS_DIR) so the schema form reflects
        // the same connector the run will use.
        Ok(connector_ref(plugin).spec()?)
    }

    /// Discover a source connector's streams ([[WEIR-T-0050]]) — load it with the
    /// given `config` (discovery is config-dependent) and return the stream names.
    pub fn discover_streams(&self, plugin: &str, config: &str) -> Result<Vec<String>, AppError> {
        let cfg = Config {
            json: config.to_string(),
        };
        match connector_ref(plugin).discover(&cfg)? {
            weir_connector::DiscoverOutcome::Catalog(cat) => {
                Ok(cat.streams.into_iter().map(|s| s.name).collect())
            }
            weir_connector::DiscoverOutcome::Error(e) => Err(AppError::Config(e.message)),
        }
    }

    /// Inspectable per-connection state: the committed resume `cursor`, the number
    /// of committed `chunks` (outbox), and the dead-letter count.
    pub fn connection_state(&self, tenant: &str, name: &str) -> Result<ConnectionState, AppError> {
        let c = self.get_connection(tenant, name)?;
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        // Committed resume cursor for `(tenant, connection, stream)`.
        let cursor: Option<Option<String>> = stream_state::table
            .filter(
                stream_state::tenant_id
                    .eq(tenant)
                    .and(stream_state::connection.eq(name))
                    .and(stream_state::stream.eq(&c.stream)),
            )
            .select(stream_state::cursor)
            .first(&mut conn)
            .optional()?;
        // Committed chunks: processed outbox rows for this tenant's connection.
        let chunks: i64 = outbox::table
            .filter(
                outbox::tenant_id
                    .eq(tenant)
                    .and(outbox::connection.eq(name))
                    .and(outbox::processed.eq(1)),
            )
            .count()
            .get_result(&mut conn)?;
        let dead_lettered: i64 = dead_letters::table
            .filter(
                dead_letters::tenant_id
                    .eq(tenant)
                    .and(dead_letters::connection.eq(name)),
            )
            .count()
            .get_result(&mut conn)?;
        Ok(ConnectionState {
            cursor: cursor.flatten(),
            chunks,
            dead_lettered,
        })
    }

    /// Register every scheduled connection with `scheduler` (cron preferred over
    /// interval). Returns how many schedules were registered.
    pub fn register_schedules<C: Clock>(
        &self,
        tenant: &str,
        scheduler: &Scheduler<C>,
    ) -> Result<usize, AppError> {
        let mut n = 0;
        for c in self.list_connections(tenant)? {
            if let Some(expr) = &c.cron {
                scheduler.add_cron(&c.name, &work_spec(tenant, &c), expr)?;
                n += 1;
            } else if let Some(secs) = c.every_secs {
                scheduler.add(
                    &c.name,
                    &work_spec(tenant, &c),
                    Duration::from_secs_f64(secs),
                )?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// Reconcile the scheduler's schedule set against the current connections
    /// ([[WEIR-T-0051]]): register newly-scheduled connections, re-register on a
    /// cadence/cron change, and drop schedules whose connection no longer wants one.
    /// Called each `serve` loop so UI add/edit/delete is picked up **live** (no
    /// restart). Returns the number of changes applied.
    pub fn sync_schedules<C: Clock>(&self, scheduler: &Scheduler<C>) -> Result<usize, AppError> {
        use std::collections::HashMap;
        // The single-node daemon reconciles the implicit default tenant's connections.
        let tenant = DEFAULT_TENANT;
        let conns = self.list_connections(tenant)?;
        // What each connection *wants*: (every_ms, cron) — scheduled ones only.
        let mut wanted: HashMap<String, (i64, Option<String>)> = HashMap::new();
        for c in &conns {
            if let Some(expr) = &c.cron {
                wanted.insert(c.name.clone(), (0, Some(expr.clone())));
            } else if let Some(secs) = c.every_secs {
                wanted.insert(c.name.clone(), ((secs * 1000.0) as i64, None));
            }
        }
        let existing: HashMap<String, (i64, Option<String>)> = scheduler
            .schedules()?
            .into_iter()
            .map(|(n, ms, cr)| (n, (ms, cr)))
            .collect();

        let mut changes = 0;
        for c in &conns {
            let Some(want) = wanted.get(&c.name) else {
                continue;
            };
            // Re-register only when new or the cadence/cron actually changed — so we
            // don't reset `next_due_at` (and re-fire) on every reconcile.
            if existing.get(&c.name) != Some(want) {
                scheduler.remove(&c.name)?;
                if let Some(expr) = &c.cron {
                    scheduler.add_cron(&c.name, &work_spec(tenant, c), expr)?;
                } else if let Some(secs) = c.every_secs {
                    scheduler.add(
                        &c.name,
                        &work_spec(tenant, c),
                        Duration::from_secs_f64(secs),
                    )?;
                }
                changes += 1;
            }
        }
        // Drop schedules whose connection no longer wants one (deleted / schedule cleared).
        for name in existing.keys() {
            if !wanted.contains_key(name) {
                scheduler.remove(name)?;
                changes += 1;
            }
        }
        Ok(changes)
    }

    /// The single-node daemon: each `poll`, **reconcile schedules** against the live
    /// connections ([[WEIR-T-0051]] — picks up UI add/edit/delete without a restart),
    /// tick the scheduler, drain the worker — until `shutdown` resolves. One
    /// in-process agent, no broker.
    pub async fn serve(
        &self,
        poll: Duration,
        concurrency: usize,
        shutdown: impl Future<Output = ()>,
    ) -> Result<(), AppError> {
        let scheduler = Scheduler::new(self.relay.clone(), SystemClock)?;
        // HA ([[WEIR-T-0106]]): only the lease **leader** schedules, so many control-plane replicas are
        // safe (else each would enqueue every due connection). Draining stays unguarded — the work-unit
        // lease ([[WEIR-A-0011]]) already makes that multi-claimant-safe. WEIR_DISABLE_SCHEDULER makes a
        // replica API-only (never schedules), deferring to the others.
        let sched_owner = format!("weir-serve-{}", std::process::id());
        let lease_ttl = poll * 3; // comfortably over the poll, so a live leader keeps renewing
        let scheduling_enabled = std::env::var("WEIR_DISABLE_SCHEDULER").is_err();
        // A per-tenant runner fleet ([[WEIR-T-0091]]): each poll drains every active tenant in its
        // own isolated worker. Idle tenants get no runner (natural reaping).
        let store = Arc::clone(&self.store);
        let relay = self.relay.clone();
        let fleet = Fleet::new(
            self.relay.clone(),
            move || InProcessExecutor::new(Arc::clone(&store), relay.clone()),
            WorkerConfig {
                concurrency: concurrency.max(1),
                ..WorkerConfig::default()
            },
        );
        tokio::pin!(shutdown);
        let mut was_leader = false;
        loop {
            tokio::select! {
                _ = &mut shutdown => break,
                _ = tokio::time::sleep(poll) => {
                    // Resilient: a transient sync/tick/drain error must NOT kill the
                    // loop — that would silently stop all scheduling + draining and
                    // strand runs in `pending`. Log and keep going.
                    let is_leader = scheduling_enabled
                        && match self.relay.try_acquire_lease("scheduler", &sched_owner, lease_ttl) {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::warn!(target: "weir_app", error = %e, "scheduler lease acquisition failed");
                                false
                            }
                        };
                    if is_leader != was_leader {
                        tracing::debug!(target: "weir_app", leader = is_leader, owner = %sched_owner, "scheduler leadership changed");
                        was_leader = is_leader;
                    }
                    if is_leader {
                        if let Err(e) = self.sync_schedules(&scheduler) {
                            tracing::warn!(target: "weir_app", error = %e, "schedule sync failed");
                        }
                        if let Err(e) = scheduler.tick() {
                            tracing::warn!(target: "weir_app", error = %e, "scheduler tick failed");
                        }
                    }
                    if let Err(e) = fleet.run_until_idle().await {
                        tracing::error!(target: "weir_app", error = %e, "fleet drain failed");
                    }
                }
            }
        }
        Ok(())
    }

    /// Run **only the workers** (no scheduler, no HTTP) against the shared store — the standalone
    /// `weir runner` ([[WEIR-T-0102]] / [[WEIR-A-0023]]). `tenant = Some(id)` serves **one** tenant
    /// (pod-per-tenant, [[WEIR-I-0018]]); `None` drains all active tenants (single-node). The
    /// control-plane `serve` still owns scheduling; many runners can claim from one store safely
    /// (the lease model, [[WEIR-A-0011]]). Loops until `shutdown`.
    pub async fn run_workers(
        &self,
        poll: Duration,
        concurrency: usize,
        tenant: Option<String>,
        shutdown: impl Future<Output = ()>,
    ) -> Result<(), AppError> {
        let store = Arc::clone(&self.store);
        let relay = self.relay.clone();
        let make_exec = move || InProcessExecutor::new(Arc::clone(&store), relay.clone());
        let base = WorkerConfig {
            concurrency: concurrency.max(1),
            ..WorkerConfig::default()
        };
        // Resident claim-headroom ([[WEIR-I-0035]] F1.4): if `WEIR_RESIDENT_HIGH_WATER` (a 0.0–1.0
        // mem/cpu fraction) is set, wire a real `SysUsageProbe` so a hot runner stops taking new
        // resident work. Unset → no gate (full headroom), preserving today's behaviour.
        let headroom: Option<(Arc<dyn weir_orchestrator::UsageProbe>, f64)> =
            std::env::var("WEIR_RESIDENT_HIGH_WATER")
                .ok()
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|hw| (0.0..=1.0).contains(hw))
                .map(|hw| {
                    (
                        Arc::new(weir_orchestrator::SysUsageProbe)
                            as Arc<dyn weir_orchestrator::UsageProbe>,
                        hw,
                    )
                });
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                _ = &mut shutdown => break,
                _ = tokio::time::sleep(poll) => {
                    let drained = match &tenant {
                        Some(t) => {
                            let cfg = WorkerConfig { tenant: t.clone(), ..base.clone() };
                            let mut w = Worker::new(self.relay.clone(), make_exec(), cfg);
                            if let Some((p, hw)) = &headroom {
                                w = w.with_headroom(p.clone(), *hw);
                            }
                            w.run_until_idle().await
                        }
                        None => {
                            let mut fleet =
                                Fleet::new(self.relay.clone(), make_exec.clone(), base.clone());
                            if let Some((p, hw)) = &headroom {
                                fleet = fleet.with_headroom(p.clone(), *hw);
                            }
                            fleet.run_until_idle().await
                        }
                    };
                    if let Err(e) = drained {
                        eprintln!("weir runner: drain failed: {e}");
                    }
                }
            }
        }
        Ok(())
    }

    /// Run the k8s **autoscaler** ([[WEIR-T-0104]] / [[WEIR-I-0021]]) — leader-elected, scaling
    /// per-tenant runner Deployments on queue depth. Requires the `kubernetes` feature. Loops until
    /// `shutdown`.
    #[cfg(feature = "kubernetes")]
    pub async fn run_autoscaler(
        &self,
        poll: Duration,
        owner: String,
        namespace: String,
        image: String,
        store_url: String,
        policy: weir_orchestrator::ScalePolicy,
        shutdown: impl Future<Output = ()>,
    ) -> Result<(), AppError> {
        let actuator = weir_orchestrator::KubernetesActuator::new(
            namespace,
            image,
            store_url,
            connectors_dir(),
        )
        .await
        .map_err(AppError::Config)?;
        let mut scaler = weir_orchestrator::Autoscaler::new(
            self.relay.clone(),
            actuator,
            owner,
            policy,
            poll * 3, // lease TTL comfortably over the poll
        );
        // Resident scale-out ([[WEIR-I-0035]] F1.4): if a high-water is configured, feed a
        // measured-usage saturation signal so a hot fleet gets nudged +1 instance. Process-global
        // usage → the same signal for every tenant this control-plane sees (fine for the
        // one-runner-process deployment; a per-tenant report is a follow-on).
        if let Some(hw) = std::env::var("WEIR_RESIDENT_HIGH_WATER")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|hw| (0.0..=1.0).contains(hw))
        {
            let probe = weir_orchestrator::SysUsageProbe;
            let saturated: weir_orchestrator::SaturationFn = Arc::new(move |_tenant: &str| {
                weir_orchestrator::UsageProbe::sample(&probe).max_fraction() >= hw
            });
            scaler = scaler.with_saturation(saturated);
        }
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                _ = &mut shutdown => break,
                _ = tokio::time::sleep(poll) => {
                    if let Err(e) = scaler.tick().await {
                        eprintln!("weir autoscaler: tick failed: {e}");
                    }
                }
            }
        }
        Ok(())
    }

    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }
    pub fn relay(&self) -> &Relay {
        &self.relay
    }
}

/// A registered connector in the catalog ([[WEIR-I-0010]]), keyed `(name, version)`
/// — its spec is snapshotted at registration so listing is a cheap table read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub name: String,
    pub version: String,
    pub roles: Vec<ConnectorRole>,
    pub config_schema: String,
    pub contract_version: u32,
    pub supported_sync_modes: Vec<SyncMode>,
    pub origin: Origin,
    /// `importing` | `ready` | `failed` ([[WEIR-S-0007]] ingress lifecycle).
    pub status: String,
    /// Where the built artifact lives (package + search path pointer). For a
    /// `manifest` kind this is the shared declarative-runtime package.
    pub location: String,
    /// `wasm` (compiled package) | `manifest` (data run on the shared runtime),
    /// per [[WEIR-A-0032]] / [[WEIR-I-0012]].
    #[serde(default = "default_kind")]
    pub kind: String,
    /// The declarative `manifest.yaml` for `kind = "manifest"` (else `None`).
    #[serde(default)]
    pub manifest: Option<String>,
}

fn default_kind() -> String {
    "wasm".to_string()
}

/// A `connectors` row as the typed query builder loads it ([[WEIR-T-0060]]):
/// (name, version, roles, config_schema, contract_version, supported_sync_modes,
/// origin, status, location, kind, manifest).
type CatalogTuple = (
    String,
    String,
    String,
    String,
    i64,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
);

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn ref_version(r: &ConnectorRef) -> String {
    match r {
        ConnectorRef::Wasm { version, .. } => version.clone(),
    }
}

fn from_json<T: for<'de> Deserialize<'de>>(s: &str) -> Result<T, AppError> {
    serde_json::from_str(s).map_err(|e| AppError::Config(e.to_string()))
}

fn catalog_row_to_entry(r: CatalogTuple) -> Result<CatalogEntry, AppError> {
    let (
        name,
        version,
        roles,
        config_schema,
        contract_version,
        supported_sync_modes,
        origin,
        status,
        location,
        kind,
        manifest,
    ) = r;
    Ok(CatalogEntry {
        name,
        version,
        roles: from_json(&roles)?,
        config_schema,
        contract_version: contract_version as u32,
        supported_sync_modes: from_json(&supported_sync_modes)?,
        origin: from_json(&origin)?,
        status,
        location,
        kind,
        manifest,
    })
}

fn json<T: Serialize>(v: &T) -> Result<String, AppError> {
    serde_json::to_string(v).map_err(|e| AppError::Config(e.to_string()))
}

/// Map a low-code manifest + a chosen stream onto the shared declarative runtime's
/// (`rest`) config ([[WEIR-T-0054]]): base_url / path / record_path / pagination
/// / datetime-cursor / auth-scheme translate directly. The manifest declares the auth
/// *scheme* + header/param name; the secret (`api_key`) is supplied per-connection.
/// Offset-pagination remains a runtime gap (surfaced by the preview, [[WEIR-T-0055]]).
fn manifest_stream_to_config(m: &weir_manifest::Manifest, stream: &str) -> serde_json::Value {
    use weir_manifest::{Auth, OAuthGrant, Pagination, PartitionRouter};
    let s = m
        .streams
        .iter()
        .find(|s| s.name == stream)
        .or_else(|| m.streams.first());
    let mut cfg = serde_json::Map::new();
    cfg.insert("base_url".to_string(), m.base_url.clone().into());
    if let Some(s) = s {
        cfg.insert("path".to_string(), s.path.clone().into());
        if let Some(rp) = &s.record_selector {
            cfg.insert("record_path".to_string(), rp.clone().into());
        }
        if let Some(inc) = &s.incremental {
            cfg.insert("cursor_field".to_string(), inc.cursor_field.clone().into());
            cfg.insert("cursor_param".to_string(), inc.cursor_param.clone().into());
            if let Some(sv) = &inc.start_value {
                cfg.insert("cursor_start".to_string(), sv.clone().into());
            }
            if let (Some(ep), Some(ev)) = (&inc.end_param, &inc.end_value) {
                cfg.insert("cursor_end_param".to_string(), ep.clone().into());
                cfg.insert("cursor_end".to_string(), ev.clone().into());
            }
        }
        match &s.pagination {
            Some(Pagination::Page {
                page_param,
                size_param,
                size,
            }) => {
                cfg.insert("page_param".to_string(), page_param.clone().into());
                cfg.insert("page_size_param".to_string(), size_param.clone().into());
                cfg.insert("page_size".to_string(), (*size).into());
            }
            Some(Pagination::Offset {
                offset_param,
                limit_param,
                size,
            }) => {
                cfg.insert("offset_param".to_string(), offset_param.clone().into());
                cfg.insert("page_size_param".to_string(), limit_param.clone().into());
                cfg.insert("page_size".to_string(), (*size).into());
            }
            Some(Pagination::Cursor {
                cursor_path,
                token_param,
            }) => {
                cfg.insert("page_cursor_path".to_string(), cursor_path.clone().into());
                cfg.insert("page_cursor_param".to_string(), token_param.clone().into());
            }
            Some(Pagination::LinkHeader) => {
                cfg.insert("page_link_header".to_string(), true.into());
            }
            None => {}
        }
        // Partition router: the runtime reads the stream once per slice, templating the
        // slice value into the request as `{{ stream_partition.<field> }}` ([[WEIR-T-0064]]).
        match &s.partition {
            Some(PartitionRouter::List { field, values }) => {
                cfg.insert("partition_kind".to_string(), "list".into());
                cfg.insert("partition_field".to_string(), field.clone().into());
                cfg.insert(
                    "partition_values".to_string(),
                    serde_json::Value::Array(
                        values
                            .iter()
                            .map(|v| serde_json::Value::String(v.clone()))
                            .collect(),
                    ),
                );
            }
            Some(PartitionRouter::Substream {
                field,
                parent_path,
                parent_record_selector,
                parent_key,
            }) => {
                cfg.insert("partition_kind".to_string(), "substream".into());
                cfg.insert("partition_field".to_string(), field.clone().into());
                cfg.insert("parent_path".to_string(), parent_path.clone().into());
                cfg.insert("parent_key".to_string(), parent_key.clone().into());
                if let Some(rp) = parent_record_selector {
                    cfg.insert("parent_record_path".to_string(), rp.clone().into());
                }
            }
            None => {}
        }
        // Request options ([[WEIR-T-0068]]): method / POST body / static headers.
        if let Some(m) = &s.http_method {
            cfg.insert("http_method".to_string(), m.clone().into());
        }
        if let Some(b) = &s.request_body {
            cfg.insert("request_body".to_string(), b.clone().into());
        }
        if !s.request_headers.is_empty() {
            cfg.insert(
                "request_headers".to_string(),
                serde_json::Value::Object(
                    s.request_headers
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone().into()))
                        .collect(),
                ),
            );
        }
        // In-flight transforms ([[WEIR-T-0071]]) ride the baked config as `__mapping`;
        // `work_spec` lifts them onto `ConfiguredStream.mapping` for the engine mapping stage.
        if !s.mapping.ops.is_empty()
            && let Ok(mv) = serde_json::to_value(&s.mapping)
        {
            cfg.insert("__mapping".to_string(), mv);
        }
    }
    // Auth scheme (the secret itself, `api_key`, is layered in from the connection config).
    match &m.auth {
        Auth::Bearer { .. } => {
            cfg.insert("auth_scheme".to_string(), "bearer".into());
        }
        Auth::ApiKey { header, .. } => {
            cfg.insert("auth_scheme".to_string(), "header".into());
            cfg.insert("auth_name".to_string(), header.clone().into());
        }
        Auth::ApiKeyQuery { param, .. } => {
            cfg.insert("auth_scheme".to_string(), "query".into());
            cfg.insert("auth_name".to_string(), param.clone().into());
        }
        // OAuth2 / session-token: emit only the **non-secret** metadata. The host reads
        // it (+ the per-connection secret values) and injects the credential via the
        // egress policy ([[WEIR-A-0033]]); the secret never reaches the guest.
        Auth::OAuth2 {
            token_url,
            grant,
            client_id_key,
            client_secret_key,
            refresh_token_key,
            scopes,
        } => {
            cfg.insert("auth_scheme".to_string(), "oauth2".into());
            cfg.insert("oauth_token_url".to_string(), token_url.clone().into());
            cfg.insert(
                "oauth_grant".to_string(),
                match grant {
                    OAuthGrant::RefreshToken => "refresh_token",
                    OAuthGrant::ClientCredentials => "client_credentials",
                }
                .into(),
            );
            cfg.insert(
                "oauth_client_id_key".to_string(),
                client_id_key.clone().into(),
            );
            cfg.insert(
                "oauth_client_secret_key".to_string(),
                client_secret_key.clone().into(),
            );
            if let Some(rt) = refresh_token_key {
                cfg.insert("oauth_refresh_token_key".to_string(), rt.clone().into());
            }
            cfg.insert(
                "oauth_scopes".to_string(),
                serde_json::Value::Array(
                    scopes
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect(),
                ),
            );
        }
        Auth::SessionToken {
            login_url,
            token_path,
            inject_header,
        } => {
            cfg.insert("auth_scheme".to_string(), "session".into());
            cfg.insert("session_login_url".to_string(), login_url.clone().into());
            cfg.insert("session_token_path".to_string(), token_path.clone().into());
            cfg.insert(
                "session_inject_header".to_string(),
                inject_header.clone().into(),
            );
        }
        Auth::Basic {
            username_key,
            password_key,
        } => {
            cfg.insert("auth_scheme".to_string(), "basic".into());
            cfg.insert(
                "basic_username_key".to_string(),
                username_key.clone().into(),
            );
            cfg.insert(
                "basic_password_key".to_string(),
                password_key.clone().into(),
            );
        }
        Auth::None => {}
    }
    serde_json::Value::Object(cfg)
}

/// Layer a user's connection config over the manifest-derived base (user wins).
fn merge_config(mut base: serde_json::Value, user: &str) -> Result<String, AppError> {
    let user_v: serde_json::Value = if user.trim().is_empty() {
        serde_json::json!({})
    } else {
        from_json(user)?
    };
    if let (Some(b), Some(u)) = (base.as_object_mut(), user_v.as_object()) {
        for (k, v) in u {
            b.insert(k.clone(), v.clone());
        }
    }
    Ok(base.to_string())
}

/// Bake a [`weir_manifest::DestinationManifest`] object → the `rest-dest` runtime config
/// ([[WEIR-A-0034]]) — the destination analogue of [`manifest_stream_to_config`]. Auth is
/// emitted as a scheme; the secret is layered in from the connection config and injected
/// host-side ([[WEIR-A-0033]]), never baked here.
pub fn dest_object_to_config(
    m: &weir_manifest::DestinationManifest,
    object: &str,
) -> serde_json::Value {
    use weir_manifest::{Auth, OAuthGrant};
    let mut cfg = serde_json::Map::new();
    cfg.insert("base_url".to_string(), m.base_url.clone().into());
    if let Some(o) = m.objects.iter().find(|o| o.name == object) {
        cfg.insert("path".to_string(), o.path.clone().into());
        cfg.insert("method".to_string(), o.method.clone().into());
        if !o.field_map.is_empty() {
            cfg.insert(
                "field_map".to_string(),
                serde_json::Value::Object(
                    o.field_map
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone().into()))
                        .collect(),
                ),
            );
        }
        if let Some(w) = &o.body_wrap {
            cfg.insert("body_wrap".to_string(), w.clone().into());
        }
    }
    // Auth scheme for host-side injection ([[WEIR-A-0033]]); the secret comes from the
    // connection config and is stripped before the guest sees it. Mirrors the source emission.
    match &m.auth {
        Auth::Bearer { .. } => {
            cfg.insert("auth_scheme".to_string(), "bearer".into());
        }
        Auth::ApiKey { header, .. } => {
            cfg.insert("auth_scheme".to_string(), "header".into());
            cfg.insert("auth_header".to_string(), header.clone().into());
        }
        Auth::OAuth2 {
            token_url,
            grant,
            client_id_key,
            client_secret_key,
            refresh_token_key,
            scopes,
        } => {
            cfg.insert("auth_scheme".to_string(), "oauth2".into());
            cfg.insert("oauth_token_url".to_string(), token_url.clone().into());
            cfg.insert(
                "oauth_grant".to_string(),
                match grant {
                    OAuthGrant::RefreshToken => "refresh_token",
                    OAuthGrant::ClientCredentials => "client_credentials",
                }
                .into(),
            );
            cfg.insert(
                "oauth_client_id_key".to_string(),
                client_id_key.clone().into(),
            );
            cfg.insert(
                "oauth_client_secret_key".to_string(),
                client_secret_key.clone().into(),
            );
            if let Some(rt) = refresh_token_key {
                cfg.insert("oauth_refresh_token_key".to_string(), rt.clone().into());
            }
            if !scopes.is_empty() {
                cfg.insert(
                    "oauth_scopes".to_string(),
                    serde_json::Value::Array(scopes.iter().map(|s| s.clone().into()).collect()),
                );
            }
        }
        _ => {}
    }
    serde_json::Value::Object(cfg)
}

/// Validate a connection's sync/write modes ([[WEIR-I-0028]]) — `upsert` needs business keys,
/// `incremental` needs a cursor field, and the mode strings must be known.
fn validate_connection_modes(
    sync_mode: &str,
    write_mode: &str,
    business_keys: &[String],
    cursor_field: &Option<String>,
    execution_mode: &str,
) -> Result<(), String> {
    match execution_mode {
        // `run_once` = today's batch/micro-batch/CDC path; `resident` = long-lived source
        // ([[WEIR-I-0035]] F1). The connector-capability gate (reject `resident` on a connector
        // that doesn't advertise it) lands with the guest-contract capability in a later task.
        "run_once" | "resident" => {}
        other => {
            return Err(format!(
                "unknown execution_mode `{other}` (run_once | resident)"
            ));
        }
    }
    match sync_mode {
        "full_refresh" | "cdc" => {}
        "incremental" => {
            if cursor_field.as_deref().unwrap_or("").is_empty() {
                return Err("incremental sync_mode requires a cursor_field".to_string());
            }
        }
        other => {
            return Err(format!(
                "unknown sync_mode `{other}` (full_refresh | incremental | cdc)"
            ));
        }
    }
    match write_mode {
        "append" | "overwrite" => {}
        "upsert" => {
            if business_keys.is_empty() {
                return Err("upsert write_mode requires business_keys".to_string());
            }
        }
        other => {
            return Err(format!(
                "unknown write_mode `{other}` (append | upsert | overwrite)"
            ));
        }
    }
    Ok(())
}

/// Parse a connection's `sync_mode` string → the engine enum (unknown → the safe default).
fn parse_sync_mode(s: &str) -> SyncMode {
    match s {
        "incremental" => SyncMode::Incremental,
        "cdc" => SyncMode::Cdc,
        _ => SyncMode::FullRefresh,
    }
}

/// Parse a connection's `write_mode` string (+ keys) → the engine enum.
fn parse_write_mode(s: &str, business_keys: &[String]) -> WriteMode {
    match s {
        "upsert" => WriteMode::Upsert {
            business_keys: business_keys.to_vec(),
        },
        "overwrite" => WriteMode::Overwrite,
        _ => WriteMode::Append,
    }
}

/// Parse a connection's `execution_mode` string → the orchestrator enum ([[WEIR-I-0035]] F1;
/// unknown → the safe `RunOnce` default). Cadence/backoff tuning is a follow-on (F1.3/F1.4); a
/// bare `resident` gets sensible defaults for now.
fn parse_execution_mode(s: &str, every_secs: Option<f64>) -> ExecutionMode {
    match s {
        "resident" => ExecutionMode::Resident {
            // A resident source's declared **emit cadence** reuses `every_secs`: the scheduler
            // is skipped for resident ([[WEIR-I-0035]] F1.3), so `every_secs` is free to mean
            // "emit interval", and the host paces the resident drain at it — once per poll
            // (F1.9). `None` (no `every_secs`) = an event-reader (tail/ws) that emits on upstream
            // arrival, not on a clock. Floor ~20ms (50Hz): below that, use event triggers, not
            // polling ([[WEIR-I-0035]] F1.9 decision).
            cadence_ms: every_secs.map(|s| ((s * 1000.0) as u64).max(20)),
            restart_backoff_ms: 1_000,
        },
        _ => ExecutionMode::RunOnce,
    }
}

/// Build the orchestrator [`WorkSpec`] for a connection, honoring its sync/write modes
/// ([[WEIR-I-0028]]), stamped with the owning `tenant` so the enqueued `work_units.tenant_id`
/// is set ([[WEIR-T-0090]] / [[WEIR-A-0036]]).
pub fn work_spec(tenant: &str, c: &Connection) -> WorkSpec {
    // Lift the in-flight mapping off the baked configs ([[WEIR-T-0071]]): it rode as `__mapping` on a
    // side's config; strip it from **both** so no guest sees it. The mapping is connection-level, so
    // take it from whichever side carries it ([[WEIR-I-0029]]).
    let (src_mapping, source_json) = extract_mapping(&c.source_config);
    let (dst_mapping, dest_json) = extract_mapping(&c.dest_config);
    let mapping = if src_mapping.ops.is_empty() {
        dst_mapping
    } else {
        src_mapping
    };
    WorkSpec {
        connection: c.name.clone(),
        tenant: tenant.to_string(),
        stream: ConfiguredStream {
            stream: c.stream.clone(),
            sync_mode: parse_sync_mode(&c.sync_mode),
            cursor_field: c.cursor_field.clone(),
            primary_key: None,
            write_mode: parse_write_mode(&c.write_mode, &c.business_keys),
            mapping,
        },
        // Scope the refs to the tenant's artifact namespace at execution time ([[WEIR-T-0092]]) —
        // the stored connection stays un-scoped (round-trips), while the runner loads the tenant's
        // private compiled artifact if present (else the shared runtime, via the resolver fallback).
        source: scope_wasm_to_tenant(c.source.clone(), tenant),
        dest: scope_wasm_to_tenant(c.dest.clone(), tenant),
        source_config: Config { json: source_json },
        dest_config: Config { json: dest_json },
        state_key: None,
        seed_cursor: None,
        partition: None,
        execution_mode: parse_execution_mode(&c.execution_mode, c.every_secs),
    }
}

/// Split the connection's baked config into `(in-flight mapping, guest config)` — removing the
/// `__mapping` key the importer/app embedded so the guest never sees it ([[WEIR-T-0071]]).
/// A non-JSON / mapping-less config yields an empty `MappingSpec` and the original string.
fn extract_mapping(config_json: &str) -> (MappingSpec, String) {
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(config_json) else {
        return (MappingSpec::default(), config_json.to_string());
    };
    let mapping = v
        .as_object_mut()
        .and_then(|o| o.remove("__mapping"))
        .and_then(|m| serde_json::from_value::<MappingSpec>(m).ok())
        .unwrap_or_default();
    let json = serde_json::to_string(&v).unwrap_or_else(|_| config_json.to_string());
    (mapping, json)
}

#[cfg(test)]
mod catalog_tests {
    use super::*;

    #[test]
    fn resident_cadence_from_every_secs_with_20ms_floor() {
        // [[WEIR-T-0150]] a resident connection's declared cadence (`every_secs`) is threaded into
        // ExecutionMode::Resident.cadence_ms, clamped to the ~20ms floor (faster → event triggers).
        let mut c = Connection {
            name: "res".into(),
            source: connector_ref("rest"),
            dest: connector_ref("stdout"),
            stream: "s".into(),
            source_config: "{}".into(),
            dest_config: "{}".into(),
            every_secs: Some(2.0),
            cron: None,
            sync_mode: "full_refresh".into(),
            write_mode: "append".into(),
            business_keys: vec![],
            cursor_field: None,
            execution_mode: "resident".into(),
        };
        assert!(matches!(
            work_spec(DEFAULT_TENANT, &c).execution_mode,
            ExecutionMode::Resident {
                cadence_ms: Some(2000),
                ..
            }
        ));
        c.every_secs = Some(0.001); // 1ms → clamped to the 20ms floor
        assert!(matches!(
            work_spec(DEFAULT_TENANT, &c).execution_mode,
            ExecutionMode::Resident {
                cadence_ms: Some(20),
                ..
            }
        ));
    }

    #[test]
    fn work_spec_lifts_mapping_off_baked_config() {
        use weir_connector::{ComputeExpr, MappingOp};
        // The importer/app embed the in-flight mapping as `__mapping` on the baked config
        // ([[WEIR-T-0071]]); work_spec lifts it onto ConfiguredStream.mapping and strips it
        // from the guest config so the sandbox never sees it.
        let mapping = MappingSpec {
            ops: vec![
                MappingOp::Drop {
                    fields: vec!["secret".into()],
                },
                MappingOp::Compute {
                    field: "src".into(),
                    value: ComputeExpr::Const("manual".into()),
                },
            ],
        };
        let config = serde_json::json!({
            "base_url": "https://api.example.com",
            "path": "/items",
            "__mapping": serde_json::to_value(&mapping).unwrap(),
        })
        .to_string();
        let c = Connection {
            name: "c1".into(),
            source: connector_ref("rest"),
            dest: connector_ref("stdout"),
            stream: "items".into(),
            source_config: config,
            dest_config: "{}".into(),
            every_secs: None,
            cron: None,
            sync_mode: "full_refresh".into(),
            write_mode: "append".into(),
            business_keys: vec![],
            cursor_field: None,
            execution_mode: "run_once".into(),
        };
        let ws = work_spec(DEFAULT_TENANT, &c);
        assert_eq!(
            ws.stream.mapping, mapping,
            "mapping lifted onto ConfiguredStream"
        );
        assert!(
            !ws.source_config.json.contains("__mapping"),
            "guest config stripped of __mapping"
        );
        assert!(
            ws.source_config.json.contains("api.example.com"),
            "rest of the config survives"
        );
    }

    #[test]
    fn user_mapping_in_config_reaches_work_spec() {
        use weir_connector::MappingOp;
        // The DEMO §4c path: a `__mapping` set by hand in the connection config (merged over a
        // transform-less manifest base) survives the merge and reaches ConfiguredStream.mapping,
        // and is stripped from the guest config.
        let base = serde_json::json!({ "base_url": "https://x", "path": "/y" });
        let user = r#"{"table":"t","__mapping":{"ops":[{"Drop":{"fields":["created"]}}]}}"#;
        let merged = merge_config(base, user).expect("merge");
        let c = Connection {
            name: "c".into(),
            source: connector_ref("rest"),
            dest: connector_ref("stdout"),
            stream: "s".into(),
            source_config: merged,
            dest_config: "{}".into(),
            every_secs: None,
            cron: None,
            sync_mode: "full_refresh".into(),
            write_mode: "append".into(),
            business_keys: vec![],
            cursor_field: None,
            execution_mode: "run_once".into(),
        };
        let ws = work_spec(DEFAULT_TENANT, &c);
        assert_eq!(
            ws.stream.mapping.ops,
            vec![MappingOp::Drop {
                fields: vec!["created".into()]
            }]
        );
        assert!(
            !ws.source_config.json.contains("__mapping"),
            "guest config stripped"
        );
        assert!(
            ws.source_config.json.contains("\"table\":\"t\""),
            "user config survives"
        );
    }

    #[test]
    fn connection_modes_validate_and_wire() {
        use weir_connector::{SyncMode, WriteMode};
        // Validation ([[WEIR-I-0028]]).
        assert!(
            validate_connection_modes("full_refresh", "append", &[], &None, "run_once").is_ok()
        );
        assert!(validate_connection_modes("nope", "append", &[], &None, "run_once").is_err()); // unknown sync
        assert!(
            validate_connection_modes("full_refresh", "upsert", &[], &None, "run_once").is_err()
        ); // upsert⇒keys
        assert!(
            validate_connection_modes("incremental", "append", &[], &None, "run_once").is_err()
        ); // incremental⇒cursor
        assert!(
            validate_connection_modes("incremental", "append", &[], &Some("id".into()), "run_once")
                .is_ok()
        );
        assert!(
            validate_connection_modes("cdc", "upsert", &["id".to_string()], &None, "run_once")
                .is_ok()
        );
        // execution_mode ([[WEIR-I-0035]] F1): run_once | resident are known; anything else rejected.
        assert!(
            validate_connection_modes("full_refresh", "append", &[], &None, "resident").is_ok()
        );
        assert!(validate_connection_modes("full_refresh", "append", &[], &None, "bogus").is_err());

        // work_spec wires the modes onto the ConfiguredStream (no longer hardcoded).
        let c = Connection {
            name: "c".into(),
            source: connector_ref("postgres"),
            dest: connector_ref("postgres"),
            stream: "t".into(),
            source_config: "{}".into(),
            dest_config: "{}".into(),
            every_secs: None,
            cron: None,
            sync_mode: "cdc".into(),
            write_mode: "upsert".into(),
            business_keys: vec!["id".to_string()],
            cursor_field: None,
            execution_mode: "run_once".into(),
        };
        let ws = work_spec(DEFAULT_TENANT, &c);
        assert!(matches!(ws.stream.sync_mode, SyncMode::Cdc));
        assert!(matches!(&ws.stream.write_mode,
            WriteMode::Upsert { business_keys } if business_keys == &vec!["id".to_string()]));
    }

    #[test]
    fn work_spec_splits_source_and_dest_config() {
        // [[WEIR-I-0029]]: the source and destination resolve with independent config — e.g. a
        // postgres→postgres connection reads one table and writes another (no collision).
        let c = Connection {
            name: "c".into(),
            source: connector_ref("postgres"),
            dest: connector_ref("postgres"),
            stream: "orders".into(),
            source_config: r#"{"table":"orders_src"}"#.into(),
            dest_config: r#"{"table":"orders_dst"}"#.into(),
            every_secs: None,
            cron: None,
            sync_mode: "full_refresh".into(),
            write_mode: "append".into(),
            business_keys: vec![],
            cursor_field: None,
            execution_mode: "run_once".into(),
        };
        let ws = work_spec(DEFAULT_TENANT, &c);
        assert!(ws.source_config.json.contains("orders_src"));
        assert!(ws.dest_config.json.contains("orders_dst"));
        assert!(
            !ws.source_config.json.contains("orders_dst"),
            "sides are independent"
        );
    }

    fn entry(name: &str, version: &str, contract: u32) -> CatalogEntry {
        CatalogEntry {
            name: name.to_string(),
            version: version.to_string(),
            roles: vec![ConnectorRole::Source],
            config_schema: "{}".to_string(),
            contract_version: contract,
            supported_sync_modes: vec![SyncMode::FullRefresh],
            origin: Origin::FirstParty,
            status: "ready".to_string(),
            location: format!("weir-{name}-pkg"),
            kind: "wasm".to_string(),
            manifest: None,
        }
    }

    fn pinned(name: &str, version: &str) -> ConnectorRef {
        ConnectorRef::Wasm {
            search_path: "connectors".to_string(),
            package: format!("weir-{name}-pkg"),
            version: version.to_string(),
            origin: Origin::FirstParty,
        }
    }

    /// An Airbyte stream body with the given `authenticator:` block spliced in.
    fn authed_manifest(authenticator: &str) -> String {
        format!(
            r#"type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: things
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.example.com"
        path: "/things"
{authenticator}
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
          id: {{ type: string }}
"#
        )
    }

    /// The manifest's auth *scheme* maps into the rest config; the secret (`api_key`)
    /// is NOT baked — it's supplied per-connection ([[WEIR-I-0008]] auth).
    #[test]
    fn manifest_auth_maps_to_rest_config() {
        let bearer = weir_importer::import_yaml(
            "ex",
            &authed_manifest(
                "        authenticator:\n          type: BearerAuthenticator\n          api_token: \"{{ config['api_key'] }}\"",
            ),
        )
        .expect("import bearer manifest");
        let cfg = manifest_stream_to_config(&bearer, "things");
        assert_eq!(cfg["auth_scheme"], "bearer");
        assert!(
            cfg.get("api_key").is_none(),
            "secret stays out of the baked config"
        );

        let apikey = weir_importer::import_yaml(
            "ex",
            &authed_manifest(
                "        authenticator:\n          type: ApiKeyAuthenticator\n          header: \"X-Api-Key\"\n          api_token: \"{{ config['api_key'] }}\"",
            ),
        )
        .expect("import apikey manifest");
        let cfg = manifest_stream_to_config(&apikey, "things");
        assert_eq!(cfg["auth_scheme"], "header");
        assert_eq!(cfg["auth_name"], "X-Api-Key");

        // ApiKey with inject_into=request_parameter → query-param key.
        let query = weir_importer::import_yaml(
            "ex",
            &authed_manifest(
                "        authenticator:\n          type: ApiKeyAuthenticator\n          api_token: \"{{ config['api_key'] }}\"\n          inject_into:\n            type: RequestOption\n            field_name: \"apikey\"\n            inject_into: request_parameter",
            ),
        )
        .expect("import query-key manifest");
        let cfg = manifest_stream_to_config(&query, "things");
        assert_eq!(cfg["auth_scheme"], "query");
        assert_eq!(cfg["auth_name"], "apikey");

        // No authenticator → no auth keys at all.
        let none = weir_importer::import_yaml("ex", &authed_manifest("")).expect("import no-auth");
        let cfg = manifest_stream_to_config(&none, "things");
        assert!(cfg.get("auth_scheme").is_none());
    }

    /// CursorPagination maps to the rest config's `page_cursor_path` (dot-path extracted
    /// from the `{{ response[...] }}` template) + `page_cursor_param`.
    #[test]
    fn manifest_cursor_pagination_maps_to_rest_config() {
        let yaml = r#"type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: things
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.example.com"
        path: "/things"
      record_selector:
        type: RecordSelector
        extractor:
          type: DpathExtractor
          field_path: ["data"]
      paginator:
        type: DefaultPaginator
        pagination_strategy:
          type: CursorPagination
          cursor_value: "{{ response['meta']['next_cursor'] }}"
        page_token_option:
          type: RequestOption
          field_name: "cursor"
          inject_into: request_parameter
    schema_loader:
      type: InlineSchemaLoader
      schema:
        type: object
        properties:
          id: { type: string }
"#;
        let m = weir_importer::import_yaml("ex", yaml).expect("import cursor manifest");
        let cfg = manifest_stream_to_config(&m, "things");
        assert_eq!(cfg["page_cursor_path"], "meta.next_cursor");
        assert_eq!(cfg["page_cursor_param"], "cursor");
    }

    #[test]
    fn catalog_persists_and_pin_survives_newer_registration() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = tmp.path().join("weir.db");
        let app = App::open(db.to_str().unwrap()).unwrap();
        app.register_connector(DEFAULT_TENANT, &entry("slow", "1.0.0", 1))
            .unwrap();

        // persist → reopen → still enumerable
        drop(app);
        let app = App::open(db.to_str().unwrap()).unwrap();
        assert_eq!(app.list_connectors(DEFAULT_TENANT).unwrap().len(), 1);

        // a newer registration coexists; the older pinned version survives (no auto-upgrade)
        app.register_connector(DEFAULT_TENANT, &entry("slow", "2.0.0", 1))
            .unwrap();
        assert_eq!(app.list_connectors(DEFAULT_TENANT).unwrap().len(), 2);
        assert!(
            app.get_connector(DEFAULT_TENANT, "slow", "1.0.0")
                .unwrap()
                .is_some()
        );
        assert!(
            app.get_connector(DEFAULT_TENANT, "slow", "2.0.0")
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn contract_gate_refuses_incompatible_then_allows_compatible() {
        let tmp = tempfile::TempDir::new().unwrap();
        let app = App::open(tmp.path().join("weir.db").to_str().unwrap()).unwrap();

        // register slow@9.9.9 targeting an unsupported contract; pin a connection to it
        app.register_connector(DEFAULT_TENANT, &entry("slow", "9.9.9", 99))
            .unwrap();
        app.add_connection(
            DEFAULT_TENANT,
            &Connection {
                name: "c".to_string(),
                source: pinned("slow", "9.9.9"),
                dest: pinned("arrow-sink", "0.0.0"), // uncataloged → gate passes
                stream: "s".to_string(),
                source_config: "{}".to_string(),
                dest_config: "{}".to_string(),
                every_secs: None,
                cron: None,
                sync_mode: "full_refresh".into(),
                write_mode: "append".into(),
                business_keys: vec![],
                cursor_field: None,
                execution_mode: "run_once".into(),
            },
        )
        .unwrap();
        let err = app.plan_run(DEFAULT_TENANT, "c").unwrap_err();
        assert!(
            matches!(err, AppError::Contract(_)),
            "expected Contract, got {err:?}"
        );

        // re-register at a compatible contract → the run can be planned
        app.register_connector(
            DEFAULT_TENANT,
            &entry("slow", "9.9.9", ENGINE_CONTRACT_VERSION),
        )
        .unwrap();
        assert!(app.plan_run(DEFAULT_TENANT, "c").is_ok());
    }
}
