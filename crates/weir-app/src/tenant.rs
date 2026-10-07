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
        self.upsert_tenant(DEFAULT_TENANT, "default")?;
        self.backfill_key_tenants()
    }

    /// Create a tenant row for each tenant that a stored API key names but that has no row
    /// (keys minted before WEIR-T-0210 for a tenant that was never created). `validate_api_key`
    /// refuses a key whose tenant row is missing, so without this an upgrade would lock those
    /// keys out. A deleted tenant is not brought back: the delete cascade removes its keys.
    fn backfill_key_tenants(&self) -> Result<(), AppError> {
        use weir_schema::api_keys;
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        let named: Vec<Option<String>> = api_keys::table
            .select(api_keys::tenant_id)
            .filter(api_keys::tenant_id.is_not_null())
            .distinct()
            .load(&mut conn)?;
        drop(conn);
        for t in named.into_iter().flatten() {
            if !self.tenant_exists(&t)? {
                self.upsert_tenant(&t, &t)?;
            }
        }
        Ok(())
    }

    /// Create (or rename) a tenant; idempotent by id. The id must be a safe slug
    /// ([`crate::TenantId`], [[WEIR-T-0209]]); anything else is refused with `AppError::Config`.
    pub fn create_tenant(&self, id: &str, name: &str) -> Result<(), AppError> {
        let id = crate::TenantId::parse(id)?;
        self.upsert_tenant(id.as_str(), name)
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

    /// Whether a tenant row with this id exists.
    pub fn tenant_exists(&self, id: &str) -> Result<bool, AppError> {
        let mut conn = self
            .store
            .pool()
            .get()
            .map_err(|e| AppError::Config(e.to_string()))?;
        let n: i64 = tenants::table
            .filter(tenants::id.eq(id))
            .count()
            .get_result(&mut conn)?;
        Ok(n > 0)
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

    /// Delete a tenant by id and **cascade** to everything it owns (COLLIERY-I-0252 / WEIR-T-0210).
    /// The `default` tenant cannot be deleted. Returns whether the tenant row existed.
    ///
    /// Order:
    /// 1. Stop the tenant's in-flight runs: fire the in-process stop token of each active
    ///    (`pending`/`leased`) unit of this tenant. A run in another process loses its lease when
    ///    its unit row is deleted in step 2; its next heartbeat fails and fires its stop token.
    /// 2. In ONE transaction, delete every row scoped to the tenant: schedules, connections,
    ///    work units (runs), outbox, stream state + schemas, run logs, dead letters, catalog
    ///    entries, API keys, then the tenant row. Audit events are kept (the audit trail).
    /// 3. Remove the tenant's staged connector files (`<connectors_dir>/<tenant>/`) and drop the
    ///    cached connector handles of those packages.
    ///
    /// Caveat: a batch run that is mid-chunk in another process can commit one more checkpoint
    /// (state/outbox/dead-letter/log rows) before its heartbeat notices the lost lease.
    pub fn delete_tenant(&self, id: &str) -> Result<bool, AppError> {
        use weir_schema::{
            api_keys, connections, connectors, dead_letters, outbox, run_logs, schedules,
            stream_schemas, stream_state, work_units,
        };
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

        // 1. Stop in-flight runs of THIS tenant only (a tenant-scoped query over every
        //    connection of the tenant; `Relay::cancel` is per (tenant, connection)).
        let active: Vec<i64> = work_units::table
            .filter(
                work_units::tenant_id
                    .eq(id)
                    .and(work_units::state.eq_any(["pending", "leased"])),
            )
            .select(work_units::id)
            .load(&mut conn)?;
        for unit in &active {
            self.relay.stop_resident(*unit);
        }

        // 2. Cascade the rows in one transaction.
        let existed = conn.transaction::<_, diesel::result::Error, _>(|conn| {
            diesel::delete(schedules::table.filter(schedules::tenant_id.eq(id))).execute(conn)?;
            diesel::delete(connections::table.filter(connections::tenant_id.eq(id)))
                .execute(conn)?;
            diesel::delete(work_units::table.filter(work_units::tenant_id.eq(id))).execute(conn)?;
            diesel::delete(outbox::table.filter(outbox::tenant_id.eq(id))).execute(conn)?;
            diesel::delete(stream_state::table.filter(stream_state::tenant_id.eq(id)))
                .execute(conn)?;
            diesel::delete(stream_schemas::table.filter(stream_schemas::tenant_id.eq(id)))
                .execute(conn)?;
            diesel::delete(run_logs::table.filter(run_logs::tenant_id.eq(id))).execute(conn)?;
            diesel::delete(dead_letters::table.filter(dead_letters::tenant_id.eq(id)))
                .execute(conn)?;
            diesel::delete(connectors::table.filter(connectors::tenant_id.eq(id))).execute(conn)?;
            diesel::delete(api_keys::table.filter(api_keys::tenant_id.eq(id))).execute(conn)?;
            let n = diesel::delete(tenants::table.filter(tenants::id.eq(id))).execute(conn)?;
            Ok(n > 0)
        })?;

        // 3. Staged connector files (outside the transaction: the filesystem is not
        //    transactional).
        remove_tenant_staging(id)?;
        Ok(existed)
    }
}

/// Remove `<connectors_dir>/<tenant>/` (the per-tenant staging namespace of compiled private
/// crates, [[WEIR-T-0092]]) and drop the cached handles of its packages. No-op if absent.
fn remove_tenant_staging(tenant: &str) -> Result<(), AppError> {
    remove_tenant_staging_in(&crate::connectors_dir(), tenant)
}

fn remove_tenant_staging_in(base: &str, tenant: &str) -> Result<(), AppError> {
    // The join goes through the validated slug ([[WEIR-T-0209]]): `..`, `/`, an empty id or any
    // other non-slug is never joined onto the connectors dir (no-op for such a legacy id).
    let Ok(dir) = crate::tenant_id::tenant_dir(base, tenant) else {
        return Ok(());
    };
    if !dir.is_dir() {
        return Ok(());
    }
    let search_path = dir.to_string_lossy().into_owned();
    for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        if let Some(pkg) = entry.file_name().to_str() {
            weir_orchestrator::ConnectorRef::invalidate_cache(&search_path, pkg);
        }
    }
    std::fs::remove_dir_all(&dir)
        .map_err(|e| AppError::Config(format!("remove tenant connectors dir: {e}")))
}

