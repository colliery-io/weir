//! # weir-manifest
//!
//! The **declarative connector manifest v0** ([[WEIR-A-0014]] §2): a YAML
//! description of a REST source (auth, streams, Arrow schema, incremental
//! cursor, pagination) that [`weir-codegen`] lowers into a `fidius` `Connector`.
//! This crate is the schema + parser only — it has no `fidius`/codegen deps so
//! it stays tiny and shareable.

use serde::{Deserialize, Serialize};

/// Errors parsing/validating a manifest.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("invalid manifest: {0}")]
    Invalid(String),
}

/// A declarative connector manifest (v0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub spec: Spec,
    #[serde(default)]
    pub auth: Auth,
    /// Base URL all stream paths are joined onto, e.g. `https://api.example.com`.
    pub base_url: String,
    pub streams: Vec<Stream>,
}

/// Connector identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    /// Connector name (also the generated plugin/struct identity).
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

/// Authentication strategy. The manifest declares only the *scheme* (and which
/// connection-config keys hold the secret); the secret value is supplied per
/// connection and, as of [[WEIR-A-0033]], is injected **host-side** by the egress
/// policy — it never enters the guest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Auth {
    #[default]
    None,
    /// `Authorization: Bearer <env value>`.
    Bearer { token_env: String },
    /// A custom header carrying the secret.
    ApiKey { header: String, value_env: String },
    /// An api-key carried as a query parameter (`?<param>=<value>`).
    ApiKeyQuery { param: String, value_env: String },
    /// OAuth2 — the host performs the token grant and injects `Authorization:
    /// Bearer <token>`, refreshing on expiry ([[WEIR-A-0033]]). The grant params
    /// below are non-secret; the `*_key` fields name the connection-config keys
    /// holding the (host-side) secrets.
    OAuth2 {
        /// Token endpoint the grant `POST`s to.
        token_url: String,
        grant: OAuthGrant,
        /// Connection-config key holding the client id.
        client_id_key: String,
        /// Connection-config key holding the client secret.
        client_secret_key: String,
        /// Connection-config key holding the refresh token (refresh-token grant only;
        /// absent for client-credentials).
        #[serde(default)]
        refresh_token_key: Option<String>,
        /// OAuth scopes requested in the grant.
        #[serde(default)]
        scopes: Vec<String>,
    },
    /// Session-token — the host performs a login request, extracts the token from
    /// the response at `token_path` (dot-path), and injects it as `inject_header`
    /// on subsequent requests ([[WEIR-A-0033]]).
    SessionToken {
        /// Login endpoint that returns the session token.
        login_url: String,
        /// Dot-path into the login response JSON holding the token (e.g. `data.token`).
        token_path: String,
        /// Header the token is injected as (default `Authorization`).
        inject_header: String,
    },
    /// HTTP Basic — the host injects `Authorization: Basic base64(user:pass)`
    /// ([[WEIR-A-0033]]). The `*_key` fields name the connection-config keys holding the
    /// (host-side) credentials.
    Basic {
        username_key: String,
        password_key: String,
    },
}

/// OAuth2 grant type the host uses to mint an access token ([[WEIR-A-0033]]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthGrant {
    /// `grant_type=refresh_token` (+ client id/secret + refresh token).
    RefreshToken,
    /// `grant_type=client_credentials` (+ client id/secret).
    ClientCredentials,
}

/// One discoverable stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stream {
    pub name: String,
    /// Path appended to `base_url`, e.g. `/posts`.
    pub path: String,
    #[serde(default)]
    pub primary_key: Vec<String>,
    /// Arrow logical schema for the records this stream emits.
    pub schema: Vec<Field>,
    /// JSONPath-ish selector for the record array within the response body.
    /// Empty / absent ⇒ the body is itself the array of records.
    #[serde(default)]
    pub record_selector: Option<String>,
    #[serde(default)]
    pub incremental: Option<Incremental>,
    #[serde(default)]
    pub pagination: Option<Pagination>,
    /// Partition router: read this stream once per slice (a static list, or one slice
    /// per parent-stream record). `None` = a single flat read.
    #[serde(default)]
    pub partition: Option<PartitionRouter>,
    /// Request options ([[WEIR-T-0068]]): HTTP method (`None`/`GET` | `POST`), a POST body
    /// (JSON string, config-templated), and static request headers (e.g. `Notion-Version`).
    #[serde(default)]
    pub http_method: Option<String>,
    #[serde(default)]
    pub request_body: Option<String>,
    #[serde(default)]
    pub request_headers: Vec<(String, String)>,
    /// In-flight record transforms ([[WEIR-T-0071]]): Airbyte `transformations` + `record_filter`
    /// lowered onto the engine's mapping stage ([[WEIR-T-0052]]). Empty = no reshaping.
    #[serde(default)]
    pub mapping: weir_connector_types::MappingSpec,
}

