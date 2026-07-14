//! Authorization seam ([[WEIR-A-0008]] / [[WEIR-T-0085]]) — mirrors cloacina `routes/authz.rs`.
//! A **total, default-deny** route table: `(Method, matched-path) -> Access`; a route **absent** from
//! the table is denied (fail-closed). `evaluate` gates on role level (+ tenant scope, once weir has
//! tenant-scoped routes). This is the interface the periphery's RBAC engine replaces.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use axum::{
    Json,
    extract::{MatchedPath, Request, State},
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use weir_app::{App, AuthenticatedKey};

/// Role level, projected from a key's `permissions`. Ordered `Read < Write < Admin`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Read,
    Write,
    Admin,
}

impl Level {
    /// Fail-safe to least privilege on an unknown value.
    fn from_permissions(p: &str) -> Level {
        match p {
            "admin" => Level::Admin,
            "write" => Level::Write,
            _ => Level::Read,
        }
    }
}

/// The scope a route requires. weir has no tenant-scoped routes yet; `TenantParam` lands with them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// God-mode (`is_admin`) only.
    Platform,
    /// Any authenticated key (data is scoped by the handler).
    Any,
}

/// A route's access requirement.
#[derive(Clone, Copy, Debug)]
pub struct Access {
    pub scope: Scope,
    pub level: Level,
}

impl Access {
    const fn platform(level: Level) -> Access {
        Access {
            scope: Scope::Platform,
            level,
        }
    }
    const fn any(level: Level) -> Access {
        Access {
            scope: Scope::Any,
            level,
        }
    }
}

/// The authenticated subject projected into authz attributes.
pub struct Principal {
    pub role: Level,
    pub platform_admin: bool,
}

impl Principal {
    fn from_key(k: &AuthenticatedKey) -> Principal {
        Principal {
            role: Level::from_permissions(&k.permissions),
            platform_admin: k.is_admin,
        }
    }
}

/// Total, default-deny. God-mode passes everything; `Platform` is admin-only; `Any` needs `role >= level`.
pub fn evaluate(p: &Principal, scope: Scope, level: Level) -> bool {
    if p.platform_admin {
        return true;
    }
    match scope {
        Scope::Platform => false,
        Scope::Any => p.role >= level,
    }
}

type AuthzTable = HashMap<(Method, String), Access>;

static AUTHZ_TABLE: LazyLock<AuthzTable> = LazyLock::new(build_authz_table);

