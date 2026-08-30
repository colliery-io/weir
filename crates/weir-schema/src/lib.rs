//! Portable control-plane schema + migrations for weir ([[WEIR-I-0013]] /
//! [[WEIR-A-0009]]).
//!
//! `schema.rs` and the per-backend migrations under `schema/generated/` are
//! produced from `schema/migrations/` (logical DDL) by `angreal schema gen` —
//! **do not edit them by hand**. Callers use the typed query builder against the
//! re-exported `table!` definitions; `migrate` applies the right per-backend DDL.

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel_dualdb::DualConnection;

/// The generated portable table definitions (`diesel::table!` over diesel-dualdb
/// `sql_types`). Re-exported flat so callers write `weir_schema::dead_letters`.
#[allow(unused_imports)]
pub mod schema {
    include!("../schema/generated/schema.rs");
}
pub use schema::*;

/// Ordered migrations: `(version, postgres-up, sqlite-up)`, each applied once, in order,
/// tracked in `__weir_schema_version`. Append a row here after `angreal schema gen` emits a
/// new `NNNN_name/` migration.
const MIGRATIONS: &[(&str, &str, &str)] = &[
    (
        "0001_init",
        include_str!("../schema/generated/migrations-postgres/0001_init/up.sql"),
        include_str!("../schema/generated/migrations-sqlite/0001_init/up.sql"),
    ),
    (
        "0002_auth",
        include_str!("../schema/generated/migrations-postgres/0002_auth/up.sql"),
        include_str!("../schema/generated/migrations-sqlite/0002_auth/up.sql"),
    ),
    (
        "0003_resident_runtime",
        include_str!("../schema/generated/migrations-postgres/0003_resident_runtime/up.sql"),
        include_str!("../schema/generated/migrations-sqlite/0003_resident_runtime/up.sql"),
    ),
    (
        "0004_verification",
        include_str!("../schema/generated/migrations-postgres/0004_verification/up.sql"),
        include_str!("../schema/generated/migrations-sqlite/0004_verification/up.sql"),
    ),
];

/// Apply every not-yet-applied generated migration for the **active backend**, in order.
/// Idempotent: a portable sentinel table records applied versions, so repeated `open`s — and
/// the store being shared across crates — don't re-run DDL. An existing DB with only
/// `0001_init` recorded picks up `0002_auth` on the next open.
pub fn migrate(conn: &mut DualConnection) -> QueryResult<()> {
    // Serialize concurrent migrators so the (non-idempotent) generated DDL applies
    // exactly once — several weir processes against one Postgres, or parallel tests
    // sharing a database. Postgres: a transaction-scoped advisory lock. SQLite: each
    // store is its own file with serialized writes, so the transaction alone suffices.
    conn.transaction(|conn| {
        if let DualConnection::Pg(pg) = conn {
            // 7274696772 = ASCII 'wrig' — a fixed weir-schema lock key.
            diesel::sql_query("SELECT pg_advisory_xact_lock(7274696772)").execute(pg)?;
        }
        // `IF NOT EXISTS` + TEXT are accepted verbatim by both backends.
        conn.batch_execute(
            "CREATE TABLE IF NOT EXISTS __weir_schema_version (version TEXT PRIMARY KEY NOT NULL)",
        )?;
        for (version, pg_up, sqlite_up) in MIGRATIONS {
            if version_applied(conn, version)? {
                continue;
            }
            // Pick the backend's generated DDL (the only genuinely-divergent step).
            let up = match &*conn {
                DualConnection::Pg(_) => *pg_up,
                DualConnection::Sqlite(_) => *sqlite_up,
            };
            conn.batch_execute(up)?;
            // `version` is a hardcoded const (no injection surface); inline it so the insert
            // stays parameterless + identical on both backends.
            conn.batch_execute(&format!(
                "INSERT INTO __weir_schema_version (version) VALUES ('{version}')"
            ))?;
        }
        Ok(())
    })
}

fn version_applied(conn: &mut DualConnection, version: &str) -> QueryResult<bool> {
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        n: i64,
    }
    // `version` is a hardcoded const; standard SQL, identical on both backends.
    let rows: Vec<Count> = diesel::sql_query(format!(
        "SELECT COUNT(*) AS n FROM __weir_schema_version WHERE version = '{version}'"
    ))
    .load(conn)?;
    Ok(rows.first().map(|r| r.n).unwrap_or(0) > 0)
}
