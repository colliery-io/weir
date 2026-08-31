//! # weir-orchestrator
//!
//! The async run/work-unit **orchestration** layer ([[WEIR-S-0004]]), realizing
//! the transactional outbox as a real work queue ([[WEIR-A-0010]]) with a
//! claim/heartbeat **lease** so it is agent-fleet-ready ([[WEIR-A-0002]]).
//!
//! Per [[WEIR-A-0028]], **async lives here only**; the [`weir_engine::Engine`]
//! and the connector contract stay sync, bridged via `spawn_blocking`. The
//! design follows cloacina's `dispatcher`/`executor` split (pattern, not
//! dependency — [[WEIR-A-0027]]): a lightweight ready-event is handed to a
//! [`WorkExecutor`]; context is loaded at execution time from the shared store.
//!
//! - [`Relay`] — the `work_units` queue + state machine (sync DB ops).
//! - [`WorkExecutor`] — the execution seam (`InProcessExecutor` now; remote later).
//! - [`Worker`] — the async loop: claim → execute → apply (with retry/backoff).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use weir_schema::{dead_letters, leader_leases, run_logs, schedules, work_units};

use weir_connector::{
    Config, ConfiguredStream, DiscoverOutcome, ErrorKind, Partition, PartitionScheme,
};
use weir_engine::{
    Engine, EngineError, StopHandle, Store, SyncOptions, SyncProgress, stop_channel,
};
use weir_runtime::{ConnectorHandle, Credential, HostAllowList};

mod lineage;
mod store;
pub use lineage::Lineage;

pub mod actuator;
#[cfg(feature = "kubernetes")]
pub use actuator::KubernetesActuator;
pub use actuator::{Actuator, NoopActuator, runner_deployment_json, runner_name};

pub mod autoscaler;
pub use autoscaler::{Autoscaler, SaturationFn, ScalePolicy};

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Provenance of a connector ([[WEIR-A-0018]] Publication Contract) — *how it
/// entered weir*, used for trust/curation. Defaults to `Private` (direct-import),
/// so pre-catalog connection rows backfill to the most conservative origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// On the curated first-party allowlist (trusted).
    FirstParty,
    /// Discovered on crates.io but not allowlisted (untrusted).
    Community,
    /// Brought in by explicit local/git import (the operator's trust act).
    #[default]
    Private,
}

fn default_version() -> String {
    "0.0.0".to_string()
}

/// The implicit tenant ([[WEIR-A-0036]]) for `WorkSpec`s deserialized from pre-multitenant
/// `work_units`/`schedules` rows (matches the schema `tenant_id DEFAULT 'default'`).
fn default_tenant() -> String {
    "default".to_string()
}

/// How to resolve a connector at execution time. Serializable so the same
/// work-unit row can be picked up by any in-process or (future) remote agent —
/// a `ConnectorHandle` never crosses the seam, only the *reference* does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectorRef {
    /// A WASM-component package under `search_path` — the **only** connector form
    /// ([[WEIR-A-0030]] WASM-always; native in-process retired in [[WEIR-I-0011]]).
    Wasm {
        search_path: String,
        package: String,
        /// Semver this connection is **pinned** to ([[WEIR-A-0019]]); the catalog
        /// `(name, version)` key (connector's `Cargo.toml` version, [[WEIR-A-0018]]).
        /// `#[serde(default)]` so pre-catalog `source_ref`/`dest_ref` rows backfill.
        #[serde(default = "default_version")]
        version: String,
        /// Provenance ([[WEIR-A-0018]]); `#[serde(default)]` → `Private` for old rows.
        #[serde(default)]
        origin: Origin,
    },
}

impl ConnectorRef {
    /// Resolve to a **configured** connector instance — `config` is bound once at
    /// construction (v1 configured instances, [[WEIR-A-0029]]).
    /// Load the connector (with empty config) and return its static spec —
    /// roles + `config_schema` (JSON Schema) for the UI's schema-driven config
    /// form ([[WEIR-T-0040]]).
    pub fn spec(&self) -> Result<weir_connector::ConnectorSpec, ExecutorError> {
        let handle = self.resolve(&Config {
            json: "{}".to_string(),
        })?;
        handle
            .spec()
            .map_err(|e| ExecutorError::Resolve(e.to_string()))
    }

    /// Load the connector with `config` and ask it to **discover** its streams
    /// ([[WEIR-T-0050]]) — kept live (config-dependent), not snapshotted.
    pub fn discover(&self, config: &Config) -> Result<DiscoverOutcome, ExecutorError> {
        self.resolve(config)?
            .discover(config)
            .map_err(|e| ExecutorError::Resolve(e.to_string()))
    }

    /// Evict cached handles for `(search_path, package)` across **all configs** —
    /// call after re-importing / re-syncing a connector ([[WEIR-T-0051]]) so the
    /// next run loads the freshly-built wasm instead of the stale cached handle.
    pub fn invalidate_cache(search_path: &str, package: &str) {
        if let Some(cache) = HANDLE_CACHE.get() {
            let prefix = format!("{search_path}\u{0}{package}\u{0}");
            cache.lock().unwrap().retain(|k, _| !k.starts_with(&prefix));
        }
    }

    /// Resolve to a loaded connector, **reusing a cached handle** per
    /// `(search_path, package, config)` ([[WEIR-T-0051]]). `from_wasm_package`
    /// JIT-compiles the wasm component (seconds) — caching the loaded handle makes
    /// that a one-time cost; each run then just instantiates a fresh sandbox Store
    /// (fidius does this per call, so the shared handle is safe across concurrent
    /// executions — `ConnectorHandle` is `Send + Sync`).
    fn resolve(&self, config: &Config) -> Result<Arc<ConnectorHandle>, ExecutorError> {
        match self {
            ConnectorRef::Wasm {
                search_path,
                package,
                ..
            } => {
                // Key on the full config (incl. any secret) so two connections to the
                // same package with different credentials never share a handle/token.
                let key = format!("{search_path}\u{0}{package}\u{0}{}", config.json);
                let cache = HANDLE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
                if let Some((h, last_used)) = cache.lock().unwrap().get_mut(&key) {
                    *last_used = std::time::Instant::now();
                    return Ok(Arc::clone(h));
                }
                // Resolve the host-side credential ([[WEIR-A-0033]]): the secret is
                // injected by the egress policy and the guest receives only the
                // **sanitized** config — no api-key / OAuth / session secret crosses
                // into the sandbox. `allow_all` hosts (demo default); tighten to
                // per-connection allow-lists later.
                let (credential, guest_json) = Credential::from_auth_config(&config.json);
                let guest_config = Config { json: guest_json };
                let policy = match credential {
                    Some(c) => HostAllowList::allow_all().with_credential(c),
                    None => HostAllowList::allow_all(),
                };
                // Per-tenant artifact resolution ([[WEIR-T-0092]]): a connection's `search_path` is
                // its tenant namespace (`<dir>/<tenant>`). Load the tenant's private artifact if it's
                // there, else fall back to the shared dir (`<parent>`) for the generic runtimes/guests
                // every tenant shares. (Additive: existing single-dir layouts hit the first branch.)
                let base = std::path::Path::new(search_path);
                let effective = if base.join(package).exists() {
                    base.to_path_buf()
                } else {
                    match base.parent() {
                        Some(p) if p.join(package).exists() => p.to_path_buf(),
                        _ => base.to_path_buf(),
                    }
                };
                // Build outside the lock (compile is slow; don't block other keys).
                let handle = Arc::new(
                    ConnectorHandle::from_wasm_package(
                        &effective,
                        package,
                        &guest_config,
                        policy,
                        &[],
                    )
                    .map_err(|e| ExecutorError::Resolve(e.to_string()))?,
                );
                let mut cache = cache.lock().unwrap();
                // Bound the cache ([[WEIR-T-0190]]): each entry holds a JIT-compiled
                // component, and distinct configs/versions accumulate forever
                // otherwise — evict the least-recently-used entry at the cap.
                // (Live runs keep their handle via the returned `Arc`.)
                if !cache.contains_key(&key) {
                    lru_evict(&mut cache, HANDLE_CACHE_CAP);
                }
                let (h, _) = cache
                    .entry(key)
                    .or_insert((handle, std::time::Instant::now()));
                Ok(Arc::clone(h))
            }
        }
    }
}

/// Max cached connector handles ([[WEIR-T-0190]]) — LRU-evicted past this.
const HANDLE_CACHE_CAP: usize = 32;

/// Make room for one insert: while `map` is at/over `cap`, evict the entry with
/// the oldest last-use stamp ([[WEIR-T-0190]]).
fn lru_evict<T>(map: &mut HashMap<String, (T, std::time::Instant)>, cap: usize) {
    while map.len() >= cap {
        let Some(lru) = map
            .iter()
            .min_by_key(|(_, (_, at))| *at)
            .map(|(k, _)| k.clone())
        else {
            return;
        };
        map.remove(&lru);
    }
}

/// Process-wide cache of loaded connector handles, keyed by
/// `(search_path, package, config)`, valued with a last-use stamp for LRU
/// eviction — see [`ConnectorRef::resolve`].
#[allow(clippy::type_complexity)]
static HANDLE_CACHE: OnceLock<Mutex<HashMap<String, (Arc<ConnectorHandle>, std::time::Instant)>>> =
    OnceLock::new();

/// How a connection's source is executed ([[WEIR-I-0035]] F1). **Host-side only** —
/// it never crosses the connector (wasm) boundary; the guest `read` stream is
/// identical, the runtime decides resident vs run-to-completion. F1.2/F1.3 branch
/// on this; F1.1 only threads it (no resident execution yet).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum ExecutionMode {
    /// Run to completion and exit — today's batch / micro-batch / CDC path.
    #[default]
    RunOnce,
    /// Stay resident and emit continuously under supervision ([[WEIR-I-0035]]).
    Resident {
        /// Polling cadence in ms; `None` = event-reader (emit on upstream arrival).
        #[serde(default)]
        cadence_ms: Option<u64>,
        /// Base restart backoff (ms) for supervised restart (F1.3).
        #[serde(default = "default_restart_backoff_ms")]
        restart_backoff_ms: u64,
    },
}

fn default_restart_backoff_ms() -> u64 {
    1_000
}

impl ExecutionMode {
    /// True for resident (long-lived) execution ([[WEIR-I-0035]] F1).
    pub fn is_resident(&self) -> bool {
        matches!(self, ExecutionMode::Resident { .. })
    }

