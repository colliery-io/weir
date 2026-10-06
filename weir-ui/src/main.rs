//! weir control-plane UI — Leptos (CSR) on the Colliery Aurora design system
//! ([[WEIR-A-0035]]), light and dark ([[WEIR-T-0191]]). Aurora ships the chrome;
//! weir supplies the data + vocabulary.
//! Shell + Operations + Setup ([[WEIR-T-0079]]/[[WEIR-T-0080]]/[[WEIR-T-0081]]).

use aurora_leptos::components::*;
use aurora_leptos::theme::{provide_theme, ThemeToggle};
use aurora_leptos::tokens::token;
use aurora_leptos::widgets::Banner;
use aurora_leptos::AuroraStyles;
use leptos::prelude::*;
use std::collections::HashSet;
use std::sync::Arc;

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
    // Light / dark / system ([[WEIR-T-0191]]); THEME_INIT_SCRIPT in index.html sets the first paint.
    provide_theme();
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
    // Aurora toasts: one queue at the root, one ToastStack in the view (6s, click to dismiss).
    let toaster = provide_toaster();
    let flash = move |ok: bool, msg: String| {
        if ok {
            toaster.success(msg);
        } else {
            toaster.error(msg);
        }
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


    // Confirm-destroy ([[WEIR-T-0191]]): delete + revoke go through Aurora's ConfirmDialog.
    let del_open = RwSignal::new(false);
    let del_name = RwSignal::new(String::new());
    let ask_delete = Callback::new(move |n: String| {
        del_name.set(n);
        del_open.set(true);
    });
    let revoke_open = RwSignal::new(false);
    let revoke_id = RwSignal::new(String::new());

    // Only a definite "not signed in" swaps the app for the sign-in screen; the first
    // probe (None) keeps the shell, so it does not re-render on None → Some(true).
    let needs_signin = Memo::new(move |_| authed.get() == Some(false));

    view! {
        <AuroraStyles/>
        <style>{LAYOUT_CSS}</style>
        <ToastStack duration_ms=6000/>
        {move || if needs_signin.get() {
            // Sign-in gate ([[WEIR-T-0087]]): Aurora's centred sign-in card.
            view! {
                <CenterScreen>
                    <AuthCard title="Sign in to weir" sub="Authenticate to continue."
                        brand=Box::new(|| view! { <Brand/> }.into_any())>
                        <Stack gap="sm">
                            <Button on_click=Callback::new(move |_| {
                                if let Some(w) = web_sys::window() {
                                    let _ = w.location().set_href("/auth/login");
                                }
                            })>"Sign in with OIDC"</Button>
                            <Divider/>
                            <form class="weir-form" on:submit=move |ev: leptos::ev::SubmitEvent| {
                                ev.prevent_default();
                                use_key();
                            }>
                                <PasswordInput label="Or paste an API key" placeholder="weirk_…"
                                    value=key_input autocomplete="off"/>
                                <Button variant="default" button_type="submit">"Use API key"</Button>
                            </form>
                        </Stack>
                    </AuthCard>
                </CenterScreen>
            }.into_any()
        } else {
            view! {
                <AppShell
                    brand=Arc::new(|| view! {
                        <Brand/>
                        <Text size="xs" dimmed=true mono=true>"control plane"</Text>
                    }.into_any())
                    header=Box::new(move || view! {
                        <Group justify="end" gap="sm" wrap=true>
                            <span class="weir-hide-narrow">
                                <Text size="xs" dimmed=true mono=true>{move || format!(
                                    "{} runs · {} rows", runs.get().len(),
                                    runs.get().iter().map(|r| r.rows_written).sum::<i64>())}</Text>
                            </span>
                            // Tenant context ([[WEIR-T-0095]]): a platform-admin gets a switcher; others a chip.
                            // An inline top-bar control with no visible label: `aria_label` names it.
                            {move || if is_admin.get() {
                                let current = RwSignal::new(active_tenant().unwrap_or_default());
                                let pairs = tenants.get().into_iter()
                                    .map(|(id, name)| { let label = format!("{name} · {id}"); (id, label) })
                                    .collect::<Vec<_>>();
                                view! {
                                    <span class="weir-tenant" title="view another tenant">
                                        <Select aria_label="Tenant" placeholder="⊙ self (default)"
                                            option_pairs=pairs value=current
                                            on_change=Callback::new(move |v: String| {
                                                if let Some(store) = local_storage() {
                                                    if v.is_empty() { let _ = store.remove_item("weir_active_tenant"); }
                                                    else { let _ = store.set_item("weir_active_tenant", &v); }
                                                }
                                                if let Some(w) = web_sys::window() { let _ = w.location().reload(); }
                                            })/>
                                    </span>
                                }.into_any()
                            } else {
                                view! { <Pill color=token::MUTED>{move || format!("⊙ {}", my_tenant.get())}</Pill> }.into_any()
                            }}
                            {move || {
                                let mut opts = vec!["Operations".to_string(), "Health".to_string(), "Setup".to_string()];
                                if is_admin.get() { opts.insert(2, "Platform".to_string()); }
                                view! { <SegmentedControl options=opts value=view/> }
                            }}
                            {move || is_admin.get().then(|| view! {
                                <Button variant="subtle" size="xs" title="administer tenants"
                                    on_click=Callback::new(move |_| { reload_tenants(); show_tenants.set(true); })>
                                    "Tenants"
                                </Button>
                            })}
                            {move || view! {
                                <Button variant="subtle" size="xs" title=format!("signed in as {}", signed_in_as.get())
                                    on_click=Callback::new(move |_| sign_out())>"Sign out"</Button>
                            }}
                            <ThemeToggle/>
                        </Group>
                    }.into_any())
                >
                    <div class="weir-page">
                        // Degraded-control-plane banner ([[WEIR-T-0167]]): persistent while the poll fails;
                        // sticky under the top bar so it stays in sight.
                        {move || api_error.get().map(|msg| view! {
                            <div class="weir-alert-slot" data-testid="api-error"
                                title="the dashboards show last-known data until this clears">
                                <Banner color=token::BAD>{format!("control plane error — {msg}")}</Banner>
                            </div>
                        })}
                        {move || match view.get().as_str() {
                            "Setup" => setup_view(SetupState {
                                catalog, available, props, streams, add_pkg, add_manifest, add_path, preview,
                                name, src, dst, stream, config, every, exec_mode,
                                onboard_pick: Callback::new(move |_| onboard_pick()),
                                do_preview: Callback::new(move |_| do_preview()),
                                onboard_byo: Callback::new(move |_| onboard_byo()),
                                save_conn: Callback::new(move |_| save_conn()),
                            }),
                            "Health" => health_view(health, open_detail),
                            "Platform" => platform_view(platform, drill_tenant),
                            _ => operations_view(connections, runs, ConnActions {
                                on_open: open_detail, on_run: run_conn, on_delete: ask_delete,
                                on_start: start_conn, on_stop: stop_conn,
                            }),
                        }}
                    </div>

                    <Modal open=detail_open title="Run detail" size="lg">
                        <Text bold=true mono=true>{move || selected.get()}</Text>
                        // Lineage ([[WEIR-T-0101]]): source · stream → dest + rows/duration, from the run data.
                        <SectionLabel label="Lineage" divider=true/>
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
                        <SectionLabel label="Schema" divider=true/>
                        {move || {
                            let sv = detail_schema.get();
                            let name = selected.get();
                            let drift = sv.broken.clone().map(|reason| {
                                let n = name.clone();
                                view! {
                                    <Alert title="Schema drift" color=token::BAD>
                                        <Stack gap="xs">
                                            <Text size="sm">{reason}</Text>
                                            <div>
                                                <Button variant="default" size="xs"
                                                    on_click=Callback::new(move |_| accept_schema.run(n.clone()))>
                                                    "Accept new schema"
                                                </Button>
                                            </div>
                                        </Stack>
                                    </Alert>
                                }
                            });
                            let body = if sv.fields.is_empty() {
                                view! { <Text size="sm" dimmed=true>"No schema captured yet."</Text> }.into_any()
                            } else {
                                view! {
                                    <DetailList mono=true>
                                        {sv.fields.into_iter().map(|f| {
                                            let opt = if f.nullable { "nullable" } else { "required" };
                                            view! { <KeyValue label=f.name>{format!("{} · {opt}", f.ty)}</KeyValue> }
                                        }).collect_view()}
                                    </DetailList>
                                }.into_any()
                            };
                            view! { <Stack gap="sm">{drift}{body}</Stack> }.into_any()
                        }}
                        <SectionLabel label="Dead-letters" count=Signal::derive(move || Some(detail_dls.get().len())) divider=true/>
                        {move || {
                            let dls = detail_dls.get();
                            if dls.is_empty() {
                                view! { <Text size="sm" dimmed=true>"None."</Text> }.into_any()
                            } else {
                                view! {
                                    <Table mono=true fixed=true widths=vec!["38%".into(), "62%".into()] label="Dead-letters">
                                        <thead><tr><th>"reason"</th><th>"record"</th></tr></thead>
                                        <tbody>
                                            {dls.into_iter().map(|d| view! {
                                                <tr><td title=d.reason.clone()>{d.reason.clone()}</td><td title=d.record.clone()>{d.record.clone()}</td></tr>
                                            }).collect_view()}
                                        </tbody>
                                    </Table>
                                }.into_any()
                            }
                        }}
                        <SectionLabel label="Logs" divider=true/>
                        <LogView label="Logs" empty="No logs." max_height="260px"
                            lines=Signal::derive(move || detail_logs.get().into_iter()
                                .map(|l| LogLine::new(l.message).level_color(l.level.clone(), log_color(&l.level)))
                                .collect::<Vec<_>>())/>
                    </Modal>

                    // Tenants admin ([[WEIR-T-0096]]) — platform-admin CRUD tenants + their keys.
                    <Modal open=show_tenants title="Tenants" size="lg" close_on_scrim=false on_close=Callback::new(move |_| minted_key.set(None))>
                        <Stack>
                            <Text size="sm" dimmed=true>"Administer tenants + their keys."</Text>
                            <TextInput label="New tenant id" placeholder="acme" value=new_tenant_id mono=true/>
                            <div><Button on_click=Callback::new(move |_| create_tenant())>"Create tenant"</Button></div>
                            <Table label="Tenants">
                                <thead><tr><th>"tenant"</th><th>"name"</th><th></th></tr></thead>
                                <tbody>
                                    {move || tenants.get().into_iter().map(|(id, name)| {
                                        let (id2, id3) = (id.clone(), id.clone());
                                        view! {
                                            <TableRow label=format!("keys of {id}") selected=Signal::derive(move || sel_tenant.get() == id3)
                                                on_click=Callback::new(move |_| { minted_key.set(None); load_keys(id2.clone()); })>
                                                <td><Code>{id.clone()}</Code></td>
                                                <td>{name}</td>
                                                <td class="cl-num"><Text size="xs" dimmed=true>"Keys →"</Text></td>
                                            </TableRow>
                                        }
                                    }).collect_view()}
                                </tbody>
                            </Table>
                            {move || (!sel_tenant.get().is_empty()).then(|| view! {
                                <SectionLabel label=format!("Keys · {}", sel_tenant.get()) divider=true/>
                                {move || minted_key.get().map(|k| view! {
                                    <SecretReveal secret=k label="New API key"
                                        on_done=Callback::new(move |_| minted_key.set(None))/>
                                })}
                                <TextInput label="New key name" placeholder="ci" value=new_key_name/>
                                <div><Button on_click=Callback::new(move |_| mint_key())>"Mint key"</Button></div>
                                <Table label="Keys">
                                    <thead><tr><th>"name"</th><th>"role"</th><th>"state"</th><th></th></tr></thead>
                                    <tbody>
                                        {move || tenant_keys.get().is_empty().then(|| view! {
                                            <TableEmpty message="No keys yet." colspan=4/>
                                        })}
                                        {move || tenant_keys.get().into_iter().map(|(kid, name, role, revoked)| {
                                            view! { <tr>
                                                <td>{name}</td><td>{role}</td>
                                                <td>{if revoked {
                                                    view! { <Pill color=token::MUTED>"revoked"</Pill> }.into_any()
                                                } else {
                                                    view! { <Pill color=token::OK>"active"</Pill> }.into_any()
                                                }}</td>
                                                <td class="cl-num">{(!revoked).then(|| view! {
                                                    <Button variant="subtle" size="xs" bad=true
                                                        on_click=Callback::new(move |_| { revoke_id.set(kid.clone()); revoke_open.set(true); })>
                                                        "Revoke"
                                                    </Button>
                                                })}</td>
                                            </tr> }
                                        }).collect_view()}
                                    </tbody>
                                </Table>
                            })}
                        </Stack>
                    </Modal>

                    <ConfirmDialog open=del_open title=Signal::derive(move || format!("Delete {}?", del_name.get())) confirm_label="Delete"
                        message="This removes the connection and stops its schedule. Its run history stays."
                        on_confirm=Callback::new(move |_| { del_conn.run(del_name.get_untracked()); del_open.set(false); })>
                        <Text mono=true bold=true>{move || del_name.get()}</Text>
                    </ConfirmDialog>
                    <ConfirmDialog open=revoke_open title=Signal::derive(move || format!("Revoke key {}?", revoke_id.get())) confirm_label="Revoke"
                        message="A revoked key stops working at once. This cannot be undone."
                        on_confirm=Callback::new(move |_| { revoke_key(revoke_id.get_untracked()); revoke_open.set(false); })/>
                </AppShell>
            }.into_any()
        }}
    }
}