/// Slices a stream into multiple requests (Airbyte's partition routers). Each slice's
/// value is substituted into the request path/params as `{{ stream_partition.<field> }}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PartitionRouter {
    /// A static list of values — one request per value.
    List { field: String, values: Vec<String> },
    /// One slice per record of a parent stream: read `parent_path`, extract `parent_key`
    /// from each record, and template that value into this stream's request. The importer
    /// resolves the parent stream reference into the concrete `parent_path` /
    /// `parent_record_selector` so the runtime needs no cross-stream lookup.
    Substream {
        field: String,
        parent_path: String,
        #[serde(default)]
        parent_record_selector: Option<String>,
        parent_key: String,
    },
}

/// A field in a stream's Arrow schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: ArrowType,
    #[serde(default = "default_true")]
    pub nullable: bool,
}

fn default_true() -> bool {
    true
}

/// The subset of Arrow logical types the manifest can declare (v0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrowType {
    Int64,
    Float64,
    Utf8,
    Bool,
    Timestamp,
}

/// Incremental sync via a cursor field carried as a query parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Incremental {
    /// Record field that advances the cursor (e.g. `updated_at`).
    pub cursor_field: String,
    /// Query parameter the cursor value is sent as (e.g. `since`).
    pub cursor_param: String,
    /// Initial lower-bound cursor for the first run (Airbyte `start_datetime`) — a literal
    /// or a `{{ config['…'] }}` ref the runtime renders. `None` = start from the beginning.
    #[serde(default)]
    pub start_value: Option<String>,
    /// Upper-bound param + value (Airbyte `end_datetime` / `end_time_option`), sent on every
    /// request when set (e.g. `?until=<end>`).
    #[serde(default)]
    pub end_param: Option<String>,
    #[serde(default)]
    pub end_value: Option<String>,
}

/// Pagination strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Pagination {
    /// `?page=N&per_page=S`, 1-based, stop when a short/empty page returns.
    Page {
        page_param: String,
        size_param: String,
        size: u32,
    },
    /// `?offset=N&limit=S`, stop when a short/empty page returns.
    Offset {
        offset_param: String,
        limit_param: String,
        size: u32,
    },
    /// Opaque cursor: read the next-page token at `cursor_path` (dot-sep dpath into the
    /// response) and send it as `?<token_param>=<token>`; stop when the token is absent.
    Cursor {
        cursor_path: String,
        token_param: String,
    },
    /// `Link` header (RFC 5988): follow `Link: <url>; rel="next"` from the response headers
    /// (GitHub-style) until absent. The next URL is absolute.
    LinkHeader,
}

impl Manifest {
    /// Parse + validate a manifest from YAML.
    pub fn from_yaml(yaml: &str) -> Result<Self, ManifestError> {
        let manifest: Manifest = serde_yaml::from_str(yaml)?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<(), ManifestError> {
        if self.spec.name.trim().is_empty() {
            return Err(ManifestError::Invalid("spec.name is empty".into()));
        }
        if self.streams.is_empty() {
            return Err(ManifestError::Invalid(
                "manifest declares no streams".into(),
            ));
        }
        for s in &self.streams {
            if s.schema.is_empty() {
                return Err(ManifestError::Invalid(format!(
                    "stream `{}` declares no schema fields",
                    s.name
                )));
            }
            if let Some(inc) = &s.incremental
                && !s.schema.iter().any(|f| f.name == inc.cursor_field)
            {
                return Err(ManifestError::Invalid(format!(
                    "stream `{}` cursor_field `{}` is not in its schema",
                    s.name, inc.cursor_field
                )));
            }
        }
        Ok(())
    }
}

/// A declarative **destination** manifest ([[WEIR-A-0034]]): a SaaS REST target the
/// `rest-dest` runtime upserts into — the reverse-ETL analogue of [`Manifest`] (the source
/// side). Not codegen: one shared runtime interprets this at run time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DestinationManifest {
    pub spec: Spec,
    #[serde(default)]
    pub auth: Auth,
    /// Base URL all object paths are joined onto.
    pub base_url: String,
    pub objects: Vec<DestObject>,
}

/// One writable object (the destination analogue of a [`Stream`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DestObject {
    /// Logical name — matches the configured stream being activated.
    pub name: String,
    /// Endpoint appended to `base_url`, templated with `{{ record.<field> }}` for upsert-by-key
    /// URLs (e.g. `/contacts/{{ record.id }}`).
    pub path: String,
    /// `POST` (create) or `PATCH` (upsert by the keyed URL). Default `POST`.
    #[serde(default = "default_post")]
    pub method: String,
    /// Business key the upsert is keyed on (the field templated into `path`).
    #[serde(default)]
    pub upsert_key: Option<String>,
    /// record field → SaaS property. Empty = identity (pass the record through).
    #[serde(default)]
    pub field_map: Vec<(String, String)>,
    /// Nest the shaped fields under this key (e.g. HubSpot's `properties`). `None` = top-level.
    #[serde(default)]
    pub body_wrap: Option<String>,
    /// Max records per bulk request where the API supports it. `None` = per-record.
    #[serde(default)]
    pub batch_size: Option<u32>,
}

