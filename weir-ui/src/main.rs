//! weir control-plane UI — Leptos (CSR) on the Colliery Aurora Dark design system
//! ([[WEIR-A-0035]]). Aurora ships the chrome; weir supplies the data + vocabulary.
//! Shell + Operations + Setup ([[WEIR-T-0079]]/[[WEIR-T-0080]]/[[WEIR-T-0081]]).

use aurora_leptos::components::*;
use aurora_leptos::tokens::token;
use aurora_leptos::AuroraStyles;
use leptos::prelude::*;
use std::collections::HashSet;

fn main() {
    leptos::mount::mount_to_body(App);
}

// ---------------------------------------------------------------------------- models

#[derive(serde::Deserialize, Clone, PartialEq)]
struct Connection {
    name: String,
    source: String,
    dest: String,
    stream: String,
    // F1 ([[WEIR-I-0035]]): "run_once" (default) | "resident". Drives the Start/Stop
    // controls + the resident live badge. `#[serde(default)]` keeps older payloads valid.
    #[serde(default)]
    execution_mode: String,
}

#[derive(serde::Deserialize, Clone, PartialEq)]
struct RunRow {
    id: i64,
    connection: String,
    state: String,
    #[serde(default)]
    rows_written: i64,
    #[serde(default)]
    dead_lettered: i64,
    #[serde(default)]
    duration_ms: Option<i64>,
    #[serde(default)]
    error: Option<String>,
}

/// Per-connection health ([[WEIR-T-0110]]) from `GET /overview`.
#[derive(serde::Deserialize, Clone, PartialEq, Default)]
struct ConnHealth {
    connection: String,
    status: String, // green | amber | red | unknown
    #[serde(default)]
    lag_ms: Option<i64>,
    #[serde(default)]
    error_rate: f64,
    #[serde(default)]
    dead_letters: u64,
    #[serde(default)]
    rows_recent: i64,
    #[serde(default)]
    throughput: Vec<i64>,
}

/// One tenant's rolled-up health for the super-operator view ([[WEIR-T-0112]]).
#[derive(serde::Deserialize, Clone, PartialEq, Default)]
struct TenantHealth {
    tenant: String,
    status: String,
    #[serde(default)]
    connections: u32,
    #[serde(default)]
    needs_attention: u32,
    #[serde(default)]
    dead_letters: u64,
    #[serde(default)]
    queue_depth: i64,
}

#[derive(serde::Deserialize, Clone, PartialEq, Default)]
struct AttentionItem {
    tenant: String,
    connection: String,
    status: String,
}

/// The platform-wide rollup from `GET /platform/health` (admin only).
#[derive(serde::Deserialize, Clone, PartialEq, Default)]
struct PlatformHealth {
    #[serde(default)]
    tenants: Vec<TenantHealth>,
    #[serde(default)]
    needs_attention: Vec<AttentionItem>,
    #[serde(default)]
    active_tenants: u32,
    #[serde(default)]
    total_queue_depth: i64,
}

#[derive(serde::Deserialize, Clone, PartialEq)]
struct LogRow {
    level: String,
    message: String,
}

#[derive(serde::Deserialize, Clone, PartialEq)]
struct DeadLetterRow {
    record: String,
    reason: String,
}

/// A registered connector — drives the role-filtered source/dest selects.
#[derive(serde::Deserialize, Clone, PartialEq)]
struct CatalogItem {
    name: String,
    version: String,
    #[serde(default)]
    roles: Vec<String>,
}

/// An onboardable connector for "discover & select" — a crate package, a source
/// `manifest`, or a `dest-manifest`.
#[derive(serde::Deserialize, Clone, PartialEq, Default)]
struct AvailableItem {
    name: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    summary: String,
}

/// A manifest preview report — tier / confidence / gaps before onboarding.
#[derive(serde::Deserialize, Clone, PartialEq, Default)]
struct PreviewReport {
    tier: String,
    confidence: f32,
    #[serde(default)]
    streams: Vec<String>,
    #[serde(default)]
    unsupported: Vec<String>,
}

/// One field of a connector's config contract (from its JSON-Schema `config_schema`).
#[derive(Clone, PartialEq)]
struct Prop {
    key: String,
    kind: String,
    secret: bool,
}

#[derive(serde::Serialize)]
struct NewConnection {
    name: String,
    source: String,
    dest: String,
    stream: String,
    config: serde_json::Value,
    every_secs: Option<f64>,
    cron: Option<String>,
    // F1: "run_once" | "resident"; for resident, `every_secs` is the emit cadence.
    execution_mode: String,
}

// ---------------------------------------------------------------------------- fetch

/// The stored API-key bearer (interim, [[WEIR-T-0084]]; the sign-in gate is [[WEIR-T-0087]]).
/// Reads `localStorage["weir_api_key"]`; empty until a key is set.
fn bearer() -> String {
    let key = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("weir_api_key").ok().flatten())
        .unwrap_or_default();
    format!("Bearer {key}")
}

/// The tenant a platform-admin has switched to ([[WEIR-T-0095]]) — `None` = own/`default` scope.
/// Held in localStorage so it survives the reload that re-scopes the views.
fn active_tenant() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("weir_active_tenant").ok().flatten())
        .filter(|t| !t.is_empty())
}

/// Re-scope a data call to the active tenant: a switched admin routes through `/tenants/{id}/…`.
/// Auth + the tenant-admin surface itself are never re-scoped.
/// One typed field of a stream schema ([[WEIR-T-0121]]).
#[derive(Clone, serde::Deserialize, Default)]
struct SchemaField {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    nullable: bool,
}

/// A connection's captured schema + any breaking-drift flag.
#[derive(Clone, serde::Deserialize, Default)]
struct SchemaView {
    fields: Vec<SchemaField>,
    broken: Option<String>,
}

fn apath(url: &str) -> String {
    if url.starts_with("/auth") || url.starts_with("/tenants") || url.starts_with("/platform") {
        return url.to_string();
    }
    match active_tenant() {
        Some(tid) => format!("/tenants/{tid}{url}"),
        None => url.to_string(),
    }
}

// Auth-aware gloo request builders — every API call carries the bearer + the active-tenant scope.
fn areq_get(url: &str) -> gloo_net::http::RequestBuilder {
    gloo_net::http::Request::get(&apath(url)).header("authorization", &bearer())
}
fn areq_post(url: &str) -> gloo_net::http::RequestBuilder {
    gloo_net::http::Request::post(&apath(url)).header("authorization", &bearer())
}
fn areq_delete(url: &str) -> gloo_net::http::RequestBuilder {
    gloo_net::http::Request::delete(&apath(url)).header("authorization", &bearer())
}

async fn get_json<T: serde::de::DeserializeOwned + Default>(url: String) -> T {
    match areq_get(&url).send().await {
        Ok(r) => r.json::<T>().await.unwrap_or_default(),
        Err(_) => T::default(),
    }
}

/// The outcome of an authed GET, with failure classes kept distinct ([[WEIR-T-0167]]) —
/// a 401 flips the sign-in gate, everything else feeds the error banner instead of
/// masquerading as an empty dashboard.
enum Fetched<T> {
    Ok(T),
    Unauthorized,
    /// HTTP error or a decode failure: (status, server's error message).
    Failed(u16, String),
    /// No answer at all.
    Network,
}

async fn get_fetch<T: serde::de::DeserializeOwned>(url: String) -> Fetched<T> {
    let resp = match areq_get(&url).send().await {
        Ok(r) => r,
        Err(_) => return Fetched::Network,
    };
    let status = resp.status();
    if status == 401 {
        return Fetched::Unauthorized;
    }
    if !resp.ok() {
        return Fetched::Failed(status, server_error(resp).await);
    }
    match resp.json::<T>().await {
        Ok(v) => Fetched::Ok(v),
        Err(e) => Fetched::Failed(status, format!("bad response: {e}")),
    }
}

/// The server's `{"error": …}` body (the API's uniform error shape), else the status line.
async fn server_error(resp: gloo_net::http::Response) -> String {
    let fallback = format!("HTTP {}", resp.status());
    match resp.json::<serde_json::Value>().await {
        Ok(v) => v
            .get("error")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string())
            .unwrap_or(fallback),
        Err(_) => fallback,
    }
}

/// Classify a mutation response: `Ok` passes the response through, everything else
/// becomes a human reason **including the server's error body** ([[WEIR-T-0167]]).
async fn check(
    sent: Result<gloo_net::http::Response, gloo_net::Error>,
) -> Result<gloo_net::http::Response, String> {
    match sent {
        Err(_) => Err("server unreachable".into()),
        Ok(r) if r.ok() => Ok(r),
        Ok(r) => Err(server_error(r).await),
    }
}