    /// Base restart backoff (ms) for a resident source; `None` for run-once. Drives the
    /// supervised-restart backoff in [`Worker::tick`] ([[WEIR-I-0035]] F1.3).
    pub fn restart_backoff_ms(&self) -> Option<u64> {
        match self {
            ExecutionMode::Resident {
                restart_backoff_ms, ..
            } => Some(*restart_backoff_ms),
            ExecutionMode::RunOnce => None,
        }
    }

    /// Polling cadence (ms) for a resident source; `None` = run-once or an event-reader
    /// (emit on arrival). Threaded to the engine's resident drain to pace emission
    /// ([[WEIR-I-0035]] F1.6).
    pub fn cadence_ms(&self) -> Option<u64> {
        match self {
            ExecutionMode::Resident { cadence_ms, .. } => *cadence_ms,
            ExecutionMode::RunOnce => None,
        }
    }
}

/// The work to enqueue with [`Relay::plan`]. Stored (as JSON columns) in the
/// `work_units` row and reloaded by the executor at run time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkSpec {
    pub connection: String,
    /// Owning tenant ([[WEIR-A-0036]] / [[WEIR-T-0090]]) — stamped onto the enqueued
    /// `work_units.tenant_id` by [`Relay::plan`] so runner isolation is preserved.
    /// Rides through the `schedules.spec` JSON too, so scheduler-fired runs stay
    /// tenant-scoped. `#[serde(default)]` backfills pre-multitenant rows to `"default"`.
    #[serde(default = "default_tenant")]
    pub tenant: String,
    pub stream: ConfiguredStream,
    pub source: ConnectorRef,
    pub dest: ConnectorRef,
    /// Per-side connector config ([[WEIR-I-0029]]): the source and destination resolve independently.
    pub source_config: Config,
    pub dest_config: Config,
    /// Checkpoint key override (backfill isolation); `None` = the live stream.
    #[serde(default)]
    pub state_key: Option<String>,
    /// Backfill lower-bound cursor (used when no state exists under `state_key`).
    #[serde(default)]
    pub seed_cursor: Option<String>,
    /// Which partition this unit reads ([[WEIR-T-0027]]); `None` = the whole
    /// stream. A partitioned plan fans out one unit per partition.
    #[serde(default)]
    pub partition: Option<Partition>,
    /// Execution mode ([[WEIR-I-0035]] F1): run-once (default) vs resident.
    /// `#[serde(default)]` backfills existing `work_units` rows to `RunOnce`.
    #[serde(default)]
    pub execution_mode: ExecutionMode,
}

/// Materialize concrete partitions from a stream's declared [`PartitionScheme`]
/// ([[WEIR-T-0027]]). v0 handles `Unpartitioned` (one partition) and
/// `ByKeyShards` (N hash shards); other schemes fall back to one partition.
pub fn materialize_partitions(scheme: &PartitionScheme) -> Vec<Partition> {
    match scheme {
        PartitionScheme::ByKeyShards { key, count } => (0..*count)
            .map(|i| Partition {
                id: format!("shard-{i}"),
                bounds: Some(
                    serde_json::json!({ "key": key, "shard": i, "of": count }).to_string(),
                ),
            })
            .collect(),
        _ => vec![Partition {
            id: "p0".to_string(),
            bounds: None,
        }],
    }
}

/// Lightweight ready event (cloacina-style): IDs only; the executor lazy-loads
/// the [`WorkSpec`] from the store.
#[derive(Debug, Clone)]
pub struct WorkReadyEvent {
    pub work_unit_id: i64,
    pub attempt: i64,
}

/// Outcome of executing a work unit.
#[derive(Debug, Clone)]
pub enum ExecutionResult {
    Completed {
        rows_written: u64,
        dead_lettered: u64,
    },
    Failed {
        kind: ErrorKind,
        message: String,
    },
    /// The unit vanished between claim and load (e.g. reclaimed elsewhere).
    Skipped,
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("connector resolve: {0}")]
    Resolve(String),
    #[error("blocking task join: {0}")]
    Join(String),
    #[error("store: {0}")]
    Store(String),
    #[error("db: {0}")]
    Db(#[from] diesel::result::Error),
    /// A drain pass processed this many units without reaching idle
    /// ([[WEIR-T-0190]]) — something is perpetually requeuing itself.
    #[error("drain did not reach idle after {0} units — a unit may be perpetually requeuing")]
    DrainStuck(u64),
}

/// Retention knobs ([[WEIR-T-0188]]) for [`Relay::prune_retention`]. `None`
/// disables that cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionConfig {
    /// Delete terminal rows older than this (epoch-ms age).
    pub max_age_ms: Option<i64>,
    /// Keep at most this many rows per table **per tenant** (newest win).
    pub max_rows: Option<i64>,
}

impl RetentionConfig {
    /// Read the operator knobs: `WEIR_RETENTION_DAYS` (default 30, `0`
    /// disables) and `WEIR_RETENTION_MAX_ROWS` (default 10_000, `0` disables).
    pub fn from_env() -> Self {
        let days = std::env::var("WEIR_RETENTION_DAYS")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(30);
        let rows = std::env::var("WEIR_RETENTION_MAX_ROWS")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(10_000);
        Self {
            max_age_ms: (days > 0).then(|| days.saturating_mul(86_400_000)),
            max_rows: (rows > 0).then_some(rows),
        }
    }
}

/// Retry/backoff policy for the relay (transients requeue; fatal/config fail).
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub owner: String,
    /// The tenant this worker claims for ([[WEIR-A-0036]]): a worker only ever sees this tenant's
    /// work. The `Fleet` runs one worker per active tenant.
    pub tenant: String,
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub lease: Duration,
    /// How often to heartbeat (extend the lease) while a unit executes. Should
    /// be well under `lease`. `Duration::ZERO` disables heartbeating.
    pub heartbeat: Duration,
    /// Max units executing **simultaneously** in `run_until_idle` — a cap on
    /// in-flight executions, independent of how many workers/nodes drain the queue
    /// (the atomic claim keeps concurrent claimants from double-running a unit).
    pub concurrency: usize,
    /// Max units one `run_until_idle` pass may process before failing loudly
    /// with [`ExecutorError::DrainStuck`] ([[WEIR-T-0190]]) — a unit that
    /// perpetually requeues itself due-now must surface, not hang the drain.
    pub max_drain_units: u64,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            owner: "in-process".to_string(),
            tenant: "default".to_string(),
            max_attempts: 1,
            base_delay: Duration::ZERO,
            lease: Duration::from_secs(30),
            heartbeat: Duration::from_secs(10),
            concurrency: 16,
            max_drain_units: 10_000,
        }
    }
}

/// Measured runner resource usage, as fractions in `[0.0, 1.0]` ([[WEIR-I-0035]] F1.4).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Usage {
    pub mem_fraction: f64,
    pub cpu_fraction: f64,
}

impl Usage {
    /// The dominant pressure signal — whichever of mem/cpu is higher.
    pub fn max_fraction(&self) -> f64 {
        self.mem_fraction.max(self.cpu_fraction)
    }
}

/// An injectable source of measured runner usage ([[WEIR-I-0035]] F1.4 / [[WEIR-A-0022]]).
/// The default [`NullUsageProbe`] reports full headroom, so the resident claim-headroom gate is
/// inert unless a real probe is configured — a concrete cross-platform probe (e.g. `sysinfo`) is
/// a follow-on hookup; the gate + scale plumbing is wired against this trait so it stays testable.
pub trait UsageProbe: Send + Sync {
    fn sample(&self) -> Usage;
}

/// Reports full headroom (`0.0`) → the gate never trips. The default probe.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullUsageProbe;
impl UsageProbe for NullUsageProbe {
    fn sample(&self) -> Usage {
        Usage::default()
    }
}

/// A fixed usage reading — for tests or a manual override.
#[derive(Debug, Clone, Copy)]
pub struct StaticUsageProbe(pub Usage);
impl UsageProbe for StaticUsageProbe {
    fn sample(&self) -> Usage {
        self.0
    }
}

/// Best-effort, dependency-free runner usage probe ([[WEIR-I-0035]] F1.4 / [[WEIR-A-0022]]).
///
/// On Linux it reads `/proc` (system memory pressure from `MemTotal`/`MemAvailable`, CPU pressure
/// from `loadavg` ÷ core count). On any other platform, or on any read/parse error, it **degrades
/// safely to full headroom** (`Usage::default()`, `0.0`) so the claim-headroom gate never trips
/// spuriously — a missing probe is never worse than no gate. No external dependency (avoids a
/// network fetch), cross-platform-safe.
#[derive(Debug, Clone, Copy, Default)]
pub struct SysUsageProbe;

impl SysUsageProbe {
    #[cfg(target_os = "linux")]
    fn sample_linux() -> Option<Usage> {
        let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
        let mut total = None;
        let mut avail = None;
        for line in meminfo.lines() {
            if let Some(v) = line.strip_prefix("MemTotal:") {
                total = v
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<f64>().ok());
            } else if let Some(v) = line.strip_prefix("MemAvailable:") {
                avail = v
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<f64>().ok());
            }
        }
        let (total, avail) = (total?, avail?);
        let mem_fraction = if total > 0.0 {
            (1.0 - (avail / total)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        // 1-minute load average ÷ available cores as a cheap CPU-pressure proxy (no sampling
        // interval needed, unlike per-process %CPU).
        let loadavg = std::fs::read_to_string("/proc/loadavg").ok()?;
        let load1: f64 = loadavg.split_whitespace().next()?.parse().ok()?;
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1) as f64;
        let cpu_fraction = if cores > 0.0 {
            (load1 / cores).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Some(Usage {
            mem_fraction,
            cpu_fraction,
        })
    }
}

impl UsageProbe for SysUsageProbe {
    fn sample(&self) -> Usage {
        #[cfg(target_os = "linux")]
        {
            Self::sample_linux().unwrap_or_default()
        }
        #[cfg(not(target_os = "linux"))]
        {
            // No portable dependency-free probe → full headroom (gate off).
            Usage::default()
        }
    }
}

/// The resident claim-headroom gate decision ([[WEIR-I-0035]] F1.4): step aside from a **new
/// resident** claim once measured usage reaches the high-water mark. Run-once is never gated;
/// with no `high_water` configured the gate is off. Pure, so it is unit-tested directly.
fn gate_new_resident(high_water: Option<f64>, usage_max: f64, is_resident: bool) -> bool {
    matches!(high_water, Some(hw) if is_resident && usage_max >= hw)
}

