//! HTTP helpers: the auth header, tenant route rewriting, and the authed
//! GET / mutation wrappers every view uses.

use crate::helpers::parse_props;
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

/// True for a data call that a switched admin re-scopes to `/tenants/{id}/…`. Auth + the
/// tenant-admin surface itself are never re-scoped. Each re-scoped call must be listed in
/// `tenant_routes.txt`, which the server mirrors ([[WEIR-T-0217]]; tests on both sides).
pub(crate) fn is_tenant_scoped(url: &str) -> bool {
    !(url.starts_with("/auth") || url.starts_with("/tenants") || url.starts_with("/platform"))
}

/// Re-scope a data call to the active tenant: a switched admin routes through `/tenants/{id}/…`.
pub(crate) fn apath(url: &str) -> String {
    if !is_tenant_scoped(url) {
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
    parse_props(&schema)
}

/// Discover a source's streams — POST the config, get stream names. A failure is the
/// server's reason, for the form to show ([[WEIR-T-0218]]).
pub(crate) async fn fetch_streams(plugin: &str, config: &str) -> Result<Vec<String>, String> {
    let req = areq_post(&format!("/connectors/{plugin}/discover"))
        .body(config.to_string())
        .map_err(|e| e.to_string())?;
    let resp = check(req.send().await).await?;
    resp.json::<Vec<String>>().await.map_err(|e| format!("bad response: {e}"))
}

#[cfg(test)]
mod tests {
    use super::is_tenant_scoped;
    use std::collections::BTreeSet;
    use std::path::Path;

    /// `METHOD /path` with each `{param}` folded to `{}`, so `{n}` and `{name}` compare equal.
    fn norm(method: &str, url: &str) -> String {
        let path = url.split('?').next().unwrap_or(url);
        let mut out = String::new();
        let mut in_param = false;
        for c in path.chars() {
            match c {
                '{' => {
                    in_param = true;
                    out.push_str("{}");
                }
                '}' => in_param = false,
                _ if !in_param => out.push(c),
                _ => {}
            }
        }
        format!("{method} {out}")
    }

    fn listed() -> BTreeSet<String> {
        include_str!("tenant_routes.txt")
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| {
                let (m, p) = l.split_once(' ').expect("METHOD PATH");
                norm(m, p.trim())
            })
            .collect()
    }

    /// The URL literal at the start of a helper's argument (`"…"`, `format!("…")`, `&format!("…")`).
    fn url_literal(arg: &str) -> Option<&str> {
        let arg = arg.trim_start();
        let arg = arg
            .strip_prefix("&format!(")
            .or_else(|| arg.strip_prefix("format!("))
            .unwrap_or(arg);
        let lit = arg.strip_prefix('"')?;
        Some(&lit[..lit.find('"')?])
    }

    /// Every re-scoped URL literal passed to an authed request helper in the UI source.
    fn called(dir: &Path, out: &mut BTreeSet<String>) {
        // (call prefix, HTTP method). A turbofish helper's argument starts after its next `(`.
        const HELPERS: [(&str, &str); 5] = [
            ("areq_get(", "GET"),
            ("areq_post(", "POST"),
            ("areq_delete(", "DELETE"),
            ("get_json::<", "GET"),
            ("get_fetch::<", "GET"),
        ];
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                called(&path, out);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            for (helper, method) in HELPERS {
                for (at, _) in src.match_indices(helper) {
                    if src[..at].ends_with("fn ") || src[..at].ends_with('"') {
                        continue; // the helper's own definition, or this table
                    }
                    let mut rest = &src[at + helper.len()..];
                    if helper.ends_with('<') {
                        let Some(open) = rest.find('(') else { continue };
                        rest = &rest[open + 1..];
                    }
                    if let Some(url) = url_literal(rest).filter(|u| is_tenant_scoped(u)) {
                        out.insert(norm(method, url));
                    }
                }
            }
        }
    }

    #[test]
    fn tenant_route_list_matches_the_calls_in_the_ui() {
        let mut calls = BTreeSet::new();
        called(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut calls);
        assert!(calls.len() > 5, "the scanner found too few calls: {calls:?}");
        let listed = listed();
        let missing: Vec<_> = calls.difference(&listed).collect();
        let stale: Vec<_> = listed.difference(&calls).collect();
        assert!(
            missing.is_empty(),
            "UI calls not in tenant_routes.txt (add them, and mirror them on the server): {missing:?}"
        );
        assert!(stale.is_empty(), "tenant_routes.txt lists calls the UI no longer makes: {stale:?}");
    }

    #[test]
    fn auth_tenants_and_platform_are_never_rescoped() {
        assert!(!is_tenant_scoped("/auth/me"));
        assert!(!is_tenant_scoped("/tenants/acme/keys"));
        assert!(!is_tenant_scoped("/platform/health"));
        assert!(is_tenant_scoped("/connections"));
    }
}