/// Fetch a connector's spec + parse its `config_schema` into a flat field list.
async fn fetch_props(plugin: &str) -> Vec<Prop> {
    if plugin.is_empty() {
        return Vec::new();
    }
    let Ok(resp) = areq_get(&format!("/connectors/{plugin}/spec")).send().await
    else {
        return Vec::new();
    };
    let Ok(spec) = resp.json::<serde_json::Value>().await else { return Vec::new() };
    let schema: serde_json::Value = spec
        .get("config_schema")
        .and_then(|s| s.as_str())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else { return Vec::new() };
    props
        .iter()
        .map(|(k, v)| {
            let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or("string").to_string();
            let secret = v.get("format").and_then(|f| f.as_str()) == Some("password")
                || v.get("airbyte_secret").and_then(|b| b.as_bool()) == Some(true);
            Prop { key: k.clone(), kind, secret }
        })
        .collect()
}

/// Discover a source's streams — POST the config, get stream names.
async fn fetch_streams(plugin: &str, config: &str) -> Vec<String> {
    if plugin.is_empty() {
        return Vec::new();
    }
    let req = match areq_post(&format!("/connectors/{plugin}/discover")).body(config.to_string()) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    match req.send().await {
        Ok(r) => r.json::<Vec<String>>().await.unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

// ---------------------------------------------------------------------------- helpers

fn fmt_dur(ms: i64) -> String {
    if ms < 1000 { format!("{ms}ms") } else { format!("{:.1}s", ms as f64 / 1000.0) }
}

fn run_metrics(r: &RunRow) -> String {
    match r.state.as_str() {
        "pending" => "queued".to_string(),
        "leased" => {
            if r.rows_written > 0 { format!("running… {} rows", r.rows_written) } else { "running…".to_string() }
        }
        "failed" => r.error.clone().filter(|e| !e.is_empty()).unwrap_or_else(|| "failed".to_string()),
        _ => {
            let dead = if r.dead_lettered > 0 { format!(" · {} dead", r.dead_lettered) } else { String::new() };
            match r.duration_ms {
                Some(d) => format!("{} rows · {}{}", r.rows_written, fmt_dur(d), dead),
                None => format!("{} rows{}", r.rows_written, dead),
            }
        }
    }
}

fn latest_run(runs: &[RunRow], conn: &str) -> Option<RunRow> {
    runs.iter().find(|r| r.connection == conn).cloned()
}

fn state_color(state: &str) -> &'static str {
    match state {
        "done" => token::OK,
        "leased" => token::ICE,
        "failed" => token::BAD,
        "pending" => token::VIOLET,
        _ => token::MUTED,
    }
}

/// Health status → an Aurora colour token ([[WEIR-T-0111]]).
fn health_color(status: &str) -> &'static str {
    match status {
        "green" => token::OK,
        "amber" => token::GOLD,
        "red" => token::BAD,
        _ => token::MUTED,
    }
}

/// Worst-first ordering: red < amber < green < unknown.
fn health_rank(status: &str) -> u8 {
    match status {
        "red" => 0,
        "amber" => 1,
        "green" => 2,
        _ => 3,
    }
}

/// Freshness lag, humanized.
fn fmt_lag(ms: Option<i64>) -> String {
    match ms {
        Some(m) if m < 1000 => format!("{m}ms"),
        Some(m) if m < 60_000 => format!("{:.0}s", m as f64 / 1000.0),
        Some(m) if m < 3_600_000 => format!("{:.0}m", m as f64 / 60_000.0),
        Some(m) => format!("{:.1}h", m as f64 / 3_600_000.0),
        None => "—".to_string(),
    }
}

/// Sparkline points (`x,y …`) over a 120×22 box, or `None` for no data.
fn spark_points(vals: &[i64]) -> Option<String> {
    if vals.is_empty() {
        return None;
    }
    let max = (*vals.iter().max().unwrap_or(&1)).max(1);
    let n = vals.len();
    let (w, h) = (120.0_f64, 22.0_f64);
    let pts = vals
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = if n > 1 { i as f64 / (n - 1) as f64 * w } else { 0.0 };
            let y = h - (*v as f64 / max as f64) * h;
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    Some(pts)
}

/// Friendly picker label for an onboardable connector.
fn friendly(p: &AvailableItem) -> String {
    match p.kind.as_str() {
        "manifest" => format!("{}  ·  low-code", p.name),
        "dest-manifest" => format!("{}  ·  destination", p.name),
        _ => {
            let n = p.name.strip_prefix("weir-").and_then(|s| s.strip_suffix("-pkg")).unwrap_or(&p.name);
            format!("{n}  ·  crate")
        }
    }
}

/// Already-onboarded connectors drop out of the picker.
fn is_onboarded(p: &AvailableItem, registered: &HashSet<String>) -> bool {
    let n = if p.kind == "manifest" || p.kind == "dest-manifest" {
        p.name.as_str()
    } else {
        p.name.strip_prefix("weir-").and_then(|s| s.strip_suffix("-pkg")).unwrap_or(&p.name)
    };
    registered.contains(n)
}

/// Read a key from a config-JSON string as a display string.
fn cfg_get(cfg: &str, key: &str) -> String {
    serde_json::from_str::<serde_json::Value>(cfg)
        .ok()
        .and_then(|v| v.get(key).cloned())
        .map(|v| match v {
            serde_json::Value::String(s) => s,
            other => other.to_string(),
        })
        .unwrap_or_default()
}

/// Set a key in a config-JSON string, coercing by the field kind; empty clears it.
fn cfg_set(cfg: &str, key: &str, val: &str, kind: &str) -> String {
    let mut obj = serde_json::from_str::<serde_json::Value>(cfg)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    if val.is_empty() {
        obj.remove(key);
    } else {
        let v = match kind {
            "integer" => val.parse::<i64>().map(serde_json::Value::from).unwrap_or_else(|_| serde_json::Value::from(val)),
            "number" => val.parse::<f64>().map(serde_json::Value::from).unwrap_or_else(|_| serde_json::Value::from(val)),
            "boolean" => serde_json::Value::from(matches!(val, "true" | "1")),
            _ => serde_json::Value::from(val),
        };
        obj.insert(key.to_string(), v);
    }
    serde_json::Value::Object(obj).to_string()
}

// ---------------------------------------------------------------------------- app

