//! # weir-importer
//!
//! Maps an **Airbyte low-code (declarative) manifest** onto a
//! [`weir_manifest::Manifest`] ([[WEIR-A-0020]] / [[WEIR-A-0003]]). The produced
//! manifest is fed to `weir-codegen`, so "Airbyte YAML → connector" reuses the
//! whole codegen path. v0 covers the common declarative shape; unknown fields
//! are ignored and unsupported constructs surface a clear [`ImportError`].

use std::collections::BTreeMap;

use serde::Deserialize;
use weir_connector_types::{CompareOp, ComputeExpr, MappingOp, MappingSpec};
use weir_manifest::{
    ArrowType, Auth, Field, Incremental, Manifest, ManifestError, OAuthGrant, Pagination,
    PartitionRouter, Spec, Stream,
};

/// Errors importing an Airbyte manifest.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("unsupported manifest: {0}")]
    Unsupported(String),
    #[error("produced an invalid weir manifest: {0}")]
    Manifest(#[from] ManifestError),
}

// ---- Airbyte declarative-manifest subset (only the fields we read) ----

#[derive(Debug, Deserialize)]
pub struct AirbyteManifest {
    #[serde(default)]
    streams: Vec<DeclarativeStream>,
}

#[derive(Debug, Deserialize)]
struct DeclarativeStream {
    name: String,
    retriever: Retriever,
    #[serde(default)]
    primary_key: PrimaryKey,
    #[serde(default)]
    incremental_sync: Option<IncrementalSync>,
    #[serde(default)]
    schema_loader: Option<SchemaLoader>,
    /// In-flight record transforms ([[WEIR-T-0071]]) — `AddFields` / `RemoveFields`.
    #[serde(default)]
    transformations: Vec<Transformation>,
}

