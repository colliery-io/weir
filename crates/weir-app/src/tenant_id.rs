//! The validated tenant id ([[WEIR-A-0036]] composite identity, [[WEIR-T-0209]]). A tenant id
//! is joined into filesystem paths (the per-tenant connector namespace `<dir>/<tenant>`), so it
//! must be a safe slug: `^[a-z0-9][a-z0-9-]{0,62}$`. No `/`, no `.`, no `..`, no uppercase,
//! no leading `-`. [`TenantId::parse`] is the only way to build one; every filesystem join
//! that uses a tenant id goes through [`tenant_dir`].

use std::path::{Path, PathBuf};

use crate::AppError;

/// The longest tenant id that is accepted (1 leading char + 62 more).
pub const TENANT_ID_MAX_LEN: usize = 63;

/// A tenant id that is known to match `^[a-z0-9][a-z0-9-]{0,62}$`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantId(String);

impl TenantId {
    /// Validate `id` as a tenant slug. The error is an [`AppError::Config`] (HTTP 400).
    pub fn parse(id: &str) -> Result<Self, AppError> {
        if is_valid_tenant_id(id) {
            Ok(Self(id.to_string()))
        } else {
            Err(AppError::Config(format!(
                "invalid tenant id `{id}`: use 1 to {TENANT_ID_MAX_LEN} characters, \
                 lowercase letters, digits and `-` only, starting with a letter or digit"
            )))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for TenantId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether `id` matches `^[a-z0-9][a-z0-9-]{0,62}$`.
pub fn is_valid_tenant_id(id: &str) -> bool {
    let b = id.as_bytes();
    !b.is_empty()
        && b.len() <= TENANT_ID_MAX_LEN
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b[1..]
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// `base/<tenant>` — the only way a tenant id is joined into a filesystem path. Refuses an
/// id that is not a valid slug, so a stored or key-supplied `../x` can never escape `base`.
pub fn tenant_dir(base: impl AsRef<Path>, tenant: &str) -> Result<PathBuf, AppError> {
    let t = TenantId::parse(tenant)?;
    Ok(base.as_ref().join(t.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_safe_slugs() {
        for id in [
            "default",
            "acme",
            "t1",
            "0",
            "a-b-c",
            "9lives",
            &"a".repeat(63),
        ] {
            assert!(TenantId::parse(id).is_ok(), "`{id}` should be valid");
        }
    }

    #[test]
    fn refuses_traversal_and_other_bad_ids() {
        for id in [
            "",
            "../x",
            "..",
            ".",
            "a/b",
            "a\\b",
            "/abs",
            "Acme",
            "-lead",
            "a_b",
            "a.b",
            "a b",
            "ümlaut",
            "a\0b",
            &"a".repeat(64),
        ] {
            let err = TenantId::parse(id).expect_err(id);
            assert!(
                matches!(err, AppError::Config(ref m) if m.contains("invalid tenant id")),
                "`{id}`: {err}"
            );
        }
    }

    #[test]
    fn tenant_dir_joins_only_valid_ids() {
        assert_eq!(
            tenant_dir("/c", "acme").unwrap(),
            PathBuf::from("/c").join("acme")
        );
        assert!(tenant_dir("/c", "../etc").is_err());
    }
}
