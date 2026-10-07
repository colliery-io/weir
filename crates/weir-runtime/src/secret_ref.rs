//! Secret references ([[WEIR-T-0202]] / COLLIERY-I-0253): a secret config field can hold
//! `env:NAME` or `file:/abs/path` instead of the value. Weir stores the reference text only.
//! The host resolves it when it builds the credential, on **each** run, so a rotated
//! variable or file takes effect on the next run ([[WEIR-A-0037]]); nothing caches the
//! resolved value across runs.
//!
//! The syntax is strict so that an ordinary value is not taken as a reference by accident:
//! - `env:NAME` — `NAME` is `[A-Za-z_][A-Za-z0-9_]*`;
//! - `file:/path` — the path is absolute (starts with `/`).
//!
//! Only top-level string values of a config are references (secret fields are top-level
//! keys). The API refuses a reference in a field that is not secret at create (weir-app),
//! so at run time each reference is in a secret field.
//!
//! Errors name the field and the reference, never a resolved value.

use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

/// A file reference reads at most this many bytes (a secret is small; a bigger file is a
/// mistake, not a secret).
pub const MAX_SECRET_FILE_BYTES: u64 = 1024 * 1024;

/// A parsed secret reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretRef<'a> {
    /// `env:NAME` — the value of the host environment variable `NAME`.
    Env(&'a str),
    /// `file:/path` — the contents of the host file at `/path` (one trailing newline removed).
    File(&'a Path),
}

/// Parse `s` as a secret reference. `None` when `s` is an ordinary value.
pub fn parse_secret_ref(s: &str) -> Option<SecretRef<'_>> {
    if let Some(name) = s.strip_prefix("env:") {
        let mut chars = name.chars();
        let first = chars.next()?;
        let valid = (first.is_ascii_alphabetic() || first == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        return valid.then_some(SecretRef::Env(name));
    }
    if let Some(path) = s.strip_prefix("file:") {
        return (path.starts_with('/') && path.len() > 1).then(|| SecretRef::File(Path::new(path)));
    }
    None
}

/// Is `value` a string that holds a secret reference?
pub fn is_secret_ref(value: &Value) -> bool {
    value.as_str().and_then(parse_secret_ref).is_some()
}

/// A secret reference could not be resolved. The message names the field and the
/// reference text, never a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRefError {
    /// The config key that holds the reference.
    pub field: String,
    /// The reference text as stored (`env:NAME` / `file:/path`).
    pub reference: String,
    /// Why it did not resolve.
    pub reason: String,
}

impl std::fmt::Display for SecretRefError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "config field `{}`: secret reference `{}` did not resolve: {}",
            self.field, self.reference, self.reason
        )
    }
}

impl std::error::Error for SecretRefError {}

/// A config with its references resolved.
pub struct ResolvedConfig {
    /// The config JSON with each reference replaced by its value. Host-side only: it goes
    /// to [`crate::Credential::from_auth_config`], which strips the auth secrets before the
    /// guest sees the config.
    pub json: String,
    /// SHA-256 (hex) over the resolved values, `None` when the config holds no reference.
    /// A cache can add it to a key so that a rotated value gives a new entry, without the
    /// value itself in the key.
    pub fingerprint: Option<String>,
}

/// A random salt, drawn once per process, that [`cache_key_digest`] mixes into each
/// digest: a cache key cannot be compared with a hash computed outside this process (a
/// guess at a short secret cannot be checked against it).
fn cache_salt() -> &'static [u8; 32] {
    static SALT: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    SALT.get_or_init(|| {
        use ring::rand::SecureRandom;
        let mut salt = [0u8; 32];
        ring::rand::SystemRandom::new()
            .fill(&mut salt)
            .expect("the system random source is available");
        salt
    })
}

