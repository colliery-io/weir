//! Hand-written WASM guest for the `postgres` connector (WEIR-I-0011 S4 + parity):
//! a **sync** Postgres client over `fidius_guest::sockets::tcp` + `postgres-protocol`
//! (no tokio/libpq). **Source** honors `SyncMode` (FullRefresh / Incremental by
//! cursor_field / Cdc via `pg_logical_slot_get_changes`) + key-shard partitions;
//! **destination** honors `WriteMode` (Append / Overwrite / Upsert). Config:
//! `url` (postgres://user:pw@host:port/db) or `host`/`port`/`user`/`password`/
//! `dbname`, plus `table` (default = stream name). Capability: `tcp`.
#![allow(clippy::all)]

use std::io::{Read, Write};

use fidius_macro::{plugin_impl, plugin_interface, WitType};
use weir_guest_types::*;

mod weir_guest_types;

#[plugin_interface(version = 1, buffer = PluginAllocated, crate = "fidius_guest")]
pub trait Connector: Send + Sync {
    fn spec(&self) -> ConnectorSpec;
    fn check(&self, config: Config) -> CheckResult;
    #[optional(since = 1)]
    fn discover(&self, config: Config) -> DiscoverOutcome;
    #[optional(since = 1)]
    fn read(&self, ctx: ReadContext) -> fidius_guest::Stream<ReadMessage>;
    #[optional(since = 1)]
    fn write(&self, ctx: WriteContext, batches: fidius_guest::Stream<RecordBatch>) -> WriteOutcome;
}

pub struct Postgres { cfg: weir_connector_types::Config }

#[plugin_impl(Connector, crate = "fidius_guest", config = weir_connector_types::Config)]
impl Connector for Postgres {
    fn spec(&self) -> ConnectorSpec {
        ConnectorSpec {
            name: "postgres".to_string(),
            connector_version: "0.1.0".to_string(),
            contract_version: 1,
            config_schema: "{\"type\":\"object\",\"properties\":{\
                \"url\":{\"type\":\"string\"},\"host\":{\"type\":\"string\"},\"port\":{\"type\":\"integer\"},\
                \"user\":{\"type\":\"string\"},\"password\":{\"type\":\"string\",\"format\":\"password\"},\
                \"dbname\":{\"type\":\"string\"},\"table\":{\"type\":\"string\"}}}".to_string(),
            roles: vec![ConnectorRole::Source, ConnectorRole::Destination],
            supported_sync_modes: vec![SyncMode::FullRefresh, SyncMode::Incremental, SyncMode::Cdc],
        }
    }

    fn check(&self, _config: Config) -> CheckResult {
        match PgConn::connect(&Conn::from_json(&self.cfg.json)) {
            Ok(mut c) => match c.query_rows("SELECT 1") {
                Ok(_) => CheckResult { success: true, message: None },
                Err(e) => CheckResult { success: false, message: Some(e) },
            },
            Err(e) => CheckResult { success: false, message: Some(e) },
        }
    }

    fn discover(&self, _config: Config) -> DiscoverOutcome {
        DiscoverOutcome::Catalog(Catalog {
            streams: vec![StreamInfo {
                name: "table".to_string(),
                namespace: None,
                schema: ArrowSchemaIpc { ipc: Vec::new() },
                supported_sync_modes: vec![SyncMode::FullRefresh, SyncMode::Incremental, SyncMode::Cdc],
                source_defined_cursor: false,
                default_cursor_field: None,
                source_defined_primary_key: None,
                partitioning: PartitionScheme::Unpartitioned,
            }],
        })
    }

