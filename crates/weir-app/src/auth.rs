//! Control-plane auth ([[WEIR-A-0008]] / [[WEIR-I-0017]]) — mirrors Colliery cloacina. The one
//! primitive is the **API key**: `weirk_<base64url(32)>`, stored as its SHA-256 hex hash (keys are
//! high-entropy → a plain hash gives O(1) lookup + caching, no argon2). `AuthenticatedKey` is what
//! the middleware ([[WEIR-T-0084]]) resolves and projects into the authz `Principal` ([[WEIR-T-0085]]).
//! Tenant + role ride on the key; an OIDC login mints a short-lived key ([[WEIR-T-0086]]).

use crate::{App, AppError};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use diesel::prelude::*;
use diesel_dualdb::types::Uuid as DbUuid;
use rand::Rng;
use sha2::{Digest, Sha256};
use weir_schema::{api_keys, audit_events};

/// The authenticated identity resolved from a valid key (cloacina's `AuthenticatedKey`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedKey {
    pub key_id: String,
    pub name: String,
    /// Role: `read` | `write` | `admin` (projected to a `Level` by the authz seam).
    pub permissions: String,
    /// `None` == a global key (no tenant); admin keys are typically global.
    pub tenant_id: Option<String>,
    /// God-mode: cross-tenant platform superuser.
    pub is_admin: bool,
}

/// Public metadata about a stored key — never the secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyInfo {
    pub id: String,
    pub name: String,
    pub permissions: String,
    pub tenant_id: Option<String>,
    pub is_admin: bool,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub expires_at: Option<i64>,
    pub revoked: bool,
}

const KEY_PREFIX: &str = "weirk_";

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Generate a key `(plaintext, sha256_hash)`. Plaintext = `weirk_<base64url(32 random bytes)>`.
pub fn generate_api_key() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill(&mut bytes);
    let plaintext = format!("{KEY_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes));
    let hash = hash_api_key(&plaintext);
    (plaintext, hash)
}

/// Lowercase hex SHA-256 of a key string (the stored + looked-up form).
pub fn hash_api_key(key: &str) -> String {
    let mut h = Sha256::new();
    h.update(key.as_bytes());
    format!("{:x}", h.finalize())
}

impl App {
    /// Create a manual (non-expiring) key; returns the plaintext **once**.
    pub fn create_api_key(
        &self,
        name: &str,
        permissions: &str,
        tenant_id: Option<&str>,
        is_admin: bool,
    ) -> Result<String, AppError> {
        self.insert_key(name, permissions, tenant_id, is_admin, None, None)
    }

    /// Mint a short-lived key (OIDC login, [[WEIR-T-0086]]) with `expires_at` (epoch-ms) + provenance.
    pub fn mint_api_key(
        &self,
        name: &str,
        permissions: &str,
        tenant_id: Option<&str>,
        is_admin: bool,
        expires_at: i64,
        issued_via: &str,
    ) -> Result<String, AppError> {
        self.insert_key(
            name,
            permissions,
            tenant_id,
            is_admin,
            Some(expires_at),
            Some(issued_via),
        )
    }

    fn insert_key(
        &self,
        name: &str,
        permissions: &str,
        tenant_id: Option<&str>,
        is_admin: bool,
        expires_at: Option<i64>,
        issued_via: Option<&str>,
    ) -> Result<String, AppError> {
        let (plaintext, hash) = generate_api_key();
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        diesel::insert_into(api_keys::table)
            .values((
                api_keys::id.eq(DbUuid(uuid::Uuid::new_v4())),
                api_keys::name.eq(name),
                api_keys::key_hash.eq(&hash),
                api_keys::permissions.eq(permissions),
                api_keys::tenant_id.eq(tenant_id),
                api_keys::is_admin.eq(is_admin as i32),
                api_keys::issued_via.eq(issued_via),
                api_keys::created_at.eq(now_ms()),
                api_keys::expires_at.eq(expires_at),
            ))
            .execute(&mut conn)?;
        Ok(plaintext)
    }

