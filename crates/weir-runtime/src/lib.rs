//! # weir-runtime
//!
//! The host-side **execution seam** ([[WEIR-S-0005]] / [[WEIR-A-0002]]): load a
//! connector and invoke the [`Connector`](weir_connector::Connector) contract
//! over either execution primitive — native **dylib** (trusted) or **WASM**
//! (open) — behind one backend-agnostic [`ConnectorHandle`]. Callers work in
//! typed contract terms ([`ReadContext`] → [`ReadMessage`] stream, etc.); the seam owns
//! the fidius dispatch, method indices, capability checks, and the raw
//! read/write envelope codec.
//!
//! Reached through `weir_connector::fidius` (white-label) — no direct `fidius`
//! dependency.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub mod sigv4;

use ed25519_dalek::VerifyingKey;
// All host-side fidius surface is reached white-label via the facade (fidius 0.5.2
// completed it): loader/handle types, plus the WASM egress contract under `wasm`.
use weir_connector::fidius::{
    CallError, EgressDenied, EgressPolicy, LoadError, PluginHandle, PluginHost, TcpTarget,
    ValueError, from_value,
};
use weir_connector::{
    CheckResult, CodecError, Config, ConnectorRole, ConnectorSpec, DiscoverOutcome, ReadContext,
    ReadMessage, RecordBatch, WriteContext, WriteOutcome,
};

/// Method indices, in `Connector` declaration order (the fidius vtable order).
mod method {
    pub const SPEC: usize = 0;
    pub const CHECK: usize = 1;
    pub const DISCOVER: usize = 2;
    pub const READ: usize = 3;
    pub const WRITE: usize = 4;
}

