//! Boundary types for the connector contract (WEIR-S-0006 "Detailed Design").
//!
//! All types are `serde`-serializable so they can cross the fidius boundary.
//! **Two bincode constraints shape these types** (fidius's typed wire is
//! `bincode`, and our raw envelopes use it too):
//! 1. bincode is *not self-describing*, so `serde_json::Value` (which
//!    deserializes via `deserialize_any`) cannot cross it — free-form JSON is
//!    carried as **JSON text (`String`)** ([`JsonText`]), which is also cleanly
//!    projectable to a WIT `string` for the WASM path.
//! 2. bincode reads fields positionally, so `#[serde(skip_serializing_if)]` /
//!    `#[serde(default)]` would desync the stream — **every field always
//!    serializes** (no field-level serde attributes here).

use serde::{Deserialize, Serialize};

use crate::error::ConnectorError;

/// A JSON document carried as text. Used wherever the contract needs free-form
/// JSON that must survive bincode and project to WIT.
pub type JsonText = String;

/// Free-form, connector-specific configuration (a JSON document validated
/// against [`ConnectorSpec::config_schema`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct Config {
    /// The JSON config document, as text.
    pub json: JsonText,
}

/// Canonical Arrow schema, carried as Arrow IPC **schema-message bytes**.
///
/// Arrow's type system is canonical (WEIR-S-0006); its IPC serialization is the
/// wire form. The contract treats it as opaque bytes; the SDK/runtime decode it
/// with the `arrow` crate (wired in WEIR-T-0003/0004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct ArrowSchemaIpc {
    /// Arrow IPC schema-message bytes.
    pub ipc: Vec<u8>,
}

/// A record batch serialized as Arrow IPC **stream bytes**.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct ArrowIpc {
    /// Arrow IPC stream bytes.
    pub ipc: Vec<u8>,
}

/// Supported sync modes for a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum SyncMode {
    FullRefresh,
    Incremental,
    /// Log-based change data capture.
    Cdc,
}

/// The role(s) a connector fulfils. Authoritative dispatch capabilities come
/// from fidius `#[optional]` capability bits on the descriptor; this is the
/// human/UX-facing declaration in [`ConnectorSpec`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum ConnectorRole {
    Source,
    Destination,
    /// A destination that writes to operational systems (reverse-ETL).
    ReverseEtl,
}

/// Static descriptor returned by `Connector::spec`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct ConnectorSpec {
    pub name: String,
    pub connector_version: String,
    /// The contract version this connector targets (see WEIR-A-0019).
    pub contract_version: u32,
    /// JSON Schema (as JSON text) describing valid [`Config`].
    pub config_schema: JsonText,
    pub roles: Vec<ConnectorRole>,
    pub supported_sync_modes: Vec<SyncMode>,
}

/// Result of `Connector::check` — config + connectivity validation; moves no data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct CheckResult {
    pub success: bool,
    pub message: Option<String>,
}

/// How a source stream may be sliced into independent work units. The connector
/// **declares** the scheme in `discover`; the Engine **plans** the concrete
/// partitions (WEIR-A-0012).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum PartitionScheme {
    /// Not partitionable; one work unit per stream.
    /// (Named `Unpartitioned`, not `None`, because `none` is a reserved WIT keyword.)
    Unpartitioned,
    /// Sliceable along the cursor (e.g. by day).
    ByCursorRange { granularity: String },
    /// Sliceable by hash/shard of a key.
    ByKeyShards { key: String, count: u32 },
    /// Sliceable by parent-stream id (substreams).
    ByParent { parent_stream: String, key: String },
}

/// A stream as discovered from the source.
/// (Named `StreamInfo`, not `Stream`, because `stream` is a reserved WIT keyword.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct StreamInfo {
    pub name: String,
    pub namespace: Option<String>,
    /// Canonical Arrow schema for the stream's records.
    pub schema: ArrowSchemaIpc,
    pub supported_sync_modes: Vec<SyncMode>,
    pub source_defined_cursor: bool,
    pub default_cursor_field: Option<String>,
    pub source_defined_primary_key: Option<Vec<String>>,
    pub partitioning: PartitionScheme,
}