/// The execution seam (ported from cloacina `TaskExecutor`). Async so a remote
/// backend is natural; the in-process impl bridges to the sync engine.
#[async_trait]
pub trait WorkExecutor: Send + Sync {
    async fn execute(&self, event: WorkReadyEvent) -> Result<ExecutionResult, ExecutorError>;
    fn name(&self) -> &str;
    /// Best-effort cancel of an in-flight run: fire its resident stop token, if any.
    /// Default no-op; `InProcessExecutor` fires the run's `StopHandle` so a durable cancel
    /// observed via lease-loss halts the ACTUAL run, cross-process ([[WEIR-T-0149]]).
    fn stop(&self, _work_unit_id: i64) {}
}

// ---- Relay: the work_units queue + state machine (sync DB) ----

/// A client-generated, monotonic work-unit / schedule id: `now_ms() << 20 | counter`
/// (the generator has no autoincrement; this keeps ids unique + roughly ordered, so
/// the FIFO `ORDER BY id` claim and the run feed are preserved). [[WEIR-T-0061]]
///
/// The 20-bit counter is SEEDED with a per-process nonce ([[WEIR-T-0190]]):
/// a zero-start counter meant a restarted process — or a second worker — minting
/// an id in the same millisecond re-minted the exact same id.
fn next_id() -> i64 {
    use std::sync::atomic::{AtomicI64, Ordering};
    static COUNTER: OnceLock<AtomicI64> = OnceLock::new();
    let counter = COUNTER.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as i64)
            .unwrap_or(0);
        AtomicI64::new((std::process::id() as i64) ^ nanos)
    });
    let c = counter.fetch_add(1, Ordering::Relaxed) & 0xF_FFFF; // 20-bit per-process counter
    (now_ms() << 20) | c
}

/// Absolute cap on the resident restart delay ([[WEIR-T-0190]]): 5 minutes.
const RESIDENT_BACKOFF_CAP_MS: u64 = 300_000;
/// A resident run alive at least this long counts as healthy uptime — its exit
/// restarts on the BASE backoff (decay), not the accumulated attempt shift.
const RESIDENT_HEALTHY_UPTIME_MS: u64 = 60_000;

/// Restart delay for a resident exit ([[WEIR-T-0190]]): exponential in the
/// unit's attempt count, but (a) a run that stayed healthy ≥
/// [`RESIDENT_HEALTHY_UPTIME_MS`] resets to the base delay — `attempt` only ever
/// grows over the unit's lifetime, so without decay one bad night would leave
/// the connector permanently slow to restart — and (b) absolutely capped at
/// [`RESIDENT_BACKOFF_CAP_MS`] so backoff can never grow into hours.
fn resident_restart_delay(base_ms: u64, attempt: i64, run_ms: u64) -> i64 {
    let shift = if run_ms >= RESIDENT_HEALTHY_UPTIME_MS {
        0
    } else {
        (attempt - 1).clamp(0, 16) as u32
    };
    base_ms
        .saturating_mul(1u64 << shift)
        .min(RESIDENT_BACKOFF_CAP_MS) as i64
}

/// A work unit's status (for run history).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkUnitStatus {
    pub id: i64,
    pub state: String,
    pub attempt: i64,
    /// Why the run failed, if it did.
    pub error: Option<String>,
}

/// A recent run across all connections (for the live feed) — with throughput
/// and timing so the UI can show what a connector did / is doing right now.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRow {
    pub id: i64,
    pub connection: String,
    pub state: String,
    pub attempt: i64,
    pub rows_written: i64,
    pub dead_lettered: i64,
    /// Wall-clock duration in ms once finished; `None` while in flight.
    pub duration_ms: Option<i64>,
    /// Why the run failed (the connector/engine error), if it did; `None` otherwise.
    pub error: Option<String>,
}

/// The durable work queue + state machine. Sync DB ops over the shared store's
/// pool; the async [`Worker`] calls these via `spawn_blocking`.
#[derive(Clone)]
pub struct Relay {
    store: Arc<Store>,
    /// Live resident StopHandles ([[WEIR-T-0170]]): registered by the in-process executor
    /// for a run's lifetime; fired by heartbeat lease-loss, manual stop, or daemon shutdown
    /// (`stop_all_residents`). Lives on the relay — shared across every executor the fleet
    /// spawns — so one shutdown call reaches every live resident. Each entry carries a
    /// registration generation so a STALE run's deregister (its lease expired and the unit
    /// was re-claimed in-process) cannot remove — and thereby drop-cancel — the
    /// replacement run's handle.
    resident_stops: Arc<Mutex<HashMap<i64, (u64, StopHandle)>>>,
    stop_generation: Arc<std::sync::atomic::AtomicU64>,
}

impl Relay {
    /// Create the relay and ensure the `work_units` table exists.
    pub fn new(store: Arc<Store>) -> Result<Self, ExecutorError> {
        // Schema (work_units + schedules) is created by Store::open →
        // weir_schema::migrate ([[WEIR-T-0061]] / [[WEIR-I-0013]]).
        let relay = Self {
            store,
            resident_stops: Arc::new(Mutex::new(HashMap::new())),
            stop_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        };
        Ok(relay)
    }

