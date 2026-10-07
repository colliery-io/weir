//! Pure formatting + lookup helpers (durations, status colours, config JSON).

use crate::models::{AvailableItem, Prop, RunRow};
use aurora_leptos::tokens::token;
use std::collections::HashSet;

pub(crate) fn fmt_dur(ms: i64) -> String {
    if ms < 1000 { format!("{ms}ms") } else { format!("{:.1}s", ms as f64 / 1000.0) }
}

pub(crate) fn run_metrics(r: &RunRow) -> String {
    match r.state.as_str() {
        "pending" => "queued".to_string(),
        "leased" => {
            if r.rows_written > 0 { format!("running… {} rows", r.rows_written) } else { "running…".to_string() }
        }
        "failed" => r.error.clone().filter(|e| !e.is_empty()).unwrap_or_else(|| "failed".to_string()),
        _ => {
            let dead = if r.dead_lettered > 0 { format!(" · {} dead", r.dead_lettered) } else { String::new() };
            match r.duration_ms {
                Some(d) => format!("{} rows · {}{}", r.rows_written, fmt_dur(d), dead),
                None => format!("{} rows{}", r.rows_written, dead),
            }
        }
    }
}

pub(crate) fn latest_run(runs: &[RunRow], conn: &str) -> Option<RunRow> {
    runs.iter().find(|r| r.connection == conn).cloned()
}

pub(crate) fn state_color(state: &str) -> &'static str {
    match state {
        "done" => token::OK,
        "leased" => token::ICE,
        "failed" => token::BAD,
        "pending" => token::VIOLET,
        _ => token::MUTED,
    }
}

/// Health status → an Aurora colour token ([[WEIR-T-0111]]).
pub(crate) fn health_color(status: &str) -> &'static str {
    match status {
        "green" => token::OK,
        "amber" => token::GOLD,
        "red" => token::BAD,
        _ => token::MUTED,
    }
}

/// Worst-first ordering: red < amber < green < unknown.
pub(crate) fn health_rank(status: &str) -> u8 {
    match status {
        "red" => 0,
        "amber" => 1,
        "green" => 2,
        _ => 3,
    }
}

/// Freshness lag, humanized.
pub(crate) fn fmt_lag(ms: Option<i64>) -> String {
    match ms {
        Some(m) if m < 1000 => format!("{m}ms"),
        Some(m) if m < 60_000 => format!("{:.0}s", m as f64 / 1000.0),
        Some(m) if m < 3_600_000 => format!("{:.0}m", m as f64 / 60_000.0),
        Some(m) => format!("{:.1}h", m as f64 / 3_600_000.0),
        None => "—".to_string(),
    }
}

/// Friendly picker label for an onboardable connector.
pub(crate) fn friendly(p: &AvailableItem) -> String {
    match p.kind.as_str() {
        "manifest" => format!("{}  ·  low-code", p.name),
        "dest-manifest" => format!("{}  ·  destination", p.name),
        _ => {
            let n = p.name.strip_prefix("weir-").and_then(|s| s.strip_suffix("-pkg")).unwrap_or(&p.name);
            format!("{n}  ·  crate")
        }
    }
}

/// Already-onboarded connectors drop out of the picker.
pub(crate) fn is_onboarded(p: &AvailableItem, registered: &HashSet<String>) -> bool {
    let n = if p.kind == "manifest" || p.kind == "dest-manifest" {
        p.name.as_str()
    } else {
        p.name.strip_prefix("weir-").and_then(|s| s.strip_suffix("-pkg")).unwrap_or(&p.name)
    };
    registered.contains(n)
}

/// Read a key from a config-JSON string as a display string.
pub(crate) fn cfg_get(cfg: &str, key: &str) -> String {
    serde_json::from_str::<serde_json::Value>(cfg)
        .ok()
        .and_then(|v| v.get(key).cloned())
        .map(|v| match v {
            serde_json::Value::String(s) => s,
            other => other.to_string(),
        })
        .unwrap_or_default()
}