/// The catalog returned by `Connector::discover`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct Catalog {
    pub streams: Vec<StreamInfo>,
}

/// Destination write semantics for a configured stream. Reverse-ETL is
/// [`WriteMode::Upsert`] to an operational destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum WriteMode {
    Append,
    Upsert { business_keys: Vec<String> },
    Overwrite,
}

/// Light in-flight mapping applied by the Engine between read and write
/// (WEIR-A-0026). v0 placeholder: an ordered, deliberately bounded op list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct MappingSpec {
    pub ops: Vec<MappingOp>,
}

/// A single bounded mapping operation (no joins/aggregates/UDFs — WEIR-A-0026).
/// Every variant is **flat / non-recursive** so the spec stays `WitType`-safe
/// across the wasm boundary (it rides `ConfiguredStream` → `ReadContext`); richer
/// nested expressions are a follow-on (engine-side parse), per WEIR-T-0052.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum MappingOp {
    Select {
        fields: Vec<String>,
    },
    Drop {
        fields: Vec<String>,
    },
    Rename {
        from: String,
        to: String,
    },
    /// Coerce `field` to a scalar type; a failed coercion dead-letters the record.
    Cast {
        field: String,
        to: CastType,
    },
    /// Keep only records where `field` `op` `value` (numeric compare when both
    /// parse as numbers, else string). Non-matching records are dropped + counted.
    Filter {
        field: String,
        op: CompareOp,
        value: String,
    },
    /// Set/overwrite `field` from a bounded expression over the record.
    Compute {
        field: String,
        value: ComputeExpr,
    },
}

/// Scalar target for [`MappingOp::Cast`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum CastType {
    Str,
    Integer,
    Float,
    Boolean,
}

/// Comparison for [`MappingOp::Filter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// A bounded compute expression for [`MappingOp::Compute`] — flat, no I/O /
/// network / arbitrary code (the dbt boundary, WEIR-A-0026).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum ComputeExpr {
    /// A literal string value.
    Const(String),
    /// Copy another field's value.
    Field(String),
    /// Concatenate the string values of the named fields.
    Concat(Vec<String>),
    /// Lower-case a field's string value.
    Lower(String),
    /// Upper-case a field's string value.
    Upper(String),
}

/// A user-configured stream within a connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct ConfiguredStream {
    /// References [`StreamInfo::name`].
    pub stream: String,
    pub sync_mode: SyncMode,
    pub cursor_field: Option<String>,
    pub primary_key: Option<Vec<String>>,
    pub write_mode: WriteMode,
    pub mapping: MappingSpec,
}

/// A configured catalog (the selected streams of a connection).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredCatalog {
    pub streams: Vec<ConfiguredStream>,
}

/// A concrete partition (work-unit slice) the Engine planned from a
/// [`PartitionScheme`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct Partition {
    pub id: String,
    /// Scheme-specific bounds as JSON text (cursor range, shard index, parent id, …).
    pub bounds: Option<JsonText>,
}

/// Opaque, connector-owned resumption state plus an optional cursor (JSON text).
/// The **Engine** commits this transactionally (WEIR-A-0011); the connector
/// never persists durability itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct StreamState {
    pub cursor: Option<JsonText>,
    pub opaque: Vec<u8>,
}

/// The change operation a CDC record carries ([[WEIR-T-0113]] / [[WEIR-I-0026]]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum ChangeOp {
    Insert,
    Update,
    Delete,
}

/// One change: an op + the row JSON (which includes the key columns, so a delete
/// carries its key). Flows through the engine + mapping like a `Rows` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct ChangeRecord {
    pub op: ChangeOp,
    pub data: JsonText,
}

/// A batch of records in one of the canonical encodings (WEIR-A-0014 §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum RecordBatch {
    /// Long-tail row encoding; each entry is a JSON-object row (as text)
    /// conforming to the stream's Arrow schema.
    Rows(Vec<JsonText>),
    /// Bulk native path: Arrow IPC stream bytes.
    Arrow(ArrowIpc),
    /// CDC changes: each entry is a row plus its op (insert/update/delete),
    /// so deletes propagate to the destination ([[WEIR-I-0026]]).
    Changes(Vec<ChangeRecord>),
}