    /// Enqueue a `Pending` work unit; returns its id.
    pub fn plan(&self, spec: &WorkSpec) -> Result<i64, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let id = next_id();
        // The work-unit row mapping lives in `store` ([[WEIR-I-0030]]).
        store::insert_pending(&mut conn, id, spec)?;
        Ok(id)
    }

    /// Enqueue a **backfill**: re-sync from `from_cursor` under an isolated
    /// checkpoint key, so the live incremental cursor is never touched
    /// ([[WEIR-T-0010]]). The connector still reads the real stream.
    pub fn plan_backfill(&self, spec: &WorkSpec, from_cursor: &str) -> Result<i64, ExecutorError> {
        let mut spec = spec.clone();
        spec.state_key = Some(format!("backfill:{}:{}", spec.stream.stream, from_cursor));
        spec.seed_cursor = Some(from_cursor.to_string());
        self.plan(&spec)
    }

    /// Fan out a **partitioned** plan ([[WEIR-T-0027]]): one work unit per
    /// partition, each with the `Partition` set and its own `state_key` so
    /// partitions checkpoint + resume independently and the worker(s) run them in
    /// parallel. Returns the planned ids.
    pub fn plan_partitioned(
        &self,
        spec: &WorkSpec,
        partitions: &[Partition],
    ) -> Result<Vec<i64>, ExecutorError> {
        let mut ids = Vec::with_capacity(partitions.len());
        for p in partitions {
            let mut s = spec.clone();
            s.state_key = Some(format!("part:{}:{}", spec.stream.stream, p.id));
            s.partition = Some(p.clone());
            ids.push(self.plan(&s)?);
        }
        Ok(ids)
    }

    /// Atomically lease the next due `Pending` unit, incrementing its attempt.
    /// Returns `None` if nothing is claimable (or another claimant won the race).
    pub fn claim(
        &self,
        owner: &str,
        tenant: &str,
        lease: Duration,
    ) -> Result<Option<WorkReadyEvent>, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let now = now_ms();
        let lease_expires = now + lease.as_millis() as i64;
        conn.transaction::<Option<WorkReadyEvent>, diesel::result::Error, _>(|conn| {
            // Lowest-id due pending unit **for this tenant** (per-tenant FIFO, [[WEIR-A-0036]]):
            // a runner only ever claims its own tenant's work. Then the state-guarded UPDATE —
            // the rows-affected check is the atomic lock (portable; no FOR UPDATE / SKIP LOCKED).
            // Uses the `(tenant_id, state, next_attempt_at)` index ([[WEIR-T-0089]]).
            let candidate: Option<(i64, i64)> = work_units::table
                .filter(
                    work_units::tenant_id
                        .eq(tenant)
                        .and(work_units::state.eq("pending"))
                        .and(work_units::next_attempt_at.le(now)),
                )
                .order(work_units::id.asc())
                .select((work_units::id, work_units::attempt))
                .first(conn)
                .optional()?;
            let Some((cid, cattempt)) = candidate else {
                return Ok(None);
            };
            let updated = diesel::update(
                work_units::table
                    .filter(work_units::id.eq(cid).and(work_units::state.eq("pending"))),
            )
            .set((
                work_units::state.eq("leased"),
                work_units::lease_owner.eq(owner),
                work_units::lease_expires_at.eq(lease_expires),
                work_units::attempt.eq(work_units::attempt + 1),
                work_units::started_at.eq(now),
                work_units::finished_at.eq(None::<i64>),
            ))
            .execute(conn)?;
            if updated == 1 {
                Ok(Some(WorkReadyEvent {
                    work_unit_id: cid,
                    attempt: cattempt + 1,
                }))
            } else {
                Ok(None)
            }
        })
        .map_err(ExecutorError::from)
    }

    /// Tenants with due, claimable work right now ([[WEIR-T-0091]]) — the `Fleet` runs a worker per
    /// one. A tenant with no pending work simply isn't returned (so it gets no runner — natural reaping).
    pub fn active_tenants(&self) -> Result<Vec<String>, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let now = now_ms();
        work_units::table
            .filter(
                work_units::state
                    .eq("pending")
                    .and(work_units::next_attempt_at.le(now)),
            )
            .select(work_units::tenant_id)
            .distinct()
            .load::<String>(&mut conn)
            .map_err(ExecutorError::from)
    }

    /// Count of **due, claimable** work for a tenant ([[WEIR-T-0104]]) — the autoscaler's signal.
    pub fn pending_depth(&self, tenant: &str) -> Result<i64, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let now = now_ms();
        work_units::table
            .filter(
                work_units::tenant_id
                    .eq(tenant)
                    .and(work_units::state.eq("pending"))
                    .and(work_units::next_attempt_at.le(now)),
            )
            .count()
            .get_result(&mut conn)
            .map_err(ExecutorError::from)
    }

    /// Acquire or renew a named leader lease ([[WEIR-T-0104]]) — atomic, store-based (portable, no
    /// k8s dependency). Returns `true` if we hold `name` for `ttl` from now; take it if it's expired
    /// or already ours. A concurrent contender that loses the insert simply isn't leader.
    pub fn try_acquire_lease(
        &self,
        name: &str,
        owner: &str,
        ttl: Duration,
    ) -> Result<bool, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let now = now_ms();
        let expires = now + ttl.as_millis() as i64;
        // Take the lease if it's expired or ours (state-guarded UPDATE, like `claim`).
        let took = diesel::update(
            leader_leases::table.filter(
                leader_leases::name.eq(name).and(
                    leader_leases::expires_at
                        .le(now)
                        .or(leader_leases::owner.eq(owner)),
                ),
            ),
        )
        .set((
            leader_leases::owner.eq(owner),
            leader_leases::expires_at.eq(expires),
        ))
        .execute(&mut conn)
        .map_err(ExecutorError::from)?;
        if took == 1 {
            return Ok(true);
        }
        // No matching row: either the lease doesn't exist yet (our insert wins → leader) or it's held
        // by a live other owner (insert conflicts → not leader).
        Ok(diesel::insert_into(leader_leases::table)
            .values((
                leader_leases::name.eq(name),
                leader_leases::owner.eq(owner),
                leader_leases::expires_at.eq(expires),
            ))
            .execute(&mut conn)
            .is_ok())
    }

    /// Load the [`WorkSpec`] for a leased unit (executor-side lazy load).
    pub fn load(&self, id: i64) -> Result<Option<WorkSpec>, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        store::load_spec(&mut conn, id)?
            .map(|r| r.into_work_spec())
            .transpose()
    }

    /// Update an in-flight unit's progress counters (per committed checkpoint) so
    /// the live feed shows a run climbing. Only touches a still-`leased` unit, so
    /// it never clobbers a terminal state ([[WEIR-T-0037]]).
    pub fn update_progress(
        &self,
        id: i64,
        rows_written: u64,
        dead_lettered: u64,
    ) -> Result<(), ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        diesel::update(
            work_units::table.filter(work_units::id.eq(id).and(work_units::state.eq("leased"))),
        )
        .set((
            work_units::rows_written.eq(rows_written as i64),
            work_units::dead_lettered.eq(dead_lettered as i64),
        ))
        .execute(&mut conn)?;
        Ok(())
    }

    /// Owner-guarded terminal transition ([[WEIR-T-0190]]): only the worker that
    /// still HOLDS the unit (matching `lease_owner`, state in-flight) may finish
    /// it. A lease-expired worker finishing late — after the unit was reclaimed,
    /// re-claimed elsewhere, or cancelled — matches nothing: its result is void
    /// (warn, `Ok`), never a clobber of the new holder's state.
    pub fn mark_done(
        &self,
        id: i64,
        owner: &str,
        rows_written: u64,
        dead_lettered: u64,
    ) -> Result<(), ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let n = diesel::update(
            work_units::table.filter(
                work_units::id
                    .eq(id)
                    .and(work_units::lease_owner.eq(owner))
                    .and(work_units::state.eq_any(["leased", "running"])),
            ),
        )
        .set((
            work_units::state.eq("done"),
            work_units::rows_written.eq(rows_written as i64),
            work_units::dead_lettered.eq(dead_lettered as i64),
            work_units::finished_at.eq(now_ms()),
        ))
        .execute(&mut conn)?;
        if n == 0 {
            tracing::warn!(target: "weir_orchestrator", unit = id, owner, "stale mark_done ignored (unit no longer held by this owner)");
        }
        Ok(())
    }

    /// Owner-guarded — see [`Relay::mark_done`].
    pub fn mark_failed(&self, id: i64, owner: &str, error: &str) -> Result<(), ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let n = diesel::update(
            work_units::table.filter(
                work_units::id
                    .eq(id)
                    .and(work_units::lease_owner.eq(owner))
                    .and(work_units::state.eq_any(["leased", "running"])),
            ),
        )
        .set((
            work_units::state.eq("failed"),
            work_units::error.eq(error),
            work_units::finished_at.eq(now_ms()),
        ))
        .execute(&mut conn)?;
        if n == 0 {
            tracing::warn!(target: "weir_orchestrator", unit = id, owner, "stale mark_failed ignored (unit no longer held by this owner)");
        }
        Ok(())
    }

    /// Return a unit to `Pending`, claimable at `next_attempt_at` (epoch ms).
    /// Owner-guarded like [`Relay::mark_done`] — a stale requeue must not
    /// resurrect a unit that was cancelled or re-claimed elsewhere.
    pub fn requeue(&self, id: i64, owner: &str, next_attempt_at: i64) -> Result<(), ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let n = diesel::update(
            work_units::table.filter(
                work_units::id
                    .eq(id)
                    .and(work_units::lease_owner.eq(owner))
                    .and(work_units::state.eq_any(["leased", "running"])),
            ),
        )
        .set((
            work_units::state.eq("pending"),
            work_units::lease_owner.eq(None::<String>),
            work_units::lease_expires_at.eq(None::<i64>),
            work_units::next_attempt_at.eq(next_attempt_at),
        ))
        .execute(&mut conn)?;
        if n == 0 {
            tracing::warn!(target: "weir_orchestrator", unit = id, owner, "stale requeue ignored (unit no longer held by this owner)");
        }
        Ok(())
    }

    /// Return a **claimed-but-not-run** unit to `Pending` without penalty — undo the claim's
    /// `attempt` increment and defer the next claim by `defer`. Used by the resident
    /// claim-headroom gate ([[WEIR-I-0035]] F1.4): a saturated runner steps aside so a runner
    /// with headroom (or a freshly scaled-out instance) picks the resident unit up. Distinct from
    /// [`Relay::requeue`], which is a *failure* restart (keeps the attempt, drives the backoff).
    pub fn release(&self, id: i64, defer: Duration) -> Result<(), ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        diesel::update(
            work_units::table.filter(work_units::id.eq(id).and(work_units::state.eq("leased"))),
        )
        .set((
            work_units::state.eq("pending"),
            work_units::lease_owner.eq(None::<String>),
            work_units::lease_expires_at.eq(None::<i64>),
            work_units::next_attempt_at.eq(now_ms() + defer.as_millis() as i64),
            work_units::attempt.eq(work_units::attempt - 1),
        ))
        .execute(&mut conn)?;
        Ok(())
    }

    /// Extend a still-owned lease (heartbeat). Returns `true` if the lease was
    /// refreshed; `false` (no-op) if the unit is no longer `leased` by `owner`
    /// — i.e. the lease was lost or the unit already reached a terminal state.
    pub fn heartbeat(&self, id: i64, owner: &str, lease: Duration) -> Result<bool, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let n = diesel::update(
            work_units::table.filter(
                work_units::id
                    .eq(id)
                    .and(work_units::state.eq("leased"))
                    .and(work_units::lease_owner.eq(owner)),
            ),
        )
        .set(work_units::lease_expires_at.eq(now_ms() + lease.as_millis() as i64))
        .execute(&mut conn)?;
        Ok(n == 1)
    }

    /// Register a live resident run's stop token ([[WEIR-T-0170]]). Returns the
    /// registration's generation — pass it back to `deregister_resident_stop`.
    /// Overwriting an earlier registration for the same unit drops (= cancels) the
    /// stale run's handle, which is exactly right for a supervised re-claim.
    pub fn register_resident_stop(&self, work_unit_id: i64, handle: StopHandle) -> u64 {
        let generation = self
            .stop_generation
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.resident_stops
            .lock()
            .unwrap()
            .insert(work_unit_id, (generation, handle));
        generation
    }

    /// Deregister a resident's stop token (its run ended). Only the matching
    /// registration is removed: a stale run returning AFTER its unit was re-claimed
    /// must not drop — and thereby drop-cancel — the replacement run's handle.
    /// Dropping the matching handle here is safe: drop-to-cancel only matters while
    /// the run is still draining.
    pub fn deregister_resident_stop(&self, work_unit_id: i64, generation: u64) {
        let mut stops = self.resident_stops.lock().unwrap();
        if stops
            .get(&work_unit_id)
            .is_some_and(|(g, _)| *g == generation)
        {
            stops.remove(&work_unit_id);
        }
    }

    /// Fire one live resident's stop token, if any (manual stop / lease-loss).
    pub fn stop_resident(&self, work_unit_id: i64) {
        if let Some((_, h)) = self.resident_stops.lock().unwrap().remove(&work_unit_id) {
            h.stop();
        }
    }

    /// Fire EVERY live resident's stop token ([[WEIR-T-0170]] daemon shutdown): their
    /// blocking runs end at the next poll boundary instead of hanging runtime drop.
    /// Returns how many were live.
    pub fn stop_all_residents(&self) -> usize {
        let handles: Vec<StopHandle> = self
            .resident_stops
            .lock()
            .unwrap()
            .drain()
            .map(|(_, (_, h))| h)
            .collect();
        let n = handles.len();
        for h in handles {
            h.stop();
        }
        n
    }

    /// Retention pass ([[WEIR-T-0188]]): prune **terminal** (`done`/`failed`)
    /// `work_units`, plus `run_logs` and `dead_letters`, so a long-lived
    /// deployment never eats its store. Two caps per table, both per tenant:
    /// an age cap (rows older than the cutoff go) then a count cap (newest N
    /// kept, oldest deleted). In-flight units (`pending`/`leased`/`running`)
    /// are NEVER touched — terminal units are recognized by state and always
    /// carry `finished_at`. Dead letters are purged, not replayed. Returns the
    /// total rows deleted.
    pub fn prune_retention(&self, cfg: &RetentionConfig) -> Result<u64, ExecutorError> {
        const TERMINAL: [&str; 2] = ["done", "failed"];
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let mut deleted: u64 = 0;

        if let Some(age_ms) = cfg.max_age_ms {
            let cutoff = now_ms() - age_ms;
            deleted += diesel::delete(
                work_units::table.filter(
                    work_units::state
                        .eq_any(TERMINAL)
                        .and(work_units::finished_at.lt(cutoff)),
                ),
            )
            .execute(&mut conn)? as u64;
            deleted += diesel::delete(run_logs::table.filter(run_logs::ts.lt(cutoff)))
                .execute(&mut conn)? as u64;
            deleted += diesel::delete(dead_letters::table.filter(dead_letters::ts.lt(cutoff)))
                .execute(&mut conn)? as u64;
        }

        if let Some(max_rows) = cfg.max_rows {
            // Per-tenant caps: one noisy tenant must not evict another's history.
            let tenants: Vec<String> = work_units::table
                .select(work_units::tenant_id)
                .distinct()
                .load(&mut conn)?;
            for t in &tenants {
                // The (max_rows+1)-th newest terminal unit marks the eviction
                // threshold; everything at or below it goes. Ids are monotonic
                // (timestamp-ordered), so `ORDER BY id DESC` is newest-first.
                let threshold: Option<i64> = work_units::table
                    .filter(
                        work_units::tenant_id
                            .eq(t)
                            .and(work_units::state.eq_any(TERMINAL)),
                    )
                    .select(work_units::id)
                    .order(work_units::id.desc())
                    .offset(max_rows)
                    .limit(1)
                    .first(&mut conn)
                    .optional()?;
                if let Some(th) = threshold {
                    deleted += diesel::delete(
                        work_units::table.filter(
                            work_units::tenant_id
                                .eq(t)
                                .and(work_units::state.eq_any(TERMINAL))
                                .and(work_units::id.le(th)),
                        ),
                    )
                    .execute(&mut conn)? as u64;
                }
            }
            // run_logs / dead_letters cap on the `ts` recency order. The
            // (max_rows+1)-th newest ts marks the threshold; it and everything
            // older goes. With ts ties at the boundary this may evict a tying
            // row too — the caps are bounds, not exact counts.
            let log_tenants: Vec<String> = run_logs::table
                .select(run_logs::tenant_id)
                .distinct()
                .load(&mut conn)?;
            for t in &log_tenants {
                let threshold: Option<i64> = run_logs::table
                    .filter(run_logs::tenant_id.eq(t))
                    .select(run_logs::ts)
                    .order(run_logs::ts.desc())
                    .offset(max_rows)
                    .limit(1)
                    .first(&mut conn)
                    .optional()?;
                if let Some(th) = threshold {
                    deleted += diesel::delete(
                        run_logs::table.filter(run_logs::tenant_id.eq(t).and(run_logs::ts.le(th))),
                    )
                    .execute(&mut conn)? as u64;
                }
            }
            let dl_tenants: Vec<String> = dead_letters::table
                .select(dead_letters::tenant_id)
                .distinct()
                .load(&mut conn)?;
            for t in &dl_tenants {
                let threshold: Option<i64> = dead_letters::table
                    .filter(dead_letters::tenant_id.eq(t))
                    .select(dead_letters::ts)
                    .order(dead_letters::ts.desc())
                    .offset(max_rows)
                    .limit(1)
                    .first(&mut conn)
                    .optional()?;
                if let Some(th) = threshold {
                    deleted += diesel::delete(
                        dead_letters::table
                            .filter(dead_letters::tenant_id.eq(t).and(dead_letters::ts.le(th))),
                    )
                    .execute(&mut conn)? as u64;
                }
            }
        }

        if deleted > 0 {
            tracing::info!(target: "weir_orchestrator", deleted, "retention pruned rows");
        }
        Ok(deleted)
    }

    /// Return expired leases (dead workers) to `Pending`. Returns the count.
    pub fn reclaim_expired(&self) -> Result<usize, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let n = diesel::update(
            work_units::table.filter(
                work_units::state
                    .eq("leased")
                    .and(work_units::lease_expires_at.le(now_ms())),
            ),
        )
        .set((
            work_units::state.eq("pending"),
            work_units::lease_owner.eq(None::<String>),
            work_units::lease_expires_at.eq(None::<i64>),
        ))
        .execute(&mut conn)?;
        if n > 0 {
            tracing::debug!(target: "weir_orchestrator", reclaimed = n, "reclaimed expired leases → pending");
        }
        Ok(n)
    }

    /// Inspect a unit's state (`pending`/`leased`/`done`/`failed`).
    pub fn state(&self, id: i64) -> Result<Option<String>, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        Ok(work_units::table
            .filter(work_units::id.eq(id))
            .select(work_units::state)
            .first::<String>(&mut conn)
            .optional()?)
    }

    /// The most recent work units across all connections, newest first — powers
    /// the UI's live run feed and per-card last-run status.
    pub fn recent(&self, limit: i64) -> Result<Vec<RunRow>, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        Ok(store::run_feed(&mut conn, limit)?
            .into_iter()
            .map(|r| r.into_run_row())
            .collect())
    }

    /// Work-unit history for a connection (id/state/attempt), oldest first.
    pub fn history(&self, connection: &str) -> Result<Vec<WorkUnitStatus>, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let rows: Vec<(i64, String, i64, Option<String>)> = work_units::table
            .filter(work_units::connection.eq(connection))
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

    /// Whether the connection has an in-flight (`pending`/`leased`) unit — the
    /// scheduler's no-double-start guard. NOTE: name-only — same-named connections in
    /// different tenants alias each other here; prefer [`has_active_in`](Self::has_active_in)
    /// wherever the tenant is known ([[WEIR-T-0171]]).
    pub fn has_active(&self, connection: &str) -> Result<bool, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let n: i64 = work_units::table
            .filter(
                work_units::connection
                    .eq(connection)
                    .and(work_units::state.eq_any(["pending", "leased"])),
            )
            .count()
            .get_result(&mut conn)?;
        Ok(n > 0)
    }

    /// Tenant-scoped no-double-start guard ([[WEIR-T-0171]] / [[WEIR-A-0036]] composite
    /// identity): same-named connections in different tenants schedule independently.
    pub fn has_active_in(&self, tenant: &str, connection: &str) -> Result<bool, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let n: i64 = work_units::table
            .filter(
                work_units::tenant_id
                    .eq(tenant)
                    .and(work_units::connection.eq(connection))
                    .and(work_units::state.eq_any(["pending", "leased"])),
            )
            .count()
            .get_result(&mut conn)?;
        Ok(n > 0)
    }

    /// Durable **stop** for a resident source ([[WEIR-I-0035]] F1.5): mark every active
    /// (`pending`/`leased`) unit of `connection` as `done` so the supervised restart loop
    /// ([[WEIR-T-0139]]) stops re-claiming/requeuing it. Returns the number of units stopped.
    ///
    /// NOTE: this ends the *restart* loop and prevents re-claim. Interrupting a resident run that
    /// is *actively* draining in a separate runner process (firing the in-process `StopHandle`
    /// mid-stream) is same-process only today via [`InProcessExecutor::stop`]; cross-process
    /// mid-stream cancellation is a follow-on (the run stops at its next lease expiry / checkpoint).
    pub fn cancel(&self, connection: &str) -> Result<u64, ExecutorError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let n = diesel::update(
            work_units::table.filter(
                work_units::connection
                    .eq(connection)
                    .and(work_units::state.eq_any(["pending", "leased"])),
            ),
        )
        .set((
            work_units::state.eq("done"),
            work_units::finished_at.eq(now_ms()),
        ))
        .execute(&mut conn)?;
        Ok(n as u64)
    }
}

