//! Tenant CRUD + the implicit `default` tenant ([[WEIR-A-0036]] / [[WEIR-I-0018]]). Every scoped
//! resource carries a `tenant_id` (default `'default'`); single-tenant deploys use [`DEFAULT_TENANT`]
//! transparently. Tenant-scoping of the *other* stores + the `/tenants/*` routes land in [[WEIR-T-0090]];
//! this module is the tenant table + the ensure-default called at open.

use crate::{App, AppError};
use diesel::prelude::*;
use weir_schema::tenants;

/// The implicit tenant for single-tenant deploys + backfilled rows (matches the schema `DEFAULT 'default'`).
pub const DEFAULT_TENANT: &str = "default";

/// A tenant row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl App {
    /// Ensure the `default` tenant exists (idempotent) — called at open so scoped rows that default
    /// their `tenant_id` to `'default'` always reference a real tenant.
    pub fn ensure_default_tenant(&self) -> Result<(), AppError> {
        self.upsert_tenant(DEFAULT_TENANT, "default")
    }

    /// Create (or rename) a tenant; idempotent by id.
    pub fn create_tenant(&self, id: &str, name: &str) -> Result<(), AppError> {
        self.upsert_tenant(id, name)
    }

    fn upsert_tenant(&self, id: &str, name: &str) -> Result<(), AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        // Portable upsert (MultiBackend has no `on_conflict`): update-then-insert.
        let updated = diesel::update(tenants::table.filter(tenants::id.eq(id)))
            .set(tenants::name.eq(name))
            .execute(&mut conn)?;
        if updated == 0 {
            diesel::insert_into(tenants::table)
                .values((
                    tenants::id.eq(id),
                    tenants::name.eq(name),
                    tenants::created_at.eq(now_ms()),
                ))
                .execute(&mut conn)?;
        }
        Ok(())
    }

    /// List all tenants, ascending by id.
    pub fn list_tenants(&self) -> Result<Vec<Tenant>, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        let rows: Vec<(String, String, i64)> = tenants::table
            .select((tenants::id, tenants::name, tenants::created_at))
            .order(tenants::id.asc())
            .load(&mut conn)?;
        Ok(rows
            .into_iter()
            .map(|(id, name, created_at)| Tenant {
                id,
                name,
                created_at,
            })
            .collect())
    }

    /// Delete a tenant by id. The `default` tenant cannot be deleted.
    pub fn delete_tenant(&self, id: &str) -> Result<bool, AppError> {
        if id == DEFAULT_TENANT {
            return Err(AppError::Config(
                "the default tenant cannot be deleted".into(),
            ));
        }
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        let n = diesel::delete(tenants::table.filter(tenants::id.eq(id))).execute(&mut conn)?;
        Ok(n > 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::App;
    use tempfile::TempDir;

    fn app() -> (App, TempDir) {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("t.db");
        let app = App::open(db.to_str().unwrap()).unwrap();
        (app, dir)
    }

    #[test]
    fn default_tenant_ensured_at_open() {
        let (app, _dir) = app();
        let ts = app.list_tenants().unwrap();
        assert!(
            ts.iter().any(|t| t.id == "default"),
            "default tenant exists after open"
        );
    }

    #[test]
    fn create_list_delete() {
        let (app, _dir) = app();
        app.create_tenant("acme", "Acme Inc").unwrap();
        assert!(
            app.list_tenants()
                .unwrap()
                .iter()
                .any(|t| t.id == "acme" && t.name == "Acme Inc")
        );
        // rename (idempotent upsert)
        app.create_tenant("acme", "Acme Corp").unwrap();
        assert!(
            app.list_tenants()
                .unwrap()
                .iter()
                .any(|t| t.id == "acme" && t.name == "Acme Corp")
        );
        assert!(app.delete_tenant("acme").unwrap());
        assert!(!app.list_tenants().unwrap().iter().any(|t| t.id == "acme"));
        // default is protected
        assert!(app.delete_tenant("default").is_err());
    }
}