impl RecordBatch {
    /// Number of rows when known cheaply (`Rows`); `None` for opaque Arrow IPC.
    pub fn row_count_hint(&self) -> Option<usize> {
        match self {
            RecordBatch::Rows(r) => Some(r.len()),
            RecordBatch::Changes(c) => Some(c.len()),
            RecordBatch::Arrow(_) => None,
        }
    }
}

/// Parse one Postgres `test_decoding` line → `(op, row-JSON)` ([[WEIR-T-0114]]). `None` for
/// `BEGIN`/`COMMIT` framing (they don't start with `table `). Format:
/// `table <schema>.<t>: INSERT|UPDATE|DELETE: <col>[<type>]:<val> …` (UPDATE under REPLICA IDENTITY
/// FULL prints `old-key: … new-tuple: …` — we take the new tuple). Pure + host-testable; the guest
/// connector maps `ChangeOp` to its own WIT enum.
pub fn parse_test_decoding(line: &str) -> Option<(String, ChangeOp, String)> {
    let rest = line.strip_prefix("table ")?;
    let colon = rest.find(": ")?;
    // `<schema>.<table>` → the bare table name (strip schema + any quotes) for filtering.
    let table = rest[..colon]
        .rsplit('.')
        .next()
        .unwrap_or(&rest[..colon])
        .trim_matches('"')
        .to_string();
    let after_table = &rest[colon + 2..];
    let (op_str, cols) = after_table.split_once(": ")?;
    let op = match op_str {
        "INSERT" => ChangeOp::Insert,
        "UPDATE" => ChangeOp::Update,
        "DELETE" => ChangeOp::Delete,
        _ => return None,
    };
    Some((table, op, parse_td_columns(cols)))
}

/// `<col>[<type>]:<val> …` → a JSON object (as text). `'quoted'` (with `''` escapes) → string;
/// `null` → null; numeric/bool coerced; else string. For UPDATE, only the `new-tuple:` part.
fn parse_td_columns(cols: &str) -> String {
    let cols = match cols.split_once("new-tuple: ") {
        Some((_, new)) => new,
        None => cols.strip_prefix("old-key: ").unwrap_or(cols),
    };
    let bytes = cols.as_bytes();
    let mut obj = serde_json::Map::new();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let name_start = i;
        while i < bytes.len() && bytes[i] != b'[' {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let name = cols[name_start..i].to_string();
        while i < bytes.len() && bytes[i] != b']' {
            i += 1;
        }
        i += 1; // past ']'
        if i >= bytes.len() || bytes[i] != b':' {
            break;
        }
        i += 1; // past ':'
        let val = if i < bytes.len() && bytes[i] == b'\'' {
            i += 1;
            let vstart = i;
            loop {
                while i < bytes.len() && bytes[i] != b'\'' {
                    i += 1;
                }
                if i + 1 < bytes.len() && bytes[i] == b'\'' && bytes[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                break;
            }
            let raw = cols[vstart..i].replace("''", "'");
            i += 1; // past closing quote
            serde_json::Value::String(raw)
        } else {
            let vstart = i;
            while i < bytes.len() && bytes[i] != b' ' {
                i += 1;
            }
            let raw = &cols[vstart..i];
            if raw == "null" {
                serde_json::Value::Null
            } else if raw == "true" {
                serde_json::Value::Bool(true)
            } else if raw == "false" {
                serde_json::Value::Bool(false)
            } else if let Ok(n) = raw.parse::<i64>() {
                serde_json::Value::from(n)
            } else if let Ok(f) = raw.parse::<f64>() {
                serde_json::Value::from(f)
            } else {
                serde_json::Value::String(raw.to_string())
            }
        };
        obj.insert(name, val);
    }
    serde_json::Value::Object(obj).to_string()
}

/// Postgres CDC-destination SQL builders ([[WEIR-T-0115]]). Inputs are already quoted/escaped by the
/// caller (the connector's `quote_ident`/`lit`); these assemble the statement shape so it's unit-testable
/// off the wire (the connector's fidius macros preclude `cargo test`).
pub mod pg_cdc {
    /// Upsert a change's row by primary key (`pk` = comma-joined quoted key idents).
    pub fn upsert(table: &str, pk: &str, key_lits: &str, data_lit: &str) -> String {
        format!(
            "INSERT INTO {table} ({pk}, data) VALUES ({key_lits}, {data_lit}::jsonb) \
             ON CONFLICT ({pk}) DO UPDATE SET data = EXCLUDED.data"
        )
    }

    /// Hard delete by key (`where_clause` = `k1 = 'v1' AND …`).
    pub fn delete(table: &str, where_clause: &str) -> String {
        format!("DELETE FROM {table} WHERE {where_clause}")
    }

    /// Soft tombstone: stamp the tombstone column (already quoted) with the delete time.
    pub fn tombstone(table: &str, tombstone_col: &str, where_clause: &str) -> String {
        format!("UPDATE {table} SET {tombstone_col} = now()::text WHERE {where_clause}")
    }
}

/// A weir-native field type ([[WEIR-I-0025]]) — the mapping `CastType` set + `timestamp`/`json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Str,
    Integer,
    Float,
    Boolean,
    Timestamp,
    Json,
}

