//! MSSQL (TDS) statement builders + type mapping ([[WEIR-T-0161]]): pure functions for
//! the `mssql` source guest — hoisted here so the SQL/type logic is unit-testable
//! (`cdylib` guests can't `cargo test`, [[WEIR-I-0032]]).
//!
//! Reads use SQL Server's **`FOR JSON PATH`** so the server produces JSON — the direct
//! analogue of the `postgres` guest's `row_to_json`, sidestepping per-type row decoding.
//! Identifiers are `[bracket]`-quoted; the incremental cursor compares as text (matching
//! the postgres connector's `::text >` semantics — datetime/ISO cursors sort correctly).

use crate::FieldType;

/// Bracket-quote an identifier (`]` doubled) — MSSQL's delimited-identifier form.
pub fn quote_ident(name: &str) -> String {
    format!("[{}]", name.replace(']', "]]"))
}

/// A possibly `schema.table` name → `[schema].[table]` (or just `[table]`).
pub fn qualified_table(name: &str) -> String {
    match name.split_once('.') {
        Some((schema, table)) => format!("{}.{}", quote_ident(schema), quote_ident(table)),
        None => quote_ident(name),
    }
}

/// Full-refresh read: the whole table as a JSON array (`FOR JSON PATH`). SQL Server
/// returns **no rows** for an empty result, so the guest treats an empty concat as `[]`.
pub fn full_select_json_sql(table: &str) -> String {
    format!(
        "SELECT * FROM {} FOR JSON PATH, INCLUDE_NULL_VALUES",
        qualified_table(table)
    )
}

/// Incremental read: rows past the cursor, ordered by it, as a JSON array. `has_cursor`
/// false (first run, no state) omits the `WHERE`. The bound param is `@P1` (tiberius
/// positional). Comparison casts the column to text to match the stored string cursor
/// (postgres-parity semantics).
pub fn incremental_select_json_sql(table: &str, cursor_col: &str, has_cursor: bool) -> String {
    let t = qualified_table(table);
    let c = quote_ident(cursor_col);
    let where_clause = if has_cursor {
        format!(" WHERE CONVERT(nvarchar(4000), {c}) > @P1")
    } else {
        String::new()
    };
    format!("SELECT * FROM {t}{where_clause} ORDER BY {c} FOR JSON PATH, INCLUDE_NULL_VALUES")
}

/// List base tables (schema-qualified) for `discover`, as a JSON array of `{name}`
/// (`FOR JSON PATH`, so it reads back through the same `query_json` path as data reads).
pub const DISCOVER_TABLES_SQL: &str = "SELECT TABLE_SCHEMA + '.' + TABLE_NAME AS name \
     FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_TYPE = 'BASE TABLE' \
     ORDER BY TABLE_SCHEMA, TABLE_NAME FOR JSON PATH, INCLUDE_NULL_VALUES";

/// Map an MSSQL `INFORMATION_SCHEMA.COLUMNS.DATA_TYPE` (or TDS type name) onto the weir
/// typed-schema model ([[WEIR-I-0025]]). Unknown types fall back to `Str` (their JSON
/// scalar still round-trips).
pub fn map_sql_type(data_type: &str) -> FieldType {
    match data_type.trim().to_ascii_lowercase().as_str() {
        "int" | "bigint" | "smallint" | "tinyint" => FieldType::Integer,
        "decimal" | "numeric" | "float" | "real" | "money" | "smallmoney" => FieldType::Float,
        "bit" => FieldType::Boolean,
        "date" | "datetime" | "datetime2" | "smalldatetime" | "datetimeoffset" | "time" => {
            FieldType::Timestamp
        }
        _ => FieldType::Str, // char/varchar/nchar/nvarchar/text/uniqueidentifier/…
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_bracket_quote_and_escape() {
        assert_eq!(quote_ident("Orders"), "[Orders]");
        assert_eq!(quote_ident("weird]name"), "[weird]]name]");
        assert_eq!(qualified_table("sales.Orders"), "[sales].[Orders]");
        assert_eq!(qualified_table("Orders"), "[Orders]");
    }

    #[test]
    fn full_and_incremental_selects_compile() {
        assert_eq!(
            full_select_json_sql("dbo.Contacts"),
            "SELECT * FROM [dbo].[Contacts] FOR JSON PATH, INCLUDE_NULL_VALUES"
        );
        // First run (no state): ordered, no WHERE.
        assert_eq!(
            incremental_select_json_sql("dbo.Contacts", "updated_at", false),
            "SELECT * FROM [dbo].[Contacts] ORDER BY [updated_at] FOR JSON PATH, INCLUDE_NULL_VALUES"
        );
        // Resume: bound, text-cast comparison + order.
        assert_eq!(
            incremental_select_json_sql("dbo.Contacts", "updated_at", true),
            "SELECT * FROM [dbo].[Contacts] WHERE CONVERT(nvarchar(4000), [updated_at]) > @P1 \
             ORDER BY [updated_at] FOR JSON PATH, INCLUDE_NULL_VALUES"
        );
    }

    #[test]
    fn discover_query_returns_json() {
        assert!(DISCOVER_TABLES_SQL.contains("FOR JSON PATH"));
        assert!(DISCOVER_TABLES_SQL.contains("TABLE_TYPE = 'BASE TABLE'"));
    }

    #[test]
    fn type_mapping_covers_the_common_families() {
        assert_eq!(map_sql_type("int"), FieldType::Integer);
        assert_eq!(map_sql_type("BIGINT"), FieldType::Integer);
        assert_eq!(map_sql_type("decimal"), FieldType::Float);
        assert_eq!(map_sql_type("bit"), FieldType::Boolean);
        assert_eq!(map_sql_type("datetime2"), FieldType::Timestamp);
        assert_eq!(map_sql_type("nvarchar"), FieldType::Str);
        assert_eq!(map_sql_type("uniqueidentifier"), FieldType::Str);
    }
}