    fn read(&self, ctx: ReadContext) -> fidius_guest::Stream<ReadMessage> {
        // CDC emits structured Changes (op + row); other modes emit Rows ([[WEIR-T-0114]]).
        let msgs = if ctx.stream.sync_mode == SyncMode::Cdc {
            match self.read_cdc(&ctx) {
                Ok((changes, cursor, opaque)) => vec![
                    ReadMessage::Records(RecordBatch::Changes(changes)),
                    ReadMessage::Checkpoint(StreamState { cursor, opaque }),
                ],
                Err(e) => vec![ReadMessage::Fatal(ConnectorError::transient(e))],
            }
        } else {
            match self.read_inner(&ctx) {
                Ok((rows, cursor, opaque)) => vec![
                    ReadMessage::Records(RecordBatch::Rows(rows)),
                    ReadMessage::Checkpoint(StreamState { cursor, opaque }),
                ],
                Err(e) => vec![ReadMessage::Fatal(ConnectorError::transient(e))],
            }
        };
        fidius_guest::Stream::from_iter(msgs)
    }

    fn write(&self, ctx: WriteContext, mut batches: fidius_guest::Stream<RecordBatch>) -> WriteOutcome {
        let mut rows: Vec<String> = Vec::new();
        let mut changes: Vec<ChangeRecord> = Vec::new();
        while let Some(batch) = batches.next_item() {
            match batch {
                RecordBatch::Rows(mut r) => rows.append(&mut r),
                RecordBatch::Changes(mut c) => changes.append(&mut c),
                RecordBatch::Arrow(_) => {
                    return write_err(ConnectorError::fatal(
                        "postgres dest takes Rows/Changes (JSON text), not Arrow batches",
                    ));
                }
            }
        }
        // CDC changes ([[WEIR-T-0115]]) apply op-aware (upsert / delete / tombstone); plain Rows use
        // the write-mode path (Append / Upsert / Overwrite).
        let result = if changes.is_empty() {
            self.write_inner(&ctx.stream, &rows)
        } else {
            self.apply_changes(&ctx.stream, &changes)
        };
        match result {
            Ok((accepted, dead_letters)) => WriteOutcome {
                state: StreamState { cursor: None, opaque: Vec::new() },
                diagnostics: Vec::new(),
                dead_letters,
                result: WriteResult::Ok(WriteReceipt { accepted }),
            },
            Err(e) => write_err(ConnectorError::transient(e)),
        }
    }
}

fn write_err(e: ConnectorError) -> WriteOutcome {
    WriteOutcome {
        state: StreamState { cursor: None, opaque: Vec::new() },
        diagnostics: Vec::new(),
        dead_letters: Vec::new(),
        result: WriteResult::Err(e),
    }
}

impl Postgres {
    fn configure(cfg: weir_connector_types::Config) -> Self { Self { cfg } }

    fn table(&self, stream: &str) -> String {
        serde_json::from_str::<serde_json::Value>(&self.cfg.json)
            .ok()
            .and_then(|v| v.get("table").and_then(|t| t.as_str()).map(str::to_string))
            .unwrap_or_else(|| stream.to_string())
    }

    /// CDC source ([[WEIR-T-0114]]): returns `(changes, next cursor = last LSN, opaque = slot name)`.
    fn read_cdc(
        &self,
        ctx: &ReadContext,
    ) -> Result<(Vec<ChangeRecord>, Option<String>, Vec<u8>), String> {
        let conn = Conn::from_json(&self.cfg.json);
        let mut c = PgConn::connect(&conn)?;
        let slot = if ctx.state.opaque.is_empty() {
            cdc_slot_name(&ctx.stream.stream)
        } else {
            String::from_utf8_lossy(&ctx.state.opaque).into_owned()
        };
        let (changes, last_lsn) = cdc_read(&mut c, &slot, &self.table(&ctx.stream.stream))?;
        Ok((changes, last_lsn.or_else(|| ctx.state.cursor.clone()), slot.into_bytes()))
    }