/// One typed field of a stream schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: FieldType,
    #[serde(default = "yes")]
    pub nullable: bool,
}

fn yes() -> bool {
    true
}

/// A stream's typed schema — the field set with types + nullability, persisted per stream.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamSchema {
    pub fields: Vec<Field>,
}

fn value_type(v: &serde_json::Value) -> Option<FieldType> {
    match v {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(_) => Some(FieldType::Boolean),
        serde_json::Value::Number(n) => Some(if n.is_i64() || n.is_u64() {
            FieldType::Integer
        } else {
            FieldType::Float
        }),
        serde_json::Value::String(_) => Some(FieldType::Str),
        _ => Some(FieldType::Json), // object / array
    }
}

impl StreamSchema {
    /// Infer a schema from a sample of JSON-object records ([[WEIR-T-0118]]): a field's type is the
    /// common type of its non-null values (int+float → float; a genuine mix → json); a field that is
    /// ever null or absent-in-some-records is nullable. Field order follows first appearance.
    pub fn infer(sample: &[String]) -> StreamSchema {
        use std::collections::{HashMap, HashSet};
        let n = sample.len();
        let mut order: Vec<String> = Vec::new();
        let mut types: HashMap<String, HashSet<FieldType>> = HashMap::new();
        let mut nullable: HashMap<String, bool> = HashMap::new();
        let mut count: HashMap<String, usize> = HashMap::new();
        for rec in sample {
            let Ok(serde_json::Value::Object(o)) = serde_json::from_str::<serde_json::Value>(rec)
            else {
                continue;
            };
            for (k, v) in &o {
                if !types.contains_key(k) {
                    order.push(k.clone());
                    types.insert(k.clone(), HashSet::new());
                    nullable.insert(k.clone(), false);
                    count.insert(k.clone(), 0);
                }
                *count.get_mut(k).unwrap() += 1;
                match value_type(v) {
                    None => *nullable.get_mut(k).unwrap() = true,
                    Some(t) => {
                        types.get_mut(k).unwrap().insert(t);
                    }
                }
            }
        }
        let fields = order
            .into_iter()
            .map(|k| {
                let ts = &types[&k];
                let field_type = if ts.is_empty() {
                    FieldType::Str // all-null / never-observed → default string
                } else if ts.len() == 1 {
                    *ts.iter().next().unwrap()
                } else if *ts == HashSet::from([FieldType::Integer, FieldType::Float]) {
                    FieldType::Float // numeric widening
                } else {
                    FieldType::Json // genuine mix
                };
                let nullable = nullable[&k] || count[&k] < n; // ever null, or absent in some records
                Field {
                    name: k,
                    field_type,
                    nullable,
                }
            })
            .collect();
        StreamSchema { fields }
    }