/// Errors from loading or invoking a connector through the seam.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("load error: {0}")]
    Load(#[from] LoadError),
    #[error("call error: {0}")]
    Call(#[from] CallError),
    #[error("envelope codec error: {0}")]
    Codec(#[from] CodecError),
    #[error("stream item decode error: {0}")]
    Value(#[from] ValueError),
    #[error("connector does not implement `{0}` (capability bit not set)")]
    Unsupported(&'static str),
    #[error("invalid trusted public key: {0}")]
    Key(String),
}

/// A loaded connector, invoked in typed contract terms regardless of execution
/// primitive. Both the native (dylib/in-process) and WASM-component backends
/// resolve to a fidius [`PluginHandle`] with identical `call_method` dispatch —
/// so the seam is backend-agnostic by construction.
pub struct ConnectorHandle {
    handle: PluginHandle,
}

impl ConnectorHandle {
    /// Load a connector packaged as a **WASM component** from a package dir under
    /// `search_path` — the open/sandboxed path ([[WEIR-A-0002]]). Host policies are
    /// **parameters**, not a constructor per combination:
    /// - `egress`: an [`EgressPolicy`] governing the guest's `wasi:http`
    ///   ([[WEIR-T-0023]]) — authorizes each request, can inject credentials
    ///   ([[WEIR-A-0013]]), enforces an allow-list. Use [`HostAllowList::allow_all`]
    ///   for no restriction, or [`HostAllowList`] with `allowed_hosts`.
    /// - `trusted_keys`: Ed25519 public keys (32 bytes) the package must be signed
    ///   by ([[WEIR-A-0015]]). **Empty = don't require a signature.** Non-empty →
    ///   unsigned/tampered/untrusted packages are rejected at load.
    pub fn from_wasm_package(
        search_path: &Path,
        package_name: &str,
        config: &Config,
        egress: impl EgressPolicy,
        trusted_keys: &[[u8; 32]],
    ) -> Result<Self, RuntimeError> {
        let mut builder = PluginHost::builder()
            .search_path(search_path)
            .egress(egress);
        if !trusted_keys.is_empty() {
            let keys: Vec<VerifyingKey> = trusted_keys
                .iter()
                .map(VerifyingKey::from_bytes)
                .collect::<Result<_, _>>()
                .map_err(|e| RuntimeError::Key(e.to_string()))?;
            builder = builder.require_signature(true).trusted_keys(&keys);
        }
        let host = builder.build()?;
        let handle = host.load_wasm_configured(
            package_name,
            &weir_connector::__fidius_Connector::Connector_WASM_DESCRIPTOR,
            config,
        )?;
        Ok(Self { handle })
    }

    fn handle(&self) -> &PluginHandle {
        &self.handle
    }

    /// Declared roles from `spec()`. Backend-agnostic — fidius `#[optional]`
    /// capability bits are only populated for the cdylib backend, so role
    /// detection reads the spec instead.
    pub fn roles(&self) -> Result<Vec<ConnectorRole>, RuntimeError> {
        Ok(self.spec()?.roles)
    }

    /// Whether this connector declares the Source role.
    pub fn is_source(&self) -> Result<bool, RuntimeError> {
        Ok(self.roles()?.contains(&ConnectorRole::Source))
    }

    /// Whether this connector declares the Destination role.
    pub fn is_destination(&self) -> Result<bool, RuntimeError> {
        Ok(self.roles()?.contains(&ConnectorRole::Destination))
    }

    /// `Connector::spec` — static descriptor.
    pub fn spec(&self) -> Result<ConnectorSpec, RuntimeError> {
        Ok(self
            .handle()
            .call_method::<(), ConnectorSpec>(method::SPEC, &())?)
    }

    /// `Connector::check` — validate config + connectivity.
    pub fn check(&self, config: &Config) -> Result<CheckResult, RuntimeError> {
        Ok(self
            .handle()
            .call_method::<(Config,), CheckResult>(method::CHECK, &(config.clone(),))?)
    }

    /// `Connector::discover` — introspect streams (SOURCE).
    pub fn discover(&self, config: &Config) -> Result<DiscoverOutcome, RuntimeError> {
        Ok(self
            .handle()
            .call_method::<(Config,), DiscoverOutcome>(method::DISCOVER, &(config.clone(),))?)
    }

    /// `Connector::read` — the v1 **streaming** read (SOURCE, [[WEIR-A-0029]]):
    /// yields [`ReadMessage`]s (records, checkpoints, logs, dead-letters) lazily
    /// with backpressure. `config` is bound at construction. A `RuntimeError` item
    /// is a transport/decode failure; a connector-domain failure ends the stream.
    pub async fn read(
        &self,
        ctx: &ReadContext,
    ) -> Result<impl futures::Stream<Item = Result<ReadMessage, RuntimeError>>, RuntimeError> {
        use futures::StreamExt;
        let stream = self
            .handle()
            .call_streaming::<ReadContext, ReadMessage>(method::READ, ctx)
            .await?;
        Ok(stream.map(|item| {
            item.map_err(RuntimeError::Call)
                .and_then(|v| from_value::<ReadMessage>(v).map_err(RuntimeError::Value))
        }))
    }

    /// `Connector::write` — the v1 **client-streaming** write (DESTINATION,
    /// [[WEIR-A-0029]]): the host produces `batches`, the connector pulls them at
    /// its own pace (backpressure) and returns one [`WriteOutcome`]. `config` is
    /// bound at construction; the per-segment context is `ctx`.
    pub fn write(
        &self,
        ctx: &WriteContext,
        batches: Vec<RecordBatch>,
    ) -> Result<WriteOutcome, RuntimeError> {
        Ok(self
            .handle()
            .call_client_streaming::<RecordBatch, (WriteContext,), WriteOutcome>(
                method::WRITE,
                batches,
                &(ctx.clone(),),
            )?)
    }
}

/// A host [`EgressPolicy`] for codegen'd WASM connectors ([[WEIR-T-0023]]): allow
/// `wasi:http` only to `allowed_hosts` (empty = allow any), and inject
/// `inject_headers` (e.g. credentials, [[WEIR-A-0013]]) into each authorized
/// request. The guest never sees the credentials or the deny reason.
pub struct HostAllowList {
    pub allowed_hosts: Vec<String>,
    pub inject_headers: Vec<(String, String)>,
    /// A host-side [`Credential`] injected per request ([[WEIR-A-0033]]) — the secret
    /// (api-key / OAuth token / session token) is resolved and attached here, on the
    /// host, so it never enters the guest config. `None` = static `inject_headers` only.
    pub credential: Option<Credential>,
}

impl HostAllowList {
    /// Allow `wasi:http` egress to any host, injecting no headers — the default
    /// "no network restriction" policy for connectors that don't need an allow-list.
    pub fn allow_all() -> Self {
        Self {
            allowed_hosts: Vec::new(),
            inject_headers: Vec::new(),
            credential: None,
        }
    }

    /// Attach a host-side [`Credential`] the policy resolves + injects per request.
    pub fn with_credential(mut self, credential: Credential) -> Self {
        self.credential = Some(credential);
        self
    }
}

impl EgressPolicy for HostAllowList {
    fn authorize(
        &self,
        parts: &mut weir_connector::fidius::http_types::request::Parts,
    ) -> Result<(), EgressDenied> {
        use weir_connector::fidius::http_types::header::{HeaderName, HeaderValue};
        let host = parts.uri.host().unwrap_or_default().to_string();
        if !self.allowed_hosts.is_empty() && !self.allowed_hosts.iter().any(|h| h == &host) {
            return Err(EgressDenied::new(format!("egress to `{host}` not allowed")));
        }
        for (k, v) in &self.inject_headers {
            if let (Ok(name), Ok(val)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                parts.headers.insert(name, val);
            }
        }
        // Resolve + inject the host-side credential (api-key / OAuth / session). For
        // OAuth/session this may mint or refresh a token (a host-side HTTP grant) the
        // first time / on expiry; the guest never sees the secret ([[WEIR-A-0033]]).
        if let Some(cred) = &self.credential {
            cred.apply(parts)?;
        }
        Ok(())
    }

    /// TCP egress (e.g. the `postgres` connector's wire protocol). Mirrors `authorize`:
    /// `allow_all` (empty `allowed_hosts`) permits any connect; otherwise the target host
    /// or `host:port` must be listed. Overrides the trait's default-deny so TCP connectors
    /// run under the same "no restriction" policy as HTTP ones ([[WEIR-I-0008]]).
    fn authorize_tcp(&self, addr: &std::net::SocketAddr) -> Result<(), EgressDenied> {
        if self.allowed_hosts.is_empty()
            || self
                .allowed_hosts
                .iter()
                .any(|h| h == &addr.ip().to_string() || h == &addr.to_string())
        {
            Ok(())
        } else {
            Err(EgressDenied::new(format!(
                "tcp egress to `{addr}` not allowed"
            )))
        }
    }

    /// Name-aware TCP egress ([[WEIR-A-0041]] review trigger / fidius FIDIUS-I-0034):
    /// allow-list entries may be **hostnames** — `db.example.com` or
    /// `db.example.com:5432` — matched against the name the guest actually dialed
    /// (fidius resolves-and-pins the guest's lookup and hands it here, lowercased).
    /// IP-literal dials arrive with `host: None` and can only match IP/`ip:port`
    /// entries via the resolved-addr fallback — a hostname entry never authorizes an
    /// IP dial. IP allow-lists stay supported but are operationally broken for
    /// managed endpoints (RDS/Cloud SQL/Azure) that rotate IPs; list names instead.
    fn authorize_tcp_target(&self, target: &TcpTarget<'_>) -> Result<(), EgressDenied> {
        if let Some(host) = target.host {
            let hostport = format!("{host}:{}", target.addr.port());
            if self
                .allowed_hosts
                .iter()
                .any(|h| h.eq_ignore_ascii_case(host) || h.eq_ignore_ascii_case(&hostport))
            {
                return Ok(());
            }
        }
        self.authorize_tcp(&target.addr).map_err(|_| {
            EgressDenied::new(match target.host {
                Some(host) => {
                    format!("tcp egress to `{host}` ({}) not allowed", target.addr)
                }
                None => format!(
                    "tcp egress to IP-literal `{}` not allowed (dial by hostname, or list the IP)",
                    target.addr
                ),
            })
        })
    }
}

/// A host-side credential the egress policy resolves and injects on each outbound
/// request, so the secret never enters the guest ([[WEIR-A-0033]]). Built by the
/// host (weir-app/orchestrator) from the connection's secret config + the manifest's
/// declared scheme.
pub enum Credential {
    /// Inject a fixed header — bearer (`Authorization: Bearer <value>`) is just
    /// `name = "Authorization"`, `value = "Bearer <key>"`; a header api-key is
    /// `name = "<Header>"`, `value = "<key>"`.
    Header { name: String, value: String },
    /// Carry an api-key as a query parameter — the host rewrites the outbound URI to
    /// append `?<param>=<value>` (the key still never reaches the guest config).
    Query { param: String, value: String },
    /// OAuth2 — the host performs the token grant, caches the access token to its
    /// `expires_in`, re-mints on expiry, and injects `Authorization: Bearer <token>`.
    OAuth2(OAuth2Provider),
    /// Session token — the host performs a login request, extracts the token at a
    /// dot-path, caches it, and injects it as a header on subsequent requests.
    Session(SessionProvider),
    /// AWS SigV4 — the host signs each outbound request (S3/object-store, [[WEIR-T-0109]]);
    /// the secret key never enters the guest. GET-only (empty-payload hash).
    AwsSigV4 {
        access_key: String,
        secret_key: String,
        region: String,
        service: String,
    },
    /// Google service account ([[WEIR-T-0155]]) — the host builds the RS256 JWT-bearer
    /// assertion from the SA JSON key, exchanges it for a scoped access token, caches +
    /// refreshes it, and injects `Authorization: Bearer <token>`.
    GoogleServiceAccount(GoogleSaProvider),
    /// Snowflake key-pair JWT ([[WEIR-T-0156]]) — the host signs a self-issued RS256 JWT
    /// (no token endpoint) and injects it as a bearer plus the
    /// `X-Snowflake-Authorization-Token-Type: KEYPAIR_JWT` header.
    SnowflakeKeypairJwt(SnowflakeKeypairProvider),
}

impl Credential {
    /// Build a host-side credential from the resolved connection config — the
    /// `auth_*` metadata weir-app layered in plus the secret values the connection
    /// supplied — and return it alongside a **sanitized** config JSON with every
    /// secret + auth key removed. That sanitized JSON is what reaches the guest, so
    /// no credential ever crosses into the sandbox ([[WEIR-A-0033]]). A config with
    /// no recognized auth scheme yields `(None, <json unchanged>)`.
    pub fn from_auth_config(json: &str) -> (Option<Credential>, String) {
        let Ok(mut v) = serde_json::from_str::<serde_json::Value>(json) else {
            return (None, json.to_string());
        };
        let Some(obj) = v.as_object_mut() else {
            return (None, json.to_string());
        };
        let get = |o: &serde_json::Map<String, serde_json::Value>, k: &str| {
            o.get(k).and_then(|x| x.as_str()).map(str::to_string)
        };

        // Metadata keys are always stripped; secret-value keys are added as we discover
        // their (possibly dynamic) names so they're stripped too.
        let mut strip: Vec<String> = vec![
            "auth_scheme".into(),
            "auth_name".into(),
            "api_key".into(),
            "oauth_token_url".into(),
            "oauth_grant".into(),
            "oauth_client_id_key".into(),
            "oauth_client_secret_key".into(),
            "oauth_refresh_token_key".into(),
            "oauth_scopes".into(),
            "session_login_url".into(),
            "session_token_path".into(),
            "session_inject_header".into(),
            "basic_username_key".into(),
            "basic_password_key".into(),
            "google_sa_key_key".into(),
            "google_scopes".into(),
            "snowflake_account_key".into(),
            "snowflake_user_key".into(),
            "snowflake_private_key_key".into(),
        ];

        let cred = match get(obj, "auth_scheme").as_deref() {
            Some("bearer") => get(obj, "api_key").map(|k| Credential::Header {
                name: "Authorization".into(),
                value: format!("Bearer {k}"),
            }),
            Some("header") => match (get(obj, "auth_name"), get(obj, "api_key")) {
                (Some(name), Some(value)) => Some(Credential::Header { name, value }),
                _ => None,
            },
            Some("query") => match (get(obj, "auth_name"), get(obj, "api_key")) {
                (Some(param), Some(value)) => Some(Credential::Query { param, value }),
                _ => None,
            },
            Some("oauth2") => {
                let token_url = get(obj, "oauth_token_url").unwrap_or_default();
                let cid_key = get(obj, "oauth_client_id_key").unwrap_or_else(|| "client_id".into());
                let csec_key =
                    get(obj, "oauth_client_secret_key").unwrap_or_else(|| "client_secret".into());
                let client_id = get(obj, &cid_key).unwrap_or_default();
                let client_secret = get(obj, &csec_key).unwrap_or_default();
                strip.push(cid_key);
                strip.push(csec_key);
                let scopes = obj
                    .get("oauth_scopes")
                    .and_then(|s| s.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let grant = match get(obj, "oauth_grant").as_deref() {
                    Some("client_credentials") => OAuthGrantKind::ClientCredentials,
                    _ => {
                        let rt_key = get(obj, "oauth_refresh_token_key")
                            .unwrap_or_else(|| "refresh_token".into());
                        let refresh_token = get(obj, &rt_key).unwrap_or_default();
                        strip.push(rt_key);
                        OAuthGrantKind::RefreshToken { refresh_token }
                    }
                };
                Some(Credential::OAuth2(OAuth2Provider::new(
                    token_url,
                    grant,
                    client_id,
                    client_secret,
                    scopes,
                )))
            }
            Some("session") => {
                let login_url = get(obj, "session_login_url").unwrap_or_default();
                // `session_token_path` is an array of segments or a dot-joined string.
                let token_path = match obj.get("session_token_path") {
                    Some(serde_json::Value::Array(a)) => a
                        .iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect(),
                    Some(serde_json::Value::String(s)) => {
                        s.split('.').map(str::to_string).collect()
                    }
                    _ => Vec::new(),
                };
                let inject_header =
                    get(obj, "session_inject_header").unwrap_or_else(|| "Authorization".into());
                Some(Credential::Session(SessionProvider::new(
                    login_url,
                    token_path,
                    inject_header,
                )))
            }
            Some("basic") => {
                let user_key = get(obj, "basic_username_key").unwrap_or_else(|| "username".into());
                let pass_key = get(obj, "basic_password_key").unwrap_or_else(|| "password".into());
                let user = get(obj, &user_key).unwrap_or_default();
                let pass = get(obj, &pass_key).unwrap_or_default();
                strip.push(user_key);
                strip.push(pass_key);
                use base64::Engine;
                let encoded =
                    base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
                Some(Credential::Header {
                    name: "Authorization".into(),
                    value: format!("Basic {encoded}"),
                })
            }
            Some("google_service_account") => {
                let key_key =
                    get(obj, "google_sa_key_key").unwrap_or_else(|| "service_account_key".into());
                // The SA JSON key arrives as a JSON object or a JSON-encoded string.
                let sa: Option<serde_json::Value> = match obj.get(&key_key) {
                    Some(serde_json::Value::Object(_)) => obj.get(&key_key).cloned(),
                    Some(serde_json::Value::String(s)) => serde_json::from_str(s).ok(),
                    _ => None,
                };
                strip.push(key_key);
                let scopes: Vec<String> = obj
                    .get("google_scopes")
                    .and_then(|s| s.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                sa.map(|sa| {
                    let f = |k: &str| {
                        sa.get(k)
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string()
                    };
                    let token_uri = match f("token_uri") {
                        t if t.is_empty() => "https://oauth2.googleapis.com/token".to_string(),
                        t => t,
                    };
                    Credential::GoogleServiceAccount(GoogleSaProvider::new(
                        f("client_email"),
                        f("private_key"),
                        token_uri,
                        scopes,
                    ))
                })
            }
            Some("snowflake_keypair_jwt") => {
                let acct_key =
                    get(obj, "snowflake_account_key").unwrap_or_else(|| "account".into());
                let user_key = get(obj, "snowflake_user_key").unwrap_or_else(|| "user".into());
                let pk_key =
                    get(obj, "snowflake_private_key_key").unwrap_or_else(|| "private_key".into());
                let account = get(obj, &acct_key).unwrap_or_default();
                let user = get(obj, &user_key).unwrap_or_default();
                let private_key = get(obj, &pk_key);
                // Only the private key is a secret — account/user stay in the guest
                // config (the manifest templates them into `url_base`, [[WEIR-T-0157]]).
                strip.push(pk_key);
                private_key.map(|pk| {
                    Credential::SnowflakeKeypairJwt(SnowflakeKeypairProvider::new(
                        account, user, pk,
                    ))
                })
            }
            Some("aws_sigv4") => {
                let ak_key =
                    get(obj, "aws_access_key_id_key").unwrap_or_else(|| "access_key_id".into());
                let sk_key = get(obj, "aws_secret_access_key_key")
                    .unwrap_or_else(|| "secret_access_key".into());
                let access_key = get(obj, &ak_key).unwrap_or_default();
                let secret_key = get(obj, &sk_key).unwrap_or_default();
                strip.push(ak_key);
                strip.push(sk_key);
                strip.push("aws_access_key_id_key".into());
                strip.push("aws_secret_access_key_key".into());
                strip.push("aws_region".into());
                strip.push("aws_service".into());
                Some(Credential::AwsSigV4 {
                    access_key,
                    secret_key,
                    region: get(obj, "aws_region").unwrap_or_else(|| "us-east-1".into()),
                    service: get(obj, "aws_service").unwrap_or_else(|| "s3".into()),
                })
            }
            _ => None,
        };

        for k in &strip {
            obj.remove(k);
        }
        (
            cred,
            serde_json::to_string(&v).unwrap_or_else(|_| json.to_string()),
        )
    }

    /// Resolve the credential and apply it to one outbound request.
    fn apply(
        &self,
        parts: &mut weir_connector::fidius::http_types::request::Parts,
    ) -> Result<(), EgressDenied> {
        match self {
            Credential::Header { name, value } => inject_header(parts, name, value),
            Credential::Query { param, value } => append_query(parts, param, value),
            Credential::OAuth2(p) => {
                let token = p.bearer()?;
                inject_header(parts, "Authorization", &format!("Bearer {token}"))
            }
            Credential::Session(p) => {
                let token = p.token()?;
                inject_header(parts, &p.inject_header, &token)
            }
            Credential::GoogleServiceAccount(p) => {
                let token = p.bearer()?;
                inject_header(parts, "Authorization", &format!("Bearer {token}"))
            }
            Credential::SnowflakeKeypairJwt(p) => {
                let token = p.bearer()?;
                inject_header(parts, "Authorization", &format!("Bearer {token}"))?;
                inject_header(parts, "X-Snowflake-Authorization-Token-Type", "KEYPAIR_JWT")
            }
            Credential::AwsSigV4 {
                access_key,
                secret_key,
                region,
                service,
            } => {
                // The signed `host` must equal the Host header the client sends — the URI
                // authority, INCLUDING a non-default port (S3/MinIO on :9000 sign with it).
                let host = parts
                    .uri
                    .authority()
                    .map(|a| a.as_str().to_string())
                    .or_else(|| {
                        parts
                            .headers
                            .get("host")
                            .and_then(|h| h.to_str().ok())
                            .map(str::to_string)
                    })
                    .ok_or_else(|| EgressDenied::new("sigv4: request has no host"))?;
                let (amz_date, datestamp) = now_amz();
                let payload_hash = sigv4::EMPTY_PAYLOAD_SHA256; // GET-only: empty body
                let canonical_uri = canonical_uri(parts.uri.path());
                let canonical_query = canonical_query(parts.uri.query().unwrap_or(""));
                // The x-amz-* headers are part of what we sign, so inject them first.
                inject_header(parts, "x-amz-date", &amz_date)?;
                inject_header(parts, "x-amz-content-sha256", payload_hash)?;
                let signed = vec![
                    ("host".to_string(), host),
                    ("x-amz-content-sha256".to_string(), payload_hash.to_string()),
                    ("x-amz-date".to_string(), amz_date.clone()),
                ];
                let auth = sigv4::SigV4 {
                    access_key,
                    secret_key,
                    region,
                    service,
                }
                .authorization(
                    parts.method.as_str(),
                    &canonical_uri,
                    &canonical_query,
                    &signed,
                    payload_hash,
                    &amz_date,
                    &datestamp,
                );
                inject_header(parts, "Authorization", &auth)
            }
        }
    }
}

/// Current UTC as SigV4's `(YYYYMMDDটHHMMSSZ, YYYYMMDD)`.
fn now_amz() -> (String, String) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    epoch_to_amz(secs)
}

/// Convert Unix seconds → SigV4 timestamps (civil date via Howard Hinnant's `days → y/m/d`).
fn epoch_to_amz(secs: u64) -> (String, String) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (
        format!("{y:04}{m:02}{d:02}T{h:02}{mi:02}{s:02}Z"),
        format!("{y:04}{m:02}{d:02}"),
    )
}

/// Percent-encode per RFC 3986 (unreserved `A-Za-z0-9-._~` pass through). `keep_slash` leaves `/`
/// unescaped, as S3's canonical URI requires.
fn uri_encode(s: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        let unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~');
        if unreserved || (keep_slash && b == b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// S3 canonical URI: the path with each segment percent-encoded, `/` preserved.
fn canonical_uri(path: &str) -> String {
    if path.is_empty() {
        "/".to_string()
    } else {
        uri_encode(path, true)
    }
}

/// SigV4 canonical query: each `k=v` percent-encoded, then sorted by encoded key.
fn canonical_query(query: &str) -> String {
    if query.is_empty() {
        return String::new();
    }
    let mut pairs: Vec<(String, String)> = query
        .split('&')
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (uri_encode(k, false), uri_encode(v, false))
        })
        .collect();
    pairs.sort();
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// Insert `name: value` into the request headers, failing closed on an invalid name/value.
fn inject_header(
    parts: &mut weir_connector::fidius::http_types::request::Parts,
    name: &str,
    value: &str,
) -> Result<(), EgressDenied> {
    use weir_connector::fidius::http_types::header::{HeaderName, HeaderValue};
    let n = HeaderName::from_bytes(name.as_bytes())
        .map_err(|e| EgressDenied::new(format!("invalid header name `{name}`: {e}")))?;
    let v = HeaderValue::from_str(value)
        .map_err(|_| EgressDenied::new("credential is not a valid header value"))?;
    parts.headers.insert(n, v);
    Ok(())
}

/// Append `?<param>=<value>` (or `&…`) to the outbound URI.
fn append_query(
    parts: &mut weir_connector::fidius::http_types::request::Parts,
    param: &str,
    value: &str,
) -> Result<(), EgressDenied> {
    let mut s = parts.uri.to_string();
    s.push(if parts.uri.query().is_some() {
        '&'
    } else {
        '?'
    });
    s.push_str(param);
    s.push('=');
    s.push_str(value);
    parts.uri = s
        .parse()
        .map_err(|e| EgressDenied::new(format!("uri rewrite for query-param auth failed: {e}")))?;
    Ok(())
}

/// OAuth2 grant the host runs to mint an access token ([[WEIR-A-0033]]).
pub enum OAuthGrantKind {
    /// `grant_type=refresh_token` — exchange a long-lived refresh token.
    RefreshToken { refresh_token: String },
    /// `grant_type=client_credentials` — client id/secret only.
    ClientCredentials,
}

/// Mints + caches an OAuth2 bearer token **host-side**. The grant `POST` is performed
/// by the host (`ureq`), so client secret/refresh token never reach the guest.
pub struct OAuth2Provider {
    token_url: String,
    grant: OAuthGrantKind,
    client_id: String,
    client_secret: String,
    scopes: Vec<String>,
    cache: Mutex<Option<CachedToken>>,
}

struct CachedToken {
    token: String,
    expires_at: Instant,
}

/// Re-mint this long before the stated expiry, to absorb clock skew + request latency.
const REFRESH_MARGIN: Duration = Duration::from_secs(60);

impl OAuth2Provider {
    pub fn new(
        token_url: String,
        grant: OAuthGrantKind,
        client_id: String,
        client_secret: String,
        scopes: Vec<String>,
    ) -> Self {
        Self {
            token_url,
            grant,
            client_id,
            client_secret,
            scopes,
            cache: Mutex::new(None),
        }
    }

    /// A valid bearer token — cached, re-minted when within [`REFRESH_MARGIN`] of expiry.
    fn bearer(&self) -> Result<String, EgressDenied> {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = cache.as_ref()
            && Instant::now() + REFRESH_MARGIN < c.expires_at
        {
            return Ok(c.token.clone());
        }
        let (token, ttl) = self.mint()?;
        *cache = Some(CachedToken {
            token: token.clone(),
            expires_at: Instant::now() + ttl,
        });
        Ok(token)
    }

    /// Perform the grant `POST` and parse `{ access_token, expires_in }`. The blocking
    /// `ureq` call runs on a **dedicated thread** ([`run_blocking`]): the egress hook can
    /// fire on the wasmtime async-executor thread, where ureq's blocking socket reads
    /// fail with `EINVAL` — a clean thread isolates the I/O.
    fn mint(&self) -> Result<(String, Duration), EgressDenied> {
        let token_url = self.token_url.clone();
        let client_id = self.client_id.clone();
        let client_secret = self.client_secret.clone();
        let scope = self.scopes.join(" ");
        let (grant_type, refresh_token) = match &self.grant {
            OAuthGrantKind::RefreshToken { refresh_token } => {
                ("refresh_token", Some(refresh_token.clone()))
            }
            OAuthGrantKind::ClientCredentials => ("client_credentials", None),
        };
        run_blocking(move || {
            let mut form: Vec<(&str, &str)> = vec![
                ("client_id", &client_id),
                ("client_secret", &client_secret),
                ("grant_type", grant_type),
            ];
            if let Some(rt) = &refresh_token {
                form.push(("refresh_token", rt));
            }
            if !scope.is_empty() {
                form.push(("scope", &scope));
            }
            let resp = ureq::post(&token_url)
                .send_form(&form)
                .map_err(|e| format!("oauth token grant failed: {e}"))?;
            let body: serde_json::Value = resp
                .into_json()
                .map_err(|e| format!("oauth token response not JSON: {e}"))?;
            let token = body
                .get("access_token")
                .and_then(|t| t.as_str())
                .ok_or("oauth token response missing `access_token`")?
                .to_string();
            // RFC 6749 makes `expires_in` optional; default to a conservative hour.
            let ttl = body
                .get("expires_in")
                .and_then(|e| e.as_u64())
                .unwrap_or(3600);
            Ok((token, Duration::from_secs(ttl)))
        })
    }
}

/// Google service-account JWT-bearer grant ([[WEIR-T-0155]]): the host builds the RS256
/// assertion from the SA JSON key's `client_email`/`private_key`, exchanges it at the
/// key's `token_uri` for an access token scoped to `scopes`, and injects it as a bearer.
/// Tokens are cached **process-wide** per `(client_email, scopes)` with the same
/// [`REFRESH_MARGIN`] as OAuth2 — concurrent streams, and connections sharing one key,
/// share one token. The key material never leaves the host.
pub struct GoogleSaProvider {
    client_email: String,
    private_key_pem: String,
    token_uri: String,
    scopes: Vec<String>,
}

/// Process-wide minted-token cache shared by the JWT-based schemes ([[WEIR-T-0155]] /
/// [[WEIR-T-0156]]), keyed `<scheme>|<identity>|<scopes>` so concurrent streams — and
/// connections sharing one key — share one token.
static MINTED_TOKENS: std::sync::OnceLock<Mutex<std::collections::HashMap<String, CachedToken>>> =
    std::sync::OnceLock::new();

impl GoogleSaProvider {
    pub fn new(
        client_email: String,
        private_key_pem: String,
        token_uri: String,
        scopes: Vec<String>,
    ) -> Self {
        Self {
            client_email,
            private_key_pem,
            token_uri,
            scopes,
        }
    }

    /// A valid bearer token — cached per `(client_email, scopes)`, re-minted when within
    /// [`REFRESH_MARGIN`] of expiry.
    fn bearer(&self) -> Result<String, EgressDenied> {
        let cache_key = format!("google|{}|{}", self.client_email, self.scopes.join(" "));
        cached_bearer(&cache_key, || self.mint())
    }

    /// Sign the assertion and run the JWT-bearer grant `POST` (on a dedicated thread —
    /// see [`run_blocking`]); parse `{ access_token, expires_in }`.
    fn mint(&self) -> Result<(String, Duration), EgressDenied> {
        let assertion = sa_assertion(
            &self.client_email,
            &self.scopes.join(" "),
            &self.token_uri,
            unix_now_secs(),
            &self.private_key_pem,
        )
        .map_err(EgressDenied::new)?;
        let token_uri = self.token_uri.clone();
        run_blocking(move || {
            let resp = ureq::post(&token_uri)
                .send_form(&[
                    ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                    ("assertion", &assertion),
                ])
                .map_err(|e| format!("google service-account token grant failed: {e}"))?;
            let body: serde_json::Value = resp
                .into_json()
                .map_err(|e| format!("google token response not JSON: {e}"))?;
            let token = body
                .get("access_token")
                .and_then(|t| t.as_str())
                .ok_or("google token response missing `access_token`")?
                .to_string();
            let ttl = body
                .get("expires_in")
                .and_then(|e| e.as_u64())
                .unwrap_or(3600);
            Ok((token, Duration::from_secs(ttl)))
        })
    }
}

/// Serve a bearer from the process-wide [`MINTED_TOKENS`] cache, minting via `mint` when
/// absent or within [`REFRESH_MARGIN`] of expiry. The lock is released during the mint (it
/// may be a network call); a concurrent racer minting the same token twice is benign —
/// last write wins.
fn cached_bearer(
    cache_key: &str,
    mint: impl FnOnce() -> Result<(String, Duration), EgressDenied>,
) -> Result<String, EgressDenied> {
    let cache = MINTED_TOKENS.get_or_init(Default::default);
    {
        let map = cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = map.get(cache_key)
            && Instant::now() + REFRESH_MARGIN < c.expires_at
        {
            return Ok(c.token.clone());
        }
    }
    let (token, ttl) = mint()?;
    let mut map = cache.lock().unwrap_or_else(|e| e.into_inner());
    map.insert(
        cache_key.to_string(),
        CachedToken {
            token: token.clone(),
            expires_at: Instant::now() + ttl,
        },
    );
    Ok(token)
}

/// Build the signed RS256 JWT-bearer assertion for a Google service account
/// ([[WEIR-T-0155]]). Pure — tests fix `iat`. Claims per Google's server-to-server
/// OAuth2 flow: `iss` = the SA email, `scope`, `aud` = the token endpoint, 1h validity
/// (Google's maximum; the short window also bounds clock-skew exposure).
fn sa_assertion(
    client_email: &str,
    scope: &str,
    aud: &str,
    iat: u64,
    private_key_pem: &str,
) -> Result<String, String> {
    let key = parse_pkcs8_keypair(private_key_pem)?;
    rs256_sign_jwt(
        &key,
        &serde_json::json!({
            "iss": client_email,
            "scope": scope,
            "aud": aud,
            "iat": iat,
            "exp": iat + 3600,
        })
        .to_string(),
    )
}

/// Parse a PKCS#8 PEM private key into a ring RSA keypair.
fn parse_pkcs8_keypair(private_key_pem: &str) -> Result<ring::signature::RsaKeyPair, String> {
    let der = pkcs8_pem_to_der(private_key_pem)?;
    ring::signature::RsaKeyPair::from_pkcs8(&der).map_err(|e| format!("private key rejected: {e}"))
}

/// Assemble + RS256-sign a JWT (`header.claims.signature`, base64url-no-pad) over the
/// given claims JSON ([[WEIR-T-0155]] / [[WEIR-T-0156]]).
fn rs256_sign_jwt(key: &ring::signature::RsaKeyPair, claims_json: &str) -> Result<String, String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let header = b64.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
    let claims = b64.encode(claims_json);
    let signing_input = format!("{header}.{claims}");
    let mut sig = vec![0u8; key.public().modulus_len()];
    key.sign(
        &ring::signature::RSA_PKCS1_SHA256,
        &ring::rand::SystemRandom::new(),
        signing_input.as_bytes(),
        &mut sig,
    )
    .map_err(|e| format!("RS256 signing failed: {e}"))?;
    Ok(format!("{signing_input}.{}", b64.encode(sig)))
}

/// Snowflake key-pair JWT auth ([[WEIR-T-0156]]): the host signs a **self-issued** RS256
/// JWT — there is no token endpoint; the JWT *is* the bearer. Claims per Snowflake's
/// key-pair spec: `iss` = `ACCOUNT.USER.SHA256:<public-key fingerprint>`, `sub` =
/// `ACCOUNT.USER` (both uppercased), 1h validity. Injected as `Authorization: Bearer`
/// **plus** `X-Snowflake-Authorization-Token-Type: KEYPAIR_JWT`. The private key stays
/// host-side; minting is a local sign (no network), cached per `(account, user)`.
pub struct SnowflakeKeypairProvider {
    /// Uppercased at construction (Snowflake matches identifiers uppercase).
    account: String,
    user: String,
    private_key_pem: String,
}

impl SnowflakeKeypairProvider {
    pub fn new(account: String, user: String, private_key_pem: String) -> Self {
        Self {
            account: account.to_uppercase(),
            user: user.to_uppercase(),
            private_key_pem,
        }
    }

    /// A valid self-signed bearer — cached per `(account, user)`, re-signed when within
    /// [`REFRESH_MARGIN`] of its 1h expiry.
    fn bearer(&self) -> Result<String, EgressDenied> {
        let cache_key = format!("snowflake|{}.{}", self.account, self.user);
        cached_bearer(&cache_key, || {
            let jwt = snowflake_jwt(
                &self.account,
                &self.user,
                unix_now_secs(),
                &self.private_key_pem,
            )
            .map_err(EgressDenied::new)?;
            Ok((jwt, Duration::from_secs(3600)))
        })
    }
}

/// Build Snowflake's self-signed key-pair JWT ([[WEIR-T-0156]]). Pure — tests fix `iat`.
fn snowflake_jwt(
    account: &str,
    user: &str,
    iat: u64,
    private_key_pem: &str,
) -> Result<String, String> {
    let key = parse_pkcs8_keypair(private_key_pem)?;
    let fingerprint = snowflake_fingerprint(&key);
    rs256_sign_jwt(
        &key,
        &serde_json::json!({
            "iss": format!("{account}.{user}.{fingerprint}"),
            "sub": format!("{account}.{user}"),
            "iat": iat,
            "exp": iat + 3600,
        })
        .to_string(),
    )
}

/// Snowflake's public-key fingerprint: `SHA256:` + base64 of the SHA-256 over the
/// **SubjectPublicKeyInfo** DER — the same encoding `ALTER USER … SET RSA_PUBLIC_KEY`
/// stores (what `openssl rsa -pubout -outform DER` emits). ring exposes the public key
/// as PKCS#1, so wrap it into SPKI first.
fn snowflake_fingerprint(key: &ring::signature::RsaKeyPair) -> String {
    use base64::Engine;
    use sha2::Digest;
    let spki = spki_from_pkcs1(ring::signature::KeyPair::public_key(key).as_ref());
    let digest = sha2::Sha256::digest(&spki);
    format!(
        "SHA256:{}",
        base64::engine::general_purpose::STANDARD.encode(digest)
    )
}

/// Wrap a PKCS#1 `RSAPublicKey` DER into an X.509 `SubjectPublicKeyInfo` DER:
/// `SEQUENCE { AlgorithmIdentifier(rsaEncryption, NULL), BIT STRING { 0, pkcs1 } }`.
fn spki_from_pkcs1(pkcs1: &[u8]) -> Vec<u8> {
    // DER definite-length encoding for `len` bytes of content.
    fn der_len(len: usize) -> Vec<u8> {
        match len {
            0..=0x7f => vec![len as u8],
            0x80..=0xff => vec![0x81, len as u8],
            _ => vec![0x82, (len >> 8) as u8, (len & 0xff) as u8],
        }
    }
    // AlgorithmIdentifier for rsaEncryption (OID 1.2.840.113549.1.1.1) + NULL params.
    const ALG_ID: [u8; 15] = [
        0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00,
    ];
    let mut bit_string = vec![0x03];
    bit_string.extend(der_len(pkcs1.len() + 1)); // +1 for the leading unused-bits byte
    bit_string.push(0x00);
    bit_string.extend_from_slice(pkcs1);
    let content_len = ALG_ID.len() + bit_string.len();
    let mut spki = vec![0x30];
    spki.extend(der_len(content_len));
    spki.extend_from_slice(&ALG_ID);
    spki.extend(bit_string);
    spki
}

/// Decode a PKCS#8 PEM (`-----BEGIN PRIVATE KEY-----`) body to DER. SA JSON keys embed
/// the PEM with `\n` escapes — serde unescapes them before this sees the string.
fn pkcs8_pem_to_der(pem: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    let body: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    base64::engine::general_purpose::STANDARD
        .decode(body.trim())
        .map_err(|e| format!("service-account private key is not valid PEM base64: {e}"))
}

/// Current Unix time (seconds) for JWT `iat`/`exp` claims.
fn unix_now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Run a blocking closure on a dedicated OS thread and wait for it. The egress hook
/// (`authorize`) can be invoked on the wasmtime async-executor thread, where blocking
/// `ureq` socket I/O fails with `EINVAL`; offloading to a fresh thread gives a clean
/// blocking context. Grants are infrequent (once per token lifetime), so the per-call
/// thread is negligible.
fn run_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, EgressDenied> {
    std::thread::spawn(f)
        .join()
        .map_err(|_| EgressDenied::new("credential grant thread panicked"))?
        .map_err(EgressDenied::new)
}

/// Logs in once host-side and injects the returned session token. v1 performs a plain
/// `POST` to the login endpoint and reads the token at `token_path`; credentialed login
/// bodies are a follow-up (the seam is the same).
pub struct SessionProvider {
    login_url: String,
    /// Dot-path segments into the login response JSON holding the token.
    token_path: Vec<String>,
    /// Header the token is injected as on subsequent requests.
    inject_header: String,
    cache: Mutex<Option<String>>,
}

impl SessionProvider {
    pub fn new(login_url: String, token_path: Vec<String>, inject_header: String) -> Self {
        Self {
            login_url,
            token_path,
            inject_header,
            cache: Mutex::new(None),
        }
    }

    fn token(&self) -> Result<String, EgressDenied> {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = cache.as_ref() {
            return Ok(t.clone());
        }
        let login_url = self.login_url.clone();
        let token_path = self.token_path.clone();
        // Offloaded to a clean thread for the same reason as the OAuth grant (see `mint`).
        let token = run_blocking(move || {
            let resp = ureq::post(&login_url)
                .call()
                .map_err(|e| format!("session login failed: {e}"))?;
            let body: serde_json::Value = resp
                .into_json()
                .map_err(|e| format!("session login response not JSON: {e}"))?;
            let mut cur = &body;
            for seg in &token_path {
                cur = cur
                    .get(seg)
                    .ok_or_else(|| format!("session token path `{seg}` not found"))?;
            }
            cur.as_str()
                .map(str::to_string)
                .ok_or_else(|| "session token is not a string".into())
        })?;
        *cache = Some(token.clone());
        Ok(token)
    }
}

#[cfg(test)]
mod tcp_egress_policy_tests {
    use super::*;

    fn addr(s: &str) -> std::net::SocketAddr {
        s.parse().unwrap()
    }

    fn named<'a>(host: &'a str, a: &str) -> TcpTarget<'a> {
        TcpTarget {
            host: Some(host),
            addr: addr(a),
        }
    }

    fn ip_literal(a: &str) -> TcpTarget<'static> {
        TcpTarget {
            host: None,
            addr: addr(a),
        }
    }

    fn list(hosts: &[&str]) -> HostAllowList {
        HostAllowList {
            allowed_hosts: hosts.iter().map(|s| s.to_string()).collect(),
            inject_headers: Vec::new(),
            credential: None,
        }
    }

    #[test]
    fn allow_all_permits_any_target() {
        let p = HostAllowList::allow_all();
        assert!(
            p.authorize_tcp_target(&named("db.internal", "10.0.0.9:5432"))
                .is_ok()
        );
        assert!(p.authorize_tcp_target(&ip_literal("10.0.0.9:5432")).is_ok());
    }

    #[test]
    fn hostname_entry_matches_by_name_dial() {
        // Bare-name entry: any port; fidius hands the dialed name lowercased.
        let p = list(&["db.example.com"]);
        assert!(
            p.authorize_tcp_target(&named("db.example.com", "203.0.113.7:5432"))
                .is_ok()
        );
        // Entries are matched case-insensitively (DNS is case-insensitive).
        let p = list(&["DB.Example.COM"]);
        assert!(
            p.authorize_tcp_target(&named("db.example.com", "203.0.113.7:5432"))
                .is_ok()
        );
        // host:port entry pins the port too.
        let p = list(&["db.example.com:5432"]);
        assert!(
            p.authorize_tcp_target(&named("db.example.com", "203.0.113.7:5432"))
                .is_ok()
        );
        let denied = p
            .authorize_tcp_target(&named("db.example.com", "203.0.113.7:1433"))
            .unwrap_err();
        assert!(
            denied.reason.contains("db.example.com"),
            "{}",
            denied.reason
        );
        assert!(
            denied.reason.contains("203.0.113.7:1433"),
            "{}",
            denied.reason
        );
    }

    #[test]
    fn hostname_entry_never_authorizes_ip_literal_dial() {
        // The guest dialed the resolved IP directly — no pinned name, so a
        // name-keyed list must fail closed even though the addr "belongs" to it.
        let p = list(&["db.example.com"]);
        let denied = p
            .authorize_tcp_target(&ip_literal("203.0.113.7:5432"))
            .unwrap_err();
        assert!(denied.reason.contains("IP-literal"), "{}", denied.reason);
    }

    #[test]
    fn ip_entries_still_match_via_resolved_addr() {
        // Existing IP/`ip:port` allow-lists keep working for both dial styles.
        let p = list(&["203.0.113.7"]);
        assert!(
            p.authorize_tcp_target(&ip_literal("203.0.113.7:5432"))
                .is_ok()
        );
        assert!(
            p.authorize_tcp_target(&named("db.example.com", "203.0.113.7:5432"))
                .is_ok()
        );
        let p = list(&["203.0.113.7:5432"]);
        assert!(
            p.authorize_tcp_target(&ip_literal("203.0.113.7:5432"))
                .is_ok()
        );
        assert!(
            p.authorize_tcp_target(&ip_literal("203.0.113.7:1433"))
                .is_err()
        );
    }

    #[test]
    fn unlisted_name_is_denied_with_both_name_and_addr() {
        let p = list(&["db.example.com"]);
        let denied = p
            .authorize_tcp_target(&named("evil.example.net", "198.51.100.1:5432"))
            .unwrap_err();
        assert!(
            denied.reason.contains("evil.example.net"),
            "{}",
            denied.reason
        );
        assert!(
            denied.reason.contains("198.51.100.1:5432"),
            "{}",
            denied.reason
        );
    }
}

#[cfg(test)]
mod sigv4_credential_tests {
    use super::*;

    #[test]
    fn epoch_converts_to_amz_timestamps() {
        // 1440938160 == 2015-08-30T12:36:00Z (the AWS test-vector instant).
        assert_eq!(
            epoch_to_amz(1_440_938_160),
            ("20150830T123600Z".to_string(), "20150830".to_string())
        );
        assert_eq!(epoch_to_amz(0).0, "19700101T000000Z");
    }

    #[test]
    fn aws_sigv4_credential_parses_and_strips_secrets() {
        let (cred, sanitized) = Credential::from_auth_config(
            r#"{"auth_scheme":"aws_sigv4","access_key_id":"AKIA","secret_access_key":"shh",
                "aws_region":"eu-west-1","bucket":"data"}"#,
        );
        match cred {
            Some(Credential::AwsSigV4 {
                access_key,
                secret_key,
                region,
                service,
            }) => {
                assert_eq!(access_key, "AKIA");
                assert_eq!(secret_key, "shh");
                assert_eq!(region, "eu-west-1");
                assert_eq!(service, "s3"); // default
            }
            other => panic!("expected AwsSigV4, got {other:?}", other = other.is_some()),
        }
        // The secret + auth metadata never reach the guest; the harmless config field remains.
        assert!(!sanitized.contains("shh"));
        assert!(!sanitized.contains("AKIA"));
        assert!(!sanitized.contains("auth_scheme"));
        assert!(sanitized.contains("\"bucket\":\"data\""));
    }
}