/// The weir mark: the gradient glyph + wordmark (brand stays in the product).
#[component]
fn Brand() -> impl IntoView {
    view! {
        <span class="weir-brand">
            <span class="weir-glyph">"≋"</span>
            <span class="weir-wordmark">"weir"</span>
        </span>
    }
}

/// Log level → an Aurora hue token.
fn log_color(level: &str) -> &'static str {
    match level.to_ascii_lowercase().as_str() {
        "error" => token::BAD,
        "warn" | "warning" => token::GOLD,
        "info" => token::ICE,
        _ => token::MUTED,
    }
}

// ---------------------------------------------------------------------------- views

/// The Setup view's signals + actions.
struct SetupState {
    catalog: RwSignal<Vec<CatalogItem>>,
    available: RwSignal<Vec<AvailableItem>>,
    props: RwSignal<Vec<Prop>>,
    streams: RwSignal<Vec<String>>,
    add_pkg: RwSignal<String>,
    add_manifest: RwSignal<String>,
    add_path: RwSignal<String>,
    preview: RwSignal<Option<PreviewReport>>,
    name: RwSignal<String>,
    src: RwSignal<String>,
    dst: RwSignal<String>,
    stream: RwSignal<String>,
    config: RwSignal<String>,
    every: RwSignal<String>,
    exec_mode: RwSignal<String>,
    onboard_pick: Callback<()>,
    do_preview: Callback<()>,
    onboard_byo: Callback<()>,
    save_conn: Callback<()>,
}

