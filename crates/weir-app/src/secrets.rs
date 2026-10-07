//! Secret fields of a connection config ([[WEIR-T-0201]] / COLLIERY-I-0253): which keys are secret,
//! how a read redacts them, and how a write keeps the stored value.
//!
//! A key is secret when either rule says so:
//! - the connector's `config_schema` marks the property `airbyte_secret: true` or
//!   `format: "password"` (the same rule the UI form uses);
//! - the baked auth metadata names it ([`auth_secret_keys`]) — `api_key`, or the key that an
//!   `oauth_client_secret_key` / `basic_password_key` / ... entry points at. This mirrors the
//!   host-side strip list in `weir_runtime::Credential::from_auth_config`.
//!
//! A read returns [`SECRET_SENTINEL`] in place of each secret value. A write that sends the
//! sentinel, or omits a secret key, keeps the stored value; a new value replaces it; an empty
//! string clears it. The sentinel on a key with no stored value is an error (nothing to keep).
//!
//! A secret key can hold a reference, `env:NAME` or `file:/path` ([[WEIR-T-0202]]), in place
//! of the value. The host resolves it on each run (`weir_runtime::resolve_secret_refs`); the
//! API never does. A read returns the reference text as stored (it names where the secret is,
//! not the secret). A reference in a key that is not secret is refused at create
//! ([`check_references`]).

use std::collections::BTreeSet;

use serde_json::{Map, Value};

/// The value a read gives in place of a secret, and a write sends to keep the stored value.
pub const SECRET_SENTINEL: &str = "__weir_secret_unchanged__";

/// The top-level properties that a connector `config_schema` marks secret
/// (`airbyte_secret: true` or `format: "password"`). A schema that does not parse gives none.
pub fn schema_secret_keys(config_schema: &str) -> BTreeSet<String> {
    let Ok(schema) = serde_json::from_str::<Value>(config_schema) else {
        return BTreeSet::new();
    };
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return BTreeSet::new();
    };
    props
        .iter()
        .filter(|(_, p)| {
            p.get("airbyte_secret").and_then(Value::as_bool) == Some(true)
                || p.get("format").and_then(Value::as_str) == Some("password")
        })
        .map(|(k, _)| k.clone())
        .collect()
}

/// The keys of `config` that hold a secret value according to the baked `auth_*` metadata.
/// `api_key` is always secret. The `*_key` metadata entries name the key that holds the
/// value (with the same defaults the host uses when the entry is absent).
pub fn auth_secret_keys(config: &Map<String, Value>) -> BTreeSet<String> {
    let get = |k: &str| config.get(k).and_then(Value::as_str).map(str::to_string);
    let named = |meta: &str, default: &str| get(meta).unwrap_or_else(|| default.to_string());
    let mut keys = BTreeSet::from(["api_key".to_string()]);
    match get("auth_scheme").as_deref() {
        Some("oauth2") => {
            keys.insert(named("oauth_client_secret_key", "client_secret"));
            keys.insert(named("oauth_refresh_token_key", "refresh_token"));
        }
        Some("basic") => {
            keys.insert(named("basic_password_key", "password"));
        }
        Some("google_service_account") => {
            keys.insert(named("google_sa_key_key", "service_account_key"));
        }
        Some("snowflake_keypair_jwt") => {
            keys.insert(named("snowflake_private_key_key", "private_key"));
        }
        Some("aws_sigv4") => {
            keys.insert(named("aws_secret_access_key_key", "secret_access_key"));
        }
        Some("session") => {
            // `{{key}}` templates in the login body name config keys the host substitutes.
            if let Some(body) = config.get("session_login_body").and_then(Value::as_object) {
                for v in body.values().filter_map(Value::as_str) {
                    if let Some(k) = v.strip_prefix("{{").and_then(|s| s.strip_suffix("}}")) {
                        keys.insert(k.trim().to_string());
                    }
                }
            }
        }
        _ => {}
    }
    keys
}

/// The secret keys of one side (source or dest) of a connection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SecretFields(BTreeSet<String>);

impl SecretFields {
    /// The secret keys of one side: the schema markers of each given `config_schema`, plus the
    /// baked-auth keys of each given config.
    pub fn for_side<'a>(
        schemas: impl IntoIterator<Item = &'a str>,
        configs: impl IntoIterator<Item = &'a Value>,
    ) -> Self {
        let mut keys = BTreeSet::new();
        for s in schemas {
            keys.extend(schema_secret_keys(s));
        }
        for c in configs {
            if let Some(obj) = c.as_object() {
                keys.extend(auth_secret_keys(obj));
            }
        }
        Self(keys)
    }

    /// Is `key` a secret field?
    pub fn is_secret(&self, key: &str) -> bool {
        self.0.contains(key)
    }
}