#[cfg(test)]
mod google_sa_tests {
    use super::*;

    /// A throwaway 2048-bit PKCS#8 RSA key generated for these tests — never a real
    /// credential.
    const TEST_KEY_PEM: &str = include_str!("../tests/fixtures/google_sa_test_key.pem");

    #[test]
    fn assertion_has_rs256_header_and_google_claims() {
        let jwt = sa_assertion(
            "sa@x.iam.gserviceaccount.com",
            "scope-a scope-b",
            "https://oauth2.googleapis.com/token",
            1_700_000_000,
            TEST_KEY_PEM,
        )
        .expect("sign assertion");
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3, "header.claims.signature");
        use base64::Engine;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let header: serde_json::Value =
            serde_json::from_slice(&b64.decode(parts[0]).unwrap()).unwrap();
        assert_eq!(header["alg"], "RS256");
        assert_eq!(header["typ"], "JWT");
        let claims: serde_json::Value =
            serde_json::from_slice(&b64.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(claims["iss"], "sa@x.iam.gserviceaccount.com");
        assert_eq!(claims["scope"], "scope-a scope-b");
        assert_eq!(claims["aud"], "https://oauth2.googleapis.com/token");
        assert_eq!(claims["iat"], 1_700_000_000u64);
        assert_eq!(claims["exp"], 1_700_003_600u64, "1h validity");
        // RS256 over a 2048-bit key → a 256-byte signature.
        assert_eq!(b64.decode(parts[2]).unwrap().len(), 256);
    }