fn setup_view(s: SetupState) -> AnyView {
    let SetupState {
        catalog, available, props, streams, add_pkg, add_manifest, add_path, preview,
        name, src, dst, stream, config, every, exec_mode,
        onboard_pick, do_preview, onboard_byo, save_conn,
    } = s;
    // Picker options (raw_name, friendly), onboarded dropped.
    let cat = catalog.get();
    let avail = available.get();
    let registered: HashSet<String> = cat.iter().map(|c| c.name.clone()).collect();
    let mut opts: Vec<(String, String)> = Vec::new();
    for want in ["manifest", "dest-manifest", "other"] {
        for p in avail.iter().filter(|p| {
            let k = if want == "other" { p.kind != "manifest" && p.kind != "dest-manifest" } else { p.kind == want };
            k && !is_onboarded(p, &registered)
        }) {
            opts.push((p.name.clone(), friendly(p)));
        }
    }
    let versioned = |c: &CatalogItem| (c.name.clone(), format!("{} · {}", c.name, c.version));
    let sources: Vec<(String, String)> =
        cat.iter().filter(|c| c.roles.iter().any(|r| r == "Source")).map(versioned).collect();
    let dests: Vec<(String, String)> = cat
        .iter()
        .filter(|c| c.roles.iter().any(|r| r == "Destination" || r == "ReverseEtl"))
        .map(versioned)
        .collect();
    let prop_list = props.get();
    let stream_list = streams.get();
    let modes = vec![
        ("run_once".to_string(), "run once · scheduled / batch".to_string()),
        ("resident".to_string(), "resident · long-lived (Start/Stop; every = cadence)".to_string()),
    ];

    view! {
        <Panel title="Add a connector" caption="discover · onboard">
            <div class="weir-form">
                <Select label="Pick a connector" placeholder="— select a connector —" option_pairs=opts value=add_pkg/>
                <div><Button on_click=onboard_pick>"Onboard"</Button></div>
            </div>
        </Panel>

        <Panel title="Bring your own" caption="paste a manifest · or a crate path">
            <div class="weir-form">
                <Textarea label="Paste a declarative manifest (YAML)" rows=6 mono=true
                    placeholder="type: DeclarativeSource\nstreams:\n  - ..." value=add_manifest/>
                <TextInput label="Or a crate path (full-code)" placeholder="/path/to/weir-connector-foo"
                    value=add_path mono=true/>
                <Group gap="sm">
                    <Button variant="default" on_click=do_preview>"Preview"</Button>
                    <Button on_click=onboard_byo>"Onboard"</Button>
                </Group>
                {move || preview.get().map(|rep| view! {
                    <Alert title="Preview" color=token::ICE>
                        <Text size="xs" mono=true>
                            {format!("tier {} · confidence {:.2} · streams {}", rep.tier, rep.confidence, rep.streams.len())}
                        </Text>
                        {if rep.unsupported.is_empty() {
                            view! { <Text size="xs" dimmed=true>"Fully supported by the runtime."</Text> }.into_any()
                        } else {
                            view! { <Text size="xs" dimmed=true>{format!("gaps: {}", rep.unsupported.join(", "))}</Text> }.into_any()
                        }}
                    </Alert>
                })}
            </div>
        </Panel>

        <Panel title="New / edit connection" caption="wire a source to a destination">
            <div class="weir-form">
                <TextInput label="Name" placeholder="my-sync" value=name mono=true/>
                <SimpleGrid cols=2>
                    <Select label="Source" placeholder="— source —" option_pairs=sources value=src/>
                    <Select label="Destination" placeholder="— destination —" option_pairs=dests value=dst/>
                </SimpleGrid>
                <SimpleGrid cols=2>
                    {if stream_list.is_empty() {
                        view! { <TextInput label="Stream" value=stream mono=true/> }.into_any()
                    } else {
                        view! { <Select label="Stream" placeholder="— stream —" options=stream_list value=stream/> }.into_any()
                    }}
                    {move || {
                        let label = if exec_mode.get() == "resident" { "Every (secs) · emit cadence" } else { "Every (secs)" };
                        view! { <TextInput label=label placeholder="—" value=every mono=true/> }
                    }}
                </SimpleGrid>
                <Select label="Execution mode" option_pairs=modes value=exec_mode/>
                {(!prop_list.is_empty()).then(|| view! {
                    <SectionLabel label=format!("config · {} contract", src.get_untracked()) divider=true/>
                    <SimpleGrid cols=2>
                        {prop_list.into_iter().map(|p| config_field(p, config)).collect_view()}
                    </SimpleGrid>
                })}
                <Textarea label="Config (JSON)" rows=3 mono=true value=config/>
                <div><Button on_click=save_conn>"Save connection"</Button></div>
            </div>
        </Panel>
    }
    .into_any()
}