fn default_post() -> String {
    "POST".to_string()
}

impl DestinationManifest {
    /// Parse + validate a destination manifest from YAML.
    pub fn from_yaml(yaml: &str) -> Result<Self, ManifestError> {
        let m: DestinationManifest = serde_yaml::from_str(yaml)?;
        if m.spec.name.trim().is_empty() {
            return Err(ManifestError::Invalid("spec.name is empty".into()));
        }
        if m.objects.is_empty() {
            return Err(ManifestError::Invalid(
                "destination declares no objects".into(),
            ));
        }
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
spec:
  name: example-rest
base_url: https://api.example.com
auth:
  kind: bearer
  token_env: EXAMPLE_TOKEN
streams:
  - name: posts
    path: /posts
    primary_key: [id]
    schema:
      - { name: id, type: int64, nullable: false }
      - { name: title, type: utf8 }
      - { name: updated_at, type: timestamp }
    incremental:
      cursor_field: updated_at
      cursor_param: since
    pagination:
      kind: page
      page_param: _page
      size_param: _limit
      size: 50
"#;

    #[test]
    fn parses_full_manifest() {
        let m = Manifest::from_yaml(SAMPLE).expect("parse");
        assert_eq!(m.spec.name, "example-rest");
        assert_eq!(m.spec.version, "0.1.0"); // defaulted
        assert_eq!(
            m.auth,
            Auth::Bearer {
                token_env: "EXAMPLE_TOKEN".into()
            }
        );
        assert_eq!(m.streams.len(), 1);
        let s = &m.streams[0];
        assert_eq!(s.path, "/posts");
        assert_eq!(s.schema.len(), 3);
        assert_eq!(s.schema[1].ty, ArrowType::Utf8);
        assert!(s.schema[1].nullable); // defaulted true
        assert_eq!(
            s.incremental.as_ref().unwrap().cursor_param,
            "since".to_string()
        );
        match s.pagination.as_ref().unwrap() {
            Pagination::Page { size, .. } => assert_eq!(*size, 50),
            _ => panic!("expected page pagination"),
        }
    }

    #[test]
    fn rejects_cursor_not_in_schema() {
        let bad = r#"
spec: { name: x }
base_url: https://x
streams:
  - name: s
    path: /s
    schema:
      - { name: id, type: int64 }
    incremental: { cursor_field: nope, cursor_param: since }
"#;
        assert!(Manifest::from_yaml(bad).is_err());
    }

    #[test]
    fn defaults_auth_none() {
        let m = Manifest::from_yaml(
            "spec: { name: x }\nbase_url: https://x\nstreams:\n  - { name: s, path: /s, schema: [ { name: id, type: int64 } ] }\n",
        )
        .unwrap();
        assert_eq!(m.auth, Auth::None);
    }

    #[test]
    fn parses_destination_manifest() {
        let yaml = r#"
spec:
  name: acme-crm
base_url: https://api.acme.com
auth:
  kind: bearer
  token_env: ACME_TOKEN
objects:
  - name: contacts
    path: "/crm/v3/objects/contacts"
    method: POST
    upsert_key: email
    field_map:
      - [email, email]
      - [full_name, name]
    body_wrap: properties
"#;
        let m = DestinationManifest::from_yaml(yaml).expect("parse");
        assert_eq!(m.spec.name, "acme-crm");
        assert_eq!(m.base_url, "https://api.acme.com");
        assert_eq!(
            m.auth,
            Auth::Bearer {
                token_env: "ACME_TOKEN".into()
            }
        );
        assert_eq!(m.objects.len(), 1);
        let o = &m.objects[0];
        assert_eq!(o.name, "contacts");
        assert_eq!(o.method, "POST");
        assert_eq!(o.upsert_key.as_deref(), Some("email"));
        assert_eq!(
            o.field_map,
            vec![
                ("email".into(), "email".into()),
                ("full_name".into(), "name".into())
            ]
        );
        assert_eq!(o.body_wrap.as_deref(), Some("properties"));
    }

    #[test]
    fn destination_manifest_rejects_empty_objects() {
        let bad = "spec: { name: x }\nbase_url: https://x\nobjects: []\n";
        assert!(DestinationManifest::from_yaml(bad).is_err());
    }
}
