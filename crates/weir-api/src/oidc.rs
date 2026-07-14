//! OIDC relying party ([[WEIR-A-0008]] / [[WEIR-T-0086]]/[[WEIR-T-0088]]) — mirrors cloacina
//! `oidc.rs` + `routes/oidc_auth.rs`. Generic OIDC (issuer + client id/secret): discovery,
//! authorization-code + PKCE, JWKS ID-token verification. On success we **mint a short-lived key**
//! ([[WEIR-A-0008]]: OIDC = a key-minter, no session table) and set it in the `weir_session` cookie.
//! Dex is the test provider ([[WEIR-T-0088]]).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{
    Json,
    extract::{Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::Deserialize;

use weir_app::App;

#[derive(Clone, Debug)]
pub struct OidcConfig {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}

impl OidcConfig {
    /// Read the provider from env; `None` (OIDC disabled) unless issuer + client_id + redirect are set.
    pub fn from_env() -> Option<OidcConfig> {
        let issuer_url = std::env::var("WEIR_OIDC_ISSUER").ok()?;
        let client_id = std::env::var("WEIR_OIDC_CLIENT_ID").ok()?;
        let redirect_uri = std::env::var("WEIR_OIDC_REDIRECT_URI").ok()?;
        Some(OidcConfig {
            issuer_url,
            client_id,
            client_secret: std::env::var("WEIR_OIDC_CLIENT_SECRET").unwrap_or_default(),
            redirect_uri,
            scopes: parse_scopes(std::env::var("WEIR_OIDC_SCOPES").ok().as_deref()),
        })
    }
}

fn parse_scopes(raw: Option<&str>) -> Vec<String> {
    let parsed: Vec<String> = raw
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if parsed.is_empty() {
        vec!["openid".into(), "email".into(), "profile".into()]
    } else {
        parsed
    }
}

pub struct LoginStart {
    pub auth_url: String,
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: String,
}

/// Identity from a verified ID token.
pub struct IdentityClaims {
    pub subject: String,
    pub email: Option<String>,
}

/// A discovered relying party (cached provider metadata + the http client).
pub struct OidcProvider {
    metadata: CoreProviderMetadata,
    config: OidcConfig,
    http: reqwest::Client,
}

// The openidconnect v4 `CoreClient` is type-state generic; alias its fully-built shape.
type Client = CoreClient<
    openidconnect::EndpointSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointMaybeSet,
    openidconnect::EndpointMaybeSet,
>;

impl OidcProvider {
    pub async fn discover(config: OidcConfig) -> Result<Arc<OidcProvider>, String> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("oidc http client: {e}"))?;
        let issuer = IssuerUrl::new(config.issuer_url.clone())
            .map_err(|e| format!("oidc issuer url: {e}"))?;
        let metadata = CoreProviderMetadata::discover_async(issuer, &http)
            .await
            .map_err(|e| format!("oidc discovery failed: {e}"))?;
        Ok(Arc::new(OidcProvider {
            metadata,
            config,
            http,
        }))
    }

    /// Build the authorize URL (PKCE + state + nonce) — the caller stores state→(nonce, verifier).
    pub fn begin_login(&self) -> Result<LoginStart, String> {
        let client = self.client()?;
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let mut req = client.authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        );
        for s in &self.config.scopes {
            if s != "openid" {
                req = req.add_scope(Scope::new(s.clone()));
            }
        }
        let (auth_url, csrf, nonce) = req.set_pkce_challenge(pkce_challenge).url();
        Ok(LoginStart {
            auth_url: auth_url.to_string(),
            state: csrf.secret().clone(),
            nonce: nonce.secret().clone(),
            pkce_verifier: pkce_verifier.into_secret(),
        })
    }

    /// Exchange the code + verify the ID token (JWKS signature, iss/aud/exp, nonce) → identity.
    pub async fn complete_login(
        &self,
        code: String,
        nonce: String,
        pkce_verifier: String,
    ) -> Result<IdentityClaims, String> {
        let client = self.client()?;
        let token_response = client
            .exchange_code(AuthorizationCode::new(code))
            .map_err(|e| format!("exchange_code: {e}"))?
            .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
            .request_async(&self.http)
            .await
            .map_err(|e| format!("token exchange failed: {e}"))?;
        let id_token = token_response
            .id_token()
            .ok_or_else(|| "no id_token in token response".to_string())?;
        let verifier = client.id_token_verifier();
        let claims = id_token
            .claims(&verifier, &Nonce::new(nonce))
            .map_err(|e| format!("id_token validation failed: {e}"))?;
        Ok(IdentityClaims {
            subject: claims.subject().as_str().to_string(),
            email: claims.email().map(|e| e.as_str().to_string()),
        })
    }

    fn client(&self) -> Result<Client, String> {
        Ok(CoreClient::from_provider_metadata(
            self.metadata.clone(),
            ClientId::new(self.config.client_id.clone()),
            Some(ClientSecret::new(self.config.client_secret.clone())),
        )
        .set_redirect_uri(
            RedirectUrl::new(self.config.redirect_uri.clone()).map_err(|e| e.to_string())?,
        ))
    }
}