    /// Source (FullRefresh / Incremental): returns `(rows-as-json-text, next cursor, opaque)`.
    fn read_inner(&self, ctx: &ReadContext) -> Result<(Vec<String>, Option<String>, Vec<u8>), String> {
        let conn = Conn::from_json(&self.cfg.json);
        let mut c = PgConn::connect(&conn)?;
        let table = self.table(&ctx.stream.stream);
        let t = quote_ident(&table);

        let shard = shard_predicate(ctx.partition.bounds.as_deref())?;
        match ctx.stream.sync_mode {
            SyncMode::FullRefresh => {
                let where_clause = shard.map(|s| format!(" WHERE {s}")).unwrap_or_default();
                let sql = format!("SELECT row_to_json(t)::text FROM {t} t{where_clause}");
                let cols = c.query_rows(&sql)?;
                Ok((cols.into_iter().map(|mut r| r.remove(0).unwrap_or_default()).collect(), None, Vec::new()))
            }
            SyncMode::Incremental => {
                let cf = ctx.stream.cursor_field.as_deref().ok_or("incremental requires cursor_field")?;
                let cq = quote_ident(cf);
                let mut conds: Vec<String> = Vec::new();
                if let Some(s) = &shard {
                    conds.push(s.clone());
                }
                let cursor = ctx.state.cursor.clone();
                if let Some(cur) = &cursor {
                    conds.push(format!("t.{cq}::text > {}", lit(cur)));
                }
                let where_clause = if conds.is_empty() {
                    String::new()
                } else {
                    format!(" WHERE {}", conds.join(" AND "))
                };
                let sql = format!(
                    "SELECT row_to_json(t)::text, t.{cq}::text FROM {t} t{where_clause} ORDER BY t.{cq}"
                );
                let cols = c.query_rows(&sql)?;
                let mut next = cursor;
                let mut rows = Vec::with_capacity(cols.len());
                for mut r in cols {
                    let row = r.remove(0).unwrap_or_default();
                    if let Some(cur) = r.into_iter().next().flatten() {
                        next = Some(cur); // ASC → last wins
                    }
                    rows.push(row);
                }
                Ok((rows, next, Vec::new()))
            }
            SyncMode::Cdc => unreachable!(),
        }
    }

    /// Destination: write `rows` per the stream's `WriteMode`. Returns `(accepted,
    /// dead_letters)`.
    ///
    /// The whole write runs in **one transaction** (one fsync at COMMIT, not one per row)
    /// as **batched multi-row INSERTs**. Each chunk is **bisected**: if it fails (a bad
    /// row — constraint violation, bad json), the batch is binary-split and retried in
    /// SAVEPOINTs down to the single offending rows, which are routed to the **dead-letter
    /// queue** while every good row still commits. Happy path: one INSERT per chunk, no
    /// splitting. (`COPY` is the next step for the bulk path's raw throughput.)
    /// `(tombstone?, tombstone_column)` from config ([[WEIR-T-0115]]) — `on_delete = hard|tombstone`.
    fn delete_config(&self) -> (bool, String) {
        let v: serde_json::Value =
            serde_json::from_str(&self.cfg.json).unwrap_or(serde_json::Value::Null);
        let tombstone = v.get("on_delete").and_then(|x| x.as_str()) == Some("tombstone");
        let col = v
            .get("tombstone_column")
            .and_then(|x| x.as_str())
            .unwrap_or("_deleted_at")
            .to_string();
        (tombstone, col)
    }