fn json<T: Serialize>(v: &T) -> Result<String, ExecutorError> {
    serde_json::to_string(v).map_err(|e| ExecutorError::Store(e.to_string()))
}
fn from_json<T: for<'de> Deserialize<'de>>(s: &str) -> Result<T, ExecutorError> {
    serde_json::from_str(s).map_err(|e| ExecutorError::Store(e.to_string()))
}

// ---- InProcessExecutor: bridges to the sync engine via spawn_blocking ----

/// Runs work units in-process: resolves connectors, runs the sync [`Engine`] on
/// the blocking pool, and checkpoints. The default [`WorkExecutor`].
pub struct InProcessExecutor {
    store: Arc<Store>,
    relay: Relay,
    // Resident stop handles live on the shared `Relay` ([[WEIR-T-0170]]), not here, so
    // daemon shutdown can fire every live resident across every per-tenant executor with
    // one `stop_all_residents` call. NOTE the registry is still transitively reachable
    // from the blocking run (via its `relay` clone), so drop-to-cancel alone can never
    // end a live resident — shutdown paths MUST call `stop_all_residents` (serve/runner
    // do; tests driving real residents must too) or runtime teardown waits on the
    // blocking pool forever (the wedge `resident_real` reproduces).
}

impl InProcessExecutor {
    pub fn new(store: Arc<Store>, relay: Relay) -> Self {
        Self { store, relay }
    }

    /// Stop one in-flight resident run (manual stop, [[WEIR-I-0035]] F1.3): fire its
    /// [`StopHandle`] so the resident loop shuts down at its next poll boundary. No-op if
    /// the unit isn't a live resident run. The registry lives on the shared [`Relay`]
    /// ([[WEIR-T-0170]]) so a daemon shutdown reaches every executor's residents.
    pub fn stop(&self, work_unit_id: i64) {
        self.relay.stop_resident(work_unit_id);
    }

    /// Stop **all** in-flight resident runs (worker/fleet shutdown, [[WEIR-I-0035]] F1.3).
    pub fn stop_all(&self) {
        self.relay.stop_all_residents();
    }
}

#[async_trait]
impl WorkExecutor for InProcessExecutor {
    async fn execute(&self, event: WorkReadyEvent) -> Result<ExecutionResult, ExecutorError> {
        let store = Arc::clone(&self.store);
        let relay = self.relay.clone();
        let work_unit_id = event.work_unit_id;
        tokio::task::spawn_blocking(move || -> Result<ExecutionResult, ExecutorError> {
            let Some(spec) = relay.load(work_unit_id)? else {
                return Ok(ExecutionResult::Skipped);
            };
            // Resolve each connector with its **own** config ([[WEIR-I-0029]]).
            let source = spec.source.resolve(&spec.source_config)?;
            let dest = spec.dest.resolve(&spec.dest_config)?;
            let opts = SyncOptions {
                state_key: spec.state_key.clone(),
                seed_cursor: spec.seed_cursor.clone(),
                partition: spec.partition.clone(),
            };
            // Update the in-flight unit's counters per committed checkpoint → live feed.
            let mut on_progress = |p: SyncProgress| {
                let _ = relay.update_progress(work_unit_id, p.rows_written, p.dead_lettered);
            };
            let engine = Engine::new(&store);
            // Resident ([[WEIR-I-0035]] F1.3) holds the instance open and drains
            // indefinitely; register a StopHandle so `stop`/`stop_all` can cancel it.
            // `run_resident` returns `Ok` only on a clean stop, `Err` on exit/error
            // (the worker then requeues + resumes from the last checkpoint). Run-once is
            // unchanged.
            let outcome = if spec.execution_mode.is_resident() {
                let (handle, token) = stop_channel();
                // Registered on the shared relay ([[WEIR-T-0170]]) — NOT captured by this
                // closure — so shutdown's `stop_all_residents` reaches it and the blocking
                // task never keeps its own StopHandle alive.
                let stop_generation = relay.register_resident_stop(work_unit_id, handle);
                let out = engine.run_resident(
                    &spec.connection,
                    &spec.stream,
                    &source,
                    &dest,
                    &opts,
                    &mut on_progress,
                    token,
                    spec.execution_mode
                        .cadence_ms()
                        .map(std::time::Duration::from_millis),
                );
                // Drops the handle if stop() wasn't called; generation-guarded so a stale
                // run can't drop a re-claimed replacement's handle.
                relay.deregister_resident_stop(work_unit_id, stop_generation);
                out
            } else {
                engine.sync_with(
                    &spec.connection,
                    &spec.stream,
                    &source,
                    &dest,
                    &opts,
                    &mut on_progress,
                )
            };
            match outcome {
                Ok(out) => Ok(ExecutionResult::Completed {
                    rows_written: out.rows_written,
                    dead_lettered: out.dead_lettered,
                }),
                Err(EngineError::Connector(e)) => Ok(ExecutionResult::Failed {
                    kind: e.kind,
                    message: e.message,
                }),
                Err(e) => Ok(ExecutionResult::Failed {
                    kind: ErrorKind::Fatal,
                    message: e.to_string(),
                }),
            }
        })
        .await
        .map_err(|e| ExecutorError::Join(e.to_string()))?
    }