    /// Diff `self` (stored) against a freshly-`discovered` schema ([[WEIR-T-0120]]): a field present in
    /// `discovered` but not stored is **additive**; a shared field whose type changed is **breaking**.
    /// Dropped fields + nullability shifts are intentionally ignored — inference reads them from a
    /// sample, so they're too noisy to gate a run on. Only a concrete type change is a hard break.
    pub fn diff(&self, discovered: &StreamSchema) -> SchemaDiff {
        use std::collections::HashMap;
        let stored: HashMap<&str, &Field> =
            self.fields.iter().map(|f| (f.name.as_str(), f)).collect();
        let mut additive = Vec::new();
        let mut breaking = Vec::new();
        for f in &discovered.fields {
            match stored.get(f.name.as_str()) {
                None => additive.push(f.clone()),
                Some(s) if s.field_type != f.field_type => breaking.push(format!(
                    "field `{}` type changed {:?} → {:?}",
                    f.name, s.field_type, f.field_type
                )),
                Some(_) => {}
            }
        }
        SchemaDiff { additive, breaking }
    }

    /// `self` with `additive` fields appended ([[WEIR-T-0120]]) — accept a purely-additive evolution.
    pub fn merged(&self, additive: &[Field]) -> StreamSchema {
        let mut fields = self.fields.clone();
        fields.extend(additive.iter().cloned());
        StreamSchema { fields }
    }
}

/// The result of [`StreamSchema::diff`] ([[WEIR-T-0120]]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SchemaDiff {
    /// New fields — safe to accept (merge into the stored schema).
    pub additive: Vec<Field>,
    /// Human-readable reasons a change is breaking — surfaced, not auto-applied.
    pub breaking: Vec<String>,
}

#[cfg(test)]
mod schema_tests {
    use super::*;

    #[test]
    fn infers_types_and_nullability() {
        let sample = vec![
            r#"{"id":1,"v":"a","amt":1.5}"#.to_string(),
            r#"{"id":2,"v":"b","amt":2,"x":null}"#.to_string(), // x null; amt int here → widens to float
        ];
        let s = StreamSchema::infer(&sample);
        let by = |n: &str| s.fields.iter().find(|f| f.name == n).cloned().unwrap();
        assert_eq!(by("id").field_type, FieldType::Integer);
        assert!(!by("id").nullable);
        assert_eq!(by("v").field_type, FieldType::Str);
        assert_eq!(by("amt").field_type, FieldType::Float); // int + float → float
        assert!(by("x").nullable); // null in one, absent in the other
    }

    #[test]
    fn mixed_scalar_types_widen_to_json() {
        let sample = vec![r#"{"m":1}"#.to_string(), r#"{"m":"str"}"#.to_string()];
        assert_eq!(
            StreamSchema::infer(&sample).fields[0].field_type,
            FieldType::Json
        );
    }

    #[test]
    fn schema_json_round_trips() {
        let s = StreamSchema {
            fields: vec![Field {
                name: "id".into(),
                field_type: FieldType::Integer,
                nullable: false,
            }],
        };
        let j = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<StreamSchema>(&j).unwrap(), s);
        // `type` is the wire key; nullable defaults to true when omitted.
        let parsed: StreamSchema =
            serde_json::from_str(r#"{"fields":[{"name":"x","type":"str"}]}"#).unwrap();
        assert!(parsed.fields[0].nullable);
    }

    #[test]
    fn diff_additive_vs_breaking() {
        let stored = StreamSchema {
            fields: vec![
                Field {
                    name: "id".into(),
                    field_type: FieldType::Integer,
                    nullable: false,
                },
                Field {
                    name: "v".into(),
                    field_type: FieldType::Str,
                    nullable: true,
                },
            ],
        };
        // A new field is additive; merging appends it.
        let plus = stored.merged(&[Field {
            name: "x".into(),
            field_type: FieldType::Boolean,
            nullable: true,
        }]);
        let d = stored.diff(&plus);
        assert!(d.breaking.is_empty());
        assert_eq!(d.additive.len(), 1);
        assert_eq!(d.additive[0].name, "x");
        assert_eq!(stored.merged(&d.additive).fields.len(), 3);

        // A type change on a shared field is breaking.
        let retyped = StreamSchema {
            fields: vec![Field {
                name: "id".into(),
                field_type: FieldType::Str,
                nullable: false,
            }],
        };
        let d2 = stored.diff(&retyped);
        assert_eq!(d2.breaking.len(), 1);
        assert!(d2.breaking[0].contains("id"));
        assert!(d2.additive.is_empty());
    }
}

/// Render `{{ record.<field> }}` / `{{ record['<field>'] }}` in `path` from a record's scalar fields
/// ([[WEIR-T-0116]] — keyed URLs for upsert/delete, e.g. `/items/{{ record.id }}`). Unknown fields
/// render empty. Pure + host-testable.
pub fn render_path(path: &str, rec: &serde_json::Value) -> String {
    let mut out = path.to_string();
    while let Some(start) = out.find("{{") {
        let Some(end_rel) = out[start..].find("}}") else {
            break;
        };
        let end = start + end_rel + 2;
        let expr = out[start + 2..end - 2].trim();
        let field = expr
            .strip_prefix("record")
            .map(|r| r.trim())
            .and_then(|r| {
                if let Some(b) = r.strip_prefix('[') {
                    Some(
                        b.trim_end_matches(']')
                            .trim()
                            .trim_matches(|c| c == '\'' || c == '"'),
                    )
                } else {
                    r.strip_prefix('.').map(str::trim)
                }
            })
            .unwrap_or(expr);
        let val = rec
            .get(field)
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default();
        out.replace_range(start..end, &val);
    }
    out
}

#[cfg(test)]
mod render_path_tests {
    use super::render_path;