/// Classify every API route. A route missing here is **denied** — the critical property.
/// (`/health` + the SPA fallback are public, outside the auth layer, so they aren't listed.)
fn build_authz_table() -> AuthzTable {
    use Level::{Admin, Read, Write};
    let mut t = AuthzTable::new();
    let mut add = |m: Method, p: &str, a: Access| {
        t.insert((m, p.to_string()), a);
    };
    add(Method::GET, "/connections", Access::any(Read));
    add(Method::POST, "/connections", Access::any(Write));
    add(Method::GET, "/connections/{name}", Access::any(Read));
    add(Method::DELETE, "/connections/{name}", Access::any(Write));
    add(Method::POST, "/connections/{name}/run", Access::any(Write));
    add(
        Method::POST,
        "/connections/{name}/start",
        Access::any(Write),
    );
    add(Method::POST, "/connections/{name}/stop", Access::any(Write));
    add(Method::GET, "/connections/{name}/runs", Access::any(Read));
    add(
        Method::GET,
        "/connections/{name}/dead-letters",
        Access::any(Read),
    );
    add(Method::GET, "/connections/{name}/logs", Access::any(Read));
    add(Method::GET, "/connections/{name}/state", Access::any(Read));
    add(Method::GET, "/connections/{name}/schema", Access::any(Read));
    add(
        Method::POST,
        "/connections/{name}/schema/accept",
        Access::any(Write),
    );
    add(Method::GET, "/connectors/{plugin}/spec", Access::any(Read));
    add(
        Method::POST,
        "/connectors/{plugin}/discover",
        Access::any(Read),
    );
    add(Method::GET, "/catalog", Access::any(Read));
    add(Method::GET, "/catalog/available", Access::any(Read));
    add(Method::POST, "/catalog/import", Access::any(Write));
    add(Method::POST, "/catalog/preview", Access::any(Read));
    add(
        Method::DELETE,
        "/catalog/{name}/{version}",
        Access::any(Write),
    );
    add(Method::GET, "/runs", Access::any(Read));
    // Holistic ops health ([[WEIR-T-0110]]): own-tenant is `Any` (handler-scoped); the explicit
    // cross-tenant + the platform rollup are platform-admin.
    add(Method::GET, "/overview", Access::any(Read));
    add(
        Method::GET,
        "/tenants/{id}/overview",
        Access::platform(Admin),
    );
    add(Method::GET, "/platform/health", Access::platform(Admin));
    add(Method::GET, "/auth/me", Access::any(Read));
    // Tenant administration ([[WEIR-T-0090]]) — platform-admin only (`is_admin`), the explicit
    // cross-tenant surface ([[WEIR-A-0036]] decision 3). Data routes above stay `Any` + are
    // scoped to the caller's own tenant by their handlers.
    add(Method::GET, "/tenants", Access::platform(Admin));
    add(Method::POST, "/tenants", Access::platform(Admin));
    add(Method::DELETE, "/tenants/{id}", Access::platform(Admin));
    add(Method::GET, "/tenants/{id}/keys", Access::platform(Admin));
    add(Method::POST, "/tenants/{id}/keys", Access::platform(Admin));
    add(
        Method::DELETE,
        "/tenants/{id}/keys/{kid}",
        Access::platform(Admin),
    );
    // Admin cross-tenant data browse ([[WEIR-T-0094]]) — platform-admin acts on the path tenant.
    add(
        Method::GET,
        "/tenants/{id}/connections",
        Access::platform(Admin),
    );
    add(
        Method::GET,
        "/tenants/{id}/connections/{name}",
        Access::platform(Admin),
    );
    add(
        Method::POST,
        "/tenants/{id}/connections/{name}/run",
        Access::platform(Admin),
    );
    add(
        Method::POST,
        "/tenants/{id}/connections/{name}/start",
        Access::platform(Admin),
    );
    add(
        Method::POST,
        "/tenants/{id}/connections/{name}/stop",
        Access::platform(Admin),
    );
    add(
        Method::GET,
        "/tenants/{id}/connections/{name}/runs",
        Access::platform(Admin),
    );
    add(
        Method::GET,
        "/tenants/{id}/connections/{name}/dead-letters",
        Access::platform(Admin),
    );
    add(
        Method::GET,
        "/tenants/{id}/connections/{name}/logs",
        Access::platform(Admin),
    );
    add(
        Method::GET,
        "/tenants/{id}/connections/{name}/state",
        Access::platform(Admin),
    );
    add(
        Method::GET,
        "/tenants/{id}/catalog",
        Access::platform(Admin),
    );
    add(Method::GET, "/tenants/{id}/runs", Access::platform(Admin));
    t
}

fn forbidden(reason: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({ "error": reason })),
    )
        .into_response()
}

/// AuthZ + audit layer. Runs **after** AuthN (the `AuthenticatedKey` extension is present): looks the
/// matched route up in the default-deny table, `evaluate`s, and writes an audit event for mutations.
pub async fn authz_mw(State(app): State<Arc<App>>, req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let matched = req
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string());
    let key = req.extensions().get::<AuthenticatedKey>().cloned();

    let actor = key
        .as_ref()
        .map(|k| format!("key:{}", k.name))
        .unwrap_or_else(|| "anonymous".into());
    let is_mutation = method != Method::GET;
    let resource = matched.clone().unwrap_or_default();
    let action = format!("{method} {resource}");
    let audit_deny = |outcome: &str| {
        if is_mutation {
            app.record_audit(&actor, &action, &resource, outcome);
        }
    };

    // Fail-closed: unclassified route (or no matched path) → deny.
    let Some(matched) = matched else {
        audit_deny("denied");
        return forbidden("route not authorized");
    };
    let Some(access) = AUTHZ_TABLE.get(&(method.clone(), matched)).copied() else {
        audit_deny("denied");
        return forbidden("route not authorized");
    };
    let Some(key) = key else {
        return forbidden("missing authentication");
    };
    if !evaluate(&Principal::from_key(&key), access.scope, access.level) {
        audit_deny("denied");
        return forbidden("insufficient permissions");
    }

    let resp = next.run(req).await;
    if is_mutation {
        let outcome = if resp.status().is_success() {
            "ok"
        } else {
            "error"
        };
        app.record_audit(&actor, &action, &resource, outcome);
    }
    resp
}