    fn stop(&self, work_unit_id: i64) {
        InProcessExecutor::stop(self, work_unit_id);
    }

    fn name(&self) -> &str {
        "in-process"
    }
}

// ---- Worker: the async claim → execute → apply loop ----

/// Drives the relay: claims work, dispatches to a [`WorkExecutor`], and applies
/// the result as a state transition (done / requeue-with-backoff / failed).
/// Emit run-completion metrics ([[WEIR-T-0099]]) to the global recorder — labelled by connection +
/// state; per-tenant labels are opt-in via `WEIR_METRICS_TENANT_LABELS` (aggregate by default, so an
/// unauthenticated `/metrics` scraper sees no tenant names).
fn emit_run_metrics(spec: Option<&WorkSpec>, result: &ExecutionResult, dur_secs: f64) {
    let (state, rows, dead) = match result {
        ExecutionResult::Completed {
            rows_written,
            dead_lettered,
        } => ("done", *rows_written, *dead_lettered),
        ExecutionResult::Failed { .. } => ("failed", 0, 0),
        ExecutionResult::Skipped => return, // vanished between claim + load — not a real run outcome
    };
    let conn = spec
        .map(|s| s.connection.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let tenant = spec
        .map(|s| s.tenant.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let with_tenant = std::env::var("WEIR_METRICS_TENANT_LABELS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if with_tenant {
        metrics::counter!("weir_runs_total", "connection" => conn.clone(), "state" => state, "tenant" => tenant.clone()).increment(1);
        metrics::counter!("weir_rows_written_total", "connection" => conn.clone(), "tenant" => tenant.clone()).increment(rows);
        metrics::histogram!("weir_run_duration_seconds", "connection" => conn.clone(), "tenant" => tenant.clone()).record(dur_secs);
        if dead > 0 {
            metrics::counter!("weir_dead_letters_total", "connection" => conn, "tenant" => tenant)
                .increment(dead);
        }
    } else {
        metrics::counter!("weir_runs_total", "connection" => conn.clone(), "state" => state)
            .increment(1);
        metrics::counter!("weir_rows_written_total", "connection" => conn.clone()).increment(rows);
        metrics::histogram!("weir_run_duration_seconds", "connection" => conn.clone())
            .record(dur_secs);
        if dead > 0 {
            metrics::counter!("weir_dead_letters_total", "connection" => conn).increment(dead);
        }
    }
}

pub struct Worker<E: WorkExecutor> {
    relay: Relay,
    executor: Arc<E>,
    config: WorkerConfig,
    lineage: Lineage,
    /// Measured usage source for the resident claim-headroom gate ([[WEIR-I-0035]] F1.4).
    usage: Arc<dyn UsageProbe>,
    /// High-water fraction `[0.0,1.0]` above which this worker stops claiming **new resident**
    /// units. `None` = gate off (default). Run-once claiming is never gated.
    high_water: Option<f64>,
}

/// True if `e` is a SQLite "database is locked" — write contention under
/// concurrency. cloacina avoids it with `busy_timeout`; since diesel-dualdb's pool
/// doesn't expose per-connection pragmas, we retry the op (the lock is transient).
fn is_db_locked(e: &ExecutorError) -> bool {
    e.to_string().contains("locked")
}

/// Run a sync relay write, retrying briefly on "database is locked" (bounded
/// exponential backoff). Runs inside `spawn_blocking`, so `thread::sleep` is fine.
fn retry_locked<T>(mut f: impl FnMut() -> Result<T, ExecutorError>) -> Result<T, ExecutorError> {
    let start = std::time::Instant::now();
    let mut delay_ms = 5u64;
    loop {
        match f() {
            Err(e) if is_db_locked(&e) && start.elapsed() < Duration::from_secs(30) => {
                std::thread::sleep(Duration::from_millis(delay_ms));
                delay_ms = (delay_ms * 2).min(200);
            }
            other => return other,
        }
    }
}

impl<E: WorkExecutor + 'static> Worker<E> {
    pub fn new(relay: Relay, executor: E, config: WorkerConfig) -> Self {
        Self {
            relay,
            executor: Arc::new(executor),
            config,
            lineage: Lineage::from_env(),
            usage: Arc::new(NullUsageProbe),
            high_water: None,
        }
    }

    /// Enable the resident claim-headroom gate ([[WEIR-I-0035]] F1.4): while `usage` reports a
    /// dominant fraction ≥ `high_water`, this worker steps aside from starting **new resident**
    /// units (run-once is unaffected) so load spreads to runners with headroom — and the unclaimed
    /// unit keeps the queue non-empty, which (with the saturation-aware autoscaler) pulls in
    /// another instance.
    pub fn with_headroom(mut self, usage: Arc<dyn UsageProbe>, high_water: f64) -> Self {
        self.usage = usage;
        self.high_water = Some(high_water);
        self
    }

    /// One iteration: claim a due unit (if any), execute it, apply the result.
    /// Returns `true` if a unit was processed.
    pub async fn tick(&self) -> Result<bool, ExecutorError> {
        let relay = self.relay.clone();
        let owner = self.config.owner.clone();
        let tenant = self.config.tenant.clone();
        let lease = self.config.lease;
        let claimed = tokio::task::spawn_blocking(move || {
            retry_locked(|| relay.claim(&owner, &tenant, lease))
        })
        .await
        .map_err(|e| ExecutorError::Join(e.to_string()))??;
        let Some(event) = claimed else {
            return Ok(false);
        };

        // Load the spec once (best-effort) for metric labels ([[WEIR-T-0099]]) + OpenLineage
        // ([[WEIR-T-0100]]); emit the lineage START. Never blocks the run.
        let run_spec = {
            let r = self.relay.clone();
            let id = event.work_unit_id;
            tokio::task::spawn_blocking(move || r.load(id).ok().flatten())
                .await
                .ok()
                .flatten()
        };
        // Resident claim-headroom gate ([[WEIR-I-0035]] F1.4): if this runner is at/over its
        // high-water mark, step aside from starting a NEW resident unit — release it back to
        // Pending (deferred, no penalty) so a runner with headroom / a scaled-out instance takes
        // it. Run-once is never gated. Also surfaces the usage signal via metrics ([[WEIR-A-0022]]).
        if let Some(hw) = self.high_water {
            let usage_max = self.usage.sample().max_fraction();
            metrics::gauge!("weir_runner_usage_fraction", "tenant" => self.config.tenant.clone())
                .set(usage_max);
            let is_resident = run_spec
                .as_ref()
                .map(|s| s.execution_mode.is_resident())
                .unwrap_or(false);
            if gate_new_resident(Some(hw), usage_max, is_resident) {
                let relay = self.relay.clone();
                let id = event.work_unit_id;
                let defer = self.config.base_delay.max(Duration::from_secs(2));
                tokio::task::spawn_blocking(move || retry_locked(|| relay.release(id, defer)))
                    .await
                    .map_err(|e| ExecutorError::Join(e.to_string()))??;
                metrics::counter!("weir_resident_gated", "tenant" => self.config.tenant.clone())
                    .increment(1);
                return Ok(false);
            }
        }

        if let Some(s) = run_spec.as_ref().filter(|_| self.lineage.enabled()) {
            self.lineage.emit_start(s, event.work_unit_id);
        }
        let is_resident = run_spec
            .as_ref()
            .map(|s| s.execution_mode.is_resident())
            .unwrap_or(false);
        let unit_id = event.work_unit_id;
        let conn_name = run_spec
            .as_ref()
            .map(|s| s.connection.clone())
            .unwrap_or_default();
        tracing::debug!(target: "weir_orchestrator", unit = unit_id, connection = %conn_name, resident = is_resident, attempt = event.attempt, "claimed unit");

        // Owned clones so the run executes either inline (run-once) or in a detached task (resident).
        let relay = self.relay.clone();
        let executor = Arc::clone(&self.executor);
        let lineage = self.lineage.clone();
        let config = self.config.clone();

        if is_resident {
            // [[WEIR-T-0146]]: a resident run **never completes**, so awaiting it inline here would
            // park `run_until_idle` forever (its tick future never resolves) and strand ALL other
            // work in `pending`. Detach it as an independent supervised task (it owns its own
            // heartbeat + requeue-on-exit) and report the tick *processed* so the drain loop keeps
            // going and returns. The unit stays leased + heartbeated, so it is never re-claimed /
            // double-run; a manual stop (F1.3 `StopHandle`) or lease expiry still applies.
            tracing::info!(target: "weir_orchestrator", unit = unit_id, connection = %conn_name, "resident run detached to independent task");
            let log_conn = conn_name.clone();
            tokio::spawn(async move {
                if let Err(e) =
                    Self::run_and_apply(relay, executor, lineage, config, event, run_spec).await
                {
                    tracing::error!(target: "weir_orchestrator", unit = unit_id, connection = %log_conn, error = %e, "resident run task exited with error");
                }
            });
            return Ok(true);
        }

        Self::run_and_apply(relay, executor, lineage, config, event, run_spec).await?;
        Ok(true)
    }

    /// Execute one claimed unit — heartbeat the lease, run it, and apply the terminal transition
    /// (done / requeue-with-backoff / failed). Run-once callers `await` this inline in
    /// [`run_until_idle`]; a **resident** unit runs it in a detached task ([[WEIR-T-0146]]) so its
    /// never-completing run does not wedge the drain loop.
    async fn run_and_apply(
        relay: Relay,
        executor: Arc<E>,
        lineage: Lineage,
        config: WorkerConfig,
        event: WorkReadyEvent,
        run_spec: Option<WorkSpec>,
    ) -> Result<(), ExecutorError> {
        // Run span ([[WEIR-T-0098]]) — connection/run/tenant on everything the execute emits.
        let run_span = run_spec.as_ref().map(|s| {
            tracing::info_span!(
                "run",
                run = event.work_unit_id,
                connection = %s.connection,
                tenant = %s.tenant
            )
        });
        let run_started = std::time::Instant::now();

        // Heartbeat the lease while the unit executes, so a run longer than the lease isn't
        // reclaimed mid-run by `reclaim_expired` and double-run. For a resident run this beats for
        // the run's whole life, keeping the unit leased.
        let heartbeat = if config.heartbeat.is_zero() {
            None
        } else {
            let hb_relay = relay.clone();
            let hb_owner = config.owner.clone();
            let hb_lease = config.lease;
            let hb_every = config.heartbeat;
            let hb_id = event.work_unit_id;
            let hb_exec = Arc::clone(&executor);
            Some(tokio::spawn(async move {
                loop {
                    tokio::time::sleep(hb_every).await;
                    let r = hb_relay.clone();
                    let owner = hb_owner.clone();
                    let alive =
                        tokio::task::spawn_blocking(move || r.heartbeat(hb_id, &owner, hb_lease))
                            .await;
                    // Stop on join error, DB error, or a lost lease (false). A lost lease means the
                    // unit was cancelled/reclaimed — possibly by a control-plane `stop` on ANOTHER
                    // process — so fire the run's stop token to halt the ACTUAL resident run, not
                    // just the DB state ([[WEIR-T-0149]] cross-process mid-stream stop). Within one
                    // heartbeat interval the run ends cleanly (unit already terminal).
                    if !matches!(alive, Ok(Ok(true))) {
                        hb_exec.stop(hb_id);
                        break;
                    }
                }
            }))
        };

        let exec = executor.execute(event.clone());
        let result = match run_span {
            Some(sp) => {
                use tracing::Instrument;
                exec.instrument(sp).await
            }
            None => exec.await,
        };
        if let Some(hb) = heartbeat {
            hb.abort();
        }
        // Per-run isolation ([[WEIR-T-0190]], the [[WEIR-T-0066]] wedge): an executor
        // error — including a JoinError from a panicking run — must fail only THIS
        // run. Propagating it aborted the whole drain pass and stranded the unit
        // leased-to-a-ghost until lease expiry. It becomes a transient failure and
        // flows through the normal terminal transition below.
        let result = result.unwrap_or_else(|e| ExecutionResult::Failed {
            kind: ErrorKind::Transient,
            message: format!("executor error: {e}"),
        });

        // Metrics ([[WEIR-T-0099]]) + OpenLineage COMPLETE/FAIL ([[WEIR-T-0100]]).
        emit_run_metrics(
            run_spec.as_ref(),
            &result,
            run_started.elapsed().as_secs_f64(),
        );
        if let Some(s) = &run_spec {
            match &result {
                ExecutionResult::Completed { rows_written, .. } => {
                    lineage.emit_complete(s, event.work_unit_id, *rows_written as i64)
                }
                _ => lineage.emit_fail(s, event.work_unit_id),
            }
        }

        let max_attempts = config.max_attempts;
        let base_delay = config.base_delay.as_millis() as i64;
        // Resident sources ([[WEIR-I-0035]] F1.3) restart on their own backoff —
        // `Some(base_ms)` iff this unit is resident. A clean stop reports `Completed` (→ done);
        // any exit/error requeues so it resumes from the last checkpoint (never silently dies).
        let resident_backoff_ms = run_spec
            .as_ref()
            .and_then(|s| s.execution_mode.restart_backoff_ms());
        let id = event.work_unit_id;
        let attempt = event.attempt;
        let owner = config.owner.clone();
        let run_ms = run_started.elapsed().as_millis() as u64;
        tokio::task::spawn_blocking(move || {
            retry_locked(|| -> Result<(), ExecutorError> {
                match &result {
                    ExecutionResult::Completed {
                        rows_written,
                        dead_lettered,
                    } => {
                        tracing::debug!(target: "weir_orchestrator", unit = id, rows = *rows_written, dead = *dead_lettered, "run done");
                        relay.mark_done(id, &owner, *rows_written, *dead_lettered)
                    }
                    ExecutionResult::Skipped => relay.requeue(id, &owner, now_ms()),
                    ExecutionResult::Failed { kind, message } => {
                        if let Some(base_ms) = resident_backoff_ms {
                            // Supervised restart: requeue with capped, decaying
                            // backoff ([[WEIR-T-0190]]) regardless of `max_attempts`.
                            let delay = resident_restart_delay(base_ms, attempt, run_ms);
                            tracing::info!(target: "weir_orchestrator", unit = id, attempt, delay_ms = delay, error = %message, "resident run exited → requeue with backoff");
                            relay.requeue(id, &owner, now_ms() + delay)
                        } else if matches!(kind, ErrorKind::Transient)
                            && (attempt as u32) < max_attempts
                        {
                            let shift = (attempt - 1).clamp(0, 16) as u32;
                            let delay = base_delay * (1i64 << shift);
                            tracing::warn!(target: "weir_orchestrator", unit = id, attempt, delay_ms = delay, error = %message, "transient failure → requeue with backoff");
                            relay.requeue(id, &owner, now_ms() + delay)
                        } else {
                            tracing::error!(target: "weir_orchestrator", unit = id, attempt, error = %message, "run failed (terminal)");
                            relay.mark_failed(id, &owner, message)
                        }
                    }
                }
            })
        })
        .await
        .map_err(|e| ExecutorError::Join(e.to_string()))??;
        Ok(())
    }

    /// Reclaim expired leases, then drain all currently-due work, running up to
    /// `config.concurrency` units **simultaneously**. Each in-flight `tick`
    /// atomically claims a distinct unit (so concurrent claimants never double-run);
    /// when one reports nothing left to claim, we stop topping up and finish the
    /// rest. `concurrency = 1` is the original serial drain. With a zero `base_delay`
    /// this also drains retries to a terminal state.
    pub async fn run_until_idle(&self) -> Result<(), ExecutorError> {
        use futures::stream::StreamExt;
        let relay = self.relay.clone();
        tokio::task::spawn_blocking(move || retry_locked(|| relay.reclaim_expired()))
            .await
            .map_err(|e| ExecutorError::Join(e.to_string()))??;

        let concurrency = self.config.concurrency.max(1);
        tracing::debug!(target: "weir_orchestrator", tenant = %self.config.tenant, concurrency, "run_until_idle: drain pass begin");
        let mut processed_count: u64 = 0;
        let mut inflight = futures::stream::FuturesUnordered::new();
        for _ in 0..concurrency {
            inflight.push(self.tick());
        }
        // Refill a slot whenever a tick *processed* a unit — there may be more due,
        // including immediately-requeued retries (zero `base_delay`). When a tick
        // finds nothing to claim AND no others are in flight, the queue is drained.
        while let Some(processed) = inflight.next().await {
            if processed? {
                processed_count += 1;
                // Containment ([[WEIR-T-0190]]): a unit perpetually requeuing
                // itself due-now would spin this loop forever — fail loudly.
                if processed_count >= self.config.max_drain_units {
                    return Err(ExecutorError::DrainStuck(processed_count));
                }
                inflight.push(self.tick());
            } else if inflight.is_empty() {
                break;
            }
        }
        tracing::debug!(target: "weir_orchestrator", tenant = %self.config.tenant, processed = processed_count, "run_until_idle: drain pass end");
        Ok(())
    }
}

/// The per-tenant runner fleet ([[WEIR-A-0036]] / [[WEIR-T-0091]]). In-process: each cycle discovers
/// the active tenants and runs a per-tenant [`Worker`] to idle for each — a tenant with no claimable
/// work gets no runner (natural reaping), and no tenant starves another (each gets its own worker).
/// The `make_executor` factory yields a fresh executor per tenant-worker (cheap for `InProcessExecutor`).
/// [[WEIR-I-0021]] swaps the in-process worker for a Kubernetes pod without touching claim/lease.
pub struct Fleet<E, F: Fn() -> E> {
    relay: Relay,
    make_executor: F,
    base: WorkerConfig,
    /// Optional resident claim-headroom ([[WEIR-I-0035]] F1.4): applied to every per-tenant worker
    /// this fleet spawns, so a hot runner stops taking new resident work.
    headroom: Option<(Arc<dyn UsageProbe>, f64)>,
}

impl<E: WorkExecutor + 'static, F: Fn() -> E> Fleet<E, F> {
    pub fn new(relay: Relay, make_executor: F, base: WorkerConfig) -> Self {
        Self {
            relay,
            make_executor,
            base,
            headroom: None,
        }
    }

    /// Wire a measured-usage headroom gate ([[WEIR-I-0035]] F1.4) onto every worker this fleet
    /// spawns: past `high_water` (dominant mem/cpu fraction) the worker steps aside from starting
    /// new **resident** units. Run-once is unaffected.
    pub fn with_headroom(mut self, usage: Arc<dyn UsageProbe>, high_water: f64) -> Self {
        self.headroom = Some((usage, high_water));
        self
    }

    /// Drain **every** tenant to idle: each round runs a per-tenant worker for each active tenant,
    /// repeating until no tenant has claimable work. This is the one-shot (`drain`) + per-poll
    /// (`serve`) drive.
    pub async fn run_until_idle(&self) -> Result<(), ExecutorError> {
        // Sweep expired leases FIRST, independent of due-pending depth ([[WEIR-T-0170]]):
        // a crashed resident with no other due work anywhere was previously never
        // reclaimed — workers only ran (and only they swept) when a tenant had due
        // pending work, so the stranded unit stayed leased-to-a-ghost forever.
        self.relay.reclaim_expired()?;
        // Containment ([[WEIR-T-0190]]): bounded rounds, mirroring the per-worker
        // `max_drain_units` cap — a stuck tenant surfaces instead of spinning.
        for _round in 0..1_000 {
            let tenants = self.relay.active_tenants()?;
            if tenants.is_empty() {
                return Ok(());
            }
            for tenant in tenants {
                let cfg = WorkerConfig {
                    tenant,
                    ..self.base.clone()
                };
                let mut worker = Worker::new(self.relay.clone(), (self.make_executor)(), cfg);
                if let Some((usage, hw)) = &self.headroom {
                    worker = worker.with_headroom(usage.clone(), *hw);
                }
                worker.run_until_idle().await?;
            }
        }
        Err(ExecutorError::DrainStuck(1_000))
    }
}