/// The key of a credential cache entry ([[WEIR-T-0204]]): a salted SHA-256 (hex) over
/// `parts`. A cache whose entry depends on secret material (a config, a resolved value, a
/// private key) keys it by this digest, so no secret text is held in a map key, a log line
/// or an error. Each part is length-prefixed, so no two different lists give the same
/// digest; the salt is per process, so a digest is stable only within one process.
pub fn cache_key_digest(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(cache_salt());
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Resolve one reference to its value. `field` and `raw` name it in the error.
fn resolve_one(field: &str, raw: &str, r: SecretRef<'_>) -> Result<String, SecretRefError> {
    let err = |reason: String| SecretRefError {
        field: field.to_string(),
        reference: raw.to_string(),
        reason,
    };
    match r {
        SecretRef::Env(name) => match std::env::var(name) {
            Ok(v) if v.is_empty() => Err(err("the environment variable is empty".into())),
            Ok(v) => Ok(v),
            Err(std::env::VarError::NotPresent) => {
                Err(err("the environment variable is not set".into()))
            }
            Err(std::env::VarError::NotUnicode(_)) => {
                Err(err("the environment variable is not valid UTF-8".into()))
            }
        },
        SecretRef::File(path) => {
            let meta = std::fs::metadata(path)
                .map_err(|e| err(format!("the file cannot be read ({})", e.kind())))?;
            if !meta.is_file() {
                return Err(err("the path is not a regular file".into()));
            }
            if meta.len() > MAX_SECRET_FILE_BYTES {
                return Err(err(format!(
                    "the file is larger than {MAX_SECRET_FILE_BYTES} bytes"
                )));
            }
            let bytes = std::fs::read(path)
                .map_err(|e| err(format!("the file cannot be read ({})", e.kind())))?;
            let mut s =
                String::from_utf8(bytes).map_err(|_| err("the file is not valid UTF-8".into()))?;
            // A secret file usually ends with one newline; it is not part of the value.
            if s.ends_with('\n') {
                s.pop();
                if s.ends_with('\r') {
                    s.pop();
                }
            }
            if s.is_empty() {
                return Err(err("the file is empty".into()));
            }
            Ok(s)
        }
    }
}

/// Resolve each top-level secret reference of `config_json`, reading the environment and
/// the files **now** (call it once per run; never cache the result across runs).
/// A config that is not a JSON object, or that holds no reference, comes back unchanged
/// with no fingerprint.
pub fn resolve_secret_refs(config_json: &str) -> Result<ResolvedConfig, SecretRefError> {
    let unchanged = || ResolvedConfig {
        json: config_json.to_string(),
        fingerprint: None,
    };
    let Ok(Value::Object(mut obj)) = serde_json::from_str::<Value>(config_json) else {
        return Ok(unchanged());
    };
    // serde_json's map is ordered by key, so the fingerprint is stable.
    let mut hasher = Sha256::new();
    let mut any = false;
    for (k, v) in obj.iter_mut() {
        let Some(raw) = v.as_str() else { continue };
        let Some(r) = parse_secret_ref(raw) else {
            continue;
        };
        let value = resolve_one(k, raw, r)?;
        // Length-prefix each part so that no two different sets hash alike.
        for part in [k.as_bytes(), value.as_bytes()] {
            hasher.update((part.len() as u64).to_le_bytes());
            hasher.update(part);
        }
        *v = Value::String(value);
        any = true;
    }
    if !any {
        return Ok(unchanged());
    }
    let fingerprint = hex::encode(hasher.finalize());
    Ok(ResolvedConfig {
        json: Value::Object(obj).to_string(),
        fingerprint: Some(fingerprint),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_only_the_strict_syntax() {
        assert_eq!(parse_secret_ref("env:TOKEN"), Some(SecretRef::Env("TOKEN")));
        assert_eq!(parse_secret_ref("env:_a1"), Some(SecretRef::Env("_a1")));
        assert_eq!(
            parse_secret_ref("file:/run/secrets/pw"),
            Some(SecretRef::File(Path::new("/run/secrets/pw")))
        );
        for plain in [
            "hunter2",
            "env:",
            "env:1ABC",
            "env:A-B",
            "env:A B",
            "file:",
            "file:/",
            "file:relative/path",
            "ENV:TOKEN",
            "https://example.com",
        ] {
            assert_eq!(parse_secret_ref(plain), None, "{plain}");
        }
        assert!(is_secret_ref(&json!("env:X")));
        assert!(!is_secret_ref(&json!({"env": "X"})));
    }

    #[test]
    fn resolves_env_and_file_and_re_reads_each_call() {
        let var = "WEIR_T0202_UNIT_ENV";
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("pw");
        std::fs::write(&file, "file-one\n").unwrap();
        // SAFETY: the variable name is unique to this test.
        unsafe { std::env::set_var(var, "env-one") };
        let cfg = json!({
            "host": "db",
            "api_key": format!("env:{var}"),
            "password": format!("file:{}", file.display()),
        })
        .to_string();

        let a = resolve_secret_refs(&cfg).unwrap();
        let av: Value = serde_json::from_str(&a.json).unwrap();
        assert_eq!(
            av,
            json!({"host":"db","api_key":"env-one","password":"file-one"})
        );
        let fp_a = a.fingerprint.clone().unwrap();
        assert!(!fp_a.contains("env-one") && !fp_a.contains("file-one"));

        // Rotate both: the next call reads the new values and gives a new fingerprint.
        unsafe { std::env::set_var(var, "env-two") };
        std::fs::write(&file, "file-two\r\n").unwrap();
        let b = resolve_secret_refs(&cfg).unwrap();
        let bv: Value = serde_json::from_str(&b.json).unwrap();
        assert_eq!(bv["api_key"], "env-two");
        assert_eq!(bv["password"], "file-two");
        assert_ne!(b.fingerprint.unwrap(), fp_a);
        unsafe { std::env::remove_var(var) };
    }

    #[test]
    fn no_reference_is_unchanged() {
        let cfg = r#"{"api_key":"literal","n":1}"#;
        let r = resolve_secret_refs(cfg).unwrap();
        assert_eq!(r.json, cfg);
        assert!(r.fingerprint.is_none());
        assert_eq!(resolve_secret_refs("not json").unwrap().json, "not json");
    }

    #[test]
    fn errors_name_field_and_reference_never_a_value() {
        let var = "WEIR_T0202_UNIT_MISSING";
        unsafe { std::env::remove_var(var) };
        let e = resolve_secret_refs(&json!({"api_key": format!("env:{var}")}).to_string())
            .err()
            .unwrap();
        assert_eq!(e.field, "api_key");
        let msg = e.to_string();
        assert!(
            msg.contains("`api_key`") && msg.contains(&format!("`env:{var}`")),
            "{msg}"
        );
        assert!(msg.contains("not set"), "{msg}");

        let e = resolve_secret_refs(r#"{"password":"file:/nonexistent/weir-t-0202"}"#)
            .err()
            .unwrap();
        let msg = e.to_string();
        assert!(msg.contains("`file:/nonexistent/weir-t-0202`"), "{msg}");

        // An empty file is refused; the error does not echo the contents of a file.
        let dir = tempfile::TempDir::new().unwrap();
        let empty = dir.path().join("empty");
        std::fs::write(&empty, "\n").unwrap();
        let e = resolve_secret_refs(
            &json!({"password": format!("file:{}", empty.display())}).to_string(),
        )
        .err()
        .unwrap();
        assert!(e.to_string().contains("empty"), "{e}");
    }
}