    /// Validate a presented key → the `AuthenticatedKey` (or `None`). Hash lookup (indexed), skips
    /// revoked + expired, stamps `last_used_at`. The middleware caches the result ([[WEIR-T-0084]]).
    pub fn validate_api_key(&self, presented: &str) -> Result<Option<AuthenticatedKey>, AppError> {
        if !presented.starts_with(KEY_PREFIX) {
            return Ok(None);
        }
        let hash = hash_api_key(presented);
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        let row: Option<(DbUuid, String, String, Option<String>, i32, Option<i64>)> =
            api_keys::table
                .filter(api_keys::key_hash.eq(&hash))
                .filter(api_keys::revoked_at.is_null())
                .select((
                    api_keys::id,
                    api_keys::name,
                    api_keys::permissions,
                    api_keys::tenant_id,
                    api_keys::is_admin,
                    api_keys::expires_at,
                ))
                .first(&mut conn)
                .optional()?;
        let Some((id, name, permissions, tenant_id, is_admin, expires_at)) = row else {
            return Ok(None);
        };
        let now = now_ms();
        if expires_at.is_some_and(|exp| exp <= now) {
            return Ok(None); // expired
        }
        diesel::update(api_keys::table.filter(api_keys::key_hash.eq(&hash)))
            .set(api_keys::last_used_at.eq(Some(now)))
            .execute(&mut conn)?;
        Ok(Some(AuthenticatedKey {
            key_id: id.0.to_string(),
            name,
            permissions,
            tenant_id,
            is_admin: is_admin != 0,
        }))
    }

    /// Any keys present? (bootstrap check.)
    pub fn has_any_keys(&self) -> Result<bool, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        let n: i64 = api_keys::table.count().get_result(&mut conn)?;
        Ok(n > 0)
    }

    /// List stored keys (metadata only — never the secret).
    pub fn list_api_keys(&self) -> Result<Vec<ApiKeyInfo>, AppError> {
        // (id, name, permissions, tenant_id, is_admin, created_at, last_used_at, expires_at, revoked_at)
        type KeyRow = (
            DbUuid,
            String,
            String,
            Option<String>,
            i32,
            i64,
            Option<i64>,
            Option<i64>,
            Option<i64>,
        );
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        let rows: Vec<KeyRow> = api_keys::table
            .select((
                api_keys::id,
                api_keys::name,
                api_keys::permissions,
                api_keys::tenant_id,
                api_keys::is_admin,
                api_keys::created_at,
                api_keys::last_used_at,
                api_keys::expires_at,
                api_keys::revoked_at,
            ))
            .load(&mut conn)?;
        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    name,
                    permissions,
                    tenant_id,
                    is_admin,
                    created_at,
                    last_used_at,
                    expires_at,
                    revoked_at,
                )| {
                    ApiKeyInfo {
                        id: id.0.to_string(),
                        name,
                        permissions,
                        tenant_id,
                        is_admin: is_admin != 0,
                        created_at,
                        last_used_at,
                        expires_at,
                        revoked: revoked_at.is_some(),
                    }
                },
            )
            .collect())
    }

    /// Revoke live keys matching `ident` (id, prefix-of-id, or name). Returns the count revoked.
    pub fn revoke_api_key(&self, ident: &str) -> Result<usize, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        // Match by name (exact); id equality is handled by the CLI resolving name→id if needed.
        let n = diesel::update(
            api_keys::table
                .filter(api_keys::revoked_at.is_null())
                .filter(api_keys::name.eq(ident)),
        )
        .set(api_keys::revoked_at.eq(Some(now_ms())))
        .execute(&mut conn)?;
        Ok(n)
    }

    /// Record an audit event ([[WEIR-T-0085]]) — a mutation's actor/action/resource/outcome
    /// (`ok`|`denied`|`error`). Best-effort at the store layer: logs loudly on write failure.
    pub fn record_audit(&self, actor: &str, action: &str, resource: &str, outcome: &str) {
        let write = (|| -> Result<(), AppError> {
            let mut conn = self
                .store
                .pool()
                .get()
                .map_err(|e| AppError::Config(e.to_string()))?;
            diesel::insert_into(audit_events::table)
                .values((
                    audit_events::id.eq(DbUuid(uuid::Uuid::new_v4())),
                    audit_events::actor.eq(actor),
                    audit_events::action.eq(action),
                    audit_events::resource.eq(resource),
                    audit_events::ts.eq(now_ms()),
                    audit_events::outcome.eq(outcome),
                ))
                .execute(&mut conn)?;
            Ok(())
        })();
        if let Err(e) = write {
            eprintln!("audit write failed (actor={actor} action={action} outcome={outcome}): {e}");
        }
    }

    /// Recent audit events (newest first).
    pub fn recent_audit(
        &self,
        limit: i64,
    ) -> Result<Vec<(String, String, String, i64, String)>, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        Ok(audit_events::table
            .select((
                audit_events::actor,
                audit_events::action,
                audit_events::resource,
                audit_events::ts,
                audit_events::outcome,
            ))
            .order(audit_events::ts.desc())
            .limit(limit)
            .load(&mut conn)?)
    }

    /// Mint an initial global **admin** key (`is_admin`, role `admin`, no tenant) iff none exist.
    /// Returns the plaintext once; a no-op (`None`) once any key is present.
    pub fn bootstrap_admin_key(&self) -> Result<Option<String>, AppError> {
        if self.has_any_keys()? {
            return Ok(None);
        }
        Ok(Some(self.create_api_key("admin", "admin", None, true)?))
    }
}