#[component]
fn App() -> impl IntoView {
    let view = RwSignal::new("Operations".to_string());

    // Auth gate ([[WEIR-T-0087]]): probe `/auth/me` → `authed` (None=checking, Some(false)=needs sign-in).
    let authed = RwSignal::new(Option::<bool>::None);
    let signed_in_as = RwSignal::new(String::new());
    let key_input = RwSignal::new(String::new());
    // Tenant context ([[WEIR-T-0095]]): the signed-in key's tenant + whether it's a platform-admin,
    // and (for admins) the list of tenants for the switcher.
    let my_tenant = RwSignal::new(String::new());
    let is_admin = RwSignal::new(false);
    let tenants = RwSignal::new(Vec::<(String, String)>::new());
    // Tenants admin panel ([[WEIR-T-0096]]): an overlay (admins) to CRUD tenants + their keys.
    let show_tenants = RwSignal::new(false);
    let sel_tenant = RwSignal::new(String::new());
    let tenant_keys = RwSignal::new(Vec::<(String, String, String, bool)>::new()); // (id, name, role, revoked)
    let minted_key = RwSignal::new(Option::<String>::None);
    let new_tenant_id = RwSignal::new(String::new());
    let new_key_name = RwSignal::new(String::new());
    fn local_storage() -> Option<web_sys::Storage> {
        web_sys::window().and_then(|w| w.local_storage().ok().flatten())
    }
    let recheck = move || {
        leptos::task::spawn_local(async move {
            match areq_get("/auth/me").send().await {
                Ok(r) if r.ok() => {
                    let me = r.json::<serde_json::Value>().await.unwrap_or_default();
                    signed_in_as.set(me.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string());
                    my_tenant.set(me.get("tenant").and_then(|v| v.as_str()).unwrap_or("default").to_string());
                    let admin = me.get("is_admin").and_then(|v| v.as_bool()).unwrap_or(false);
                    is_admin.set(admin);
                    // A platform-admin can switch tenants → load the list for the switcher.
                    if admin {
                        if let Ok(tr) = areq_get("/tenants").send().await {
                            if tr.ok() {
                                let list = tr.json::<Vec<serde_json::Value>>().await.unwrap_or_default();
                                tenants.set(
                                    list.iter()
                                        .filter_map(|t| {
                                            Some((
                                                t.get("id")?.as_str()?.to_string(),
                                                t.get("name")?.as_str()?.to_string(),
                                            ))
                                        })
                                        .collect(),
                                );
                            }
                        }
                    }
                    authed.set(Some(true));
                }
                _ => authed.set(Some(false)),
            }
        });
    };
    recheck();
    let use_key = move || {
        if let Some(store) = local_storage() {
            let _ = store.set_item("weir_api_key", key_input.get().trim());
        }
        key_input.set(String::new());
        recheck();
    };
    let sign_out = move || {
        if let Some(store) = local_storage() {
            let _ = store.remove_item("weir_api_key");
        }
        leptos::task::spawn_local(async move {
            let _ = areq_get("/auth/logout").send().await;
        });
        authed.set(Some(false));
    };

    let connections = RwSignal::new(Vec::<Connection>::new());
    let runs = RwSignal::new(Vec::<RunRow>::new());
    let toast = RwSignal::new(Option::<(bool, String)>::None);
    // Auto-dismiss after 6s unless a newer toast replaced this one; also closable in the view.
    let toast_seq = StoredValue::new(0u64);
    let flash = move |ok: bool, msg: String| {
        let id = toast_seq.get_value() + 1;
        toast_seq.set_value(id);
        toast.set(Some((ok, msg)));
        leptos::task::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(6_000).await;
            if toast_seq.get_value() == id {
                toast.set(None);
            }
        });
    };
    // Control-plane reachability ([[WEIR-T-0167]]): Some(reason) renders the error banner —
    // an outage must never masquerade as "No connections yet".
    let api_error = RwSignal::new(Option::<String>::None);

    // Tenants admin ([[WEIR-T-0096]]) — CRUD tenants + their keys via /tenants/* (never re-scoped).
    let reload_tenants = move || {
        leptos::task::spawn_local(async move {
            if let Ok(r) = areq_get("/tenants").send().await {
                if r.ok() {
                    let list = r.json::<Vec<serde_json::Value>>().await.unwrap_or_default();
                    tenants.set(list.iter().filter_map(|t| Some((
                        t.get("id")?.as_str()?.to_string(),
                        t.get("name")?.as_str()?.to_string(),
                    ))).collect());
                }
            }
        });
    };
    let load_keys = move |tid: String| {
        sel_tenant.set(tid.clone());
        leptos::task::spawn_local(async move {
            if let Ok(r) = areq_get(&format!("/tenants/{tid}/keys")).send().await {
                if r.ok() {
                    let list = r.json::<Vec<serde_json::Value>>().await.unwrap_or_default();
                    tenant_keys.set(list.iter().filter_map(|k| Some((
                        k.get("id")?.as_str()?.to_string(),
                        k.get("name")?.as_str()?.to_string(),
                        k.get("role")?.as_str()?.to_string(),
                        k.get("revoked").and_then(|v| v.as_bool()).unwrap_or(false),
                    ))).collect());
                }
            }
        });
    };
    let create_tenant = move || {
        let id = new_tenant_id.get().trim().to_string();
        if id.is_empty() { return; }
        leptos::task::spawn_local(async move {
            let body = serde_json::json!({ "id": id, "name": id });
            let sent = match areq_post("/tenants").json(&body) {
                Ok(r) => check(r.send().await).await,
                Err(e) => Err(format!("bad request: {e}")),
            };
            match sent {
                Ok(_) => flash(true, "tenant created".into()),
                Err(e) => flash(false, format!("create failed: {e}")),
            }
            new_tenant_id.set(String::new());
            reload_tenants();
        });
    };
    let mint_key = move || {
        let tid = sel_tenant.get();
        let name = new_key_name.get().trim().to_string();
        if tid.is_empty() || name.is_empty() { return; }
        leptos::task::spawn_local(async move {
            let body = serde_json::json!({ "name": name, "role": "write" });
            let sent = match areq_post(&format!("/tenants/{tid}/keys")).json(&body) {
                Ok(req) => check(req.send().await).await,
                Err(e) => Err(format!("bad request: {e}")),
            };
            match sent {
                Ok(r) => {
                    let v = r.json::<serde_json::Value>().await.unwrap_or_default();
                    minted_key.set(v.get("key").and_then(|k| k.as_str()).map(|s| s.to_string()));
                    flash(true, "key minted — copy it now".into());
                    new_key_name.set(String::new());
                    load_keys(tid);
                }
                Err(e) => flash(false, format!("mint failed: {e}")),
            }
        });
    };
    let revoke_key = move |kid: String| {
        let tid = sel_tenant.get();
        leptos::task::spawn_local(async move {
            match check(areq_delete(&format!("/tenants/{tid}/keys/{kid}")).send().await).await {
                Ok(_) => flash(true, "key revoked".into()),
                Err(e) => flash(false, format!("revoke failed: {e}")),
            }
            load_keys(tid);
        });
    };

    // Catalog + available (discover) — refetched on demand after onboard/save.
    let catalog = RwSignal::new(Vec::<CatalogItem>::new());
    let available = RwSignal::new(Vec::<AvailableItem>::new());
    let reload_catalog = move || {
        leptos::task::spawn_local(async move {
            catalog.set(get_json::<Vec<CatalogItem>>("/catalog".into()).await);
            available.set(get_json::<Vec<AvailableItem>>("/catalog/available".into()).await);
        });
    };
    reload_catalog();

    // Connection form state.
    let name = RwSignal::new(String::new());
    let src = RwSignal::new(String::new());
    let dst = RwSignal::new(String::new());
    let stream = RwSignal::new(String::new());
    let config = RwSignal::new("{}".to_string());
    let every = RwSignal::new(String::new());
    // F1 execution mode ([[WEIR-I-0035]]): run_once (default) | resident.
    let exec_mode = RwSignal::new("run_once".to_string());
    // Onboarding state.
    let add_pkg = RwSignal::new(String::new());
    let add_manifest = RwSignal::new(String::new());
    let add_path = RwSignal::new(String::new());
    let preview = RwSignal::new(Option::<PreviewReport>::None);
    // Schema fields + streams for the selected source (refetch when source changes).
    let props = RwSignal::new(Vec::<Prop>::new());
    let streams = RwSignal::new(Vec::<String>::new());
    Effect::new(move |_| {
        let s = src.get();
        let cfg = config.get_untracked();
        leptos::task::spawn_local(async move {
            props.set(fetch_props(&s).await);
            streams.set(fetch_streams(&s, &cfg).await);
        });
    });

    // Per-connection health ([[WEIR-T-0111]]) + the platform rollup ([[WEIR-T-0112]], admin only).
    let health = RwSignal::new(Vec::<ConnHealth>::new());
    let platform = RwSignal::new(PlatformHealth::default());

    // Live poll: /connections + /runs + /overview (health) every 800ms; /platform/health for
    // admins. [[WEIR-T-0167]]: /connections is the canary that classifies auth/server/network
    // state; each fetch only overwrites its signal on success, so an outage shows the error
    // banner over the last-known data instead of fake empty states — and the loop backs off
    // to 3.2s while degraded rather than hammering a dead server.
    leptos::task::spawn_local(async move {
        loop {
            match get_fetch::<Vec<Connection>>("/connections".into()).await {
                Fetched::Ok(v) => {
                    connections.set(v);
                    api_error.set(None);
                }
                Fetched::Unauthorized => {
                    api_error.set(None);
                    authed.set(Some(false));
                }
                Fetched::Failed(s, m) => api_error.set(Some(format!("{m} · HTTP {s}"))),
                Fetched::Network => api_error.set(Some("server unreachable — retrying".into())),
            }
            if let Fetched::Ok(v) = get_fetch::<Vec<RunRow>>("/runs".into()).await {
                runs.set(v);
            }
            if let Fetched::Ok(v) = get_fetch::<Vec<ConnHealth>>("/overview".into()).await {
                health.set(v);
            }
            if is_admin.get_untracked() {
                if let Fetched::Ok(v) = get_fetch::<PlatformHealth>("/platform/health".into()).await
                {
                    platform.set(v);
                }
            }
            let wait = if api_error.get_untracked().is_some() {
                3_200
            } else {
                800
            };
            gloo_timers::future::TimeoutFuture::new(wait).await;
        }
    });

    // Drill from the platform view into a tenant's health ([[WEIR-T-0112]]): re-scope the active
    // tenant (apath then routes /overview → /tenants/{id}/overview) + switch to the Health view.
    let drill_tenant = Callback::new(move |tid: String| {
        if let Some(store) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = store.set_item("weir_active_tenant", &tid);
        }
        view.set("Health".to_string());
    });

    // Run-detail modal.
    let selected = RwSignal::new(String::new());
    let detail_open = RwSignal::new(false);
    let detail_logs = RwSignal::new(Vec::<LogRow>::new());
    let detail_dls = RwSignal::new(Vec::<DeadLetterRow>::new());
    let detail_schema = RwSignal::new(SchemaView::default());
    let open_detail = Callback::new(move |n: String| {
        selected.set(n.clone());
        detail_open.set(true);
        leptos::task::spawn_local(async move {
            detail_logs.set(get_json::<Vec<LogRow>>(format!("/connections/{n}/logs")).await);
            detail_dls.set(get_json::<Vec<DeadLetterRow>>(format!("/connections/{n}/dead-letters")).await);
            detail_schema.set(get_json::<SchemaView>(format!("/connections/{n}/schema")).await);
        });
    });
    // Accept an evolved schema ([[WEIR-T-0121]]): clear the breaking flag, then refetch.
    let accept_schema = Callback::new(move |n: String| {
        leptos::task::spawn_local(async move {
            if let Err(e) = check(
                areq_post(&format!("/connections/{n}/schema/accept"))
                    .send()
                    .await,
            )
            .await
            {
                flash(false, format!("Couldn't accept schema: {e}"));
            }
            detail_schema.set(get_json::<SchemaView>(format!("/connections/{n}/schema")).await);
        });
    });
    let run_conn = Callback::new(move |n: String| {
        leptos::task::spawn_local(async move {
            match check(areq_post(&format!("/connections/{n}/run")).send().await).await {
                Ok(_) => flash(true, format!("Run queued · {n}")),
                Err(e) => flash(false, format!("Couldn't run {n}: {e}")),
            }
        });
    });
    let del_conn = Callback::new(move |n: String| {
        leptos::task::spawn_local(async move {
            match check(areq_delete(&format!("/connections/{n}")).send().await).await {
                Ok(_) => flash(true, format!("Deleted {n}")),
                Err(e) => flash(false, format!("Couldn't delete {n}: {e}")),
            }
        });
    });
    // F1 ([[WEIR-I-0035]]): launch / stop a resident source (enqueue-once + durable stop).
    let start_conn = Callback::new(move |n: String| {
        leptos::task::spawn_local(async move {
            match check(areq_post(&format!("/connections/{n}/start")).send().await).await {
                Ok(_) => {
                    flash(true, format!("Started {n}"));
                    // Refetch immediately so the pill flips without waiting for the 800ms poll.
                    connections.set(get_json::<Vec<Connection>>("/connections".into()).await);
                    runs.set(get_json::<Vec<RunRow>>("/runs".into()).await);
                }
                Err(e) => flash(false, format!("Couldn't start {n}: {e}")),
            }
        });
    });
    let stop_conn = Callback::new(move |n: String| {
        leptos::task::spawn_local(async move {
            match check(areq_post(&format!("/connections/{n}/stop")).send().await).await {
                Ok(_) => {
                    flash(true, format!("Stopped {n}"));
                    // Refetch immediately so the pill flips without waiting for the 800ms poll.
                    connections.set(get_json::<Vec<Connection>>("/connections".into()).await);
                    runs.set(get_json::<Vec<RunRow>>("/runs".into()).await);
                }
                Err(e) => flash(false, format!("Couldn't stop {n}: {e}")),
            }
        });
    });

    // Onboard the selected discover-picker connector.
    let onboard_pick = move || {
        let pkg = add_pkg.get().trim().to_string();
        if pkg.is_empty() {
            flash(false, "Select a connector".into());
            return;
        }
        let kind = available.get_untracked().iter().find(|a| a.name == pkg).map(|a| a.kind.clone()).unwrap_or_default();
        let body = match kind.as_str() {
            "manifest" => serde_json::json!({ "manifest_name": pkg }),
            "dest-manifest" => serde_json::json!({ "dest_manifest_name": pkg }),
            _ => serde_json::json!({ "package": pkg }),
        };
        let label = pkg.clone();
        leptos::task::spawn_local(async move {
            let sent = match areq_post("/catalog/import").json(&body) {
                Ok(req) => check(req.send().await).await,
                Err(e) => Err(format!("bad request: {e}")),
            };
            match sent {
                Ok(_) => {
                    add_pkg.set(String::new());
                    reload_catalog();
                    flash(true, format!("Onboarded {label}"));
                }
                Err(e) => flash(false, format!("Couldn't onboard {label}: {e}")),
            }
        });
    };
    // Preview a pasted manifest.
    let do_preview = move || {
        let m = add_manifest.get().trim().to_string();
        if m.is_empty() {
            flash(false, "Paste a manifest to preview".into());
            return;
        }
        leptos::task::spawn_local(async move {
            let sent = match areq_post("/catalog/preview").json(&serde_json::json!({ "manifest": m }))
            {
                Ok(req) => check(req.send().await).await,
                Err(e) => Err(format!("bad request: {e}")),
            };
            match sent {
                Ok(resp) => match resp.json::<PreviewReport>().await {
                    Ok(rep) => preview.set(Some(rep)),
                    Err(e) => flash(false, format!("Couldn't parse preview: {e}")),
                },
                Err(e) => flash(false, format!("Couldn't preview: {e}")),
            }
        });
    };
    // Onboard a pasted manifest or a crate path.
    let onboard_byo = move || {
        let manifest = add_manifest.get().trim().to_string();
        let path = add_path.get().trim().to_string();
        let body = if !manifest.is_empty() {
            serde_json::json!({ "manifest": manifest })
        } else if !path.is_empty() {
            serde_json::json!({ "path": path })
        } else {
            flash(false, "Paste a manifest or enter a crate path".into());
            return;
        };
        leptos::task::spawn_local(async move {
            let sent = match areq_post("/catalog/import").json(&body) {
                Ok(req) => check(req.send().await).await,
                Err(e) => Err(format!("bad request: {e}")),
            };
            match sent {
                Ok(_) => {
                    preview.set(None);
                    add_manifest.set(String::new());
                    add_path.set(String::new());
                    reload_catalog();
                    flash(true, "Onboarded connector".into());
                }
                Err(e) => flash(false, format!("Couldn't onboard: {e}")),
            }
        });
    };
    // Save the connection.
    let save_conn = move || {
        if name.get().trim().is_empty() {
            flash(false, "Name is required".into());
            return;
        }
        let cfg_str = config.get();
        let config_val: serde_json::Value = if cfg_str.trim().is_empty() {
            serde_json::json!({})
        } else {
            match serde_json::from_str(&cfg_str) {
                Ok(v) => v,
                Err(e) => { flash(false, format!("Config JSON: {e}")); return; }
            }
        };
        let every_secs = match every.get().trim() {
            "" => None,
            t => match t.parse::<f64>() {
                Ok(n) => Some(n),
                Err(_) => { flash(false, "Every must be a number of seconds".into()); return; }
            },
        };
        let saved = name.get();
        let body = NewConnection {
            name: name.get(), source: src.get(), dest: dst.get(), stream: stream.get(),
            config: config_val, every_secs, cron: None,
            execution_mode: exec_mode.get(),
        };
        leptos::task::spawn_local(async move {
            let sent = match areq_post("/connections").json(&body) {
                Ok(req) => check(req.send().await).await,
                Err(e) => Err(format!("bad request: {e}")),
            };
            match sent {
                Ok(_) => {
                    name.set(String::new());
                    flash(true, format!("Saved connection {saved}"));
                }
                // Carries the server's reason — e.g. the [[WEIR-T-0166]] validation messages.
                Err(e) => flash(false, format!("Couldn't save {saved}: {e}")),
            }
        });
    };

    view! {
        <AuroraStyles/>
        <style>{LAYOUT_CSS}</style>
        <header class="weir-header">
            <Group justify="between" top=true>
                <Group gap="sm">
                    <span class="weir-glyph">"≋"</span>
                    <span class="weir-wordmark">"weir"</span>
                    <Text size="xs" dimmed=true mono=true>"control plane"</Text>
                </Group>
                <Group gap="sm">
                    <span class="weir-stat">{move || format!("{} runs", runs.get().len())}</span>
                    <span class="weir-stat">
                        {move || format!("{} rows", runs.get().iter().map(|r| r.rows_written).sum::<i64>())}
                    </span>
                    // Tenant context ([[WEIR-T-0095]]): a platform-admin gets a switcher; others a chip.
                    {move || if is_admin.get() {
                        view! {
                            <select class="weir-tenant"
                                title="view another tenant"
                                on:change=move |ev| {
                                    let v = leptos::prelude::event_target_value(&ev);
                                    if let Some(store) = local_storage() {
                                        if v.is_empty() { let _ = store.remove_item("weir_active_tenant"); }
                                        else { let _ = store.set_item("weir_active_tenant", &v); }
                                    }
                                    if let Some(w) = web_sys::window() { let _ = w.location().reload(); }
                                }>
                                <option value="" selected=active_tenant().is_none()>"⊙ self (default)"</option>
                                {tenants.get().into_iter().map(|(id, name)| {
                                    let sel = active_tenant().as_deref() == Some(id.as_str());
                                    view! { <option value=id.clone() selected=sel>{format!("{name} · {id}")}</option> }
                                }).collect::<Vec<_>>()}
                            </select>
                        }.into_any()
                    } else {
                        view! { <span class="weir-tenant-chip">{move || format!("⊙ {}", my_tenant.get())}</span> }.into_any()
                    }}
                    {move || {
                        let mut opts = vec!["Operations".to_string(), "Health".to_string(), "Setup".to_string()];
                        if is_admin.get() { opts.insert(2, "Platform".to_string()); }
                        view! { <SegmentedControl options=opts value=view/> }
                    }}
                    {move || is_admin.get().then(|| view! {
                        <span class="weir-signout" title="administer tenants"
                            on:click=move |_| { reload_tenants(); show_tenants.set(true); }>"tenants"</span>
                    })}
                    <span class="weir-signout" on:click=move |_| sign_out()
                        title=move || format!("signed in as {}", signed_in_as.get())>"sign out"</span>
                </Group>
            </Group>
        </header>

        <main class="weir-main">
            {move || match view.get().as_str() {
                "Setup" => {
                    // Picker options (raw_name, friendly, summary), onboarded dropped.
                    let cat = catalog.get();
                    let avail = available.get();
                    let registered: HashSet<String> = cat.iter().map(|c| c.name.clone()).collect();
                    let mut opts: Vec<(String, String, String)> = Vec::new();
                    for want in ["manifest", "dest-manifest", "other"] {
                        for p in avail.iter().filter(|p| {
                            let k = if want == "other" { p.kind != "manifest" && p.kind != "dest-manifest" } else { p.kind == want };
                            k && !is_onboarded(p, &registered)
                        }) {
                            opts.push((p.name.clone(), friendly(p), p.summary.clone()));
                        }
                    }
                    let sources: Vec<CatalogItem> = cat.iter().filter(|c| c.roles.iter().any(|r| r == "Source")).cloned().collect();
                    let dests: Vec<CatalogItem> = cat.iter().filter(|c| c.roles.iter().any(|r| r == "Destination" || r == "ReverseEtl")).cloned().collect();
                    let prop_list = props.get();
                    let stream_list = streams.get();

                    view! {
                        <Panel title="Add a connector" caption="discover · onboard">
                            <div class="weir-form">
                                <label class="weir-fld">"pick a connector"
                                    <select class="cl-input cl-select" prop:value=move || add_pkg.get()
                                        on:change=move |e| add_pkg.set(event_target_value(&e))>
                                        <option value="">"— select a connector —"</option>
                                        {opts.into_iter().map(|(val, lbl, summ)| view! {
                                            <option value=val.clone() title=summ>{lbl}</option>
                                        }).collect_view()}
                                    </select>
                                </label>
                                <div><Button on_click=Callback::new(move |_| onboard_pick())>"Onboard"</Button></div>
                            </div>
                        </Panel>

                        <Panel title="Bring your own" caption="paste a manifest · or a crate path">
                            <div class="weir-form">
                                <label class="weir-fld">"paste a declarative manifest (YAML)"
                                    <textarea class="cl-input" rows="6"
                                        placeholder="type: DeclarativeSource\nstreams:\n  - ..."
                                        prop:value=move || add_manifest.get()
                                        on:input=move |e| add_manifest.set(event_target_value(&e))></textarea>
                                </label>
                                <label class="weir-fld">"or a crate path (full-code)"
                                    <input class="cl-input" placeholder="/path/to/weir-connector-foo"
                                        prop:value=move || add_path.get()
                                        on:input=move |e| add_path.set(event_target_value(&e))/>
                                </label>
                                <Group gap="sm">
                                    <Button variant="default" on_click=Callback::new(move |_| do_preview())>"Preview"</Button>
                                    <Button on_click=Callback::new(move |_| onboard_byo())>"Onboard"</Button>
                                </Group>
                                {move || preview.get().map(|rep| view! {
                                    <div class="weir-preview">
                                        <Text size="xs" mono=true>
                                            {format!("preview · tier {} · confidence {:.2} · streams {}", rep.tier, rep.confidence, rep.streams.len())}
                                        </Text>
                                        {if rep.unsupported.is_empty() {
                                            view! { <Text size="xs" dimmed=true>"fully supported by the runtime"</Text> }.into_any()
                                        } else {
                                            view! { <Text size="xs" dimmed=true>{format!("gaps: {}", rep.unsupported.join(", "))}</Text> }.into_any()
                                        }}
                                    </div>
                                })}
                            </div>
                        </Panel>

                        <Panel title="New / edit connection" caption="wire a source to a destination">
                            <div class="weir-form">
                                <label class="weir-fld">"name"
                                    <input class="cl-input" placeholder="my-sync"
                                        prop:value=move || name.get()
                                        on:input=move |e| name.set(event_target_value(&e))/>
                                </label>
                                <div class="weir-two">
                                    <label class="weir-fld">"source"
                                        <select class="cl-input cl-select" prop:value=move || src.get()
                                            on:change=move |e| src.set(event_target_value(&e))>
                                            <option value="">"— source —"</option>
                                            {sources.into_iter().map(|c| view! {
                                                <option value=c.name.clone()>{format!("{} · {}", c.name, c.version)}</option>
                                            }).collect_view()}
                                        </select>
                                    </label>
                                    <label class="weir-fld">"destination"
                                        <select class="cl-input cl-select" prop:value=move || dst.get()
                                            on:change=move |e| dst.set(event_target_value(&e))>
                                            <option value="">"— destination —"</option>
                                            {dests.into_iter().map(|c| view! {
                                                <option value=c.name.clone()>{format!("{} · {}", c.name, c.version)}</option>
                                            }).collect_view()}
                                        </select>
                                    </label>
                                </div>
                                <div class="weir-two">
                                    <label class="weir-fld">"stream"
                                        {if stream_list.is_empty() {
                                            view! {
                                                <input class="cl-input" prop:value=move || stream.get()
                                                    on:input=move |e| stream.set(event_target_value(&e))/>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <select class="cl-input cl-select" prop:value=move || stream.get()
                                                    on:change=move |e| stream.set(event_target_value(&e))>
                                                    {stream_list.into_iter().map(|s| { let v = s.clone(); view! { <option value=v>{s}</option> } }).collect_view()}
                                                </select>
                                            }.into_any()
                                        }}
                                    </label>
                                    <label class="weir-fld">
                                        {move || if exec_mode.get() == "resident" { "every (secs) · emit cadence" } else { "every (secs)" }}
                                        <input class="cl-input" placeholder="—"
                                            prop:value=move || every.get()
                                            on:input=move |e| every.set(event_target_value(&e))/>
                                    </label>
                                </div>
                                <label class="weir-fld">"execution mode"
                                    <select class="cl-input cl-select" prop:value=move || exec_mode.get()
                                        on:change=move |e| exec_mode.set(event_target_value(&e))>
                                        <option value="run_once">"run once · scheduled / batch"</option>
                                        <option value="resident">"resident · long-lived (Start/Stop; every = cadence)"</option>
                                    </select>
                                </label>
                                {(!prop_list.is_empty()).then(|| view! {
                                    <div class="weir-schema">
                                        <Text size="xs" dimmed=true mono=true>{move || format!("config · {} contract", src.get())}</Text>
                                        {prop_list.into_iter().map(|p| {
                                            let (key, kind) = (p.key.clone(), p.kind.clone());
                                            let kg = key.clone();
                                            let kind_ph = p.kind.clone();
                                            view! {
                                                <label class="weir-fld">{p.key.clone()}
                                                    <input class="cl-input"
                                                        r#type=if p.secret { "password" } else { "text" }
                                                        placeholder=kind_ph
                                                        prop:value=move || cfg_get(&config.get(), &kg)
                                                        on:input=move |e| config.set(cfg_set(&config.get_untracked(), &key, &event_target_value(&e), &kind))/>
                                                </label>
                                            }
                                        }).collect_view()}
                                    </div>
                                })}
                                <label class="weir-fld">"config (JSON)"
                                    <textarea class="cl-input" rows="3"
                                        prop:value=move || config.get()
                                        on:input=move |e| config.set(event_target_value(&e))></textarea>
                                </label>
                                <div><Button on_click=Callback::new(move |_| save_conn())>"Save connection"</Button></div>
                            </div>
                        </Panel>
                    }.into_any()
                }
                "Health" => view! {
                    <Panel title="Needs attention" caption="amber + red connections, worst first">
                        {move || {
                            let mut hs = health.get();
                            hs.sort_by_key(|h| health_rank(&h.status));
                            let attn: Vec<ConnHealth> = hs.into_iter()
                                .filter(|h| h.status == "red" || h.status == "amber").collect();
                            if attn.is_empty() {
                                view! { <Text dimmed=true>"All connections healthy."</Text> }.into_any()
                            } else {
                                view! {
                                    <Table mono=true>
                                        <thead><tr><th>"status"</th><th>"connection"</th><th>"lag"</th><th>"errors"</th><th>"dead"</th></tr></thead>
                                        <tbody>
                                            {attn.into_iter().map(|h| {
                                                let name = h.connection.clone();
                                                let color = health_color(&h.status).to_string();
                                                view! {
                                                    <tr class="weir-feed-row" on:click=move |_| open_detail.run(name.clone())>
                                                        <td><Pill color=color>{h.status.clone()}</Pill></td>
                                                        <td>{h.connection.clone()}</td>
                                                        <td>{fmt_lag(h.lag_ms)}</td>
                                                        <td>{format!("{:.0}%", h.error_rate * 100.0)}</td>
                                                        <td>{h.dead_letters.to_string()}</td>
                                                    </tr>
                                                }
                                            }).collect_view()}
                                        </tbody>
                                    </Table>
                                }.into_any()
                            }
                        }}
                    </Panel>
                    <Panel title="Connection health" caption="freshness · errors · dead letters · throughput">
                        {move || {
                            let mut hs = health.get();
                            hs.sort_by_key(|h| health_rank(&h.status));
                            if hs.is_empty() {
                                view! { <Text dimmed=true>"No connections yet — add one in Setup."</Text> }.into_any()
                            } else {
                                view! {
                                    <SimpleGrid cols=3>
                                        {hs.into_iter().map(|h| {
                                            let name = h.connection.clone();
                                            let color = health_color(&h.status).to_string();
                                            let pts = spark_points(&h.throughput);
                                            view! {
                                                <div class="weir-health-card" on:click=move |_| open_detail.run(name.clone())>
                                                    <div class="weir-health-head">
                                                        <Pill color=color>{h.status.clone()}</Pill>
                                                        <span class="weir-health-name">{h.connection.clone()}</span>
                                                    </div>
                                                    <div class="weir-health-stats">
                                                        <span>{format!("lag {}", fmt_lag(h.lag_ms))}</span>
                                                        <span>{format!("err {:.0}%", h.error_rate * 100.0)}</span>
                                                        <span>{format!("dl {}", h.dead_letters)}</span>
                                                        <span>{format!("{} rows", h.rows_recent)}</span>
                                                    </div>
                                                    {pts.map(|p| view! {
                                                        <svg class="weir-spark" viewBox="0 0 120 22" preserveAspectRatio="none">
                                                            <polyline points=p fill="none" stroke="currentColor" stroke-width="1.5"></polyline>
                                                        </svg>
                                                    })}
                                                </div>
                                            }
                                        }).collect_view()}
                                    </SimpleGrid>
                                }.into_any()
                            }
                        }}
                    </Panel>
                }
                .into_any(),
                "Platform" => view! {
                    <Panel title="Platform" caption="cross-tenant health · fleet">
                        {move || {
                            let p = platform.get();
                            view! {
                                <div class="weir-health-stats">
                                    <span>{format!("{} tenants", p.tenants.len())}</span>
                                    <span>{format!("{} active", p.active_tenants)}</span>
                                    <span>{format!("queue {}", p.total_queue_depth)}</span>
                                </div>
                            }
                        }}
                    </Panel>
                    <Panel title="Tenant health" caption="worst first · click to drill in">
                        {move || {
                            let mut ts = platform.get().tenants;
                            ts.sort_by_key(|t| health_rank(&t.status));
                            if ts.is_empty() {
                                view! { <Text dimmed=true>"No tenants."</Text> }.into_any()
                            } else {
                                view! {
                                    <SimpleGrid cols=3>
                                        {ts.into_iter().map(|t| {
                                            let tid = t.tenant.clone();
                                            let color = health_color(&t.status).to_string();
                                            view! {
                                                <div class="weir-health-card" on:click=move |_| drill_tenant.run(tid.clone())>
                                                    <div class="weir-health-head">
                                                        <Pill color=color>{t.status.clone()}</Pill>
                                                        <span class="weir-health-name">{t.tenant.clone()}</span>
                                                    </div>
                                                    <div class="weir-health-stats">
                                                        <span>{format!("{} conns", t.connections)}</span>
                                                        <span>{format!("{} failing", t.needs_attention)}</span>
                                                        <span>{format!("dl {}", t.dead_letters)}</span>
                                                        <span>{format!("queue {}", t.queue_depth)}</span>
                                                    </div>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </SimpleGrid>
                                }.into_any()
                            }
                        }}
                    </Panel>
                    <Panel title="Needs attention" caption="failing connections across all tenants">
                        {move || {
                            let attn = platform.get().needs_attention;
                            if attn.is_empty() {
                                view! { <Text dimmed=true>"All tenants healthy."</Text> }.into_any()
                            } else {
                                view! {
                                    <Table mono=true>
                                        <thead><tr><th>"status"</th><th>"tenant"</th><th>"connection"</th></tr></thead>
                                        <tbody>
                                            {attn.into_iter().map(|a| {
                                                let color = health_color(&a.status).to_string();
                                                let tid = a.tenant.clone();
                                                view! {
                                                    <tr class="weir-feed-row" on:click=move |_| drill_tenant.run(tid.clone())>
                                                        <td><Pill color=color>{a.status.clone()}</Pill></td>
                                                        <td>{a.tenant.clone()}</td>
                                                        <td>{a.connection.clone()}</td>
                                                    </tr>
                                                }
                                            }).collect_view()}
                                        </tbody>
                                    </Table>
                                }.into_any()
                            }
                        }}
                    </Panel>
                }
                .into_any(),
                _ => view! {
                    <Panel title="Connections" caption="live runs">
                        {move || {
                            let rs = runs.get();
                            let cs = connections.get();
                            if cs.is_empty() {
                                view! { <Text dimmed=true>"No connections yet — add one in Setup."</Text> }.into_any()
                            } else {
                                view! {
                                    <SimpleGrid cols=3>
                                        {cs.into_iter().map(|c| {
                                            let last = latest_run(&rs, &c.name);
                                            view! { <ConnectionCard conn=c last=last on_open=open_detail on_run=run_conn on_delete=del_conn on_start=start_conn on_stop=stop_conn/> }
                                        }).collect_view()}
                                    </SimpleGrid>
                                }.into_any()
                            }
                        }}
                    </Panel>
                    <Panel title="Run feed" caption="most recent first">
                        {move || {
                            let rs = runs.get();
                            if rs.is_empty() {
                                view! { <Text dimmed=true>"No runs yet."</Text> }.into_any()
                            } else {
                                view! {
                                    <Table mono=true>
                                        <thead><tr><th>"#"</th><th>"connection"</th><th>"state"</th><th>"detail"</th></tr></thead>
                                        <tbody>
                                            {rs.into_iter().map(|r| {
                                                let name = r.connection.clone();
                                                let color = state_color(&r.state).to_string();
                                                let metrics = run_metrics(&r);
                                                let (id, connection, state) = (r.id, r.connection.clone(), r.state.clone());
                                                view! {
                                                    <tr class="weir-feed-row" on:click=move |_| open_detail.run(name.clone())>
                                                        <td>{id}</td>
                                                        <td>{connection}</td>
                                                        <td><Pill color=color>{state}</Pill></td>
                                                        <td>{metrics}</td>
                                                    </tr>
                                                }
                                            }).collect_view()}
                                        </tbody>
                                    </Table>
                                }.into_any()
                            }
                        }}
                    </Panel>
                }
                .into_any(),
            }}
        </main>

        <Modal open=detail_open title="run detail".to_string()>
            <Text bold=true mono=true>{move || selected.get()}</Text>
            // Lineage ([[WEIR-T-0101]]): source · stream → dest + rows/duration, from the run data.
            <div class="weir-detail-sec"><Text size="xs" dimmed=true>"Lineage"</Text></div>
            {move || {
                let name = selected.get();
                match connections.get().into_iter().find(|c| c.name == name) {
                    Some(c) => {
                        let run = runs.get().into_iter().find(|r| r.connection == name);
                        let rows = run.as_ref().map(|r| r.rows_written).unwrap_or(0);
                        let dur = run.as_ref().and_then(|r| r.duration_ms).map(fmt_dur).unwrap_or_default();
                        let metrics = if dur.is_empty() {
                            format!("· {rows} rows")
                        } else {
                            format!("· {rows} rows · {dur}")
                        };
                        view! {
                            <Group gap="sm">
                                <Text size="xs" bold=true mono=true>{c.source}</Text>
                                <Text size="xs" dimmed=true>{format!("· {} →", c.stream)}</Text>
                                <Text size="xs" bold=true mono=true>{c.dest}</Text>
                                <Text size="xs" dimmed=true mono=true>{metrics}</Text>
                            </Group>
                        }.into_any()
                    }
                    None => view! { <Text dimmed=true>"—"</Text> }.into_any(),
                }
            }}
            // Typed schema + drift ([[WEIR-T-0121]] / [[WEIR-I-0025]]).
            <div class="weir-detail-sec"><Text size="xs" dimmed=true>"Schema"</Text></div>
            {move || {
                let sv = detail_schema.get();
                let name = selected.get();
                let banner = sv.broken.clone().map(|reason| {
                    let n = name.clone();
                    view! {
                        <div class="weir-drift">
                            <Text size="xs" bold=true>{format!("⚠ schema drift — {reason}")}</Text>
                            <button class="weir-drift-accept" on:click=move |_| accept_schema.run(n.clone())>
                                "Accept new schema"
                            </button>
                        </div>
                    }
                });
                let body = if sv.fields.is_empty() {
                    view! { <Text dimmed=true>"no schema captured yet"</Text> }.into_any()
                } else {
                    view! {
                        <List>
                            {sv.fields.into_iter().map(|f| {
                                let opt = if f.nullable { "nullable" } else { "required" };
                                view! {
                                    <ListItem>
                                        <Text size="xs" bold=true mono=true>{f.name}</Text>
                                        <Text size="xs" dimmed=true mono=true>{format!("{} · {opt}", f.ty)}</Text>
                                    </ListItem>
                                }
                            }).collect_view()}
                        </List>
                    }.into_any()
                };
                view! { <div class="weir-schema-view">{banner}{body}</div> }.into_any()
            }}
            <div class="weir-detail-sec"><Text size="xs" dimmed=true>"Dead-letters"</Text></div>
            {move || {
                let dls = detail_dls.get();
                if dls.is_empty() {
                    view! { <Text dimmed=true>"none"</Text> }.into_any()
                } else {
                    view! {
                        <List>
                            {dls.into_iter().map(|d| view! {
                                <ListItem><Text size="xs" bold=true>{d.reason}</Text><Text size="xs" dimmed=true mono=true>{d.record}</Text></ListItem>
                            }).collect_view()}
                        </List>
                    }.into_any()
                }
            }}
            <div class="weir-detail-sec"><Text size="xs" dimmed=true>"Logs"</Text></div>
            {move || {
                let logs = detail_logs.get();
                if logs.is_empty() {
                    view! { <Text dimmed=true>"no logs"</Text> }.into_any()
                } else {
                    view! {
                        <List>
                            {logs.into_iter().map(|l| view! {
                                <ListItem><Text size="xs" mono=true>{format!("{} · {}", l.level, l.message)}</Text></ListItem>
                            }).collect_view()}
                        </List>
                    }.into_any()
                }
            }}
        </Modal>

        // Degraded-control-plane banner ([[WEIR-T-0167]]): persistent while the poll fails.
        {move || api_error.get().map(|msg| view! {
            <div class="weir-apierr" title="the dashboards show last-known data until this clears">
                {format!("⚠ control plane error — {msg}")}
            </div>
        })}

        {move || toast.get().map(|(ok, msg)| view! {
            <div class="weir-toast" class:weir-toast--ok=ok class:weir-toast--err=!ok
                title="click to dismiss" on:click=move |_| toast.set(None)>{msg}</div>
        })}

        // Sign-in gate ([[WEIR-T-0087]]) — a full-screen overlay until authenticated.
        {move || (authed.get() == Some(false)).then(|| view! {
            <div class="weir-auth-overlay">
                <div class="weir-auth-card">
                    <Panel title="Sign in to weir" caption="authenticate to continue">
                        <div class="weir-form">
                            <Button on_click=Callback::new(move |_| {
                                if let Some(w) = web_sys::window() {
                                    let _ = w.location().set_href("/auth/login");
                                }
                            })>"Sign in with OIDC"</Button>
                            <label class="weir-fld">"or paste an API key"
                                <input class="cl-input" placeholder="weirk_…"
                                    prop:value=move || key_input.get()
                                    on:input=move |e| key_input.set(event_target_value(&e))/>
                            </label>
                            <div><Button on_click=Callback::new(move |_| use_key())>"Use API key"</Button></div>
                        </div>
                    </Panel>
                </div>
            </div>
        })}

        // Tenants admin overlay ([[WEIR-T-0096]]) — platform-admin CRUD tenants + their keys.
        {move || show_tenants.get().then(|| view! {
            <div class="weir-auth-overlay">
                <div class="weir-tenants-card">
                    <Panel title="Tenants" caption="administer tenants + their keys">
                        <div class="weir-form">
                            <div class="weir-signout" on:click=move |_| show_tenants.set(false)>"✕ close"</div>
                            <label class="weir-fld">"new tenant id"
                                <input class="cl-input" placeholder="acme"
                                    prop:value=move || new_tenant_id.get()
                                    on:input=move |e| new_tenant_id.set(event_target_value(&e))/>
                            </label>
                            <div><Button on_click=Callback::new(move |_| create_tenant())>"Create tenant"</Button></div>
                            <table class="weir-ttable">
                                <thead><tr><th>"tenant"</th><th>"name"</th><th></th></tr></thead>
                                <tbody>
                                    {move || tenants.get().into_iter().map(|(id, name)| {
                                        let id2 = id.clone();
                                        view! { <tr>
                                            <td><code>{id.clone()}</code></td><td>{name}</td>
                                            <td><span class="weir-signout" on:click=move |_| { minted_key.set(None); load_keys(id2.clone()); }>"keys →"</span></td>
                                        </tr> }
                                    }).collect::<Vec<_>>()}
                                </tbody>
                            </table>
                            {move || (!sel_tenant.get().is_empty()).then(|| view! {
                                <div class="weir-keys">
                                    <Text size="xs" dimmed=true>{move || format!("keys · {}", sel_tenant.get())}</Text>
                                    {move || minted_key.get().map(|k| view! {
                                        <div class="weir-minted">"new key — copy now: "<code>{k}</code></div>
                                    })}
                                    <label class="weir-fld">"new key name"
                                        <input class="cl-input" placeholder="ci"
                                            prop:value=move || new_key_name.get()
                                            on:input=move |e| new_key_name.set(event_target_value(&e))/>
                                    </label>
                                    <div><Button on_click=Callback::new(move |_| mint_key())>"Mint key"</Button></div>
                                    <table class="weir-ttable">
                                        <tbody>
                                            {move || tenant_keys.get().into_iter().map(|(kid, name, role, revoked)| {
                                                let kid2 = kid.clone();
                                                view! { <tr>
                                                    <td>{name}</td><td>{role}</td>
                                                    <td>{if revoked { "revoked" } else { "" }}</td>
                                                    <td>{(!revoked).then(|| view! {
                                                        <span class="weir-signout" on:click=move |_| revoke_key(kid2.clone())>"revoke"</span>
                                                    })}</td>
                                                </tr> }
                                            }).collect::<Vec<_>>()}
                                        </tbody>
                                    </table>
                                </div>
                            })}
                        </div>
                    </Panel>
                </div>
            </div>
        })}
    }
}

#[component]
fn ConnectionCard(
    conn: Connection,
    last: Option<RunRow>,
    on_open: Callback<String>,
    on_run: Callback<String>,
    on_delete: Callback<String>,
    on_start: Callback<String>,
    on_stop: Callback<String>,
) -> impl IntoView {
    let state = last.as_ref().map(|r| r.state.clone()).unwrap_or_else(|| "idle".to_string());
    let color = state_color(&state).to_string();
    let when = match &last {
        Some(r) => format!("#{} · {}", r.id, run_metrics(r)),
        None => "no runs yet".to_string(),
    };
    // F1 ([[WEIR-I-0035]]): a resident source shows a live/stopped badge + Start/Stop instead of Run.
    // "live" = it holds an active (leased/pending) run; otherwise it's stopped.
    let resident = conn.execution_mode == "resident";
    let live = matches!(state.as_str(), "leased" | "pending");
    let (n_open, n_run, n_del, n_start, n_stop) = (
        conn.name.clone(), conn.name.clone(), conn.name.clone(), conn.name.clone(), conn.name.clone(),
    );
    view! {
        <div class="weir-card" on:click=move |_| on_open.run(n_open.clone())>
            <div class="weir-card__top">
                <span class="weir-card__name">{conn.name.clone()}</span>
                {if resident {
                    let (rc, rl) = if live { (token::OK, "resident • live") } else { (token::MUTED, "resident • stopped") };
                    view! { <Pill color=rc.to_string()>{rl}</Pill> }.into_any()
                } else {
                    view! { <Pill color=color.clone()>{state.clone()}</Pill> }.into_any()
                }}
            </div>
            <div class="weir-flow">
                <span class="weir-node">{conn.source.clone()}</span>
                <span class="weir-arr">"──▶"</span>
                <span class="weir-node">{conn.dest.clone()}</span>
            </div>
            <Text size="xs" dimmed=true mono=true>{format!("stream · {}", conn.stream)}</Text>
            <div class="weir-card__foot">
                <Text size="xs" dimmed=true mono=true>{when}</Text>
                <div class="weir-foot-btns" on:click=move |e: leptos::ev::MouseEvent| e.stop_propagation()>
                    <Button variant="default" size="xs" bad=true on_click=Callback::new(move |_| on_delete.run(n_del.clone()))>"del"</Button>
                    {if resident {
                        view! {
                            <Button variant="default" size="xs" on_click=Callback::new(move |_| on_stop.run(n_stop.clone()))>"Stop"</Button>
                            <Button size="xs" on_click=Callback::new(move |_| on_start.run(n_start.clone()))>"Start"</Button>
                        }.into_any()
                    } else {
                        view! {
                            <Button size="xs" on_click=Callback::new(move |_| on_run.run(n_run.clone()))>"Run"</Button>
                        }.into_any()
                    }}
                </div>
            </div>
        </div>
    }
}

/// Top-nav layout + card/form scaffolding (Aurora's AppShell is sidebar-oriented); colors/type
/// come from Aurora tokens.
const LAYOUT_CSS: &str = r#"
.weir-header { position: sticky; top: 0; z-index: 5; padding: 14px 26px;
  border-bottom: 1px solid var(--border); background: var(--sidebar); }
.weir-glyph, .weir-wordmark { font-family: var(--font-mono); font-weight: 700;
  background: var(--aurora-3); -webkit-background-clip: text; background-clip: text; color: transparent; }
.weir-glyph { font-size: 19px; } .weir-wordmark { font-size: 20px; letter-spacing: .05em; }
.weir-stat { font-family: var(--font-mono); font-size: 11px; letter-spacing: .06em;
  text-transform: uppercase; color: var(--faint); }
.weir-main { max-width: 1180px; margin: 0 auto; padding: 24px 26px; display: grid; gap: 20px; }
.weir-card { border: 1px solid var(--border); border-radius: var(--radius-panel);
  background: var(--panel); padding: 14px 15px; display: grid; gap: 9px; cursor: pointer;
  transition: border-color .15s, transform .15s; }
.weir-card:hover { border-color: var(--border-control); transform: translateY(-2px); }
.weir-card__top { display: flex; align-items: center; justify-content: space-between; }
.weir-card__name { font-family: var(--font-mono); font-weight: 700; }
.weir-flow { display: flex; align-items: center; gap: 8px; font-family: var(--font-mono); font-size: 12px; }
.weir-node { color: var(--fg-2); } .weir-arr { color: var(--ice); }
.weir-card__foot { display: flex; align-items: center; justify-content: space-between; margin-top: 2px; }
.weir-foot-btns { display: flex; gap: 6px; }
.weir-feed-row { cursor: pointer; }
.weir-health-card { border: 1px solid var(--border); border-radius: var(--radius-panel); padding: 12px 14px;
    display: grid; gap: 8px; cursor: pointer; transition: border-color .15s, transform .15s; }
.weir-health-card:hover { border-color: var(--border-control); transform: translateY(-2px); }
.weir-health-head { display: flex; align-items: center; gap: 8px; }
.weir-health-name { font-family: var(--font-mono); font-weight: 700; }
.weir-schema-view { display: grid; gap: 6px; }
.weir-drift { display: flex; align-items: center; justify-content: space-between; gap: 12px;
  padding: 8px 12px; margin-bottom: 8px; border: 1px solid var(--bad);
  border-radius: var(--radius-panel); background: color-mix(in srgb, var(--bad) 12%, var(--panel)); }
.weir-drift-accept { font-family: var(--font-mono); font-size: 11px; padding: 4px 10px; white-space: nowrap;
  border: 1px solid var(--border-control); border-radius: var(--radius-panel); background: var(--panel);
  color: inherit; cursor: pointer; }
.weir-drift-accept:hover { border-color: var(--bad); }
.weir-health-stats { display: flex; flex-wrap: wrap; gap: 4px 12px; font-family: var(--font-mono);
    font-size: 11px; color: var(--fg-2); }
.weir-spark { width: 100%; height: 22px; color: var(--ice); display: block; }
.weir-form { display: grid; gap: 12px; }
.weir-two { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.weir-fld { display: grid; gap: 5px; font-family: var(--font-mono); font-size: 11px; letter-spacing: .05em;
  text-transform: uppercase; color: var(--muted); }
.weir-schema { display: grid; gap: 10px; padding: 12px; border: 1px solid var(--border);
  border-radius: var(--radius-sm4); background: var(--inset); }
.weir-preview { padding: 10px 12px; border: 1px solid var(--border); border-radius: var(--radius-sm4);
  background: var(--inset); }
.weir-detail-sec { margin: 12px 0 4px; border-top: 1px solid var(--border); padding-top: 8px; }
.weir-toast { position: fixed; top: 16px; right: 16px; z-index: 50; font-family: var(--font-mono);
  font-size: 12.5px; padding: 10px 14px; border-radius: var(--radius-sm4); background: var(--panel);
  border: 1px solid var(--border); box-shadow: 0 8px 28px rgba(0,0,0,.45); }
.weir-toast--ok { border-color: var(--ok); } .weir-toast--err { border-color: var(--bad); }
.weir-apierr { position: fixed; top: 0; left: 0; right: 0; z-index: 60; text-align: center;
  font-family: var(--font-mono); font-size: 12px; padding: 6px 12px;
  background: #3a1418; color: #ffb4b4; border-bottom: 1px solid var(--bad); cursor: default; }
.weir-signout { font-family: var(--font-mono); font-size: 11px; letter-spacing: .05em;
  text-transform: uppercase; color: var(--faint); cursor: pointer; }
.weir-signout:hover { color: var(--fg-2); }
.weir-tenant { font-family: var(--font-mono); font-size: 11px; background: var(--bg-2); color: var(--fg-1); border: 1px solid var(--border); border-radius: 6px; padding: 3px 6px; cursor: pointer; }
.weir-tenant-chip { font-family: var(--font-mono); font-size: 11px; color: var(--fg-2); background: var(--bg-2); border: 1px solid var(--border); border-radius: 6px; padding: 3px 8px; }
.weir-tenants-card { width: min(560px, 92vw); max-height: 86vh; overflow: auto; }
.weir-ttable { width: 100%; border-collapse: collapse; font-size: 12px; margin-top: 8px; }
.weir-ttable th, .weir-ttable td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--border); }
.weir-keys { margin-top: 14px; border-top: 1px solid var(--border); padding-top: 10px; }
.weir-minted { font-family: var(--font-mono); font-size: 11px; background: var(--bg-2); border: 1px solid var(--ok, #3a5); border-radius: 6px; padding: 6px 8px; margin: 6px 0; word-break: break-all; }
.weir-auth-overlay { position: fixed; inset: 0; z-index: 100; display: grid; place-items: center;
  background: rgba(10, 12, 16, .92); backdrop-filter: blur(3px); }
.weir-auth-card { width: 360px; max-width: 92vw; }
"#;