    #[test]
    fn google_sa_credential_parses_and_strips_key_material() {
        let cfg = serde_json::json!({
            "auth_scheme": "google_service_account",
            "google_sa_key_key": "service_account_key",
            "google_scopes": ["https://www.googleapis.com/auth/analytics.readonly"],
            "service_account_key": {
                "client_email": "sa@x.iam.gserviceaccount.com",
                "private_key": TEST_KEY_PEM,
            },
            "property_id": "123",
        })
        .to_string();
        let (cred, sanitized) = Credential::from_auth_config(&cfg);
        assert!(matches!(cred, Some(Credential::GoogleServiceAccount(_))));
        // Key material + every auth key is gone; the harmless config field survives.
        assert!(!sanitized.contains("PRIVATE KEY"));
        assert!(!sanitized.contains("client_email"));
        assert!(!sanitized.contains("auth_scheme"));
        assert!(!sanitized.contains("google_scopes"));
        assert!(sanitized.contains("\"property_id\":\"123\""));

        // The SA key also arrives as a JSON-encoded **string** (how a secret bundle
        // field usually lands) — same result.
        let cfg2 = serde_json::json!({
            "auth_scheme": "google_service_account",
            "service_account_key":
                r#"{"client_email":"sa@x.iam.gserviceaccount.com","private_key":"k"}"#,
        })
        .to_string();
        let (cred2, sanitized2) = Credential::from_auth_config(&cfg2);
        assert!(matches!(cred2, Some(Credential::GoogleServiceAccount(_))));
        assert!(!sanitized2.contains("client_email"));
    }