/// `config` with the value of each secret key that holds a value replaced by [`SECRET_SENTINEL`].
/// A null or empty-string value is left as it is (there is nothing to hide), and so is a
/// secret reference (`env:` / `file:`, [[WEIR-T-0202]]): the API never resolves it.
pub fn redact(config: &Value, secrets: &SecretFields) -> Value {
    let Some(obj) = config.as_object() else {
        return config.clone();
    };
    let out = obj
        .iter()
        .map(|(k, v)| {
            let hidden = secrets.is_secret(k)
                && !v.is_null()
                && v.as_str() != Some("")
                && !weir_runtime::is_secret_ref(v);
            let v = if hidden {
                Value::String(SECRET_SENTINEL.to_string())
            } else {
                v.clone()
            };
            (k.clone(), v)
        })
        .collect();
    Value::Object(out)
}

/// Refuse a secret reference (`env:NAME` / `file:/path`, [[WEIR-T-0202]]) in a key that is not
/// secret: the host resolves references in secret fields only. `side` names the block in the
/// error message. The message names the key and the reference text (never a value).
pub fn check_references(config: &Value, secrets: &SecretFields, side: &str) -> Result<(), String> {
    let Some(obj) = config.as_object() else {
        return Ok(());
    };
    for (k, v) in obj {
        if weir_runtime::is_secret_ref(v) && !secrets.is_secret(k) {
            return Err(format!(
                "{side} config field `{k}` holds the secret reference `{}`, but `{k}` is not a \
                 secret field — use `env:` / `file:` references in secret fields only",
                v.as_str().unwrap_or_default()
            ));
        }
    }
    Ok(())
}

/// Does `config` send the sentinel as the value of any top-level key?
pub fn has_sentinel(config: &Value) -> bool {
    config
        .as_object()
        .is_some_and(|o| o.values().any(|v| v.as_str() == Some(SECRET_SENTINEL)))
}

