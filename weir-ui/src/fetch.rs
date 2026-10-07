//! HTTP helpers: the auth header, tenant route rewriting, and the authed
//! GET / mutation wrappers every view uses.

use crate::models::Prop;

/// The stored API-key bearer (interim, [[WEIR-T-0084]]; the sign-in gate is [[WEIR-T-0087]]).
/// Reads `localStorage["weir_api_key"]`; empty until a key is set.
pub(crate) fn bearer() -> String {
    let key = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("weir_api_key").ok().flatten())
        .unwrap_or_default();
    format!("Bearer {key}")
}

/// The tenant a platform-admin has switched to ([[WEIR-T-0095]]) — `None` = own/`default` scope.
/// Held in localStorage so it survives the reload that re-scopes the views.
pub(crate) fn active_tenant() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("weir_active_tenant").ok().flatten())
        .filter(|t| !t.is_empty())
}

/// Re-scope a data call to the active tenant: a switched admin routes through `/tenants/{id}/…`.
/// Auth + the tenant-admin surface itself are never re-scoped.
pub(crate) fn apath(url: &str) -> String {
    if url.starts_with("/auth") || url.starts_with("/tenants") || url.starts_with("/platform") {
        return url.to_string();
    }
    match active_tenant() {
        Some(tid) => format!("/tenants/{tid}{url}"),
        None => url.to_string(),
    }
}

// Auth-aware gloo request builders — every API call carries the bearer + the active-tenant scope.
pub(crate) fn areq_get(url: &str) -> gloo_net::http::RequestBuilder {
    gloo_net::http::Request::get(&apath(url)).header("authorization", &bearer())
}
pub(crate) fn areq_post(url: &str) -> gloo_net::http::RequestBuilder {
    gloo_net::http::Request::post(&apath(url)).header("authorization", &bearer())
}
pub(crate) fn areq_delete(url: &str) -> gloo_net::http::RequestBuilder {
    gloo_net::http::Request::delete(&apath(url)).header("authorization", &bearer())
}

pub(crate) async fn get_json<T: serde::de::DeserializeOwned + Default>(url: String) -> T {
    match areq_get(&url).send().await {
        Ok(r) => r.json::<T>().await.unwrap_or_default(),
        Err(_) => T::default(),
    }
}

/// The outcome of an authed GET, with failure classes kept distinct ([[WEIR-T-0167]]) —
/// a 401 flips the sign-in gate, everything else feeds the error banner instead of
/// masquerading as an empty dashboard.
pub(crate) enum Fetched<T> {
    Ok(T),
    Unauthorized,
    /// HTTP error or a decode failure: (status, server's error message).
    Failed(u16, String),
    /// No answer at all.
    Network,
}

pub(crate) async fn get_fetch<T: serde::de::DeserializeOwned>(url: String) -> Fetched<T> {
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
pub(crate) async fn server_error(resp: gloo_net::http::Response) -> String {
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
pub(crate) async fn check(
    sent: Result<gloo_net::http::Response, gloo_net::Error>,
) -> Result<gloo_net::http::Response, String> {
    match sent {
        Err(_) => Err("server unreachable".into()),
        Ok(r) if r.ok() => Ok(r),
        Ok(r) => Err(server_error(r).await),
    }
}

/// Fetch a connector's spec + parse its `config_schema` into a flat field list.
pub(crate) async fn fetch_props(plugin: &str) -> Vec<Prop> {
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
pub(crate) async fn fetch_streams(plugin: &str, config: &str) -> Vec<String> {
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