    /// Read one HTTP request (headers + content-length body) and answer with `body`.
    fn respond_json(s: &mut std::net::TcpStream, body: &str) -> String {
        use std::io::{Read as _, Write as _};
        s.set_read_timeout(Some(std::time::Duration::from_millis(500)))
            .ok();
        let mut data = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            match s.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    data.extend_from_slice(&buf[..n]);
                    if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&data[..end]);
                        let clen = head
                            .lines()
                            .find_map(|l| {
                                l.split_once(':')
                                    .filter(|(k, _)| {
                                        k.trim().eq_ignore_ascii_case("content-length")
                                    })
                                    .map(|(_, v)| v)
                            })
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        if data.len() >= end + 4 + clen {
                            break;
                        }
                    }
                }
            }
        }
        let resp = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = std::io::Write::write_all(s, resp.as_bytes());
        String::from_utf8_lossy(&data).into_owned()
    }

    /// A counting mock token endpoint: records each grant request, returns `body`.
    fn mock_token_endpoint(body: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { break };
                let req = respond_json(&mut s, body);
                let _ = tx.send(req);
            }
        });
        (format!("http://{addr}"), rx)
    }

    /// Mocked token exchange ([[WEIR-T-0155]] AC): the JWT-bearer grant mints, the
    /// process-wide cache serves the second call without a second HTTP round-trip.
    #[test]
    fn google_sa_mints_via_jwt_bearer_grant_and_caches() {
        let (token_url, rx) = mock_token_endpoint(r#"{"access_token":"tok-sa","expires_in":3600}"#);
        let p = GoogleSaProvider::new(
            "mint-test@x.iam.gserviceaccount.com".into(), // unique per test: cache is process-wide
            TEST_KEY_PEM.into(),
            token_url,
            vec!["s1".into()],
        );
        assert_eq!(p.bearer().expect("mint"), "tok-sa");
        let grant = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("grant request");
        assert!(
            grant.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer"),
            "JWT-bearer grant type; got:\n{grant}"
        );
        assert!(
            grant.contains("assertion=eyJ"),
            "signed assertion in the form"
        );
        // Second call: served from the cache — the endpoint sees no second request.
        assert_eq!(p.bearer().expect("cached"), "tok-sa");
        assert!(
            rx.try_recv().is_err(),
            "second bearer() must not re-mint inside the token lifetime"
        );
    }

    /// A token whose `expires_in` is inside [`REFRESH_MARGIN`] is re-minted on the next
    /// call (refresh-ahead-of-expiry).
    #[test]
    fn google_sa_refreshes_ahead_of_expiry() {
        let (token_url, rx) =
            mock_token_endpoint(r#"{"access_token":"tok-short","expires_in":30}"#);
        let p = GoogleSaProvider::new(
            "refresh-test@x.iam.gserviceaccount.com".into(),
            TEST_KEY_PEM.into(),
            token_url,
            vec![],
        );
        assert_eq!(p.bearer().expect("mint 1"), "tok-short");
        assert_eq!(p.bearer().expect("mint 2"), "tok-short");
        // 30s expiry < 60s refresh margin → both calls minted.
        assert!(rx.recv_timeout(std::time::Duration::from_secs(5)).is_ok());
        assert!(
            rx.recv_timeout(std::time::Duration::from_secs(5)).is_ok(),
            "inside the refresh margin the second call must re-mint"
        );
    }
}

#[cfg(test)]
mod egress_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener};
    use std::thread;

    /// Read a full request (headers + content-length body) before responding —
    /// responding after a single `read` races the client's form-body write and resets
    /// the connection mid-exchange (the flaky-mock failure the engine tests fixed the
    /// same way).
    fn read_request(s: &mut std::net::TcpStream) {
        s.set_read_timeout(Some(std::time::Duration::from_millis(500)))
            .ok();
        let mut data = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            match s.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    data.extend_from_slice(&buf[..n]);
                    if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&data[..end]);
                        let clen = head
                            .lines()
                            .find_map(|l| {
                                l.split_once(':')
                                    .filter(|(k, _)| {
                                        k.trim().eq_ignore_ascii_case("content-length")
                                    })
                                    .map(|(_, v)| v)
                            })
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        if data.len() >= end + 4 + clen {
                            break;
                        }
                    }
                }
            }
        }
    }

    /// `OAuth2Provider::mint` performs a real token grant over the wire (the host path)
    /// and returns the access token + caches it — isolated from the wasm guest.
    #[test]
    fn oauth_provider_mints_and_caches_bearer() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                read_request(&mut s);
                let body = r#"{"access_token":"tok-xyz","expires_in":3600}"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = s.write_all(resp.as_bytes());
                let _ = s.flush();
            }
        });
        let p = OAuth2Provider::new(
            format!("http://{addr}/token"),
            OAuthGrantKind::ClientCredentials,
            "cid".into(),
            "csec".into(),
            vec![],
        );
        assert_eq!(p.bearer().expect("mint"), "tok-xyz");
        // Second call is served from cache (no second connection needed).
        assert_eq!(p.bearer().expect("cached"), "tok-xyz");
    }

    /// `SessionProvider::token` logs in over the wire and extracts the token at the
    /// configured dot-path.
    #[test]
    fn session_provider_logs_in_and_extracts_token() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                read_request(&mut s);
                let body = r#"{"data":{"token":"sess-9"}}"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = s.write_all(resp.as_bytes());
                let _ = s.flush();
            }
        });
        let p = SessionProvider::new(
            format!("http://{addr}/login"),
            vec!["data".into(), "token".into()],
            "X-Session-Token".into(),
        );
        assert_eq!(p.token().expect("login"), "sess-9");
        assert_eq!(p.token().expect("cached"), "sess-9");
    }

    #[test]
    fn allow_all_permits_tcp_restricted_denies() {
        let addr: SocketAddr = "10.0.0.5:5432".parse().unwrap();
        // allow_all (empty list) permits any TCP connect — the postgres sink path.
        assert!(HostAllowList::allow_all().authorize_tcp(&addr).is_ok());
        // A non-matching allow-list denies.
        let restricted = HostAllowList {
            allowed_hosts: vec!["1.2.3.4".into()],
            inject_headers: vec![],
            credential: None,
        };
        assert!(restricted.authorize_tcp(&addr).is_err());
        // The listed IP is allowed.
        let listed = HostAllowList {
            allowed_hosts: vec!["10.0.0.5".into()],
            inject_headers: vec![],
            credential: None,
        };
        assert!(listed.authorize_tcp(&addr).is_ok());
    }
}

