//! WASM guest for the `s3` object-store source ([[WEIR-T-0109]]): lists + reads NDJSON objects from
//! an S3-compatible endpoint over HTTP. Requests are UNSIGNED in the guest — the host egress signs
//! them with SigV4 (`Credential::AwsSigV4`), so the secret never enters the sandbox ([[WEIR-A-0033]]).
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


pub struct S3 {
    cfg: weir_connector_types::Config,
}

#[plugin_impl(Connector, crate = "fidius_guest", config = weir_connector_types::Config)]
impl Connector for S3 {
    fn spec(&self) -> ConnectorSpec {
        ConnectorSpec {
            name: "s3".to_string(),
            connector_version: "0.1.0".to_string(),
            contract_version: 1,
            config_schema: "{\"type\":\"object\",\"required\":[\"endpoint\",\"bucket\"],\"properties\":{\
                \"endpoint\":{\"type\":\"string\",\"title\":\"S3 endpoint (e.g. https://s3.amazonaws.com or http://minio:9000)\"},\
                \"bucket\":{\"type\":\"string\",\"title\":\"Bucket\"},\
                \"prefix\":{\"type\":\"string\",\"title\":\"Key prefix (blank = whole bucket)\"}}}".to_string(),
            // Auth (aws_sigv4) is resolved + signed host-side ([[WEIR-A-0033]]); the guest
            // declares no credential and never sees the secret.
            roles: vec![ConnectorRole::Source],
            supported_sync_modes: vec![SyncMode::FullRefresh, SyncMode::Incremental],
        }
    }

    fn check(&self, _config: Config) -> CheckResult {
        CheckResult { success: true, message: None }
    }

    fn discover(&self, _config: Config) -> DiscoverOutcome {
        // One "objects" stream; the incremental cursor is the object key (lexicographic).
        DiscoverOutcome::Catalog(Catalog {
            streams: vec![StreamInfo {
                name: "objects".to_string(),
                namespace: None,
                schema: ArrowSchemaIpc { ipc: Vec::new() },
                supported_sync_modes: vec![SyncMode::FullRefresh, SyncMode::Incremental],
                source_defined_cursor: true,
                default_cursor_field: Some("_key".to_string()),
                source_defined_primary_key: None,
                partitioning: PartitionScheme::Unpartitioned,
            }],
        })
    }

    fn read(&self, ctx: ReadContext) -> fidius_guest::Stream<ReadMessage> {
        let msgs: Vec<ReadMessage> = match read_objects(&self.cfg, &ctx) {
            Ok((rows, cursor)) => vec![
                ReadMessage::Records(RecordBatch::Rows(rows)),
                ReadMessage::Checkpoint(StreamState { cursor, opaque: Vec::new() }),
            ],
            Err(e) => vec![ReadMessage::Fatal(e)],
        };
        fidius_guest::Stream::from_iter(msgs)
    }

    fn write(&self, _ctx: WriteContext, mut batches: fidius_guest::Stream<RecordBatch>) -> WriteOutcome {
        while batches.next_item().is_some() {}
        WriteOutcome {
            state: StreamState { cursor: None, opaque: Vec::new() },
            diagnostics: Vec::new(),
            dead_letters: Vec::new(),
            result: WriteResult::Err(ConnectorError::fatal("s3 is a source, not a destination")),
        }
    }
}

impl S3 {
    fn configure(cfg: weir_connector_types::Config) -> Self {
        Self { cfg }
    }
}

struct S3Cfg {
    endpoint: String,
    bucket: String,
    prefix: String,
}

fn parse_cfg(config: &weir_connector_types::Config) -> S3Cfg {
    let v: serde_json::Value = serde_json::from_str(&config.json).unwrap_or(serde_json::Value::Null);
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    S3Cfg {
        endpoint: s("endpoint").trim_end_matches('/').to_string(),
        bucket: s("bucket"),
        prefix: s("prefix"),
    }
}

/// List objects (ListObjectsV2), then read each new one as NDJSON. Incremental uses the object key
/// as a lexicographic cursor (`start-after`), so append-only key schemes (date-partitioned, ULID…)
/// resume cleanly. Returns (record-rows, new cursor = the greatest key read).
fn read_objects(
    config: &weir_connector_types::Config,
    ctx: &ReadContext,
) -> Result<(Vec<String>, Option<String>), ConnectorError> {
    let cfg = parse_cfg(config);
    if cfg.endpoint.is_empty() || cfg.bucket.is_empty() {
        return Err(ConnectorError::fatal("s3: `endpoint` and `bucket` are required"));
    }
    let incremental = matches!(ctx.stream.sync_mode, SyncMode::Incremental);
    let after = ctx.state.cursor.clone();

    let mut list_url = format!("{}/{}?list-type=2", cfg.endpoint, cfg.bucket);
    if !cfg.prefix.is_empty() {
        list_url.push_str(&format!("&prefix={}", uri_escape(&cfg.prefix)));
    }
    if incremental {
        if let Some(a) = &after {
            list_url.push_str(&format!("&start-after={}", uri_escape(a)));
        }
    }

    let listing = http_get(&list_url)?;
    let mut keys = parse_list_keys(&listing);
    keys.sort();

    let mut rows = Vec::new();
    let mut cursor = after.clone();
    for key in keys {
        if key.ends_with('/') {
            continue; // a "directory" placeholder, not an object
        }
        if incremental {
            if let Some(a) = &after {
                if &key <= a {
                    continue;
                }
            }
        }
        let obj_url = format!("{}/{}/{}", cfg.endpoint, cfg.bucket, uri_escape_path(&key));
        let body = http_get(&obj_url)?;
        for line in body.lines() {
            let t = line.trim();
            if !t.is_empty() {
                rows.push(t.to_string());
            }
        }
        cursor = Some(key);
    }
    Ok((rows, cursor))
}

/// Extract `<Key>…</Key>` values from a ListObjectsV2 XML response (minimal, dependency-free —
/// S3's listing schema is stable).
fn parse_list_keys(xml: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<Key>") {
        let tail = &rest[start + 5..];
        match tail.find("</Key>") {
            Some(end) => {
                keys.push(xml_unescape(&tail[..end]));
                rest = &tail[end + 6..];
            }
            None => break,
        }
    }
    keys
}

fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Percent-encode a query value (unreserved pass through).
fn uri_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Percent-encode an object key for the request path — like `uri_escape` but keep `/`.
fn uri_escape_path(s: &str) -> String {
    s.split('/').map(uri_escape).collect::<Vec<_>>().join("/")
}

/// A plain GET; the host injects the SigV4 signature. Retries transient failures.
fn http_get(url: &str) -> Result<String, ConnectorError> {
    let mut attempt: u32 = 0;
    loop {
        match fidius_guest::http::send(fidius_guest::http::Request::get(url)) {
            Ok(r) if (200..300).contains(&r.status) => return Ok(r.text()),
            Ok(r) if (r.status == 429 || (500..=599).contains(&r.status)) && attempt < 4 => {
                std::thread::sleep(std::time::Duration::from_millis(200u64 << attempt));
                attempt += 1;
            }
            Ok(r) => {
                return Err(ConnectorError::fatal(format!("GET {url} → HTTP {}", r.status)));
            }
            Err(e) if attempt < 4 => {
                std::thread::sleep(std::time::Duration::from_millis(200u64 << attempt));
                attempt += 1;
            }
            Err(e) => return Err(ConnectorError::transient(format!("GET {url} failed: {e}"))),
        }
    }
}