#[cfg(test)]
mod tests {
    use crate::App;
    use tempfile::TempDir;

    /// A fresh app plus the guard of `CONNECTORS_ENV_LOCK`, with `WEIR_CONNECTORS_DIR` set to a
    /// dir of this test only. `delete_tenant` removes `<connectors_dir>/<tenant>/`, so without the
    /// lock a delete of `acme` here removed the staged artifact of a parallel test that uses the
    /// same tenant id (`compile_isolation_two_tenants_distinct_artifacts`).
    struct TestApp {
        app: App,
        _dir: TempDir,
        _env: std::sync::MutexGuard<'static, ()>,
    }

    impl std::ops::Deref for TestApp {
        type Target = App;
        fn deref(&self) -> &App {
            &self.app
        }
    }

    fn app() -> (TestApp, ()) {
        let env = crate::CONNECTORS_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = TempDir::new().unwrap();
        unsafe { std::env::set_var("WEIR_CONNECTORS_DIR", dir.path().join("connectors")) };
        let db = dir.path().join("t.db");
        let app = App::open(db.to_str().unwrap()).unwrap();
        (
            TestApp {
                app,
                _dir: dir,
                _env: env,
            },
            (),
        )
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

    /// One row of a tenant in each tenant-scoped table. Every row uses the connection name `shared`
    /// so a cross-tenant leak of the cascade (a name-only filter) would show.
    fn populate(app: &App, tenant: &str, id_base: i64) {
        use diesel::prelude::*;
        use diesel_dualdb::types::{Bytes, Uuid as DbUuid};
        use weir_schema::{
            connections, connectors, dead_letters, outbox, run_logs, schedules, stream_schemas,
            stream_state, work_units,
        };
        let uuid = || DbUuid(uuid::Uuid::new_v4());
        let mut c = app.store.pool().get().unwrap();
        let name = "shared";
        diesel::insert_into(connections::table)
            .values((
                connections::tenant_id.eq(tenant),
                connections::name.eq(name),
                connections::source_ref.eq("{}"),
                connections::dest_ref.eq("{}"),
                connections::stream.eq("s"),
                connections::source_config.eq("{}"),
                connections::dest_config.eq("{}"),
                connections::sync_mode.eq("full_refresh"),
                connections::write_mode.eq("append"),
                connections::execution_mode.eq("resident"),
            ))
            .execute(&mut c)
            .unwrap();
        diesel::insert_into(connectors::table)
            .values((
                connectors::tenant_id.eq(tenant),
                connectors::name.eq("rest"),
                connectors::version.eq("1.0.0"),
                connectors::roles.eq("[]"),
                connectors::config_schema.eq("{}"),
                connectors::contract_version.eq(1i64),
                connectors::supported_sync_modes.eq("[]"),
                connectors::origin.eq("private"),
                connectors::status.eq("ok"),
                connectors::location.eq("x"),
                connectors::kind.eq("wasm"),
                connectors::created_at.eq(0i64),
                connectors::updated_at.eq(0i64),
            ))
            .execute(&mut c)
            .unwrap();
        // One finished run + one in-flight (leased) resident run.
        for (i, state) in [(0, "done"), (1, "leased")] {
            diesel::insert_into(work_units::table)
                .values((
                    work_units::id.eq(id_base + i),
                    work_units::tenant_id.eq(tenant),
                    work_units::connection.eq(name),
                    work_units::stream.eq("s"),
                    work_units::source_ref.eq("{}"),
                    work_units::dest_ref.eq("{}"),
                    work_units::state.eq(state),
                ))
                .execute(&mut c)
                .unwrap();
        }
        diesel::insert_into(schedules::table)
            .values((
                schedules::id.eq(id_base),
                schedules::tenant_id.eq(tenant),
                schedules::connection.eq(name),
                schedules::spec.eq("{}"),
                schedules::every_ms.eq(1000i64),
                schedules::next_due_at.eq(0i64),
            ))
            .execute(&mut c)
            .unwrap();
        diesel::insert_into(outbox::table)
            .values((
                outbox::id.eq(uuid()),
                outbox::tenant_id.eq(tenant),
                outbox::connection.eq(name),
                outbox::stream.eq("s"),
                outbox::seq.eq(1i64),
                outbox::processed.eq(0),
            ))
            .execute(&mut c)
            .unwrap();
        diesel::insert_into(stream_state::table)
            .values((
                stream_state::tenant_id.eq(tenant),
                stream_state::connection.eq(name),
                stream_state::stream.eq("s"),
                stream_state::opaque.eq(Bytes(vec![1u8])),
            ))
            .execute(&mut c)
            .unwrap();
        diesel::insert_into(stream_schemas::table)
            .values((
                stream_schemas::tenant_id.eq(tenant),
                stream_schemas::connection.eq(name),
                stream_schemas::stream.eq("s"),
                stream_schemas::schema.eq("{}"),
            ))
            .execute(&mut c)
            .unwrap();
        diesel::insert_into(run_logs::table)
            .values((
                run_logs::id.eq(uuid()),
                run_logs::tenant_id.eq(tenant),
                run_logs::connection.eq(name),
                run_logs::stream.eq("s"),
                run_logs::level.eq("info"),
                run_logs::message.eq("m"),
                run_logs::ts.eq(0i64),
            ))
            .execute(&mut c)
            .unwrap();
        diesel::insert_into(dead_letters::table)
            .values((
                dead_letters::id.eq(uuid()),
                dead_letters::tenant_id.eq(tenant),
                dead_letters::connection.eq(name),
                dead_letters::stream.eq("s"),
                dead_letters::record.eq("{}"),
                dead_letters::reason.eq("r"),
                dead_letters::ts.eq(0i64),
            ))
            .execute(&mut c)
            .unwrap();
    }

    /// The row count of `tenant` in every tenant-scoped table, by table name.
    fn rows_of(app: &App, tenant: &str) -> Vec<(&'static str, i64)> {
        use diesel::prelude::*;
        use weir_schema::{
            api_keys, connections, connectors, dead_letters, outbox, run_logs, schedules,
            stream_schemas, stream_state, tenants, work_units,
        };
        let mut c = app.store.pool().get().unwrap();
        let t = tenant;
        vec![
            (
                "tenants",
                tenants::table
                    .filter(tenants::id.eq(t))
                    .count()
                    .get_result(&mut c)
                    .unwrap(),
            ),
            (
                "connections",
                connections::table
                    .filter(connections::tenant_id.eq(t))
                    .count()
                    .get_result(&mut c)
                    .unwrap(),
            ),
            (
                "connectors",
                connectors::table
                    .filter(connectors::tenant_id.eq(t))
                    .count()
                    .get_result(&mut c)
                    .unwrap(),
            ),
            (
                "work_units",
                work_units::table
                    .filter(work_units::tenant_id.eq(t))
                    .count()
                    .get_result(&mut c)
                    .unwrap(),
            ),
            (
                "schedules",
                schedules::table
                    .filter(schedules::tenant_id.eq(t))
                    .count()
                    .get_result(&mut c)
                    .unwrap(),
            ),
            (
                "outbox",
                outbox::table
                    .filter(outbox::tenant_id.eq(t))
                    .count()
                    .get_result(&mut c)
                    .unwrap(),
            ),
            (
                "stream_state",
                stream_state::table
                    .filter(stream_state::tenant_id.eq(t))
                    .count()
                    .get_result(&mut c)
                    .unwrap(),
            ),
            (
                "stream_schemas",
                stream_schemas::table
                    .filter(stream_schemas::tenant_id.eq(t))
                    .count()
                    .get_result(&mut c)
                    .unwrap(),
            ),
            (
                "run_logs",
                run_logs::table
                    .filter(run_logs::tenant_id.eq(t))
                    .count()
                    .get_result(&mut c)
                    .unwrap(),
            ),
            (
                "dead_letters",
                dead_letters::table
                    .filter(dead_letters::tenant_id.eq(t))
                    .count()
                    .get_result(&mut c)
                    .unwrap(),
            ),
            (
                "api_keys",
                api_keys::table
                    .filter(api_keys::tenant_id.eq(t))
                    .count()
                    .get_result(&mut c)
                    .unwrap(),
            ),
        ]
    }

    #[test]
    fn delete_cascades_to_every_tenant_table_and_spares_other_tenants() {
        let (app, _dir) = app();
        app.create_tenant("acme", "Acme").unwrap();
        app.create_tenant("globex", "Globex").unwrap();
        populate(&app, "acme", 100);
        populate(&app, "globex", 200);
        let acme_key = app
            .create_api_key("acme-ci", "write", Some("acme"), false)
            .unwrap();
        let globex_key = app
            .create_api_key("globex-ci", "write", Some("globex"), false)
            .unwrap();
        assert!(app.validate_api_key(&acme_key).unwrap().is_some());
        let globex_before = rows_of(&app, "globex");
        assert!(
            rows_of(&app, "acme").iter().all(|(_, n)| *n > 0),
            "every table holds acme rows before the delete: {:?}",
            rows_of(&app, "acme")
        );

        assert!(app.delete_tenant("acme").unwrap());

        let left: Vec<_> = rows_of(&app, "acme")
            .into_iter()
            .filter(|(_, n)| *n != 0)
            .collect();
        assert!(left.is_empty(), "acme rows remain after delete: {left:?}");
        assert!(
            app.validate_api_key(&acme_key).unwrap().is_none(),
            "a deleted tenant's key no longer validates"
        );
        // The same-named connection (and everything else) of the other tenant is untouched.
        assert_eq!(rows_of(&app, "globex"), globex_before);
        assert!(app.validate_api_key(&globex_key).unwrap().is_some());
        let leased: i64 = {
            use diesel::prelude::*;
            use weir_schema::work_units;
            let mut c = app.store.pool().get().unwrap();
            work_units::table
                .filter(work_units::tenant_id.eq("globex"))
                .filter(work_units::state.eq("leased"))
                .count()
                .get_result(&mut c)
                .unwrap()
        };
        assert_eq!(leased, 1, "the other tenant's in-flight run is not stopped");
        // Deleting again: the tenant is gone (Ok(false)), not an error.
        assert!(!app.delete_tenant("acme").unwrap());
    }

    #[test]
    fn key_of_a_missing_tenant_does_not_validate() {
        let (app, _dir) = app();
        // Minting a tenant key creates its tenant row when absent ...
        let key = app
            .create_api_key("ci", "write", Some("initech"), false)
            .unwrap();
        assert!(app.tenant_exists("initech").unwrap());
        assert!(app.validate_api_key(&key).unwrap().is_some());
        // ... and a key whose tenant row is gone (removed out of band) is refused.
        {
            use diesel::prelude::*;
            let mut c = app.store.pool().get().unwrap();
            diesel::delete(
                weir_schema::tenants::table.filter(weir_schema::tenants::id.eq("initech")),
            )
            .execute(&mut c)
            .unwrap();
        }
        assert!(app.validate_api_key(&key).unwrap().is_none());
        // Global keys (no tenant) are not affected.
        let global = app.create_api_key("g", "read", None, false).unwrap();
        assert!(app.validate_api_key(&global).unwrap().is_some());
    }

    #[test]
    fn open_backfills_tenant_rows_for_keys_of_uncreated_tenants() {
        // A key stored before WEIR-T-0210 for a tenant that has no row keeps working after upgrade.
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("t.db");
        let key = {
            let app = App::open(db.to_str().unwrap()).unwrap();
            let key = app
                .create_api_key("legacy", "write", Some("legacy"), false)
                .unwrap();
            // Simulate the pre-upgrade state: the key exists, its tenant row does not.
            use diesel::prelude::*;
            let mut c = app.store.pool().get().unwrap();
            diesel::delete(
                weir_schema::tenants::table.filter(weir_schema::tenants::id.eq("legacy")),
            )
            .execute(&mut c)
            .unwrap();
            key
        };
        let app = App::open(db.to_str().unwrap()).unwrap();
        assert!(app.tenant_exists("legacy").unwrap());
        assert!(app.validate_api_key(&key).unwrap().is_some());
    }

    #[test]
    fn staging_dir_of_the_tenant_is_removed_and_others_kept() {
        let base = TempDir::new().unwrap();
        let b = base.path();
        std::fs::create_dir_all(b.join("acme").join("weir-x-pkg")).unwrap();
        std::fs::write(b.join("acme").join("weir-x-pkg").join("f"), "x").unwrap();
        std::fs::create_dir_all(b.join("globex").join("weir-x-pkg")).unwrap();
        std::fs::create_dir_all(b.join("weir-shared-pkg")).unwrap();
        let bs = b.to_str().unwrap();
        super::remove_tenant_staging_in(bs, "acme").unwrap();
        assert!(!b.join("acme").exists());
        assert!(b.join("globex").join("weir-x-pkg").exists());
        assert!(b.join("weir-shared-pkg").exists());
        // Absent tenant dir: no-op. Unsafe ids never touch the base dir.
        super::remove_tenant_staging_in(bs, "nobody").unwrap();
        super::remove_tenant_staging_in(bs, "..").unwrap();
        super::remove_tenant_staging_in(bs, "").unwrap();
        super::remove_tenant_staging_in(bs, "a/b").unwrap();
        assert!(b.join("globex").exists() && b.join("weir-shared-pkg").exists());
    }
}