#[cfg(test)]
mod snowflake_keypair_tests {
    use super::*;

    /// The same throwaway test key as the google tests — any RSA key works here.
    const TEST_KEY_PEM: &str = include_str!("../tests/fixtures/google_sa_test_key.pem");
    /// Known answer for the fixture key, computed independently:
    /// `openssl rsa -pubout -outform DER | openssl dgst -sha256 -binary | base64`.
    const TEST_KEY_FP: &str = "SHA256:BgiJGcINPQuBjB0MFoHV/SW8vP5pzN90TXJe8SpDZtI=";

    #[test]
    fn fingerprint_matches_openssl_spki_known_answer() {
        let key = parse_pkcs8_keypair(TEST_KEY_PEM).expect("parse key");
        assert_eq!(snowflake_fingerprint(&key), TEST_KEY_FP);
    }

    #[test]
    fn jwt_claims_match_snowflake_spec() {
        // Lowercase in → uppercased identifiers out (via the provider's constructor rule).
        let jwt =
            snowflake_jwt("myorg-acct1", "WEIR_DEMO", 1_700_000_000, TEST_KEY_PEM).expect("sign");
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
        use base64::Engine;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let header: serde_json::Value =
            serde_json::from_slice(&b64.decode(parts[0]).unwrap()).unwrap();
        assert_eq!(header["alg"], "RS256");
        let claims: serde_json::Value =
            serde_json::from_slice(&b64.decode(parts[1]).unwrap()).unwrap();
        // `snowflake_jwt` signs what it's given; uppercasing happens in the provider.
        assert_eq!(
            claims["iss"],
            format!("myorg-acct1.WEIR_DEMO.{TEST_KEY_FP}"),
            "iss = account.user.fingerprint"
        );
        assert_eq!(claims["sub"], "myorg-acct1.WEIR_DEMO");
        assert_eq!(claims["iat"], 1_700_000_000u64);
        assert_eq!(claims["exp"], 1_700_003_600u64, "1h validity");
    }