// ---- Scheduler: clock-driven plan() over the relay ----

/// Time source. Injectable so schedules can be tested against a deterministic
/// clock without sleeping.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
}

/// Wall-clock.
pub struct SystemClock;
impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        now_ms()
    }
}

/// Next fire time (epoch ms) for a cron expression strictly after `after_ms`.
/// Uses the 6/7-field (seconds-first) cron syntax of the `cron` crate, e.g.
/// `"0 */5 * * * *"` = every 5 minutes.
fn next_cron(expr: &str, after_ms: i64) -> Result<i64, ExecutorError> {
    use std::str::FromStr;
    let schedule = cron::Schedule::from_str(expr)
        .map_err(|e| ExecutorError::Store(format!("invalid cron `{expr}`: {e}")))?;
    let after = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(after_ms)
        .ok_or_else(|| ExecutorError::Store(format!("bad timestamp {after_ms}")))?;
    schedule
        .after(&after)
        .next()
        .map(|dt| dt.timestamp_millis())
        .ok_or_else(|| ExecutorError::Store(format!("cron `{expr}` has no next occurrence")))
}

/// Fires runs on a per-connection interval by calling [`Relay::plan`] — so
/// scheduled and manual triggers share the relay's one execution path. v0 is
/// interval-only; cron is a thin follow-on (same loop, different `next_due_at`).
pub struct Scheduler<C: Clock> {
    relay: Relay,
    clock: C,
}

