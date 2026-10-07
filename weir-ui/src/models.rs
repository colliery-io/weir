//! API DTOs: what the control plane sends and receives.

#[derive(serde::Deserialize, Clone, PartialEq)]
pub(crate) struct Connection {
    pub(crate) name: String,
    pub(crate) source: String,
    pub(crate) dest: String,
    pub(crate) stream: String,
    // F1 ([[WEIR-I-0035]]): "run_once" (default) | "resident". Drives the Start/Stop
    // controls + the resident live badge. `#[serde(default)]` keeps older payloads valid.
    #[serde(default)]
    pub(crate) execution_mode: String,
}

#[derive(serde::Deserialize, Clone, PartialEq)]
pub(crate) struct RunRow {
    pub(crate) id: i64,
    pub(crate) connection: String,
    pub(crate) state: String,
    #[serde(default)]
    pub(crate) rows_written: i64,
    #[serde(default)]
    pub(crate) dead_lettered: i64,
    #[serde(default)]
    pub(crate) duration_ms: Option<i64>,
    #[serde(default)]
    pub(crate) error: Option<String>,
}

/// Per-connection health ([[WEIR-T-0110]]) from `GET /overview`.
#[derive(serde::Deserialize, Clone, PartialEq, Default)]
pub(crate) struct ConnHealth {
    pub(crate) connection: String,
    pub(crate) status: String, // green | amber | red | unknown
    #[serde(default)]
    pub(crate) lag_ms: Option<i64>,
    #[serde(default)]
    pub(crate) error_rate: f64,
    #[serde(default)]
    pub(crate) dead_letters: u64,
    #[serde(default)]
    pub(crate) rows_recent: i64,
    #[serde(default)]
    pub(crate) throughput: Vec<i64>,
}

/// One tenant's rolled-up health for the super-operator view ([[WEIR-T-0112]]).
#[derive(serde::Deserialize, Clone, PartialEq, Default)]
pub(crate) struct TenantHealth {
    pub(crate) tenant: String,
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) connections: u32,
    #[serde(default)]
    pub(crate) needs_attention: u32,
    #[serde(default)]
    pub(crate) dead_letters: u64,
    #[serde(default)]
    pub(crate) queue_depth: i64,
}

#[derive(serde::Deserialize, Clone, PartialEq, Default)]
pub(crate) struct AttentionItem {
    pub(crate) tenant: String,
    pub(crate) connection: String,
    pub(crate) status: String,
}

/// The platform-wide rollup from `GET /platform/health` (admin only).
#[derive(serde::Deserialize, Clone, PartialEq, Default)]
pub(crate) struct PlatformHealth {
    #[serde(default)]
    pub(crate) tenants: Vec<TenantHealth>,
    #[serde(default)]
    pub(crate) needs_attention: Vec<AttentionItem>,
    #[serde(default)]
    pub(crate) active_tenants: u32,
    #[serde(default)]
    pub(crate) total_queue_depth: i64,
}

/// One run in full ([[WEIR-T-0189]]) from `GET /runs/{id}`. `logs` is the run's
/// connection log tail (run logs are connection-scoped, not per run).
#[derive(serde::Deserialize, Clone, PartialEq, Default)]
pub(crate) struct RunDetail {
    pub(crate) id: i64,
    pub(crate) connection: String,
    #[serde(default)]
    pub(crate) stream: String,
    pub(crate) state: String,
    #[serde(default)]
    pub(crate) attempt: i64,
    #[serde(default)]
    pub(crate) rows_written: i64,
    #[serde(default)]
    pub(crate) dead_lettered: i64,
    #[serde(default)]
    pub(crate) started_at: Option<i64>,
    #[serde(default)]
    pub(crate) finished_at: Option<i64>,
    #[serde(default)]
    pub(crate) duration_ms: Option<i64>,
    #[serde(default)]
    pub(crate) error: Option<String>,
    #[serde(default)]
    pub(crate) logs: Vec<LogRow>,
}

/// A connection's committed state from `GET /connections/{name}/state`: the resume
/// cursor and committed chunk count. Connection-level — the API has no per-run cursor.
#[derive(serde::Deserialize, Clone, PartialEq, Default)]
pub(crate) struct ConnState {
    #[serde(default)]
    pub(crate) cursor: Option<String>,
    #[serde(default)]
    pub(crate) chunks: i64,
}

#[derive(serde::Deserialize, Clone, PartialEq)]
pub(crate) struct LogRow {
    pub(crate) level: String,
    pub(crate) message: String,
}

#[derive(serde::Deserialize, Clone, PartialEq)]
pub(crate) struct DeadLetterRow {
    pub(crate) record: String,
    pub(crate) reason: String,
}

/// A registered connector — drives the role-filtered source/dest selects.
#[derive(serde::Deserialize, Clone, PartialEq)]
pub(crate) struct CatalogItem {
    pub(crate) name: String,
    pub(crate) version: String,
    #[serde(default)]
    pub(crate) roles: Vec<String>,
}

/// An onboardable connector for "discover & select" — a crate package, a source
/// `manifest`, or a `dest-manifest`.
#[derive(serde::Deserialize, Clone, PartialEq, Default)]
pub(crate) struct AvailableItem {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) summary: String,
}

/// A manifest preview report — tier / confidence / gaps before onboarding.
#[derive(serde::Deserialize, Clone, PartialEq, Default)]
pub(crate) struct PreviewReport {
    pub(crate) tier: String,
    pub(crate) confidence: f32,
    #[serde(default)]
    pub(crate) streams: Vec<String>,
    #[serde(default)]
    pub(crate) unsupported: Vec<String>,
}

/// One field of a connector's config contract (from its JSON-Schema `config_schema`).
#[derive(Clone, PartialEq)]
pub(crate) struct Prop {
    pub(crate) key: String,
    pub(crate) kind: String,
    pub(crate) secret: bool,
    /// Listed in the schema's `required` array.
    pub(crate) required: bool,
    /// The schema's `enum` values, as strings; empty for a free field.
    pub(crate) options: Vec<String>,
}

#[derive(serde::Serialize)]
pub(crate) struct NewConnection {
    pub(crate) name: String,
    pub(crate) source: String,
    pub(crate) dest: String,
    pub(crate) stream: String,
    // Per-side config ([[WEIR-T-0214]]). The shared `config` is not sent: the server
    // defaults it to `{}`, and each side's object overrides it.
    pub(crate) source_config: serde_json::Value,
    pub(crate) dest_config: serde_json::Value,
    pub(crate) every_secs: Option<f64>,
    pub(crate) cron: Option<String>,
    // [[WEIR-T-0215]]: full_refresh | incremental | cdc; append | upsert | overwrite.
    // `cursor_field` only for incremental, `business_keys` only for upsert.
    pub(crate) sync_mode: String,
    pub(crate) write_mode: String,
    pub(crate) business_keys: Vec<String>,
    pub(crate) cursor_field: Option<String>,
    // F1: "run_once" | "resident"; for resident, `every_secs` is the emit cadence.
    pub(crate) execution_mode: String,
}

/// One typed field of a stream schema ([[WEIR-T-0121]]).
#[derive(Clone, serde::Deserialize, Default)]
pub(crate) struct SchemaField {
    pub(crate) name: String,
    #[serde(rename = "type")]
    pub(crate) ty: String,
    pub(crate) nullable: bool,
}

/// A connection's captured schema + any breaking-drift flag.
#[derive(Clone, serde::Deserialize, Default)]
pub(crate) struct SchemaView {
    pub(crate) fields: Vec<SchemaField>,
    pub(crate) broken: Option<String>,
}