    #[test]
    fn provider_uppercases_identifiers_and_self_signs() {
        let p = SnowflakeKeypairProvider::new(
            "myorg-acct1".into(),
            "weir_demo".into(),
            TEST_KEY_PEM.into(),
        );
        let jwt = p.bearer().expect("self-signed bearer needs no endpoint");
        use base64::Engine;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let claims: serde_json::Value =
            serde_json::from_slice(&b64.decode(jwt.split('.').nth(1).unwrap()).unwrap()).unwrap();
        assert_eq!(claims["sub"], "MYORG-ACCT1.WEIR_DEMO", "uppercased");
        // Cached: a second call returns the identical token (same iat).
        assert_eq!(p.bearer().unwrap(), jwt);
    }

    #[test]
    fn snowflake_credential_strips_only_the_private_key() {
        let cfg = serde_json::json!({
            "auth_scheme": "snowflake_keypair_jwt",
            "snowflake_account_key": "account",
            "snowflake_user_key": "user",
            "snowflake_private_key_key": "private_key",
            "account": "myorg-acct1",
            "user": "weir_demo",
            "private_key": TEST_KEY_PEM,
            "warehouse": "COMPUTE_XS",
        })
        .to_string();
        let (cred, sanitized) = Credential::from_auth_config(&cfg);
        assert!(matches!(cred, Some(Credential::SnowflakeKeypairJwt(_))));
        // The secret + metadata are gone…
        assert!(!sanitized.contains("PRIVATE KEY"));
        assert!(!sanitized.contains("auth_scheme"));
        assert!(!sanitized.contains("snowflake_private_key_key"));
        // …but account/user stay: the manifest templates them into url_base ([[WEIR-T-0157]]).
        assert!(sanitized.contains("\"account\":\"myorg-acct1\""));
        assert!(sanitized.contains("\"user\":\"weir_demo\""));
        assert!(sanitized.contains("\"warehouse\":\"COMPUTE_XS\""));
    }