/// Airbyte record transforms lowered onto the engine mapping stage ([[WEIR-T-0071]]).
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum Transformation {
    AddFields {
        #[serde(default)]
        fields: Vec<AddField>,
    },
    RemoveFields {
        #[serde(default)]
        field_pointers: Vec<Vec<String>>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct AddField {
    #[serde(default)]
    path: Vec<String>,
    #[serde(default)]
    value: Option<String>,
}

/// `record_filter` under the record selector: a jinja `condition` string.
#[derive(Debug, Deserialize)]
struct RecordFilter {
    #[serde(default)]
    condition: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Retriever {
    requester: Requester,
    #[serde(default)]
    record_selector: Option<RecordSelector>,
    #[serde(default)]
    paginator: Option<Paginator>,
    #[serde(default)]
    partition_router: Option<AbPartitionRouter>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AbPartitionRouter {
    ListPartitionRouter {
        /// Static list of slice values (a YAML sequence). A `{{ config[...] }}` template
        /// here isn't a static list → reported unsupported.
        #[serde(default)]
        values: Option<serde_yaml::Value>,
        /// The partition field name (Airbyte keys it `cursor_field` on this router).
        #[serde(default)]
        cursor_field: Option<String>,
    },
    SubstreamPartitionRouter {
        #[serde(default)]
        parent_stream_configs: Vec<ParentStreamConfig>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct ParentStreamConfig {
    #[serde(default)]
    parent_key: String,
    #[serde(default)]
    partition_field: String,
    /// The parent stream — inline (or a YAML anchor, expanded at parse). Held as a raw
    /// value so an unexpected `$ref` shape doesn't fail the whole manifest parse.
    #[serde(default)]
    stream: Option<serde_yaml::Value>,
}

#[derive(Debug, Deserialize)]
struct Requester {
    url_base: String,
    path: String,
    #[serde(default)]
    authenticator: Option<Authenticator>,
    /// Request options ([[WEIR-T-0068]]): `GET`/`POST`, a JSON body, static headers.
    #[serde(default, rename = "http_method")]
    http_method: Option<String>,
    #[serde(default)]
    request_body_json: Option<serde_yaml::Value>,
    #[serde(default)]
    request_headers: Option<serde_yaml::Value>,
}

// Variant names mirror Airbyte's `type` tag values (the serde tag), so they
// must keep the `Authenticator` suffix.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum Authenticator {
    BearerAuthenticator {
        api_token: String,
    },
    ApiKeyAuthenticator {
        #[serde(default)]
        header: String,
        api_token: String,
        /// Airbyte's structured injection: `{ field_name, inject_into: header|request_parameter }`.
        /// Present → carries the key as a query param or a named header; absent → the flat
        /// `header` field (or `Authorization`).
        #[serde(default)]
        inject_into: Option<RequestInjection>,
    },
    /// OAuth2 — mapped to a host-side grant ([[WEIR-A-0033]]). The `client_id` /
    /// `client_secret` / `refresh_token` are `{{ config['…'] }}` references; we keep the
    /// config-key names, never the values.
    OAuthAuthenticator {
        token_refresh_endpoint: String,
        #[serde(default)]
        client_id: String,
        #[serde(default)]
        client_secret: String,
        #[serde(default)]
        refresh_token: Option<String>,
        /// `refresh_token` (default) | `client_credentials`.
        #[serde(default)]
        grant_type: Option<String>,
        #[serde(default)]
        scopes: Vec<String>,
    },
    /// Session token — host logs in via `login_requester`, extracts the token at
    /// `session_token_path`, injects it per `request_authentication` ([[WEIR-A-0033]]).
    SessionTokenAuthenticator {
        #[serde(default)]
        login_requester: Option<LoginRequester>,
        #[serde(default)]
        session_token_path: Vec<String>,
        #[serde(default)]
        request_authentication: Option<RequestAuthentication>,
    },
    /// HTTP Basic — `username` / `password` are `{{ config['…'] }}` refs; host injects
    /// `Authorization: Basic base64(user:pass)` ([[WEIR-A-0033]]).
    BasicHttpAuthenticator {
        #[serde(default)]
        username: String,
        #[serde(default)]
        password: String,
    },
    #[serde(other)]
    Other,
}

/// The minimal slice of Airbyte's `SessionTokenAuthenticator.login_requester` we need:
/// where the login request goes.
#[derive(Debug, Deserialize)]
struct LoginRequester {
    url_base: String,
    #[serde(default)]
    path: String,
}

/// How the session token is attached to subsequent requests.
#[derive(Debug, Deserialize)]
struct RequestAuthentication {
    #[serde(default)]
    inject_into: Option<RequestInjection>,
}

#[derive(Debug, Deserialize)]
struct RequestInjection {
    #[serde(default)]
    field_name: Option<String>,
    /// `header` | `request_parameter` (the Airbyte location tag, also keyed `inject_into`).
    #[serde(default, rename = "inject_into")]
    location: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RecordSelector {
    extractor: Extractor,
    #[serde(default)]
    record_filter: Option<RecordFilter>,
}

#[derive(Debug, Deserialize)]
struct Extractor {
    #[serde(default)]
    field_path: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum Paginator {
    DefaultPaginator {
        pagination_strategy: PaginationStrategy,
        #[serde(default)]
        page_size_option: Option<RequestOption>,
        #[serde(default)]
        page_token_option: Option<RequestOption>,
    },
    #[serde(other)]
    NoPagination,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum PaginationStrategy {
    PageIncrement {
        #[serde(default)]
        page_size: Option<u32>,
    },
    OffsetIncrement {
        #[serde(default)]
        page_size: Option<u32>,
    },
    CursorPagination {
        /// e.g. `{{ response['response_metadata']['next_cursor'] }}` — the next-page token.
        cursor_value: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct RequestOption {
    #[serde(default)]
    field_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum IncrementalSync {
    DatetimeBasedCursor {
        cursor_field: String,
        #[serde(default)]
        start_time_option: Option<RequestOption>,
        #[serde(default)]
        end_time_option: Option<RequestOption>,
        /// A literal / `{{ config[...] }}` string, or a `MinMaxDatetime` object with a
        /// `datetime` field. Kept as a raw value so unexpected shapes don't fail the parse;
        /// boxed to keep the enum variant small (clippy `large_enum_variant`).
        #[serde(default)]
        start_datetime: Option<Box<serde_yaml::Value>>,
        #[serde(default)]
        end_datetime: Option<Box<serde_yaml::Value>>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum SchemaLoader {
    InlineSchemaLoader {
        schema: JsonSchema,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct JsonSchema {
    #[serde(default)]
    properties: BTreeMap<String, Prop>,
}

#[derive(Debug, Deserialize)]
struct Prop {
    #[serde(rename = "type", default)]
    ty: Option<serde_yaml::Value>,
    #[serde(default)]
    format: Option<String>,
}

/// Airbyte `primary_key` is a string, an array, or an array of arrays.
#[derive(Debug, Default, Deserialize)]
#[serde(untagged)]
enum PrimaryKey {
    One(String),
    Many(Vec<String>),
    Nested(Vec<Vec<String>>),
    #[default]
    None,
}

impl PrimaryKey {
    fn to_vec(&self) -> Vec<String> {
        match self {
            PrimaryKey::One(s) => vec![s.clone()],
            PrimaryKey::Many(v) => v.clone(),
            PrimaryKey::Nested(vv) => vv.iter().flatten().cloned().collect(),
            PrimaryKey::None => Vec::new(),
        }
    }
}

// ---- Mapping ----

/// Parse Airbyte YAML and map it to a (validated) weir [`Manifest`] named `name`.
pub fn import_yaml(name: &str, yaml: &str) -> Result<Manifest, ImportError> {
    let airbyte: AirbyteManifest = serde_yaml::from_str(yaml)?;
    to_weir_manifest(name, &airbyte)
}

/// Map a parsed Airbyte manifest to a weir [`Manifest`].
pub fn to_weir_manifest(name: &str, airbyte: &AirbyteManifest) -> Result<Manifest, ImportError> {
    let first = airbyte
        .streams
        .first()
        .ok_or_else(|| ImportError::Unsupported("manifest declares no streams".into()))?;

    let base_url = first.retriever.requester.url_base.clone();
    let auth = match &first.retriever.requester.authenticator {
        Some(Authenticator::BearerAuthenticator { api_token }) => Auth::Bearer {
            token_env: env_ref(api_token),
        },
        Some(Authenticator::ApiKeyAuthenticator {
            header,
            api_token,
            inject_into,
        }) => {
            let value_env = env_ref(api_token);
            let injected_field = inject_into.as_ref().and_then(|i| i.field_name.clone());
            let location = inject_into.as_ref().and_then(|i| i.location.as_deref());
            if location == Some("request_parameter") {
                Auth::ApiKeyQuery {
                    param: injected_field.unwrap_or_else(|| "api_key".into()),
                    value_env,
                }
            } else {
                // Header mode: explicit `header`, else the injected field name, else default.
                let h = if !header.is_empty() {
                    header.clone()
                } else {
                    injected_field.unwrap_or_else(|| "Authorization".into())
                };
                Auth::ApiKey {
                    header: h,
                    value_env,
                }
            }
        }
        Some(Authenticator::OAuthAuthenticator {
            token_refresh_endpoint,
            client_id,
            client_secret,
            refresh_token,
            grant_type,
            scopes,
        }) => {
            let grant = match grant_type.as_deref() {
                Some("client_credentials") => OAuthGrant::ClientCredentials,
                // Default + explicit `refresh_token`: refresh-token grant.
                _ => OAuthGrant::RefreshToken,
            };
            Auth::OAuth2 {
                token_url: token_refresh_endpoint.clone(),
                grant,
                client_id_key: cfg_key_ref(client_id, "client_id"),
                client_secret_key: cfg_key_ref(client_secret, "client_secret"),
                refresh_token_key: match grant {
                    OAuthGrant::RefreshToken => Some(
                        refresh_token
                            .as_deref()
                            .map(|r| cfg_key_ref(r, "refresh_token"))
                            .unwrap_or_else(|| "refresh_token".to_string()),
                    ),
                    OAuthGrant::ClientCredentials => None,
                },
                scopes: scopes.clone(),
            }
        }
        Some(Authenticator::SessionTokenAuthenticator {
            login_requester: Some(lr),
            session_token_path,
            request_authentication,
        }) => Auth::SessionToken {
            login_url: format!("{}{}", lr.url_base, lr.path),
            token_path: session_token_path.join("."),
            inject_header: request_authentication
                .as_ref()
                .and_then(|ra| ra.inject_into.as_ref())
                .and_then(|i| i.field_name.clone())
                .unwrap_or_else(|| "Authorization".to_string()),
        },
        Some(Authenticator::BasicHttpAuthenticator { username, password }) => Auth::Basic {
            username_key: cfg_key_ref(username, "username"),
            password_key: cfg_key_ref(password, "password"),
        },
        _ => Auth::None,
    };

    let mut streams = Vec::new();
    for s in &airbyte.streams {
        let record_selector = s
            .retriever
            .record_selector
            .as_ref()
            .and_then(|rs| rs.extractor.field_path.first().cloned());

        let incremental = match &s.incremental_sync {
            Some(IncrementalSync::DatetimeBasedCursor {
                cursor_field,
                start_time_option,
                end_time_option,
                start_datetime,
                end_datetime,
            }) => Some(Incremental {
                cursor_field: cursor_field.clone(),
                cursor_param: start_time_option
                    .as_ref()
                    .and_then(|o| o.field_name.clone())
                    .unwrap_or_else(|| cursor_field.clone()),
                start_value: start_datetime.as_ref().and_then(|v| datetime_str(v)),
                end_param: end_time_option.as_ref().and_then(|o| o.field_name.clone()),
                end_value: end_datetime.as_ref().and_then(|v| datetime_str(v)),
            }),
            _ => None,
        };

        let schema = map_schema(&s.schema_loader);
        if schema.is_empty() {
            return Err(ImportError::Unsupported(format!(
                "stream `{}` has no inline schema (v0 needs an InlineSchemaLoader)",
                s.name
            )));
        }

        let req = &s.retriever.requester;
        let request_headers: Vec<(String, String)> = req
            .request_headers
            .as_ref()
            .and_then(|v| v.as_mapping())
            .map(|m| {
                m.iter()
                    .filter_map(|(k, val)| {
                        Some((k.as_str()?.to_string(), val.as_str()?.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        streams.push(Stream {
            name: s.name.clone(),
            path: req.path.clone(),
            primary_key: s.primary_key.to_vec(),
            schema,
            record_selector,
            incremental,
            pagination: map_paginator(&s.retriever.paginator),
            partition: s
                .retriever
                .partition_router
                .as_ref()
                .and_then(map_partition_router),
            http_method: req.http_method.clone(),
            // `request_body_json` (an object) → the POST body as a JSON string.
            request_body: req
                .request_body_json
                .as_ref()
                .and_then(|v| serde_json::to_string(v).ok()),
            request_headers,
            mapping: map_transforms(s),
        });
    }

    let manifest = Manifest {
        spec: Spec {
            name: name.to_string(),
            version: "0.1.0".to_string(),
        },
        auth,
        base_url,
        streams,
    };

    // Round-trip through the parser so the output is guaranteed valid + serializable.
    let yaml = serde_yaml::to_string(&manifest)?;
    Ok(Manifest::from_yaml(&yaml)?)
}

/// A pre-onboard **fidelity report** for a declarative manifest ([[WEIR-T-0055]]):
/// what the shared runtime will + won't support, *without* running anything. The
/// preview gate ([[WEIR-A-0020]]) so we never onboard a silently-broken connector.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportReport {
    /// `A` full · `B` degraded · `C` significant gaps · `F` unparseable.
    pub tier: String,
    /// 0.0–1.0 fraction of declared features the runtime supports.
    pub confidence: f32,
    pub streams: Vec<String>,
    /// Named, human-readable gaps — never silently dropped.
    pub unsupported: Vec<String>,
}

/// Analyze a declarative manifest against the **shared runtime's current
/// capability** (HTTP `base_url`/`path`/record-selector, page-increment pagination,
/// datetime cursor). Auth, offset pagination, and anything the importer can't parse
/// are reported as gaps. Pure — no run, no registration.
pub fn analyze(yaml: &str) -> ImportReport {
    let m: AirbyteManifest = match serde_yaml::from_str(yaml) {
        Ok(m) => m,
        Err(e) => {
            return ImportReport {
                tier: "F".to_string(),
                confidence: 0.0,
                streams: Vec::new(),
                unsupported: vec![format!("unparseable manifest: {e}")],
            };
        }
    };
    let mut streams = Vec::new();
    let mut unsupported = Vec::new();
    if m.streams.is_empty() {
        unsupported.push("no streams declared".to_string());
    }
    for s in &m.streams {
        streams.push(s.name.clone());
        match &s.retriever.requester.authenticator {
            // Bearer + header api-key are applied by the runtime ([[WEIR-I-0008]]); the
            // secret is supplied per-connection as `api_key`, so it's a config
            // requirement, not an unsupported construct.
            // Bearer / api-key / OAuth2 / session-token are all injected host-side by the
            // egress policy ([[WEIR-A-0033]]); the secret is supplied per-connection, so
            // these are a config requirement, not an unsupported construct.
            Some(Authenticator::BearerAuthenticator { .. })
            | Some(Authenticator::ApiKeyAuthenticator { .. })
            | Some(Authenticator::OAuthAuthenticator { .. })
            | Some(Authenticator::SessionTokenAuthenticator { .. })
            | Some(Authenticator::BasicHttpAuthenticator { .. }) => {}
            Some(Authenticator::Other) => {
                unsupported.push(format!("{}: unsupported authenticator", s.name))
            }
            None => {}
        }
        if let Some(Paginator::DefaultPaginator {
            pagination_strategy,
            ..
        }) = &s.retriever.paginator
        {
            match pagination_strategy {
                // Page-increment, offset, and opaque cursor are applied by the runtime.
                PaginationStrategy::PageIncrement { .. }
                | PaginationStrategy::OffsetIncrement { .. }
                | PaginationStrategy::CursorPagination { .. } => {}
                PaginationStrategy::Other => {
                    unsupported.push(format!("{}: unsupported pagination strategy", s.name))
                }
            }
        }
        if let Some(IncrementalSync::Other) = &s.incremental_sync {
            unsupported.push(format!("{}: unsupported incremental cursor", s.name));
        }
        // List + substream routers are supported ([[WEIR-T-0064]]); a router present but
        // not translatable (custom, templated `values`, `$ref` parent) is reported so we
        // don't silently drop the partitioning.
        if let Some(pr) = &s.retriever.partition_router
            && map_partition_router(pr).is_none()
        {
            unsupported.push(format!("{}: unsupported partition router", s.name));
        }
        // Transforms map onto the engine mapping stage ([[WEIR-T-0071]]); forms the lowering
        // can't express (complex AddFields values, unknown transforms, compound conditions)
        // are reported so they're never silently dropped ([[WEIR-A-0020]]).
        for t in &s.transformations {
            match t {
                Transformation::AddFields { fields } => {
                    for f in fields {
                        if let Some(raw) = &f.value
                            && f.path.last().is_some()
                            && compute_expr(raw).is_none()
                        {
                            unsupported
                                .push(format!("{}: unsupported AddFields value `{raw}`", s.name));
                        }
                    }
                }
                Transformation::RemoveFields { .. } => {}
                Transformation::Other => {
                    unsupported.push(format!("{}: unsupported transformation", s.name))
                }
            }
        }
        if let Some(cond) = s
            .retriever
            .record_selector
            .as_ref()
            .and_then(|r| r.record_filter.as_ref())
            .and_then(|rf| rf.condition.as_ref())
            && filter_op(cond).is_none()
        {
            unsupported.push(format!("{}: unsupported record_filter `{cond}`", s.name));
        }
        if !matches!(
            s.schema_loader,
            Some(SchemaLoader::InlineSchemaLoader { .. })
        ) {
            unsupported.push(format!(
                "{}: no inline schema (record types unknown)",
                s.name
            ));
        }
    }
    let (tier, confidence) = if streams.is_empty() {
        ("F".to_string(), 0.0)
    } else {
        let ratio = 1.0 - (unsupported.len() as f32 / (streams.len() as f32 * 4.0)).min(1.0);
        let tier = if unsupported.is_empty() {
            "A"
        } else if ratio > 0.6 {
            "B"
        } else {
            "C"
        };
        (tier.to_string(), ratio)
    };
    ImportReport {
        tier,
        confidence,
        streams,
        unsupported,
    }
}

/// Map an Airbyte partition router → weir [`PartitionRouter`]. Returns `None` (no
/// routing, reported as a gap by `analyze`) for shapes we can't translate: templated
/// `ListPartitionRouter` values, a `$ref`/missing parent stream, or a custom router.
fn map_partition_router(r: &AbPartitionRouter) -> Option<PartitionRouter> {
    match r {
        AbPartitionRouter::ListPartitionRouter {
            values,
            cursor_field,
        } => {
            let vals: Vec<String> = values
                .as_ref()
                .and_then(|v| v.as_sequence())
                .map(|seq| {
                    seq.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            if vals.is_empty() {
                return None; // templated/config-driven values aren't a static list (v1)
            }
            Some(PartitionRouter::List {
                field: cursor_field
                    .clone()
                    .unwrap_or_else(|| "partition".to_string()),
                values: vals,
            })
        }
        AbPartitionRouter::SubstreamPartitionRouter {
            parent_stream_configs,
        } => {
            let pc = parent_stream_configs.first()?;
            let (parent_path, parent_record_selector) = parent_request(pc.stream.as_ref()?)?;
            Some(PartitionRouter::Substream {
                field: if pc.partition_field.is_empty() {
                    "parent_id".to_string()
                } else {
                    pc.partition_field.clone()
                },
                parent_path,
                parent_record_selector,
                parent_key: if pc.parent_key.is_empty() {
                    "id".to_string()
                } else {
                    pc.parent_key.clone()
                },
            })
        }
        AbPartitionRouter::Other => None,
    }
}

/// Pull the request `path` + record selector out of an (inline / anchor-expanded) parent
/// stream value, without requiring it to deserialize as a full `DeclarativeStream`.
fn parent_request(stream: &serde_yaml::Value) -> Option<(String, Option<String>)> {
    let retriever = stream.get("retriever")?;
    let path = retriever
        .get("requester")?
        .get("path")?
        .as_str()?
        .to_string();
    let record_selector = retriever
        .get("record_selector")
        .and_then(|rs| rs.get("extractor"))
        .and_then(|e| e.get("field_path"))
        .and_then(|fp| fp.as_sequence())
        .and_then(|s| s.first())
        .and_then(|v| v.as_str())
        .map(String::from);
    Some((path, record_selector))
}

/// Lower a stream's Airbyte transforms + `record_filter` onto the engine mapping stage
/// ([[WEIR-T-0071]] / [[WEIR-T-0052]]). Clean forms map; jinja grammar beyond the simple
/// cases is skipped (reported by `analyze`, not silently emitted as wrong).
fn map_transforms(s: &DeclarativeStream) -> MappingSpec {
    let mut ops: Vec<MappingOp> = Vec::new();
    for t in &s.transformations {
        match t {
            Transformation::RemoveFields { field_pointers } => {
                let fields: Vec<String> = field_pointers
                    .iter()
                    .filter_map(|p| p.last().cloned())
                    .collect();
                if !fields.is_empty() {
                    ops.push(MappingOp::Drop { fields });
                }
            }
            Transformation::AddFields { fields } => {
                for f in fields {
                    if let (Some(field), Some(raw)) = (f.path.last().cloned(), f.value.as_ref())
                        && let Some(value) = compute_expr(raw)
                    {
                        ops.push(MappingOp::Compute { field, value });
                    }
                }
            }
            Transformation::Other => {}
        }
    }
    if let Some(cond) = s
        .retriever
        .record_selector
        .as_ref()
        .and_then(|r| r.record_filter.as_ref())
        .and_then(|rf| rf.condition.as_ref())
        && let Some(op) = filter_op(cond)
    {
        ops.push(op);
    }
    MappingSpec { ops }
}

/// A **lone** field reference `record['x']` / `record["x"]` / `record.x` → `x`. Rejects
/// anything that isn't a clean identifier (so compound expressions parse to `None`, not junk).
fn record_field(s: &str) -> Option<String> {
    let rest = s.trim().strip_prefix("record")?.trim();
    let field = if let Some(b) = rest.strip_prefix('[') {
        // Must be the whole `[...]` — nothing trailing (else it's a larger expression).
        b.strip_suffix(']')?
            .trim()
            .trim_matches(|c| c == '\'' || c == '"')
            .to_string()
    } else if let Some(f) = rest.strip_prefix('.') {
        f.trim().to_string()
    } else {
        return None;
    };
    (!field.is_empty() && field.chars().all(|c| c.is_alphanumeric() || c == '_')).then_some(field)
}

/// An `AddFields` value → a bounded compute expr: a literal (`Const`) or a single record
/// reference (`Field`). Richer jinja (concat, filters, arithmetic) is skipped/reported.
fn compute_expr(raw: &str) -> Option<ComputeExpr> {
    let t = raw.trim();
    if !t.contains("{{") {
        return Some(ComputeExpr::Const(t.to_string()));
    }
    let inner = t.trim_start_matches("{{").trim_end_matches("}}").trim();
    record_field(inner).map(ComputeExpr::Field)
}

/// A `record_filter` condition `{{ record['f'] OP value }}` → a `Filter` op. Compound
/// conditions (and/or, functions) are skipped/reported.
fn filter_op(cond: &str) -> Option<MappingOp> {
    let inner = cond
        .trim()
        .trim_start_matches("{{")
        .trim_end_matches("}}")
        .trim();
    // Two-char operators before one-char so `>=`/`<=` aren't split as `>`/`<`.
    for (sym, op) in [
        ("==", CompareOp::Eq),
        ("!=", CompareOp::Ne),
        (">=", CompareOp::Ge),
        ("<=", CompareOp::Le),
        (">", CompareOp::Gt),
        ("<", CompareOp::Lt),
    ] {
        if let Some(idx) = inner.find(sym) {
            let field = record_field(&inner[..idx])?;
            let rhs = inner[idx + sym.len()..].trim();
            // Reject compound conditions (a second field ref / boolean join) — reported, not
            // mis-parsed into a wrong single filter.
            if rhs.contains("record") || rhs.contains(" and ") || rhs.contains(" or ") {
                return None;
            }
            let value = rhs.trim_matches(|c| c == '\'' || c == '"');
            return Some(MappingOp::Filter {
                field,
                op,
                value: value.to_string(),
            });
        }
    }
    None
}

/// Extract a datetime string from an Airbyte `start_datetime`/`end_datetime`: either a bare
/// string (literal or `{{ config[...] }}`) or a `MinMaxDatetime` object's `datetime` field.
fn datetime_str(v: &serde_yaml::Value) -> Option<String> {
    match v {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Mapping(_) => {
            v.get("datetime").and_then(|d| d.as_str()).map(String::from)
        }
        _ => None,
    }
}

fn map_paginator(p: &Option<Paginator>) -> Option<Pagination> {
    let Some(Paginator::DefaultPaginator {
        pagination_strategy,
        page_size_option,
        page_token_option,
    }) = p
    else {
        return None;
    };
    let page_param = page_token_option
        .as_ref()
        .and_then(|o| o.field_name.clone());
    let size_param = page_size_option.as_ref().and_then(|o| o.field_name.clone());
    match pagination_strategy {
        PaginationStrategy::PageIncrement { page_size } => Some(Pagination::Page {
            page_param: page_param.unwrap_or_else(|| "page".into()),
            size_param: size_param.unwrap_or_else(|| "page_size".into()),
            size: page_size.unwrap_or(50),
        }),
        PaginationStrategy::OffsetIncrement { page_size } => Some(Pagination::Offset {
            offset_param: page_param.unwrap_or_else(|| "offset".into()),
            limit_param: size_param.unwrap_or_else(|| "limit".into()),
            size: page_size.unwrap_or(50),
        }),
        PaginationStrategy::CursorPagination { cursor_value } => {
            // Link-header form: the cursor references the response `Link` header (rel="next"),
            // not a body path → follow the header ([[WEIR-T-0070]]). Otherwise a body cursor.
            let lc = cursor_value.to_ascii_lowercase();
            if lc.contains("headers") && lc.contains("link") {
                Some(Pagination::LinkHeader)
            } else {
                Some(Pagination::Cursor {
                    cursor_path: response_path(cursor_value),
                    token_param: page_param.unwrap_or_else(|| "cursor".into()),
                })
            }
        }
        PaginationStrategy::Other => None,
    }
}

/// Extract the dpath from a `{{ response['a']['b'] }}` cursor template → `"a.b"`
/// (the bracketed keys after `response`). Returns "" if no brackets found.
fn response_path(template: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let mut rest = template;
    while let Some(i) = rest.find('[') {
        rest = &rest[i + 1..];
        let key = rest.trim_start_matches([' ', '\'', '"']);
        match key.find(['\'', '"']) {
            Some(end) => {
                parts.push(&key[..end]);
                rest = &key[end..];
            }
            None => break,
        }
    }
    parts.join(".")
}

fn map_schema(loader: &Option<SchemaLoader>) -> Vec<Field> {
    let Some(SchemaLoader::InlineSchemaLoader { schema }) = loader else {
        return Vec::new();
    };
    schema
        .properties
        .iter()
        .map(|(name, prop)| Field {
            name: name.clone(),
            ty: arrow_type(prop),
            nullable: is_nullable(prop),
        })
        .collect()
}

fn type_strings(v: &Option<serde_yaml::Value>) -> Vec<String> {
    match v {
        Some(serde_yaml::Value::String(s)) => vec![s.clone()],
        Some(serde_yaml::Value::Sequence(seq)) => seq
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

fn arrow_type(p: &Prop) -> ArrowType {
    let types = type_strings(&p.ty);
    let primary = types
        .iter()
        .find(|t| *t != "null")
        .map(String::as_str)
        .unwrap_or("string");
    match primary {
        "integer" => ArrowType::Int64,
        "number" => ArrowType::Float64,
        "boolean" => ArrowType::Bool,
        "string" if p.format.as_deref() == Some("date-time") => ArrowType::Timestamp,
        _ => ArrowType::Utf8,
    }
}

fn is_nullable(p: &Prop) -> bool {
    let types = type_strings(&p.ty);
    types.iter().any(|t| t == "null") || types.len() != 1
}

/// Best-effort extraction of an env-var name from an Airbyte token reference like
/// `{{ config['api_key'] }}` → `API_KEY`. Falls back to `API_TOKEN`.
fn env_ref(token: &str) -> String {
    if let Some(start) = token.find("config[") {
        let inner = token[start + "config[".len()..].trim_start_matches(['\'', '"']);
        if let Some(end) = inner.find(['\'', '"']) {
            return inner[..end].to_uppercase();
        }
    }
    "API_TOKEN".to_string()
}

/// Extract the connection-config **key** from a `{{ config['key'] }}` reference,
/// **preserving case** — the host reads the secret from the connection config by this
/// exact key ([[WEIR-A-0033]]). Falls back to `default` when the token isn't a config
/// reference. (Distinct from [`env_ref`], which uppercases for the legacy env-var form.)
fn cfg_key_ref(token: &str, default: &str) -> String {
    if let Some(start) = token.find("config[") {
        let inner = token[start + "config[".len()..].trim_start_matches(['\'', '"']);
        if let Some(end) = inner.find(['\'', '"']) {
            return inner[..end].to_string();
        }
    }
    default.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const AIRBYTE: &str = r#"
type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: posts
    primary_key: id
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.example.com"
        path: "/posts"
        authenticator:
          type: BearerAuthenticator
          api_token: "{{ config['api_key'] }}"
      record_selector:
        type: RecordSelector
        extractor:
          type: DpathExtractor
          field_path: ["data"]
      paginator:
        type: DefaultPaginator
        page_size_option:
          type: RequestOption
          field_name: "_limit"
          inject_into: request_parameter
        page_token_option:
          type: RequestOption
          field_name: "_page"
        pagination_strategy:
          type: PageIncrement
          page_size: 50
    incremental_sync:
      type: DatetimeBasedCursor
      cursor_field: updated_at
      start_time_option:
        type: RequestOption
        field_name: "since"
    schema_loader:
      type: InlineSchemaLoader
      schema:
        type: object
        properties:
          id: { type: integer }
          title: { type: ["null", "string"] }
          updated_at: { type: string, format: date-time }
"#;

    #[test]
    fn imports_declarative_manifest() {
        let m = import_yaml("jsonplaceholder", AIRBYTE).expect("import");
        assert_eq!(m.spec.name, "jsonplaceholder");
        assert_eq!(m.base_url, "https://api.example.com");
        assert_eq!(
            m.auth,
            Auth::Bearer {
                token_env: "API_KEY".into()
            }
        );

        assert_eq!(m.streams.len(), 1);
        let s = &m.streams[0];
        assert_eq!(s.path, "/posts");
        assert_eq!(s.primary_key, vec!["id".to_string()]);
        assert_eq!(s.record_selector.as_deref(), Some("data"));

        let inc = s.incremental.as_ref().expect("incremental");
        assert_eq!(inc.cursor_field, "updated_at");
        assert_eq!(inc.cursor_param, "since");

        match s.pagination.as_ref().expect("pagination") {
            Pagination::Page {
                page_param,
                size_param,
                size,
            } => {
                assert_eq!(page_param, "_page");
                assert_eq!(size_param, "_limit");
                assert_eq!(*size, 50);
            }
            _ => panic!("expected page pagination"),
        }

        // Schema: BTreeMap ordering → id, title, updated_at.
        let names: Vec<_> = s.schema.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["id", "title", "updated_at"]);
        assert_eq!(s.schema[0].ty, ArrowType::Int64);
        assert!(!s.schema[0].nullable); // single concrete type
        assert_eq!(s.schema[1].ty, ArrowType::Utf8);
        assert!(s.schema[1].nullable); // ["null","string"]
        assert_eq!(s.schema[2].ty, ArrowType::Timestamp); // string + date-time
    }

    #[test]
    fn errors_without_streams() {
        let r = import_yaml("x", "type: DeclarativeSource\nstreams: []\n");
        assert!(r.is_err());
    }

    const CLEAN: &str = r#"
type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: items
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.example.com"
        path: "/items"
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
          id: { type: integer }
"#;

    #[test]
    fn analyze_reports_runtime_gaps_and_clean_tier() {
        // AIRBYTE uses BearerAuthenticator — now applied by the runtime (the secret is
        // supplied per-connection), so it's a config requirement, not a gap: clean tier A.
        let r = analyze(AIRBYTE);
        assert_eq!(r.streams, vec!["posts"]);
        assert!(
            !r.unsupported.iter().any(|u| u.contains("auth")),
            "auth is applied now, not a gap: {:?}",
            r.unsupported
        );
        assert_eq!(r.tier, "A");

        // A clean no-auth manifest → tier A, no gaps.
        let rc = analyze(CLEAN);
        assert!(
            rc.unsupported.is_empty(),
            "clean → no gaps: {:?}",
            rc.unsupported
        );
        assert_eq!(rc.tier, "A");
        assert!((rc.confidence - 1.0).abs() < f32::EPSILON);

        // Garbage → tier F, named.
        let rf = analyze("not: [a, valid: manifest");
        assert_eq!(rf.tier, "F");
        assert!(!rf.unsupported.is_empty());
    }

    /// A single-stream manifest whose requester carries `auth_block` (the block must
    /// bring its own 8-space indentation to sit under `requester:`).
    fn manifest_with_auth(auth_block: &str) -> String {
        format!(
            r#"
type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: items
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.example.com"
        path: "/items"
{auth_block}
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
          id: {{ type: integer }}
"#
        )
    }

    #[test]
    fn imports_oauth_refresh_token() {
        let yaml = manifest_with_auth(
            "        authenticator:\n          type: OAuthAuthenticator\n          \
             token_refresh_endpoint: \"https://api.example.com/oauth/token\"\n          \
             client_id: \"{{ config['client_id'] }}\"\n          \
             client_secret: \"{{ config['client_secret'] }}\"\n          \
             refresh_token: \"{{ config['refresh_token'] }}\"\n          \
             scopes: [\"read\", \"write\"]",
        );
        let m = import_yaml("oauthy", &yaml).expect("import");
        assert_eq!(
            m.auth,
            Auth::OAuth2 {
                token_url: "https://api.example.com/oauth/token".into(),
                grant: OAuthGrant::RefreshToken,
                client_id_key: "client_id".into(),
                client_secret_key: "client_secret".into(),
                refresh_token_key: Some("refresh_token".into()),
                scopes: vec!["read".into(), "write".into()],
            }
        );
    }

    #[test]
    fn imports_oauth_client_credentials() {
        let yaml = manifest_with_auth(
            "        authenticator:\n          type: OAuthAuthenticator\n          \
             token_refresh_endpoint: \"https://api.example.com/oauth/token\"\n          \
             grant_type: client_credentials\n          \
             client_id: \"{{ config['client_id'] }}\"\n          \
             client_secret: \"{{ config['client_secret'] }}\"",
        );
        let m = import_yaml("ccreds", &yaml).expect("import");
        match m.auth {
            Auth::OAuth2 {
                grant,
                refresh_token_key,
                ..
            } => {
                assert_eq!(grant, OAuthGrant::ClientCredentials);
                assert_eq!(refresh_token_key, None); // no refresh token for client-credentials
            }
            other => panic!("expected OAuth2 client_credentials, got {other:?}"),
        }
    }

    #[test]
    fn imports_session_token() {
        let yaml = manifest_with_auth(
            "        authenticator:\n          type: SessionTokenAuthenticator\n          \
             login_requester:\n            url_base: \"https://api.example.com\"\n            \
             path: \"/login\"\n          session_token_path: [\"data\", \"token\"]\n          \
             request_authentication:\n            type: ApiKey\n            inject_into:\n              \
             type: RequestOption\n              field_name: \"X-Session-Token\"\n              \
             inject_into: header",
        );
        let m = import_yaml("sessiony", &yaml).expect("import");
        assert_eq!(
            m.auth,
            Auth::SessionToken {
                login_url: "https://api.example.com/login".into(),
                token_path: "data.token".into(),
                inject_header: "X-Session-Token".into(),
            }
        );
    }

    #[test]
    fn imports_basic_auth() {
        let yaml = manifest_with_auth(
            "        authenticator:\n          type: BasicHttpAuthenticator\n          \
             username: \"{{ config['username'] }}\"\n          \
             password: \"{{ config['password'] }}\"",
        );
        let m = import_yaml("basicy", &yaml).expect("import");
        assert_eq!(
            m.auth,
            Auth::Basic {
                username_key: "username".into(),
                password_key: "password".into()
            }
        );
    }

    #[test]
    fn imports_link_header_pagination() {
        let yaml = r#"
type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: items
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.example.com"
        path: "/items"
      record_selector:
        type: RecordSelector
        extractor:
          type: DpathExtractor
          field_path: []
      paginator:
        type: DefaultPaginator
        pagination_strategy:
          type: CursorPagination
          cursor_value: "{{ headers['link']['next']['url'] }}"
        page_token_option:
          type: RequestPath
    schema_loader:
      type: InlineSchemaLoader
      schema:
        type: object
        properties:
          id: { type: integer }
"#;
        let m = import_yaml("linky", yaml).expect("import");
        assert_eq!(m.streams[0].pagination, Some(Pagination::LinkHeader));
    }

    #[test]
    fn imports_richer_datetime_cursor() {
        let yaml = r#"
type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: items
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.example.com"
        path: "/items"
      record_selector:
        type: RecordSelector
        extractor:
          type: DpathExtractor
          field_path: []
    incremental_sync:
      type: DatetimeBasedCursor
      cursor_field: updated_at
      start_datetime: "{{ config['start_date'] }}"
      end_datetime: "2026-12-31T00:00:00Z"
      start_time_option:
        type: RequestOption
        field_name: "since"
      end_time_option:
        type: RequestOption
        field_name: "until"
    schema_loader:
      type: InlineSchemaLoader
      schema:
        type: object
        properties:
          id: { type: integer }
          updated_at: { type: string, format: date-time }
"#;
        let m = import_yaml("dty", yaml).expect("import");
        let inc = m.streams[0].incremental.as_ref().expect("incremental");
        assert_eq!(inc.cursor_field, "updated_at");
        assert_eq!(inc.cursor_param, "since");
        assert_eq!(
            inc.start_value.as_deref(),
            Some("{{ config['start_date'] }}")
        );
        assert_eq!(inc.end_param.as_deref(), Some("until"));
        assert_eq!(inc.end_value.as_deref(), Some("2026-12-31T00:00:00Z"));
    }

    #[test]
    fn imports_post_request_options() {
        let yaml = r#"
type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: items
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.example.com"
        path: "/query"
        http_method: POST
        request_body_json:
          page_size: 100
        request_headers:
          Notion-Version: "2022-06-28"
      record_selector:
        type: RecordSelector
        extractor:
          type: DpathExtractor
          field_path: ["results"]
    schema_loader:
      type: InlineSchemaLoader
      schema:
        type: object
        properties:
          id: { type: integer }
"#;
        let m = import_yaml("posty", yaml).expect("import");
        let s = &m.streams[0];
        assert_eq!(s.http_method.as_deref(), Some("POST"));
        assert_eq!(s.request_body.as_deref(), Some(r#"{"page_size":100}"#));
        assert_eq!(
            s.request_headers,
            vec![("Notion-Version".to_string(), "2022-06-28".to_string())]
        );
    }

    #[test]
    fn imports_transforms_to_mapping() {
        let yaml = r#"
type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: items
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.example.com"
        path: "/items"
      record_selector:
        type: RecordSelector
        extractor:
          type: DpathExtractor
          field_path: []
        record_filter:
          type: RecordFilter
          condition: "{{ record['status'] == 'active' }}"
    transformations:
      - type: AddFields
        fields:
          - path: ["source"]
            value: "manual"
          - path: ["name_copy"]
            value: "{{ record['name'] }}"
      - type: RemoveFields
        field_pointers:
          - ["secret"]
    schema_loader:
      type: InlineSchemaLoader
      schema:
        type: object
        properties:
          id: { type: integer }
"#;
        let m = import_yaml("xf", yaml).expect("import");
        assert_eq!(
            m.streams[0].mapping.ops,
            vec![
                MappingOp::Compute {
                    field: "source".into(),
                    value: ComputeExpr::Const("manual".into())
                },
                MappingOp::Compute {
                    field: "name_copy".into(),
                    value: ComputeExpr::Field("name".into())
                },
                MappingOp::Drop {
                    fields: vec!["secret".into()]
                },
                MappingOp::Filter {
                    field: "status".into(),
                    op: CompareOp::Eq,
                    value: "active".into()
                },
            ]
        );
    }

    #[test]
    fn analyze_reports_complex_transforms() {
        let yaml = r#"
type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: items
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.example.com"
        path: "/items"
      record_selector:
        type: RecordSelector
        extractor:
          type: DpathExtractor
          field_path: []
        record_filter:
          type: RecordFilter
          condition: "{{ record['x'] > 5 and record['y'] < 10 }}"
    transformations:
      - type: AddFields
        fields:
          - path: ["combo"]
            value: "{{ record['a'] ~ record['b'] }}"
    schema_loader:
      type: InlineSchemaLoader
      schema:
        type: object
        properties:
          id: { type: integer }
"#;
        let r = analyze(yaml);
        assert!(
            r.unsupported.iter().any(|u| u.contains("AddFields")),
            "complex AddFields reported: {:?}",
            r.unsupported
        );
        assert!(
            r.unsupported.iter().any(|u| u.contains("record_filter")),
            "compound record_filter reported: {:?}",
            r.unsupported
        );
    }

    #[test]
    fn analyze_treats_oauth_as_supported() {
        // OAuth is injected host-side ([[WEIR-A-0033]], proven by the wasm wire test), so
        // it's a config requirement — not a coverage gap.
        let yaml = manifest_with_auth(
            "        authenticator:\n          type: OAuthAuthenticator\n          \
             token_refresh_endpoint: \"https://api.example.com/oauth/token\"\n          \
             client_id: \"{{ config['client_id'] }}\"\n          \
             client_secret: \"{{ config['client_secret'] }}\"\n          \
             refresh_token: \"{{ config['refresh_token'] }}\"",
        );
        let r = analyze(&yaml);
        assert!(
            !r.unsupported
                .iter()
                .any(|u| u.contains("oauth") || u.contains("auth")),
            "oauth is supported host-side, not a gap: {:?}",
            r.unsupported
        );
        assert_eq!(r.tier, "A");
    }

    #[test]
    fn imports_list_partition_router() {
        let yaml = r#"
type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: items
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.example.com"
        path: "/cat/{{ stream_partition.category }}/items"
      record_selector:
        type: RecordSelector
        extractor:
          type: DpathExtractor
          field_path: []
      partition_router:
        type: ListPartitionRouter
        cursor_field: category
        values: ["a", "b", "c"]
    schema_loader:
      type: InlineSchemaLoader
      schema:
        type: object
        properties:
          id: { type: integer }
"#;
        let m = import_yaml("listy", yaml).expect("import");
        assert_eq!(
            m.streams[0].partition,
            Some(PartitionRouter::List {
                field: "category".into(),
                values: vec!["a".into(), "b".into(), "c".into()],
            })
        );
    }

    #[test]
    fn imports_substream_partition_router() {
        let yaml = r#"
type: DeclarativeSource
streams:
  - type: DeclarativeStream
    name: comments
    retriever:
      type: SimpleRetriever
      requester:
        type: HttpRequester
        url_base: "https://api.example.com"
        path: "/posts/{{ stream_partition.post_id }}/comments"
      record_selector:
        type: RecordSelector
        extractor:
          type: DpathExtractor
          field_path: []
      partition_router:
        type: SubstreamPartitionRouter
        parent_stream_configs:
          - type: ParentStreamConfig
            parent_key: id
            partition_field: post_id
            stream:
              type: DeclarativeStream
              name: posts
              retriever:
                type: SimpleRetriever
                requester:
                  type: HttpRequester
                  url_base: "https://api.example.com"
                  path: "/posts"
                record_selector:
                  type: RecordSelector
                  extractor:
                    type: DpathExtractor
                    field_path: ["data"]
    schema_loader:
      type: InlineSchemaLoader
      schema:
        type: object
        properties:
          id: { type: integer }
"#;
        let m = import_yaml("substreamy", yaml).expect("import");
        assert_eq!(
            m.streams[0].partition,
            Some(PartitionRouter::Substream {
                field: "post_id".into(),
                parent_path: "/posts".into(),
                parent_record_selector: Some("data".into()),
                parent_key: "id".into(),
            })
        );
    }
}