#[cfg(test)]
mod tests {
    use crate::App;

    fn app() -> (App, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let app = App::open(dir.path().join("auth.db").to_str().unwrap()).unwrap();
        (app, dir)
    }

    #[test]
    fn key_format_and_roundtrip() {
        let (plaintext, hash) = super::generate_api_key();
        assert!(plaintext.starts_with("weirk_"));
        assert_eq!(hash.len(), 64); // sha-256 hex
        assert_eq!(super::hash_api_key(&plaintext), hash);
    }

    #[test]
    fn create_then_validate() {
        let (app, _dir) = app();
        let key = app
            .create_api_key("ci", "write", Some("acme"), false)
            .unwrap();
        let ak = app.validate_api_key(&key).unwrap().expect("valid");
        assert_eq!(ak.name, "ci");
        assert_eq!(ak.permissions, "write");
        assert_eq!(ak.tenant_id.as_deref(), Some("acme"));
        assert!(!ak.is_admin);
    }

    #[test]
    fn wrong_and_revoked_and_expired_rejected() {
        let (app, _dir) = app();
        let key = app.create_api_key("temp", "read", None, false).unwrap();
        assert!(app.validate_api_key("weirk_bogus").unwrap().is_none());
        assert!(app.validate_api_key("not-a-key").unwrap().is_none());
        // expired minted key
        let ek = app
            .mint_api_key("old", "read", None, false, 1, "test")
            .unwrap();
        assert!(app.validate_api_key(&ek).unwrap().is_none());
        // revoke
        assert_eq!(app.revoke_api_key("temp").unwrap(), 1);
        assert!(app.validate_api_key(&key).unwrap().is_none());
    }

    #[test]
    fn bootstrap_is_idempotent_and_admin() {
        let (app, _dir) = app();
        let key = app.bootstrap_admin_key().unwrap().expect("first mints");
        let ak = app.validate_api_key(&key).unwrap().unwrap();
        assert!(ak.is_admin);
        assert_eq!(ak.permissions, "admin");
        assert!(app.bootstrap_admin_key().unwrap().is_none());
        assert_eq!(app.list_api_keys().unwrap().len(), 1);
    }
}
