//! Pure formatting + lookup helpers (durations, status colours, config JSON).

use crate::models::{AvailableItem, RunRow};
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

    #[test]
    fn config_field_round_trip_keeps_partial_numbers_stable() {
        let cfg = cfg_set("{}", "rate", "1.5", "number");
        assert_eq!(cfg_get(&cfg, "rate"), "1.5");
        assert_eq!(cfg_get(&cfg_set("{}", "port", "", "integer"), "port"), "");
        assert_eq!(cfg_get(&cfg_set("{}", "flag", "true", "boolean"), "flag"), "true");
    }
}