    #[test]
    fn renders_keyed_delete_url() {
        let rec = serde_json::json!({"id": 42, "name": "ada"});
        assert_eq!(render_path("/items/{{ record.id }}", &rec), "/items/42");
        assert_eq!(render_path("/items/{{ record['id'] }}", &rec), "/items/42");
        assert_eq!(render_path("/u/{{ record.name }}", &rec), "/u/ada");
        // Unknown field → empty.
        assert_eq!(render_path("/x/{{ record.missing }}", &rec), "/x/");
    }
}

#[cfg(test)]
mod pg_cdc_tests {
    use super::pg_cdc;

    #[test]
    fn cdc_dest_sql_per_op() {
        assert_eq!(
            pg_cdc::upsert("\"t\"", "\"id\"", "'1'", "'{\"id\":1}'"),
            "INSERT INTO \"t\" (\"id\", data) VALUES ('1', '{\"id\":1}'::jsonb) ON CONFLICT (\"id\") DO UPDATE SET data = EXCLUDED.data"
        );
        assert_eq!(
            pg_cdc::delete("\"t\"", "\"id\" = '1'"),
            "DELETE FROM \"t\" WHERE \"id\" = '1'"
        );
        assert_eq!(
            pg_cdc::tombstone("\"t\"", "\"_deleted_at\"", "\"id\" = '1'"),
            "UPDATE \"t\" SET \"_deleted_at\" = now()::text WHERE \"id\" = '1'"
        );
    }
}

#[cfg(test)]
mod test_decoding_tests {
    use super::*;

    fn data_of(line: &str) -> serde_json::Value {
        serde_json::from_str(&parse_test_decoding(line).unwrap().2).unwrap()
    }

    #[test]
    fn parses_insert_update_delete_with_table() {
        let (table, op, _) =
            parse_test_decoding("table public.t: INSERT: id[integer]:1 v[text]:'a'").unwrap();
        assert_eq!(table, "t"); // bare table name (schema stripped) for filtering
        assert!(matches!(op, ChangeOp::Insert));
        assert_eq!(
            data_of("table public.t: INSERT: id[integer]:1 v[text]:'a'"),
            serde_json::json!({"id":1,"v":"a"})
        );
        // UPDATE under REPLICA IDENTITY FULL prints old-key + new-tuple; take the new tuple.
        let upd = "table public.t: UPDATE: old-key: id[integer]:1 v[text]:'a' new-tuple: id[integer]:1 v[text]:'b'";
        assert!(matches!(
            parse_test_decoding(upd).unwrap().1,
            ChangeOp::Update
        ));
        assert_eq!(data_of(upd), serde_json::json!({"id":1,"v":"b"}));

        assert!(matches!(
            parse_test_decoding("table public.t: DELETE: id[integer]:1")
                .unwrap()
                .1,
            ChangeOp::Delete
        ));
        assert_eq!(
            data_of("table public.t: DELETE: id[integer]:1"),
            serde_json::json!({"id":1})
        );
    }