/// Set a key in a config-JSON string, coercing by the field kind; empty clears it.
pub(crate) fn cfg_set(cfg: &str, key: &str, val: &str, kind: &str) -> String {
    let mut obj = serde_json::from_str::<serde_json::Value>(cfg)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    if val.is_empty() {
        obj.remove(key);
    } else {
        let v = match kind {
            "integer" => val.parse::<i64>().map(serde_json::Value::from).unwrap_or_else(|_| serde_json::Value::from(val)),
            "number" => val.parse::<f64>().map(serde_json::Value::from).unwrap_or_else(|_| serde_json::Value::from(val)),
            "boolean" => serde_json::Value::from(matches!(val, "true" | "1")),
            _ => serde_json::Value::from(val),
        };
        obj.insert(key.to_string(), v);
    }
    serde_json::Value::Object(obj).to_string()
}

/// Flatten a connector's JSON-Schema `config_schema` into its fields, required first.
/// A field is secret when it has `format: password` or `airbyte_secret: true`.
pub(crate) fn parse_props(schema: &serde_json::Value) -> Vec<Prop> {
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else { return Vec::new() };
    let required: HashSet<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(|k| k.as_str()).collect())
        .unwrap_or_default();
    let mut out: Vec<Prop> = props
        .iter()
        .map(|(k, v)| {
            let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or("string").to_string();
            let secret = v.get("format").and_then(|f| f.as_str()) == Some("password")
                || v.get("airbyte_secret").and_then(|b| b.as_bool()) == Some(true);
            let options = v
                .get("enum")
                .and_then(|e| e.as_array())
                .map(|a| {
                    a.iter()
                        .map(|o| match o {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            Prop { key: k.clone(), kind, secret, required: required.contains(k.as_str()), options }
        })
        .collect();
    out.sort_by_key(|p| !p.required);
    out
}

/// One side's config to send: the form values with the "Advanced JSON" object merged
/// over them (its keys win). Then each required field must have a value; a required
/// boolean that is not set is sent as `false`.
pub(crate) fn side_config(
    side: &str,
    props: &[Prop],
    form: &str,
    advanced: &str,
) -> Result<serde_json::Value, String> {
    let mut obj = serde_json::from_str::<serde_json::Value>(form)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    if !advanced.trim().is_empty() {
        match serde_json::from_str::<serde_json::Value>(advanced) {
            Ok(serde_json::Value::Object(over)) => obj.extend(over),
            Ok(_) => return Err(format!("{side} advanced JSON must be an object")),
            Err(e) => return Err(format!("{side} advanced JSON: {e}")),
        }
    }
    let mut missing = Vec::new();
    for p in props.iter().filter(|p| p.required) {
        let empty = match obj.get(&p.key) {
            None | Some(serde_json::Value::Null) => true,
            Some(serde_json::Value::String(s)) => s.is_empty(),
            Some(_) => false,
        };
        if empty && p.kind == "boolean" {
            obj.insert(p.key.clone(), serde_json::Value::Bool(false));
        } else if empty {
            missing.push(p.key.clone());
        }
    }
    if !missing.is_empty() {
        return Err(format!("{side} config: missing required {}", missing.join(", ")));
    }
    Ok(serde_json::Value::Object(obj))
}

/// What the connection form holds for the sync/write modes and the schedule
/// ([[WEIR-T-0215]]), as the user typed it.
pub(crate) struct ModeInput<'a> {
    pub(crate) sync_mode: &'a str,
    pub(crate) write_mode: &'a str,
    /// Comma-separated.
    pub(crate) business_keys: &'a str,
    pub(crate) cursor_field: &'a str,
    /// `interval` (every N seconds) or `cron`.
    pub(crate) schedule: &'a str,
    pub(crate) every: &'a str,
    pub(crate) cron: &'a str,
}

/// The mode and schedule fields of a `ConnectionDto`, checked like the server checks them.
#[derive(Debug, PartialEq)]
pub(crate) struct ModeFields {
    pub(crate) sync_mode: String,
    pub(crate) write_mode: String,
    pub(crate) business_keys: Vec<String>,
    pub(crate) cursor_field: Option<String>,
    pub(crate) every_secs: Option<f64>,
    pub(crate) cron: Option<String>,
}

/// Check the form's modes and schedule, and build the fields to send. A cursor field is
/// sent only for `incremental`, business keys only for `upsert`, and at most one of
/// `every_secs` / `cron` (the one the schedule toggle picks; blank = no schedule).
pub(crate) fn mode_fields(m: &ModeInput) -> Result<ModeFields, String> {
    let cursor = m.cursor_field.trim();
    let cursor_field = match m.sync_mode {
        "full_refresh" | "cdc" => None,
        "incremental" if cursor.is_empty() => {
            return Err("Incremental sync needs a cursor field".into());
        }
        "incremental" => Some(cursor.to_string()),
        other => return Err(format!("Unknown sync mode `{other}`")),
    };
    let business_keys: Vec<String> = match m.write_mode {
        "append" | "overwrite" => Vec::new(),
        "upsert" => m
            .business_keys
            .split(',')
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(String::from)
            .collect(),
        other => return Err(format!("Unknown write mode `{other}`")),
    };
    if m.write_mode == "upsert" && business_keys.is_empty() {
        return Err("Upsert needs at least one business key".into());
    }
    let (mut every_secs, mut cron) = (None, None);
    if m.schedule == "cron" {
        let expr = m.cron.split_whitespace().collect::<Vec<_>>();
        match expr.len() {
            0 => {}
            6 | 7 => cron = Some(expr.join(" ")),
            _ => {
                return Err("Cron needs 6 or 7 fields, seconds first (e.g. 0 0 * * * *)".into());
            }
        }
    } else {
        match m.every.trim() {
            "" => {}
            t => match t.parse::<f64>() {
                Ok(n) if n > 0.0 && n.is_finite() => every_secs = Some(n),
                _ => return Err("Every must be a number of seconds above 0".into()),
            },
        }
    }
    Ok(ModeFields {
        sync_mode: m.sync_mode.to_string(),
        write_mode: m.write_mode.to_string(),
        business_keys,
        cursor_field,
        every_secs,
        cron,
    })
}

/// Page size of one "load older runs" fetch (`GET /runs?limit=…&before=…`).
pub(crate) const FEED_PAGE: usize = 50;

/// The run feed as shown: the live first page over the older pages already loaded,
/// one row per id (the live row wins — it has the newest state), newest first.
pub(crate) fn merge_feed(live: &[RunRow], older: &[RunRow]) -> Vec<RunRow> {
    let mut by_id: std::collections::BTreeMap<i64, RunRow> =
        older.iter().map(|r| (r.id, r.clone())).collect();
    for r in live {
        by_id.insert(r.id, r.clone());
    }
    by_id.into_values().rev().collect()
}

/// The `before` cursor for the next older page: the smallest id shown.
pub(crate) fn feed_cursor(rows: &[RunRow]) -> Option<i64> {
    rows.iter().map(|r| r.id).min()
}

/// Epoch milliseconds → `YYYY-MM-DD HH:MM:SS UTC` (civil-from-days, no clock or locale).
pub(crate) fn fmt_ts(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (days, sod) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        sod / 3600,
        sod % 3600 / 60,
        sod % 60
    )
}

/// A run's duration for the feed: the measured one once finished, else where it is.
pub(crate) fn run_duration(duration_ms: Option<i64>, state: &str) -> String {
    match (duration_ms, state) {
        (Some(d), _) => fmt_dur(d),
        (None, "pending") => "queued".to_string(),
        (None, "leased") => "running…".to_string(),
        (None, _) => "—".to_string(),
    }
}

/// Log level → an Aurora hue token.
pub(crate) fn log_color(level: &str) -> &'static str {
    match level.to_ascii_lowercase().as_str() {
        "error" => token::BAD,
        "warn" | "warning" => token::GOLD,
        "info" => token::ICE,
        _ => token::MUTED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_colours_are_theme_tokens() {
        for s in ["done", "leased", "failed", "pending", "idle"] {
            assert!(state_color(s).starts_with("var(--"));
        }
        for s in ["green", "amber", "red", "unknown"] {
            assert!(health_color(s).starts_with("var(--"));
        }
        assert_eq!(log_color("ERROR"), token::BAD);
    }

    fn run(id: i64, state: &str) -> RunRow {
        RunRow {
            id,
            connection: "c".into(),
            state: state.into(),
            rows_written: 0,
            dead_lettered: 0,
            duration_ms: None,
            error: None,
        }
    }

    #[test]
    fn merge_feed_dedupes_prefers_live_and_sorts_newest_first() {
        let live = vec![run(9, "leased"), run(8, "done"), run(7, "done")];
        // An older page loaded earlier still holds 7 (now stale) and 6, 5.
        let older = vec![run(7, "leased"), run(6, "done"), run(5, "failed")];
        let feed = merge_feed(&live, &older);
        assert_eq!(feed.iter().map(|r| r.id).collect::<Vec<_>>(), vec![9, 8, 7, 6, 5]);
        assert_eq!(feed[2].state, "done", "the live row wins over the stale older one");
        assert_eq!(feed_cursor(&feed), Some(5));
        assert_eq!(feed_cursor(&[]), None);
        assert!(merge_feed(&[], &[]).is_empty());
    }

    #[test]
    fn fmt_ts_renders_utc_civil_time() {
        assert_eq!(fmt_ts(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(fmt_ts(1_791_365_101_000), "2026-10-07 09:25:01 UTC");
        // Leap day; sub-second millis are dropped.
        assert_eq!(fmt_ts(951_782_400_999), "2000-02-29 00:00:00 UTC");
    }

    #[test]
    fn run_duration_covers_finished_and_in_flight() {
        assert_eq!(run_duration(Some(1_500), "done"), "1.5s");
        assert_eq!(run_duration(Some(20), "failed"), "20ms");
        assert_eq!(run_duration(None, "pending"), "queued");
        assert_eq!(run_duration(None, "leased"), "running…");
        assert_eq!(run_duration(None, "failed"), "—");
    }

    #[test]
    fn config_field_round_trip_keeps_partial_numbers_stable() {
        let cfg = cfg_set("{}", "rate", "1.5", "number");
        assert_eq!(cfg_get(&cfg, "rate"), "1.5");
        assert_eq!(cfg_get(&cfg_set("{}", "port", "", "integer"), "port"), "");
        assert_eq!(cfg_get(&cfg_set("{}", "flag", "true", "boolean"), "flag"), "true");
    }

    fn schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "required": ["host", "token", "tls"],
            "properties": {
                "host": {"type": "string"},
                "port": {"type": "integer"},
                "token": {"type": "string", "airbyte_secret": true},
                "password": {"type": "string", "format": "password"},
                "tls": {"type": "boolean"},
                "mode": {"type": "string", "enum": ["fast", "safe"]}
            }
        })
    }

    #[test]
    fn parse_props_reads_kind_secret_required_and_enum() {
        let props = parse_props(&schema());
        assert_eq!(props.len(), 6);
        // Required fields come first.
        assert!(props[..3].iter().all(|p| p.required));
        assert!(props[3..].iter().all(|p| !p.required));
        let get = |k: &str| props.iter().find(|p| p.key == k).unwrap().clone();
        assert!(get("token").secret && get("password").secret && !get("host").secret);
        assert_eq!(get("port").kind, "integer");
        assert_eq!(get("tls").kind, "boolean");
        assert_eq!(get("mode").options, vec!["fast", "safe"]);
        assert!(parse_props(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn side_config_merges_advanced_over_form() {
        let props = parse_props(&schema());
        let form = cfg_set(&cfg_set("{}", "host", "db", "string"), "port", "5432", "integer");
        let cfg = side_config("Source", &props, &form, r#"{"port": 6543, "token": "s3cret", "extra": [1]}"#)
            .unwrap();
        assert_eq!(cfg["host"], "db");
        assert_eq!(cfg["port"], 6543); // advanced wins
        assert_eq!(cfg["token"], "s3cret");
        assert_eq!(cfg["extra"], serde_json::json!([1]));
        assert_eq!(cfg["tls"], false); // a required boolean left unset is sent as false
    }

    #[test]
    fn side_config_reports_missing_required_and_bad_json() {
        let props = parse_props(&schema());
        let err = side_config("Destination", &props, r#"{"host": ""}"#, "").unwrap_err();
        assert_eq!(err, "Destination config: missing required host, token");
        assert!(side_config("Source", &props, "{}", "{nope").unwrap_err().starts_with("Source advanced JSON:"));
        assert!(side_config("Source", &props, "{}", "[1]").unwrap_err().contains("must be an object"));
        // No contract: whatever the form and override hold goes through.
        assert_eq!(side_config("Source", &[], "{}", "").unwrap(), serde_json::json!({}));
    }

    fn input() -> ModeInput<'static> {
        ModeInput {
            sync_mode: "full_refresh",
            write_mode: "append",
            business_keys: "",
            cursor_field: "",
            schedule: "interval",
            every: "",
            cron: "",
        }
    }

    #[test]
    fn mode_fields_defaults_send_no_keys_cursor_or_schedule() {
        let f = mode_fields(&input()).unwrap();
        assert_eq!(f.sync_mode, "full_refresh");
        assert_eq!(f.write_mode, "append");
        assert!(f.business_keys.is_empty() && f.cursor_field.is_none());
        assert!(f.every_secs.is_none() && f.cron.is_none());
    }

    #[test]
    fn mode_fields_incremental_upsert_cron() {
        let f = mode_fields(&ModeInput {
            sync_mode: "incremental",
            write_mode: "upsert",
            business_keys: " id, region ,,",
            cursor_field: " updated_at ",
            schedule: "cron",
            every: "60", // the hidden interval is not sent
            cron: "  0 0 3  * * * ",
        })
        .unwrap();
        assert_eq!(f.cursor_field.as_deref(), Some("updated_at"));
        assert_eq!(f.business_keys, vec!["id", "region"]);
        assert_eq!(f.cron.as_deref(), Some("0 0 3 * * *"));
        assert_eq!(f.every_secs, None);
    }

    #[test]
    fn mode_fields_only_send_what_the_mode_uses() {
        // A cursor typed and then left behind by switching to full refresh is dropped,
        // and so are keys under append; the interval wins over a hidden cron.
        let f = mode_fields(&ModeInput {
            business_keys: "id",
            cursor_field: "ts",
            every: "30",
            cron: "0 0 * * * *",
            ..input()
        })
        .unwrap();
        assert_eq!((f.cursor_field, f.business_keys.len()), (None, 0));
        assert_eq!((f.every_secs, f.cron), (Some(30.0), None));
        let cdc = ModeInput { sync_mode: "cdc", cursor_field: "ts", ..input() };
        assert!(mode_fields(&cdc).unwrap().cursor_field.is_none());
    }

    #[test]
    fn mode_fields_reject_what_the_server_rejects() {
        let err = |m: ModeInput| mode_fields(&m).unwrap_err();
        assert_eq!(
            err(ModeInput { sync_mode: "incremental", ..input() }),
            "Incremental sync needs a cursor field"
        );
        assert_eq!(
            err(ModeInput { write_mode: "upsert", business_keys: " , ", ..input() }),
            "Upsert needs at least one business key"
        );
        assert!(err(ModeInput { sync_mode: "nope", ..input() }).contains("Unknown sync mode"));
        assert!(err(ModeInput { write_mode: "merge", ..input() }).contains("Unknown write mode"));
        let five = ModeInput { schedule: "cron", cron: "0 * * * *", ..input() };
        assert!(err(five).starts_with("Cron needs 6 or 7 fields"));
        assert!(err(ModeInput { every: "soon", ..input() }).starts_with("Every must be"));
        assert!(err(ModeInput { every: "0", ..input() }).starts_with("Every must be"));
        // A blank cron is no schedule, not an error.
        assert!(mode_fields(&ModeInput { schedule: "cron", ..input() }).unwrap().cron.is_none());
    }
}