/// In-flight logins: `state -> (nonce, pkce_verifier)`, single-use, 10-min TTL (CSRF/replay defense).
pub struct LoginStore {
    inner: Mutex<HashMap<String, (String, String, Instant)>>,
    ttl: Duration,
}

impl Default for LoginStore {
    fn default() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            ttl: Duration::from_secs(600),
        }
    }
}

impl LoginStore {
    fn put(&self, state: String, nonce: String, pkce_verifier: String) {
        let mut m = self.inner.lock().unwrap();
        m.retain(|_, (_, _, at)| at.elapsed() < self.ttl);
        m.insert(state, (nonce, pkce_verifier, Instant::now()));
    }
    fn take(&self, state: &str) -> Option<(String, String)> {
        let mut m = self.inner.lock().unwrap();
        m.remove(state)
            .filter(|(_, _, at)| at.elapsed() < self.ttl)
            .map(|(n, p, _)| (n, p))
    }
}

/// Router state for the public `/auth/login` + `/auth/callback` routes. The provider is discovered
/// lazily (one round-trip) + cached.
#[derive(Clone)]
pub struct OidcState {
    pub config: Option<OidcConfig>,
    pub provider: Arc<tokio::sync::OnceCell<Arc<OidcProvider>>>,
    pub logins: Arc<LoginStore>,
    pub app: Arc<App>,
}

impl OidcState {
    pub fn from_env(app: Arc<App>) -> OidcState {
        OidcState {
            config: OidcConfig::from_env(),
            provider: Arc::new(tokio::sync::OnceCell::new()),
            logins: Arc::new(LoginStore::default()),
            app,
        }
    }

    async fn provider(&self) -> Result<Arc<OidcProvider>, Response> {
        let Some(config) = self.config.clone() else {
            return Err(not_configured());
        };
        self.provider
            .get_or_try_init(|| OidcProvider::discover(config))
            .await
            .cloned()
            .map_err(|e| internal(&e))
    }
}

fn not_configured() -> Response {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({ "error": "OIDC login is not configured on this server" })),
    )
        .into_response()
}

fn internal(msg: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": msg })),
    )
        .into_response()
}

/// `GET /auth/login` — begin the authorization-code + PKCE flow → 302 to the IdP.
pub async fn auth_login(State(oidc): State<OidcState>) -> Response {
    let provider = match oidc.provider().await {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let start = match provider.begin_login() {
        Ok(s) => s,
        Err(e) => return internal(&e),
    };
    oidc.logins
        .put(start.state, start.nonce, start.pkce_verifier);
    Redirect::to(&start.auth_url).into_response()
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// `GET /auth/callback` — the IdP redirects here; verify, mint a ~15-min key, set the cookie, → `/`.
pub async fn auth_callback(
    State(oidc): State<OidcState>,
    Query(q): Query<CallbackQuery>,
) -> Response {
    if let Some(err) = q.error {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": format!("idp error: {err}") })),
        )
            .into_response();
    }
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "missing code or state" })),
        )
            .into_response();
    };
    // Single-use lookup: a missing/expired/replayed state fails closed.
    let Some((nonce, pkce)) = oidc.logins.take(&state) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "invalid or expired login state" })),
        )
            .into_response();
    };
    let provider = match oidc.provider().await {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let claims = match provider.complete_login(code, nonce, pkce).await {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": e })),
            )
                .into_response();
        }
    };
    let subject = claims.email.unwrap_or(claims.subject);
    let issuer = oidc
        .config
        .as_ref()
        .map(|c| c.issuer_url.clone())
        .unwrap_or_default();
    let expires_at = now_ms() + 15 * 60 * 1000; // 15-minute minted key
    let key = match oidc.app.mint_api_key(
        &format!("oidc:{subject}"),
        "write",
        None,
        false,
        expires_at,
        &format!("oidc:{issuer}"),
    ) {
        Ok(k) => k,
        Err(e) => return internal(&e.to_string()),
    };
    // httpOnly cookie holds the minted key; SPA re-probes /auth/me and shows the app.
    let cookie = format!("weir_session={key}; Path=/; HttpOnly; SameSite=Lax; Max-Age=900");
    ([(header::SET_COOKIE, cookie)], Redirect::to("/")).into_response()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_default_and_parse() {
        assert_eq!(parse_scopes(None), vec!["openid", "email", "profile"]);
        assert_eq!(parse_scopes(Some("")), vec!["openid", "email", "profile"]);
        assert_eq!(
            parse_scopes(Some("openid, groups")),
            vec!["openid", "groups"]
        );
    }

    #[test]
    fn login_store_is_single_use() {
        // The state→(nonce, pkce) map is the CSRF/replay defense: single-use.
        let s = LoginStore::default();
        s.put("st".into(), "nonce".into(), "pkce".into());
        assert_eq!(
            s.take("st"),
            Some(("nonce".to_string(), "pkce".to_string()))
        );
        assert_eq!(s.take("st"), None); // replayed state → rejected
        assert_eq!(s.take("never-issued"), None); // forged state → rejected
    }
}