    /// Apply a CDC change batch ([[WEIR-T-0115]]) **in order**: Insert/Update → upsert by key;
    /// Delete → hard `DELETE` or soft tombstone, per config. Requires `WriteMode::Upsert` keys.
    fn apply_changes(
        &self,
        stream: &ConfiguredStream,
        changes: &[ChangeRecord],
    ) -> Result<(u64, Vec<DeadLetter>), String> {
        let business_keys = match &stream.write_mode {
            WriteMode::Upsert { business_keys } if !business_keys.is_empty() => business_keys.clone(),
            _ => return Err("CDC changes require write_mode Upsert with business_keys".to_string()),
        };
        let (tombstone, tomb_col) = self.delete_config();
        let conn = Conn::from_json(&self.cfg.json);
        let mut c = PgConn::connect(&conn)?;
        let t = quote_ident(&self.table(&stream.stream));
        let keycols: Vec<String> = business_keys.iter().map(|k| quote_ident(k)).collect();
        let coldefs = keycols.iter().map(|c| format!("{c} TEXT NOT NULL")).collect::<Vec<_>>().join(", ");
        let pk = keycols.join(", ");
        let mut dead: Vec<DeadLetter> = Vec::new();
        let mut accepted = 0u64;

        let txn = (|| -> Result<(), String> {
            c.query_rows("BEGIN")?;
            let tomb_def = if tombstone {
                format!(", {} TEXT", quote_ident(&tomb_col))
            } else {
                String::new()
            };
            c.query_rows(&format!(
                "CREATE TABLE IF NOT EXISTS {t} ({coldefs}, data JSONB{tomb_def}, PRIMARY KEY ({pk}))"
            ))?;
            if tombstone {
                c.query_rows(&format!(
                    "ALTER TABLE {t} ADD COLUMN IF NOT EXISTS {} TEXT",
                    quote_ident(&tomb_col)
                ))?;
            }
            for ch in changes {
                let v: serde_json::Value = match serde_json::from_str(&ch.data) {
                    Ok(v) => v,
                    Err(e) => {
                        dead.push(DeadLetter { record: ch.data.clone(), reason: format!("row json: {e}") });
                        continue;
                    }
                };
                let key_lits = business_keys
                    .iter()
                    .map(|k| lit(&key_text(&v, k)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let where_clause = keycols
                    .iter()
                    .zip(business_keys.iter())
                    .map(|(kc, k)| format!("{kc} = {}", lit(&key_text(&v, k))))
                    .collect::<Vec<_>>()
                    .join(" AND ");
                let sql = match ch.op {
                    ChangeOp::Insert | ChangeOp::Update => {
                        weir_connector_types::pg_cdc::upsert(&t, &pk, &key_lits, &lit(&ch.data))
                    }
                    ChangeOp::Delete if tombstone => {
                        weir_connector_types::pg_cdc::tombstone(&t, &quote_ident(&tomb_col), &where_clause)
                    }
                    ChangeOp::Delete => weir_connector_types::pg_cdc::delete(&t, &where_clause),
                };
                match c.query_rows(&sql) {
                    Ok(_) => accepted += 1,
                    Err(e) => dead.push(DeadLetter { record: ch.data.clone(), reason: e }),
                }
            }
            c.query_rows("COMMIT")?;
            Ok(())
        })();

        if let Err(e) = txn {
            let _ = c.query_rows("ROLLBACK");
            return Err(e);
        }
        Ok((accepted, dead))
    }

    fn write_inner(
        &self,
        stream: &ConfiguredStream,
        rows: &[String],
    ) -> Result<(u64, Vec<DeadLetter>), String> {
        /// Rows per INSERT statement — bounds statement size while killing round-trips.
        const BATCH: usize = 500;
        let conn = Conn::from_json(&self.cfg.json);
        let mut c = PgConn::connect(&conn)?;
        let t = quote_ident(&self.table(&stream.stream));
        let mut dead: Vec<DeadLetter> = Vec::new();

        let txn = (|| -> Result<(), String> {
            c.query_rows("BEGIN")?;
            match &stream.write_mode {
                WriteMode::Append | WriteMode::Overwrite => {
                    c.query_rows(&format!("CREATE TABLE IF NOT EXISTS {t} (data JSONB)"))?;
                    if matches!(stream.write_mode, WriteMode::Overwrite) {
                        c.query_rows(&format!("TRUNCATE {t}"))?;
                    }
                    let attempt = |c: &mut PgConn, sub: &[String]| -> Result<Result<(), String>, String> {
                        let values = sub
                            .iter()
                            .map(|r| format!("({}::jsonb)", lit(r)))
                            .collect::<Vec<_>>()
                            .join(",");
                        c.try_savepoint(&format!("INSERT INTO {t} (data) VALUES {values}"))
                    };
                    for chunk in rows.chunks(BATCH) {
                        bisect(&mut c, chunk, &attempt, &mut dead)?;
                    }
                }
                WriteMode::Upsert { business_keys } => {
                    if business_keys.is_empty() {
                        return Err("upsert requires non-empty business_keys".to_string());
                    }
                    let keycols: Vec<String> = business_keys.iter().map(|k| quote_ident(k)).collect();
                    let coldefs = keycols.iter().map(|c| format!("{c} TEXT NOT NULL")).collect::<Vec<_>>().join(", ");
                    let pk = keycols.join(", ");
                    c.query_rows(&format!(
                        "CREATE TABLE IF NOT EXISTS {t} ({coldefs}, data JSONB, PRIMARY KEY ({pk}))"
                    ))?;
                    let attempt = |c: &mut PgConn, sub: &[String]| -> Result<Result<(), String>, String> {
                        let mut tuples = Vec::with_capacity(sub.len());
                        for r in sub {
                            let v: serde_json::Value = match serde_json::from_str(r) {
                                Ok(v) => v,
                                Err(e) => return Ok(Err(format!("row json: {e}"))),
                            };
                            let key_lits = business_keys
                                .iter()
                                .map(|k| lit(&key_text(&v, k)))
                                .collect::<Vec<_>>()
                                .join(", ");
                            tuples.push(format!("({key_lits}, {}::jsonb)", lit(r)));
                        }
                        c.try_savepoint(&format!(
                            "INSERT INTO {t} ({pk}, data) VALUES {} \
                             ON CONFLICT ({pk}) DO UPDATE SET data = EXCLUDED.data",
                            tuples.join(",")
                        ))
                    };
                    for chunk in rows.chunks(BATCH) {
                        bisect(&mut c, chunk, &attempt, &mut dead)?;
                    }
                }
            }
            c.query_rows("COMMIT")?;
            Ok(())
        })();

        if let Err(e) = txn {
            let _ = c.query_rows("ROLLBACK");
            return Err(e);
        }
        let accepted = rows.len() as u64 - dead.len() as u64;
        Ok((accepted, dead))
    }
}

fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// A SQL string literal, single-quote-escaped (with `standard_conforming_strings`
/// on — the default — backslashes are literal, so this is injection-safe).
fn lit(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn key_text(row: &serde_json::Value, key: &str) -> String {
    match row.get(key) {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn shard_predicate(bounds: Option<&str>) -> Result<Option<String>, String> {
    let Some(bounds) = bounds else { return Ok(None) };
    let v: serde_json::Value = serde_json::from_str(bounds).map_err(|e| format!("partition bounds: {e}"))?;
    let (Some(key), Some(shard), Some(of)) = (
        v.get("key").and_then(|x| x.as_str()),
        v.get("shard").and_then(|x| x.as_u64()),
        v.get("of").and_then(|x| x.as_u64()),
    ) else {
        return Ok(None);
    };
    if of == 0 {
        return Ok(None);
    }
    let kq = quote_ident(key);
    Ok(Some(format!("(((hashtext((t.{kq})::text) % {of}) + {of}) % {of}) = {shard}")))
}

fn cdc_slot_name(stream: &str) -> String {
    let sanitized: String = stream
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect();
    format!("weir_{sanitized}").chars().take(63).collect()
}

/// CDC via `pg_logical_slot_get_changes` (test_decoding) — a plain query, slot name
/// is sanitized so it inlines safely. Requires `wal_level=logical`.
fn cdc_read(
    c: &mut PgConn,
    slot: &str,
    target_table: &str,
) -> Result<(Vec<ChangeRecord>, Option<String>), String> {
    let slot_lit = lit(slot);
    let exists = c.query_rows(
        &format!("SELECT EXISTS(SELECT 1 FROM pg_replication_slots WHERE slot_name = {slot_lit})"),
    )?;
    let is = exists.first().and_then(|r| r.first().cloned().flatten()).unwrap_or_default();
    if is != "t" {
        c.query_rows(&format!("SELECT pg_create_logical_replication_slot({slot_lit}, 'test_decoding')"))?;
    }
    let rows = c.query_rows(
        &format!("SELECT lsn::text, data FROM pg_logical_slot_get_changes({slot_lit}, NULL, NULL)"),
    )?;
    let mut out = Vec::new();
    let mut last = None;
    for r in &rows {
        let lsn = r.first().cloned().flatten().unwrap_or_default();
        let data = r.get(1).cloned().flatten().unwrap_or_default();
        // Parse the `test_decoding` line → a structured change; BEGIN/COMMIT framing yields None.
        // The logical slot captures the whole DB, so filter to this stream's table ([[WEIR-T-0117]]) —
        // otherwise a Postgres *destination*'s own writes would feed back into the source.
        if let Some((table, op, row)) = weir_connector_types::parse_test_decoding(&data) {
            if table == target_table {
                out.push(ChangeRecord { op: op_to_guest(op), data: row });
            }
        }
        last = Some(lsn); // advance past every consumed row so replay doesn't re-deliver ([[WEIR-T-0107]])
    }
    Ok((out, last))
}

/// Map the host-side parsed op to this guest's WIT `ChangeOp` ([[WEIR-T-0114]]).
fn op_to_guest(op: weir_connector_types::ChangeOp) -> ChangeOp {
    match op {
        weir_connector_types::ChangeOp::Insert => ChangeOp::Insert,
        weir_connector_types::ChangeOp::Update => ChangeOp::Update,
        weir_connector_types::ChangeOp::Delete => ChangeOp::Delete,
    }
}

// ---- connection params + the sync wire client (postgres-protocol over sockets::tcp) ----

use fallible_iterator::FallibleIterator;
use postgres_protocol::authentication::md5_hash;
use postgres_protocol::authentication::sasl::{ChannelBinding, ScramSha256};
use postgres_protocol::message::{backend, frontend};

struct Conn {
    host: String,
    port: u16,
    user: String,
    password: String,
    dbname: String,
}

impl Conn {
    fn from_json(s: &str) -> Self {
        let v: serde_json::Value = serde_json::from_str(s).unwrap_or(serde_json::Value::Null);
        // `url` (postgres://user:pw@host:port/db) wins; else discrete fields.
        if let Some(url) = v.get("url").and_then(|x| x.as_str()) {
            if let Some(c) = parse_url(url) {
                return c;
            }
        }
        let get = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
        let user = get("user").unwrap_or_else(|| "postgres".to_string());
        Conn {
            host: get("host").unwrap_or_else(|| "127.0.0.1".to_string()),
            port: v.get("port").and_then(|x| x.as_u64()).unwrap_or(5432) as u16,
            dbname: get("dbname").unwrap_or_else(|| user.clone()),
            user,
            password: get("password").unwrap_or_default(),
        }
    }
}

fn parse_url(url: &str) -> Option<Conn> {
    let rest = url.strip_prefix("postgres://").or_else(|| url.strip_prefix("postgresql://"))?;
    let (creds, hostpart) = rest.split_once('@')?;
    let (user, password) = creds.split_once(':').unwrap_or((creds, ""));
    let (hostport, dbname) = hostpart.split_once('/').unwrap_or((hostpart, ""));
    let (host, port) = hostport.split_once(':').unwrap_or((hostport, "5432"));
    Some(Conn {
        host: host.to_string(),
        port: port.parse().unwrap_or(5432),
        user: user.to_string(),
        password: password.to_string(),
        dbname: if dbname.is_empty() { user.to_string() } else { dbname.to_string() },
    })
}

struct PgConn {
    stream: fidius_guest::sockets::tcp::TcpStream,
    buf: bytes::BytesMut,
}

impl PgConn {
    fn connect(c: &Conn) -> Result<Self, String> {
        let stream = fidius_guest::sockets::tcp::connect(&c.host, c.port)
            .map_err(|e| format!("connect {}:{}: {e}", c.host, c.port))?;
        let mut conn = PgConn { stream, buf: bytes::BytesMut::new() };
        let mut out = bytes::BytesMut::new();
        frontend::startup_message(
            [("user", c.user.as_str()), ("database", c.dbname.as_str())],
            &mut out,
        )
        .map_err(|e| format!("startup encode: {e}"))?;
        conn.send(&out)?;
        conn.authenticate(c)?;
        conn.wait_ready()?;
        Ok(conn)
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.stream.write_all(bytes).map_err(|e| format!("write: {e}"))?;
        self.stream.flush().map_err(|e| format!("flush: {e}"))
    }

    fn recv(&mut self) -> Result<backend::Message, String> {
        loop {
            if let Some(msg) = backend::Message::parse(&mut self.buf).map_err(|e| format!("parse: {e}"))? {
                return Ok(msg);
            }
            let mut chunk = [0u8; 8192];
            let n = self.stream.read(&mut chunk).map_err(|e| format!("read: {e}"))?;
            if n == 0 {
                return Err("connection closed by server".to_string());
            }
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    fn authenticate(&mut self, c: &Conn) -> Result<(), String> {
        let mut scram: Option<ScramSha256> = None;
        loop {
            match self.recv()? {
                backend::Message::AuthenticationOk => return Ok(()),
                backend::Message::AuthenticationCleartextPassword => {
                    let mut out = bytes::BytesMut::new();
                    frontend::password_message(c.password.as_bytes(), &mut out)
                        .map_err(|e| format!("password encode: {e}"))?;
                    self.send(&out)?;
                }
                backend::Message::AuthenticationMd5Password(body) => {
                    let hash = md5_hash(c.user.as_bytes(), c.password.as_bytes(), body.salt());
                    let mut out = bytes::BytesMut::new();
                    frontend::password_message(hash.as_bytes(), &mut out)
                        .map_err(|e| format!("md5 encode: {e}"))?;
                    self.send(&out)?;
                }
                backend::Message::AuthenticationSasl(body) => {
                    let mut mechs = body.mechanisms();
                    let mut chose = false;
                    while let Some(m) = mechs.next().map_err(|e| format!("sasl mechs: {e}"))? {
                        if m == "SCRAM-SHA-256" {
                            chose = true;
                            break;
                        }
                    }
                    if !chose {
                        return Err("server offered no SCRAM-SHA-256 mechanism".to_string());
                    }
                    let mut s = ScramSha256::new(c.password.as_bytes(), ChannelBinding::unrequested());
                    let mut out = bytes::BytesMut::new();
                    frontend::sasl_initial_response("SCRAM-SHA-256", s.message(), &mut out)
                        .map_err(|e| format!("sasl init encode: {e}"))?;
                    self.send(&out)?;
                    scram = Some(s);
                }
                backend::Message::AuthenticationSaslContinue(body) => {
                    let s = scram.as_mut().ok_or("unexpected SASL continue")?;
                    s.update(body.data()).map_err(|e| format!("scram update: {e}"))?;
                    let mut out = bytes::BytesMut::new();
                    frontend::sasl_response(s.message(), &mut out)
                        .map_err(|e| format!("sasl resp encode: {e}"))?;
                    self.send(&out)?;
                }
                backend::Message::AuthenticationSaslFinal(body) => {
                    let s = scram.as_mut().ok_or("unexpected SASL final")?;
                    s.finish(body.data()).map_err(|e| format!("scram finish: {e}"))?;
                }
                backend::Message::ErrorResponse(body) => return Err(pg_error(body)),
                _ => return Err("unexpected message during authentication".to_string()),
            }
        }
    }

    fn wait_ready(&mut self) -> Result<(), String> {
        loop {
            match self.recv()? {
                backend::Message::ReadyForQuery(_) => return Ok(()),
                backend::Message::ErrorResponse(body) => return Err(pg_error(body)),
                _ => {}
            }
        }
    }

    /// Run one **simple Query** (the proven path) and return rows as raw text
    /// columns (`None` = SQL NULL). DDL/INSERT return no rows. Values are inlined
    /// by the caller via `lit`/`jsonb_lit` (single-quote-escaped, fail-closed).
    fn query_rows(&mut self, sql: &str) -> Result<Vec<Vec<Option<String>>>, String> {
        let mut out = bytes::BytesMut::new();
        frontend::query(sql, &mut out).map_err(|e| format!("query encode: {e}"))?;
        self.send(&out)?;
        let mut rows: Vec<Vec<Option<String>>> = Vec::new();
        // Capture an error but keep reading to `ReadyForQuery`: a simple-query error is
        // followed by ReadyForQuery, and leaving it unread desyncs the next statement —
        // which the SAVEPOINT/ROLLBACK bisection (below) depends on.
        let mut err: Option<String> = None;
        loop {
            match self.recv()? {
                backend::Message::DataRow(body) if err.is_none() => {
                    let data = body.buffer();
                    let mut row = Vec::new();
                    let mut ranges = body.ranges();
                    while let Some(range) = ranges.next().map_err(|e| format!("ranges: {e}"))? {
                        row.push(range.map(|r| String::from_utf8_lossy(&data[r]).into_owned()));
                    }
                    rows.push(row);
                }
                backend::Message::ReadyForQuery(_) => {
                    return match err {
                        Some(e) => Err(e),
                        None => Ok(rows),
                    };
                }
                backend::Message::ErrorResponse(body) => err = Some(pg_error(body)),
                _ => {} // DataRow-after-error / RowDescription / CommandComplete / …
            }
        }
    }

    /// Run `sql` inside a SAVEPOINT: RELEASE on success, ROLLBACK-to + RELEASE on failure
    /// (so the surrounding transaction survives and stays usable). The building block for
    /// fault-isolating bisection. Outer `Err` = a real connection/protocol failure (abort
    /// everything); inner `Ok(())` = landed, inner `Err(reason)` = `sql` failed but the
    /// transaction recovered cleanly.
    fn try_savepoint(&mut self, sql: &str) -> Result<Result<(), String>, String> {
        self.query_rows("SAVEPOINT _w")?;
        match self.query_rows(sql) {
            Ok(_) => {
                self.query_rows("RELEASE SAVEPOINT _w")?;
                Ok(Ok(()))
            }
            Err(e) => {
                self.query_rows("ROLLBACK TO SAVEPOINT _w")?;
                self.query_rows("RELEASE SAVEPOINT _w")?;
                Ok(Err(e))
            }
        }
    }
}

/// Write `rows` via `attempt`; on failure, **binary-split and retry** so only the rows
/// that fail *alone* are dead-lettered and everything else still commits. `attempt` runs a
/// sub-batch (Ok = landed, Err = rolled back with a reason). Generic over the executor so
/// the splitting logic is unit-testable without a database. Returns Err only on a real
/// connection/transaction failure (propagated by `attempt`'s `?`).
fn bisect<C>(
    c: &mut C,
    rows: &[String],
    attempt: &dyn Fn(&mut C, &[String]) -> Result<Result<(), String>, String>,
    dead: &mut Vec<DeadLetter>,
) -> Result<(), String> {
    if rows.is_empty() {
        return Ok(());
    }
    match attempt(c, rows)? {
        Ok(()) => Ok(()),
        Err(reason) if rows.len() == 1 => {
            dead.push(DeadLetter { record: rows[0].clone(), reason });
            Ok(())
        }
        Err(_) => {
            let mid = rows.len() / 2;
            bisect(c, &rows[..mid], attempt, dead)?;
            bisect(c, &rows[mid..], attempt, dead)
        }
    }
}

fn pg_error(body: backend::ErrorResponseBody) -> String {
    let mut fields = body.fields();
    let mut msg = String::from("postgres error");
    while let Ok(Some(f)) = fields.next() {
        if f.type_() == b'M' {
            msg = format!("postgres error: {}", String::from_utf8_lossy(f.value_bytes()));
        }
    }
    msg
}