impl<C: Clock> Scheduler<C> {
    /// Create the scheduler and ensure the `schedules` table exists.
    pub fn new(relay: Relay, clock: C) -> Result<Self, ExecutorError> {
        // schedules created by Store::open → weir_schema::migrate ([[WEIR-T-0061]]).
        let scheduler = Self { relay, clock };
        Ok(scheduler)
    }

    /// Register an interval schedule. It first fires on the next `tick` at or
    /// after now.
    pub fn add(
        &self,
        connection: &str,
        spec: &WorkSpec,
        every: Duration,
    ) -> Result<i64, ExecutorError> {
        let mut conn = self
            .relay
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let id = next_id();
        diesel::insert_into(schedules::table)
            .values((
                schedules::id.eq(id),
                schedules::connection.eq(connection),
                schedules::spec.eq(json(spec)?),
                schedules::every_ms.eq(every.as_millis() as i64),
                schedules::next_due_at.eq(self.clock.now_ms()),
            ))
            .execute(&mut conn)?;
        Ok(id)
    }

    /// Register a **cron** schedule (6/7-field, seconds-first — see [`next_cron`]).
    /// It first fires on the next `tick` at or after the cron's next occurrence.
    pub fn add_cron(
        &self,
        connection: &str,
        spec: &WorkSpec,
        cron_expr: &str,
    ) -> Result<i64, ExecutorError> {
        let next_due = next_cron(cron_expr, self.clock.now_ms())?;
        let mut conn = self
            .relay
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        let id = next_id();
        diesel::insert_into(schedules::table)
            .values((
                schedules::id.eq(id),
                schedules::connection.eq(connection),
                schedules::spec.eq(json(spec)?),
                schedules::every_ms.eq(0i64),
                schedules::next_due_at.eq(next_due),
                schedules::cron.eq(cron_expr),
            ))
            .execute(&mut conn)?;
        Ok(id)
    }

    /// Evaluate due schedules: `plan()` a unit for each (skipping any connection
    /// that already has an in-flight unit — no double-start), then advance
    /// `next_due_at`. Returns how many fired.
    pub fn tick(&self) -> Result<usize, ExecutorError> {
        let now = self.clock.now_ms();
        let due = {
            let mut conn = self
                .relay
                .store
                .pool()
                .get()
                .map_err(|e| ExecutorError::Store(e.to_string()))?;
            schedules::table
                .filter(schedules::enabled.eq(1).and(schedules::next_due_at.le(now)))
                .select((
                    schedules::id,
                    schedules::connection,
                    schedules::spec,
                    schedules::every_ms,
                    schedules::cron,
                ))
                .load::<(i64, String, String, i64, Option<String>)>(&mut conn)?
        };

        let mut fired = 0;
        for (id, key, spec_json, every_ms, cron) in due {
            // Per-schedule isolation ([[WEIR-T-0190]]): one poisoned schedule
            // (unparseable spec, bad cron, a failing plan) must not abort the
            // tick for every remaining schedule — log it and keep going.
            let one = || -> Result<bool, ExecutorError> {
                let spec: WorkSpec = from_json(&spec_json)?;
                // Resident sources ([[WEIR-I-0035]] F1.3) are NOT interval/cron-fired: they are
                // enqueued once as a standing registration (via `Relay::plan`) and kept alive by the
                // perpetual lease. Firing here would resurrect a deliberately-stopped source.
                // The guard uses the SPEC's (tenant, connection) — not the registry key, which
                // may be tenant-scoped (`{tenant}/{name}`, [[WEIR-T-0171]]).
                let fire = !spec.execution_mode.is_resident()
                    && !self.relay.has_active_in(&spec.tenant, &spec.connection)?;
                if fire {
                    self.relay.plan(&spec)?;
                }
                let mut conn = self
                    .relay
                    .store
                    .pool()
                    .get()
                    .map_err(|e| ExecutorError::Store(e.to_string()))?;
                let next_due = match cron.as_deref() {
                    Some(expr) => next_cron(expr, now)?,
                    None => now + every_ms,
                };
                diesel::update(schedules::table.filter(schedules::id.eq(id)))
                    .set(schedules::next_due_at.eq(next_due))
                    .execute(&mut conn)?;
                Ok(fire)
            };
            match one() {
                Ok(true) => fired += 1,
                Ok(false) => {}
                Err(e) => {
                    tracing::error!(target: "weir_orchestrator", schedule = id, connection = %key, error = %e, "schedule tick failed; continuing with the rest");
                }
            }
        }
        if fired > 0 {
            tracing::debug!(target: "weir_orchestrator", schedules_fired = fired, "scheduler tick: planned due connections");
        }
        Ok(fired)
    }

    /// All enabled schedules as `(connection, every_ms, cron, spec_json)` — for reconciling
    /// the schedule set against the live connections ([[WEIR-T-0051]]). The stored spec is
    /// included so the reconciler can re-register on ANY config change, not just a cadence
    /// change ([[WEIR-T-0171]] — "fixed the credential, still failing" trap).
    #[allow(clippy::type_complexity)]
    pub fn schedules(&self) -> Result<Vec<(String, i64, Option<String>, String)>, ExecutorError> {
        let mut conn = self
            .relay
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        Ok(schedules::table
            .filter(schedules::enabled.eq(1))
            .select((
                schedules::connection,
                schedules::every_ms,
                schedules::cron,
                schedules::spec,
            ))
            .load::<(String, i64, Option<String>, String)>(&mut conn)?)
    }

    /// Remove a connection's schedule(s). Idempotent (no-op if absent).
    pub fn remove(&self, connection: &str) -> Result<(), ExecutorError> {
        let mut conn = self
            .relay
            .store
            .pool()
            .get()
            .map_err(|e| ExecutorError::Store(e.to_string()))?;
        diesel::delete(schedules::table.filter(schedules::connection.eq(connection)))
            .execute(&mut conn)?;
        Ok(())
    }
}

#[cfg(test)]
mod sharp_edges_tests {
    use super::*;

    /// [[WEIR-T-0190]] item 5: the resident restart delay is exponential in the
    /// attempt while crash-looping, DECAYS to the base after healthy uptime, and
    /// is absolutely capped.
    #[test]
    fn resident_restart_delay_caps_and_decays() {
        // Crash loop (short runs): exponential in attempt.
        assert_eq!(resident_restart_delay(1_000, 1, 0), 1_000);
        assert_eq!(resident_restart_delay(1_000, 3, 0), 4_000);
        // Absolute cap: attempt 16+ on a 1s base would be 9+ hours uncapped.
        assert_eq!(
            resident_restart_delay(1_000, 17, 0),
            RESIDENT_BACKOFF_CAP_MS as i64
        );
        assert_eq!(
            resident_restart_delay(60_000, 30, 0),
            RESIDENT_BACKOFF_CAP_MS as i64
        );
        // Decay: a run healthy past the uptime threshold restarts on the BASE
        // delay again, no matter how big `attempt` has grown.
        assert_eq!(
            resident_restart_delay(1_000, 30, RESIDENT_HEALTHY_UPTIME_MS),
            1_000
        );
    }

    /// [[WEIR-T-0190]] item 6: the LRU eviction keeps the cache at the cap and
    /// drops the least-recently-used entry.
    #[test]
    fn lru_evict_drops_oldest_at_cap() {
        let mut m: HashMap<String, (u32, std::time::Instant)> = HashMap::new();
        let base = std::time::Instant::now();
        for (i, k) in ["a", "b", "c"].iter().enumerate() {
            m.insert(
                k.to_string(),
                (i as u32, base + std::time::Duration::from_millis(i as u64)),
            );
        }
        // Touch "a" so "b" becomes the LRU.
        m.get_mut("a").unwrap().1 = base + std::time::Duration::from_millis(10);
        lru_evict(&mut m, 3);
        assert_eq!(m.len(), 2);
        assert!(!m.contains_key("b"), "the least-recently-used entry goes");
        assert!(m.contains_key("a") && m.contains_key("c"));
        // Under the cap: nothing evicted.
        lru_evict(&mut m, 3);
        assert_eq!(m.len(), 2);
    }

    /// [[WEIR-T-0190]] item 3: a burst of ids is unique (the 20-bit counter is
    /// nonce-seeded per process; cross-process collisions need the same seed AND
    /// the same millisecond).
    #[test]
    fn next_id_burst_is_unique() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..50_000 {
            assert!(seen.insert(next_id()), "id collision in a rapid burst");
        }
    }
}