/// Merge an incoming config over the `stored` one for a write:
/// - the sentinel keeps the stored value (an error when no value is stored);
/// - a secret key that the incoming config omits keeps the stored value;
/// - an empty string on a secret key clears it (the key is removed);
/// - any other value replaces the stored one.
///
/// `stored` is `None` for a new connection, where any sentinel is an error. `side` names the
/// block in the error message.
pub fn merge(
    incoming: Value,
    stored: Option<&Value>,
    secrets: &SecretFields,
    side: &str,
) -> Result<Value, String> {
    let Value::Object(incoming) = incoming else {
        return Ok(incoming);
    };
    let stored = stored.and_then(Value::as_object);
    let stored_value = |k: &str| stored.and_then(|s| s.get(k)).filter(|v| !v.is_null());
    let mut out = Map::new();
    let mut cleared = BTreeSet::new();
    for (k, v) in incoming {
        if v.as_str() == Some(SECRET_SENTINEL) {
            let kept = stored_value(&k).ok_or_else(|| {
                format!(
                    "{side} config field `{k}` sends `{SECRET_SENTINEL}` but no value is stored \
                     to keep — send the value"
                )
            })?;
            out.insert(k, kept.clone());
        } else if secrets.is_secret(&k) && v.as_str() == Some("") {
            // An empty string clears the secret.
            cleared.insert(k);
        } else {
            out.insert(k, v);
        }
    }
    if let Some(stored) = stored {
        for (k, v) in stored {
            if secrets.is_secret(k) && !v.is_null() && !out.contains_key(k) && !cleared.contains(k)
            {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    Ok(Value::Object(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn secrets(keys: &[&str]) -> SecretFields {
        SecretFields(keys.iter().map(|k| k.to_string()).collect())
    }

    #[test]
    fn schema_marks_airbyte_secret_and_password_format() {
        let schema = json!({"type":"object","properties":{
            "host": {"type":"string"},
            "token": {"type":"string","airbyte_secret":true},
            "password": {"type":"string","format":"password"},
            "not_secret": {"type":"string","airbyte_secret":false},
        }})
        .to_string();
        let keys = schema_secret_keys(&schema);
        assert_eq!(
            keys,
            BTreeSet::from(["password".to_string(), "token".to_string()])
        );
        assert!(schema_secret_keys("not json").is_empty());
        assert!(schema_secret_keys("{}").is_empty());
    }

    #[test]
    fn baked_auth_keys_name_the_secret_values() {
        let keys = |v: Value| auth_secret_keys(v.as_object().unwrap());
        // `api_key` is always secret (bearer / header / query and plain configs).
        assert!(keys(json!({})).contains("api_key"));
        assert!(keys(json!({"auth_scheme":"bearer"})).contains("api_key"));
        // The `*_key` entries name the value key; the metadata key itself is not secret.
        let k = keys(json!({
            "auth_scheme":"oauth2",
            "oauth_client_secret_key":"cs",
            "oauth_refresh_token_key":"rt",
            "oauth_client_id_key":"cid"
        }));
        assert!(k.contains("cs") && k.contains("rt"));
        assert!(!k.contains("cid") && !k.contains("oauth_client_secret_key"));
        // Host defaults when the entry is absent.
        assert!(keys(json!({"auth_scheme":"oauth2"})).contains("client_secret"));
        assert!(keys(json!({"auth_scheme":"basic"})).contains("password"));
        assert!(keys(json!({"auth_scheme":"basic","basic_password_key":"pw"})).contains("pw"));
        assert!(
            keys(json!({"auth_scheme":"google_service_account"})).contains("service_account_key")
        );
        assert!(keys(json!({"auth_scheme":"snowflake_keypair_jwt"})).contains("private_key"));
        assert!(keys(json!({"auth_scheme":"aws_sigv4"})).contains("secret_access_key"));
        let k = keys(json!({
            "auth_scheme":"session",
            "session_login_body": {"user":"{{ username }}","pass":"{{secret_pw}}","x":"lit"}
        }));
        assert!(k.contains("username") && k.contains("secret_pw"));
        assert!(!k.contains("lit"));
    }

    #[test]
    fn for_side_unions_schema_and_auth_rules() {
        let schema = json!({"properties":{"password":{"format":"password"}}}).to_string();
        let cfg = json!({"auth_scheme":"oauth2","oauth_client_secret_key":"cs"});
        let s = SecretFields::for_side([schema.as_str()], [&cfg]);
        assert!(s.is_secret("password") && s.is_secret("cs") && s.is_secret("api_key"));
        assert!(!s.is_secret("host"));
    }

    #[test]
    fn redact_hides_set_secrets_only() {
        let cfg = json!({"host":"h","password":"pw","api_key":"","sa":{"k":"v"},"n":null});
        let out = redact(&cfg, &secrets(&["password", "api_key", "sa", "n"]));
        assert_eq!(
            out,
            json!({"host":"h","password":SECRET_SENTINEL,"api_key":"","sa":SECRET_SENTINEL,"n":null})
        );
    }

    #[test]
    fn redact_returns_references_as_stored() {
        let cfg = json!({"password":"env:DB_PW","api_key":"file:/run/secrets/k","token":"lit"});
        let out = redact(&cfg, &secrets(&["password", "api_key", "token"]));
        assert_eq!(
            out,
            json!({"password":"env:DB_PW","api_key":"file:/run/secrets/k","token":SECRET_SENTINEL})
        );
    }

    #[test]
    fn references_are_refused_outside_secret_fields() {
        let s = secrets(&["password"]);
        assert!(
            check_references(&json!({"password":"env:DB_PW","host":"h"}), &s, "source").is_ok()
        );
        let err = check_references(&json!({"host":"env:DB_HOST"}), &s, "source").unwrap_err();
        assert!(err.contains("source config field `host`"), "{err}");
        assert!(err.contains("`env:DB_HOST`"), "{err}");
        let err = check_references(&json!({"path":"file:/etc/x"}), &s, "dest").unwrap_err();
        assert!(err.contains("dest config field `path`"), "{err}");
        // A value that only looks close is not a reference.
        assert!(check_references(&json!({"host":"env:"}), &s, "source").is_ok());
        assert!(check_references(&json!({"url":"file:relative"}), &s, "source").is_ok());
    }

    #[test]
    fn merge_keeps_replaces_and_clears() {
        let stored = json!({"host":"old","password":"pw","api_key":"k","token":"t"});
        let s = secrets(&["password", "api_key", "token"]);
        let incoming = json!({
            "host":"new",               // non-secret: replaced
            "password":SECRET_SENTINEL, // sentinel: kept
            "api_key":""                // empty: cleared
            // token omitted: kept
        });
        let out = merge(incoming, Some(&stored), &s, "source").unwrap();
        assert_eq!(out, json!({"host":"new","password":"pw","token":"t"}));
        // A new value replaces the stored one.
        let out = merge(json!({"password":"new"}), Some(&stored), &s, "source").unwrap();
        assert_eq!(out["password"], "new");
    }

    #[test]
    fn merge_refuses_sentinel_with_nothing_stored() {
        let s = secrets(&["password"]);
        let err = merge(json!({"password":SECRET_SENTINEL}), None, &s, "dest").unwrap_err();
        assert!(err.contains("dest config field `password`"), "{err}");
        let err = merge(
            json!({"password":SECRET_SENTINEL}),
            Some(&json!({})),
            &s,
            "dest",
        )
        .unwrap_err();
        assert!(err.contains("no value is stored"), "{err}");
        assert!(has_sentinel(&json!({"x":SECRET_SENTINEL})));
        assert!(!has_sentinel(&json!({"x":"y"})));
    }
}
