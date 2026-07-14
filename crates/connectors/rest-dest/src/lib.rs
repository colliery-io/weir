//! Self-contained WASM guest for the `rest-dest` declarative **destination** (v1 streaming
//! `write`): the reverse-ETL analogue of the `rest` source runtime ([[WEIR-A-0034]]). It
//! interprets a **destination manifest** as config and upserts each record into a SaaS REST
//! API: shape the record into the object JSON (field map + optional body wrap), then
//! POST/PATCH over `wasi:http`. Auth is injected **host-side** ([[WEIR-A-0033]]) — the guest
//! sends plain requests. Transient failures (429 / 5xx) retry with backoff ([[WEIR-T-0069]]);
//! a per-record 4xx dead-letters that record instead of failing the sync. Capability: `http`.
#![allow(clippy::all)]

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

pub struct RestDest { cfg: weir_connector_types::Config }

impl RestDest {
    fn configure(cfg: weir_connector_types::Config) -> Self { Self { cfg } }
}

#[plugin_impl(Connector, crate = "fidius_guest", config = weir_connector_types::Config)]
impl Connector for RestDest {
    fn spec(&self) -> ConnectorSpec {
        ConnectorSpec {
            name: "rest-dest".to_string(),
            connector_version: "0.1.0".to_string(),
            contract_version: 1,
            config_schema: "{\"type\":\"object\",\"properties\":{\
                \"base_url\":{\"type\":\"string\",\"title\":\"SaaS API base URL\"},\
                \"path\":{\"type\":\"string\",\"title\":\"Object endpoint (templated with {{ record.<field> }})\"},\
                \"method\":{\"type\":\"string\",\"title\":\"POST | PATCH\"},\
                \"field_map\":{\"type\":\"object\",\"title\":\"record field -> SaaS property\"},\
                \"body_wrap\":{\"type\":\"string\",\"title\":\"nest shaped fields under this key (e.g. properties)\"}}}".to_string(),
            roles: vec![ConnectorRole::Destination],
            supported_sync_modes: vec![SyncMode::FullRefresh, SyncMode::Incremental],
        }
    }

    fn check(&self, _config: Config) -> CheckResult {
        // A declarative destination has no cheap liveness probe without spending a write;
        // validate the config shape instead (base_url present + parseable).
        match parse_cfg(&self.cfg.json) {
            Ok(_) => CheckResult { success: true, message: None },
            Err(e) => CheckResult { success: false, message: Some(e) },
        }
    }

    fn discover(&self, _config: Config) -> DiscoverOutcome {
        // Destinations don't discover streams; the object shape comes from the manifest.
        DiscoverOutcome::Catalog(Catalog { streams: Vec::new() })
    }

    fn read(&self, _ctx: ReadContext) -> fidius_guest::Stream<ReadMessage> {
        fidius_guest::Stream::from_iter(vec![ReadMessage::Fatal(ConnectorError::fatal(
            "rest-dest is a write-only destination; it has no read side",
        ))])
    }