    /// Both headers land on the outbound request — the bearer and Snowflake's
    /// token-type marker ([[WEIR-T-0156]] AC).
    #[test]
    fn apply_injects_bearer_and_token_type_headers() {
        let (cred, _) = Credential::from_auth_config(
            // A unique account: the token cache is process-wide, so tests sharing a
            // (account, user) key race each other's re-mints.
            &serde_json::json!({
                "auth_scheme": "snowflake_keypair_jwt",
                "account": "apply-test-acct",
                "user": "weir_demo",
                "private_key": TEST_KEY_PEM,
            })
            .to_string(),
        );
        let policy = HostAllowList::allow_all().with_credential(cred.expect("credential"));
        let req = weir_connector::fidius::http_types::Request::builder()
            .uri("https://myorg-acct1.snowflakecomputing.com/api/v2/statements")
            .body(())
            .unwrap();
        let (mut parts, _) = req.into_parts();
        policy.authorize(&mut parts).expect("authorize");
        let auth = parts.headers.get("authorization").expect("bearer header");
        assert!(auth.to_str().unwrap().starts_with("Bearer eyJ"));
        assert_eq!(
            parts
                .headers
                .get("x-snowflake-authorization-token-type")
                .expect("token-type header"),
            "KEYPAIR_JWT"
        );
    }
}