/// One field of a connector's config contract, bound both ways to the config JSON.
fn config_field(p: Prop, config: RwSignal<String>) -> impl IntoView {
    let field = RwSignal::new(cfg_get(&config.get_untracked(), &p.key));
    let (k_sync, k_set, kind_sync, kind_set) = (p.key.clone(), p.key.clone(), p.kind.clone(), p.kind.clone());
    // The JSON textarea can change the config too: follow it, unless the config already
    // holds what this field's text means (so typing "1." is not rewritten to "1").
    Effect::new(move |_| {
        let now = cfg_get(&config.get(), &k_sync);
        let mine = field.get_untracked();
        if now != cfg_get(&cfg_set("{}", &k_sync, &mine, &kind_sync), &k_sync) {
            field.set(now);
        }
    });
    let on_input = Callback::new(move |v: String| {
        config.set(cfg_set(&config.get_untracked(), &k_set, &v, &kind_set));
    });
    let input_type = if p.secret { "password" } else { "text" };
    view! { <TextInput label=p.key placeholder=p.kind value=field input_type=input_type on_input=on_input mono=true/> }
}

fn health_view(health: RwSignal<Vec<ConnHealth>>, open_detail: Callback<String>) -> AnyView {
    view! {
        <Panel title="Needs attention" caption="amber + red connections, worst first">
            {move || {
                let mut hs = health.get();
                hs.sort_by_key(|h| health_rank(&h.status));
                let attn: Vec<ConnHealth> = hs.into_iter()
                    .filter(|h| h.status == "red" || h.status == "amber").collect();
                if attn.is_empty() {
                    view! { <Text size="sm" dimmed=true>"All connections healthy."</Text> }.into_any()
                } else {
                    view! {
                        <Table mono=true label="Needs attention">
                            <thead><tr><th>"status"</th><th>"connection"</th><th class="cl-num">"lag"</th><th class="cl-num">"errors"</th><th class="cl-num">"dead"</th></tr></thead>
                            <tbody>
                                {attn.into_iter().map(|h| {
                                    let name = h.connection.clone();
                                    view! {
                                        <TableRow label=h.connection.clone() on_click=Callback::new(move |_| open_detail.run(name.clone()))>
                                            <td><Pill color=health_color(&h.status)>{h.status.clone()}</Pill></td>
                                            <td>{h.connection.clone()}</td>
                                            <td class="cl-num">{fmt_lag(h.lag_ms)}</td>
                                            <td class="cl-num">{format!("{:.0}%", h.error_rate * 100.0)}</td>
                                            <td class="cl-num">{h.dead_letters.to_string()}</td>
                                        </TableRow>
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
                    view! { <Text size="sm" dimmed=true>"No connections yet — add one in Setup."</Text> }.into_any()
                } else {
                    view! {
                        <SimpleGrid cols=3>
                            {hs.into_iter().map(|h| {
                                let name = h.connection.clone();
                                let spark: Vec<f64> = h.throughput.iter().map(|v| *v as f64).collect();
                                let spark_label = format!("{} throughput", h.connection);
                                view! {
                                    <Card label=h.connection.clone() on_click=Callback::new(move |_| open_detail.run(name.clone()))
                                        attr:data-testid="health-card">
                                        <Stack gap="xs">
                                            <Group gap="sm">
                                                <Pill color=health_color(&h.status)>{h.status.clone()}</Pill>
                                                <Text bold=true mono=true>{h.connection.clone()}</Text>
                                            </Group>
                                            <Group gap="sm" wrap=true>
                                                <Text size="xs" dimmed=true mono=true>{format!("lag {}", fmt_lag(h.lag_ms))}</Text>
                                                <Text size="xs" dimmed=true mono=true>{format!("err {:.0}%", h.error_rate * 100.0)}</Text>
                                                <Text size="xs" dimmed=true mono=true>{format!("dl {}", h.dead_letters)}</Text>
                                                <Text size="xs" dimmed=true mono=true>{format!("{} rows", h.rows_recent)}</Text>
                                            </Group>
                                            {(!spark.is_empty()).then(|| view! {
                                                <Sparkline values=spark fluid=true height=22.0 color=token::ICE
                                                    label=spark_label.clone()/>
                                            })}
                                        </Stack>
                                    </Card>
                                }
                            }).collect_view()}
                        </SimpleGrid>
                    }.into_any()
                }
            }}
        </Panel>
    }
    .into_any()
}

fn platform_view(platform: RwSignal<PlatformHealth>, drill_tenant: Callback<String>) -> AnyView {
    view! {
        <SimpleGrid cols=3>
            <StatTile label="Tenants" value=Signal::derive(move || platform.get().tenants.len().to_string())/>
            <StatTile label="Active" value=Signal::derive(move || platform.get().active_tenants.to_string()) color=token::OK/>
            <StatTile label="Queue depth" value=Signal::derive(move || platform.get().total_queue_depth.to_string()) sub="runs waiting, all tenants"/>
        </SimpleGrid>
        <Panel title="Tenant health" caption="worst first · click to drill in">
            {move || {
                let mut ts = platform.get().tenants;
                ts.sort_by_key(|t| health_rank(&t.status));
                if ts.is_empty() {
                    view! { <Text size="sm" dimmed=true>"No tenants."</Text> }.into_any()
                } else {
                    view! {
                        <SimpleGrid cols=3>
                            {ts.into_iter().map(|t| {
                                let tid = t.tenant.clone();
                                view! {
                                    <Card label=t.tenant.clone() on_click=Callback::new(move |_| drill_tenant.run(tid.clone()))
                                        attr:data-testid="tenant-card">
                                        <Stack gap="xs">
                                            <Group gap="sm">
                                                <Pill color=health_color(&t.status)>{t.status.clone()}</Pill>
                                                <Text bold=true mono=true>{t.tenant.clone()}</Text>
                                            </Group>
                                            <Group gap="sm" wrap=true>
                                                <Text size="xs" dimmed=true mono=true>{format!("{} conns", t.connections)}</Text>
                                                <Text size="xs" dimmed=true mono=true>{format!("{} failing", t.needs_attention)}</Text>
                                                <Text size="xs" dimmed=true mono=true>{format!("dl {}", t.dead_letters)}</Text>
                                                <Text size="xs" dimmed=true mono=true>{format!("queue {}", t.queue_depth)}</Text>
                                            </Group>
                                        </Stack>
                                    </Card>
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
                    view! { <Text size="sm" dimmed=true>"All tenants healthy."</Text> }.into_any()
                } else {
                    view! {
                        <Table mono=true label="Needs attention">
                            <thead><tr><th>"status"</th><th>"tenant"</th><th>"connection"</th></tr></thead>
                            <tbody>
                                {attn.into_iter().map(|a| {
                                    let tid = a.tenant.clone();
                                    view! {
                                        <TableRow label=format!("{} · {}", a.tenant, a.connection)
                                            on_click=Callback::new(move |_| drill_tenant.run(tid.clone()))>
                                            <td><Pill color=health_color(&a.status)>{a.status.clone()}</Pill></td>
                                            <td>{a.tenant.clone()}</td>
                                            <td>{a.connection.clone()}</td>
                                        </TableRow>
                                    }
                                }).collect_view()}
                            </tbody>
                        </Table>
                    }.into_any()
                }
            }}
        </Panel>
    }
    .into_any()
}

/// What a connection card can do.
#[derive(Clone, Copy)]
struct ConnActions {
    on_open: Callback<String>,
    on_run: Callback<String>,
    on_delete: Callback<String>,
    on_start: Callback<String>,
    on_stop: Callback<String>,
}

fn operations_view(
    connections: RwSignal<Vec<Connection>>,
    runs: RwSignal<Vec<RunRow>>,
    actions: ConnActions,
) -> AnyView {
    view! {
        <Panel title="Connections" caption="live runs">
            {move || {
                let rs = runs.get();
                let cs = connections.get();
                if cs.is_empty() {
                    view! { <Text size="sm" dimmed=true>"No connections yet — add one in Setup."</Text> }.into_any()
                } else {
                    view! {
                        <SimpleGrid cols=3>
                            {cs.into_iter().map(|c| {
                                let last = latest_run(&rs, &c.name);
                                view! { <ConnectionCard conn=c last=last actions=actions/> }
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
                    view! { <Text size="sm" dimmed=true>"No runs yet."</Text> }.into_any()
                } else {
                    view! {
                        <Table mono=true fixed=true widths=vec!["190px".into(), "22%".into(), "110px".into(), "auto".into()]
                            label="Run feed" min_width="680px">
                            <thead><tr><th>"#"</th><th>"connection"</th><th>"state"</th><th>"detail"</th></tr></thead>
                            <tbody>
                                {rs.into_iter().map(|r| {
                                    let name = r.connection.clone();
                                    let metrics = run_metrics(&r);
                                    view! {
                                        <TableRow label=format!("run {} · {}", r.id, r.connection)
                                            on_click=Callback::new(move |_| actions.on_open.run(name.clone()))>
                                            <td>{r.id}</td>
                                            <td>{r.connection.clone()}</td>
                                            <td><Pill color=state_color(&r.state)>{r.state.clone()}</Pill></td>
                                            <td title=metrics.clone()>{metrics.clone()}</td>
                                        </TableRow>
                                    }
                                }).collect_view()}
                            </tbody>
                        </Table>
                    }.into_any()
                }
            }}
        </Panel>
    }
    .into_any()
}

#[component]
fn ConnectionCard(conn: Connection, last: Option<RunRow>, actions: ConnActions) -> impl IntoView {
    let state = last.as_ref().map(|r| r.state.clone()).unwrap_or_else(|| "idle".to_string());
    // The run id is a long snowflake: it goes in the tooltip, the metrics stay in sight.
    let (when, when_title) = match &last {
        Some(r) => (run_metrics(r), format!("run #{}", r.id)),
        None => ("no runs yet".to_string(), String::new()),
    };
    // F1 ([[WEIR-I-0035]]): a resident source shows a live/stopped badge + Start/Stop instead of Run.
    // "live" = it holds an active (leased/pending) run; otherwise it's stopped.
    let resident = conn.execution_mode == "resident";
    let live = matches!(state.as_str(), "leased" | "pending");
    let (n_open, n_run, n_del, n_start, n_stop) = (
        conn.name.clone(), conn.name.clone(), conn.name.clone(), conn.name.clone(), conn.name.clone(),
    );
    let (pill_color, pill_label) = if resident {
        if live { (token::OK, "resident • live".to_string()) } else { (token::MUTED, "resident • stopped".to_string()) }
    } else {
        (state_color(&state), state.clone())
    };
    view! {
        <Card label=conn.name.clone() on_click=Callback::new(move |_| actions.on_open.run(n_open.clone()))
            attr:data-testid="connection-card">
            <Stack gap="xs">
                <Group justify="between" gap="sm">
                    <Text bold=true mono=true>{conn.name.clone()}</Text>
                    <Pill color=pill_color>{pill_label}</Pill>
                </Group>
                <Group gap="sm">
                    <Text size="sm" mono=true>{conn.source.clone()}</Text>
                    <span class="weir-arr" aria-label="to">"──▶"</span>
                    <Text size="sm" mono=true>{conn.dest.clone()}</Text>
                </Group>
                <Text size="xs" dimmed=true mono=true>{format!("stream · {}", conn.stream)}</Text>
                <Group justify="between" gap="sm">
                    <span title=when_title><Text size="xs" dimmed=true mono=true>{when}</Text></span>
                    <Group gap="xs">
                        <Button variant="default" size="xs" bad=true stop_propagation=true
                            on_click=Callback::new(move |_| actions.on_delete.run(n_del.clone()))>"Delete"</Button>
                        {if resident {
                            view! {
                                <Button variant="default" size="xs" stop_propagation=true
                                    on_click=Callback::new(move |_| actions.on_stop.run(n_stop.clone()))>"Stop"</Button>
                                <Button size="xs" stop_propagation=true
                                    on_click=Callback::new(move |_| actions.on_start.run(n_start.clone()))>"Start"</Button>
                            }.into_any()
                        } else {
                            view! {
                                <Button size="xs" stop_propagation=true
                                    on_click=Callback::new(move |_| actions.on_run.run(n_run.clone()))>"Run"</Button>
                            }.into_any()
                        }}
                    </Group>
                </Group>
            </Stack>
        </Card>
    }
}

/// What Aurora does not do for weir: the brand mark, the page width, the sticky error
/// slot, the connection-flow arrow and the form rhythm. Every colour is an Aurora token.
const LAYOUT_CSS: &str = r#"
.weir-brand { display: inline-flex; align-items: baseline; gap: 6px; }
.weir-glyph, .weir-wordmark { font-family: var(--font-mono); font-weight: 700;
  background: var(--aurora-3); -webkit-background-clip: text; background-clip: text; color: transparent; }
.weir-glyph { font-size: 19px; } .weir-wordmark { font-size: 20px; letter-spacing: .05em; }
.weir-page { max-width: 1180px; margin: 0 auto; display: grid; gap: var(--space-lg); }
/* grid items keep the page width: a wide table scrolls in its own box. */
.weir-page > * { min-width: 0; }
.weir-alert-slot { position: sticky; top: var(--cl-header-h); z-index: 30; }
.weir-arr { color: var(--ice); font-family: var(--font-mono); font-size: var(--fs-xs); }
.weir-form { display: grid; gap: var(--space-md); }
.weir-tenant .cl-select { width: auto; max-width: 220px; height: var(--h-xs); font-size: var(--fs-xs); }
@media (max-width: 768px) {
  .cl-appshell__header { flex-wrap: wrap; }
  .cl-appshell__header-content { flex-basis: 100%; }
  .weir-hide-narrow { display: none; }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_html_carries_the_theme_init_script() {
        let html = include_str!("../index.html");
        assert!(html.contains(aurora_leptos::THEME_INIT_SCRIPT), "index.html must inline THEME_INIT_SCRIPT");
        assert!(html.contains(r#"name="color-scheme" content="light dark""#));
    }

    #[test]
    fn layout_css_uses_only_defined_tokens_and_no_raw_colour() {
        for bad in ["--bg-2", "--fg-1", "--radius-sm4", "rgba(", "rgb(", "#"] {
            assert!(!LAYOUT_CSS.contains(bad), "LAYOUT_CSS contains {bad}");
        }
    }

    #[test]
    fn status_colours_are_theme_tokens() {
        for s in ["done", "leased", "failed", "pending", "idle"] {
            assert!(state_color(s).starts_with("var(--"));
        }
        for s in ["green", "amber", "red", "unknown"] {
            assert!(health_color(s).starts_with("var(--"));
        }
        assert_eq!(log_color("ERROR"), token::BAD);
    }

    #[test]
    fn config_field_round_trip_keeps_partial_numbers_stable() {
        let cfg = cfg_set("{}", "rate", "1.5", "number");
        assert_eq!(cfg_get(&cfg, "rate"), "1.5");
        assert_eq!(cfg_get(&cfg_set("{}", "port", "", "integer"), "port"), "");
        assert_eq!(cfg_get(&cfg_set("{}", "flag", "true", "boolean"), "flag"), "true");
    }
}