    fn write(&self, ctx: WriteContext, mut batches: fidius_guest::Stream<RecordBatch>) -> WriteOutcome {
        let mut c = match parse_cfg(&self.cfg.json) {
            Ok(c) => c,
            Err(e) => return write_err(ConnectorError::fatal(e)),
        };
        // Stream-driven upsert ([[WEIR-T-0073]]): unless the config pins a method, an
        // `Upsert` write mode means PATCH the keyed URL (create-or-update, idempotent on
        // replay); anything else creates with POST.
        if c.method.is_empty() {
            c.method = match ctx.stream.write_mode {
                WriteMode::Upsert { .. } => "PATCH".to_string(),
                _ => "POST".to_string(),
            };
        }
        let mut accepted: u64 = 0;
        let mut dead_letters: Vec<DeadLetter> = Vec::new();
        while let Some(batch) = batches.next_item() {
            match batch {
                RecordBatch::Rows(rows) => {
                    for row in rows {
                        match write_one(&c, &row) {
                            Ok(()) => accepted += 1,
                            Err(reason) => dead_letters.push(DeadLetter { record: row, reason }),
                        }
                    }
                }
                // CDC changes ([[WEIR-T-0116]]): Delete → DELETE the keyed URL; Insert/Update → write.
                RecordBatch::Changes(changes) => {
                    for ch in changes {
                        let res = match ch.op {
                            ChangeOp::Delete => delete_one(&c, &ch.data),
                            _ => write_one(&c, &ch.data),
                        };
                        match res {
                            Ok(()) => accepted += 1,
                            Err(reason) => dead_letters.push(DeadLetter { record: ch.data, reason }),
                        }
                    }
                }
                RecordBatch::Arrow(_) => {
                    return write_err(ConnectorError::fatal(
                        "rest-dest takes Rows/Changes (JSON text), not Arrow batches",
                    ));
                }
            }
        }
        WriteOutcome {
            state: StreamState { cursor: None, opaque: Vec::new() },
            diagnostics: Vec::new(),
            dead_letters,
            result: WriteResult::Ok(WriteReceipt { accepted }),
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

/// The destination manifest, flattened to config keys (baked host-side, like the source).
struct DestCfg {
    base_url: String,
    /// Object endpoint, templated with `{{ record.<field> }}` for upsert-by-key in the URL.
    path: String,
    /// `POST` (create) or `PATCH` (upsert by the keyed URL). Default `POST`.
    method: String,
    /// record field -> SaaS property. Empty = identity (pass the record through).
    field_map: Vec<(String, String)>,
    /// Nest the shaped fields under this key (e.g. HubSpot's `properties`). None = top-level.
    body_wrap: Option<String>,
    /// CDC delete ([[WEIR-T-0116]]): key-templated path for `op:delete`, e.g. `/items/{{ record.id }}`.
    /// `None` → deletes dead-letter (misconfig is visible, not silent).
    delete_path: Option<String>,
    /// HTTP method for a delete (default `DELETE`).
    delete_method: String,
    max_retries: u32,
}

fn parse_cfg(json: &str) -> Result<DestCfg, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("config json: {e}"))?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(String::from);
    let base_url = s("base_url").ok_or_else(|| "config missing base_url".to_string())?;
    let field_map = v
        .get("field_map")
        .and_then(|x| x.as_object())
        .map(|o| {
            o.iter()
                .filter_map(|(k, val)| val.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    Ok(DestCfg {
        base_url,
        path: s("path").unwrap_or_default(),
        // Empty = "not pinned by config" → resolved from the stream's WriteMode in `write`.
        method: s("method").unwrap_or_default(),
        field_map,
        body_wrap: s("body_wrap"),
        delete_path: s("delete_path"),
        delete_method: s("delete_method").unwrap_or_else(|| "DELETE".to_string()),
        max_retries: v.get("max_retries").and_then(|x| x.as_u64()).unwrap_or(4) as u32,
    })
}

/// Upsert one record. `Ok(())` = accepted (2xx); `Err(reason)` = dead-letter (4xx / exhausted).
fn write_one(c: &DestCfg, row: &str) -> Result<(), String> {
    let rec: serde_json::Value =
        serde_json::from_str(row).map_err(|e| format!("record json: {e}"))?;
    let path = weir_connector_types::render_path(&c.path, &rec);
    let url = format!("{}{}", c.base_url.trim_end_matches('/'), path);
    let body = shape_body(c, &rec)?;
    let resp = send_with_retry(c, &url, &c.method, body.as_bytes())?;
    if (200..300).contains(&resp.status) {
        Ok(())
    } else {
        Err(format!("HTTP {} from {url}: {}", resp.status, truncate(&resp.text(), 200)))
    }
}

/// Apply a CDC delete ([[WEIR-T-0116]]): render the `delete_path` template with the record's key and
/// issue an HTTP `DELETE` (or the configured `delete_method`). Missing `delete_path` → dead-letter.
fn delete_one(c: &DestCfg, row: &str) -> Result<(), String> {
    let rec: serde_json::Value =
        serde_json::from_str(row).map_err(|e| format!("record json: {e}"))?;
    let Some(delete_path) = c.delete_path.as_deref() else {
        return Err("delete on a change but no `delete_path` configured".to_string());
    };
    let path = weir_connector_types::render_path(delete_path, &rec);
    let url = format!("{}{}", c.base_url.trim_end_matches('/'), path);
    let resp = send_with_retry(c, &url, &c.delete_method, &[])?;
    // 2xx or 404 (already gone) both settle a delete.
    if (200..300).contains(&resp.status) || resp.status == 404 {
        Ok(())
    } else {
        Err(format!("HTTP {} deleting {url}: {}", resp.status, truncate(&resp.text(), 200)))
    }
}

/// Shape a record into the SaaS object body: apply the field map (or identity), then wrap.
fn shape_body(c: &DestCfg, rec: &serde_json::Value) -> Result<String, String> {
    let obj = rec.as_object().ok_or_else(|| "record is not a JSON object".to_string())?;
    let shaped: serde_json::Map<String, serde_json::Value> = if c.field_map.is_empty() {
        obj.clone()
    } else {
        c.field_map
            .iter()
            .filter_map(|(from, to)| obj.get(from).map(|v| (to.clone(), v.clone())))
            .collect()
    };
    let value = match &c.body_wrap {
        Some(wrap) => {
            let mut outer = serde_json::Map::new();
            outer.insert(wrap.clone(), serde_json::Value::Object(shaped));
            serde_json::Value::Object(outer)
        }
        None => serde_json::Value::Object(shaped),
    };
    serde_json::to_string(&value).map_err(|e| format!("shape: {e}"))
}


fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_string() } else { format!("{}…", &s[..n]) }
}

/// Exponential backoff for retry `attempt` (0-based): 200ms, 400, 800, … capped at 8s.
fn backoff_ms(attempt: u32) -> u64 {
    (200u64 << attempt.min(5)).min(8000)
}

/// Parse a `Retry-After` header (integer seconds) → milliseconds, if present.
fn retry_after_ms(headers: &[(String, String)]) -> Option<u64> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))
        .and_then(|(_, v)| v.trim().parse::<u64>().ok())
        .map(|secs| secs * 1000)
}

/// Send the shaped body to `url` with the configured method, retrying transient failures
/// (429 / 5xx / transport) with backoff, honoring `Retry-After`. Auth is host-injected; the
/// guest sends `Content-Type: application/json` + a plain request.
fn send_with_retry(
    c: &DestCfg,
    url: &str,
    method: &str,
    body: &[u8],
) -> Result<fidius_guest::http::Response, String> {
    let mut attempt: u32 = 0;
    loop {
        let mut req = fidius_guest::http::Request::post(url, body.to_vec());
        req.method = method.to_uppercase();
        req = req.header("content-type", "application/json");
        match fidius_guest::http::send(req) {
            Ok(r) => {
                let retryable = r.status == 429 || (500..=599).contains(&r.status);
                if !retryable || attempt >= c.max_retries {
                    return Ok(r);
                }
                let wait = retry_after_ms(&r.headers).unwrap_or_else(|| backoff_ms(attempt));
                std::thread::sleep(std::time::Duration::from_millis(wait));
                attempt += 1;
            }
            Err(e) => {
                if attempt >= c.max_retries {
                    return Err(format!("request to {url} failed: {e}"));
                }
                std::thread::sleep(std::time::Duration::from_millis(backoff_ms(attempt)));
                attempt += 1;
            }
        }
    }
}