    #[test]
    fn skips_begin_commit_and_handles_quoting() {
        assert!(parse_test_decoding("BEGIN 512").is_none());
        assert!(parse_test_decoding("COMMIT 512").is_none());
        let line = "table public.t: INSERT: id[integer]:1 v[text]:'a b''c' n[integer]:null ok[boolean]:true";
        assert_eq!(
            data_of(line),
            serde_json::json!({"id":1,"v":"a b'c","n":null,"ok":true})
        );
    }
}

/// Severity for a [`LogEntry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Out-of-band diagnostic emitted alongside a chunk/ack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}

/// One record the connector could **not** process but which is *not* an
/// operation-level failure ([[WEIR-A-0014]] §5 record-level). The operation
/// still succeeds; the Engine routes these to a dead-letter sink and counts
/// them. Carried in the operational channel, so a run can succeed *and* report
/// dead-letters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct DeadLetter {
    /// The offending record (raw row text, or an identifier for it).
    pub record: String,
    /// Why it was rejected.
    pub reason: String,
}

/// One item of the **streaming `read`** (v1, [[WEIR-A-0029]]): the connector
/// yields records, periodic checkpoints (the engine commits on these), logs, and
/// dead-letters as a single typed stream — replacing the v0 chunked-pull read
/// envelope. A terminal `ConnectorError` ends the stream (host-side).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum ReadMessage {
    Records(RecordBatch),
    Checkpoint(StreamState),
    Log(LogEntry),
    DeadLettered(DeadLetter),
    /// A terminal connector-domain failure — ends the read; the engine fails the
    /// run (the `kind` drives retry, like the v0 read envelope's error arm).
    Fatal(ConnectorError),
}

// --- Raw envelopes for `read` / `write` (`#[wire(raw)]`) ---

/// Input to the streaming `Connector::read` for one `(stream, partition)`
/// ([[WEIR-A-0029]]). `config` is **not** here in v1 — it is bound once at
/// construction (configured instances).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct ReadContext {
    pub stream: ConfiguredStream,
    pub partition: Partition,
    /// Resume point; `StreamState::default()` for a fresh read.
    pub state: StreamState,
}

/// The configured-stream context for a `write`. `config` is bound at
/// construction (configured instances, [[WEIR-A-0029]]); the destination
/// consumes a client-stream of [`RecordBatch`]es for this context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct WriteContext {
    pub stream: ConfiguredStream,
}

/// Records accepted by a successful `write`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct WriteReceipt {
    pub accepted: u64,
}

/// Data ⊕ error result of a `write` (an enum, not `Result`, so it crosses WIT —
/// mirrors [`DiscoverOutcome`]). `Err` ends the write; its `kind` drives retry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub enum WriteResult {
    Ok(WriteReceipt),
    Err(ConnectorError),
}

/// The outcome of one client-streaming `write` — the same three-channel split as
/// the read path: operational (`state`, `diagnostics`, always present) + the
/// data ⊕ error [`WriteResult`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]
pub struct WriteOutcome {
    pub state: StreamState,
    pub diagnostics: Vec<LogEntry>,
    pub dead_letters: Vec<DeadLetter>,
    pub result: WriteResult,
}

// --- Envelope codec (bincode) for the `#[wire(raw)]` read/write payloads ---

/// Error encoding/decoding a raw envelope.
#[derive(Debug, thiserror::Error)]
#[error("envelope codec error: {0}")]
pub struct CodecError(#[from] pub bincode::Error);

/// Encode a raw-envelope value to bytes for a `#[wire(raw)]` call.
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, CodecError> {
    Ok(bincode::serialize(value)?)
}

/// Decode a raw-envelope value from `#[wire(raw)]` bytes.
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
    Ok(bincode::deserialize(bytes)?)
}
