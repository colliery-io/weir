//! Connection health ([[WEIR-T-0110]] / [[WEIR-I-0024]]): a pure **green/amber/red** rollup from
//! recent runs + dead-letters + the schedule. No I/O — the store gathers the facts, this decides.
//! State-string-agnostic: *terminal* = anything not still in flight; *success* = `done`; a terminal
//! run that isn't `done` counts as a failure — robust to the exact failure label.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Green,
    Amber,
    Red,
    /// No terminal runs yet — nothing to judge.
    Unknown,
}

/// One run's health-relevant facts.
#[derive(Debug, Clone)]
pub struct HealthRun {
    pub state: String,
    pub finished_ms: Option<i64>,
    pub rows_written: i64,
}

/// The computed health of one connection.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionHealth {
    pub connection: String,
    pub status: HealthStatus,
    pub last_success_ms: Option<i64>,
    pub lag_ms: Option<i64>,
    pub recent_total: u32,
    pub recent_failed: u32,
    pub error_rate: f64,
    pub dead_letters: u64,
    pub rows_recent: i64,
    /// Rows per terminal run, oldest→newest — the sparkline.
    pub throughput: Vec<i64>,
}

/// v1 thresholds (fixed; per-connection config is a later pass).
#[derive(Debug, Clone, Copy)]
pub struct HealthThresholds {
    /// Freshness grace added to the schedule interval before a connection reads stale.
    pub grace_ms: i64,
    /// Staleness window for connections with no fixed schedule.
    pub unscheduled_stale_ms: i64,
    pub error_rate_amber: f64,
    pub error_rate_red: f64,
}

impl Default for HealthThresholds {
    fn default() -> Self {
        Self {
            grace_ms: 60_000,
            unscheduled_stale_ms: 24 * 3600 * 1000,
            error_rate_amber: 0.2,
            error_rate_red: 0.5,
        }
    }
}

fn in_flight(state: &str) -> bool {
    matches!(state, "pending" | "leased" | "running")
}

/// Compute a connection's health from its recent runs (any order), dead-letter count, schedule
/// interval (`None` = unscheduled), and the current time.
pub fn compute(
    connection: &str,
    runs: &[HealthRun],
    dead_letters: u64,
    schedule_ms: Option<i64>,
    now_ms: i64,
    th: &HealthThresholds,
) -> ConnectionHealth {
    let terminal: Vec<&HealthRun> = runs.iter().filter(|r| !in_flight(&r.state)).collect();
    let recent_total = terminal.len() as u32;
    let recent_failed = terminal.iter().filter(|r| r.state != "done").count() as u32;
    let error_rate = if recent_total > 0 {
        recent_failed as f64 / recent_total as f64
    } else {
        0.0
    };

    // Most recent successful run (by finish time) → freshness.
    let last_success_ms = terminal
        .iter()
        .filter(|r| r.state == "done")
        .filter_map(|r| r.finished_ms)
        .max();
    let lag_ms = last_success_ms.map(|t| (now_ms - t).max(0));

    // Throughput: rows per terminal run, oldest→newest (input is newest-first from the store).
    let mut throughput: Vec<i64> = terminal.iter().map(|r| r.rows_written).collect();
    throughput.reverse();
    let rows_recent: i64 = throughput.iter().sum();

    let window = schedule_ms
        .map(|s| s + th.grace_ms)
        .unwrap_or(th.unscheduled_stale_ms);
    let stale_amber = lag_ms.map(|l| l > window).unwrap_or(false);
    let stale_red = lag_ms.map(|l| l > 2 * window).unwrap_or(false);

    let status = if recent_total == 0 {
        HealthStatus::Unknown
    } else if error_rate >= th.error_rate_red || last_success_ms.is_none() || stale_red {
        HealthStatus::Red
    } else if error_rate >= th.error_rate_amber || stale_amber || dead_letters > 0 {
        HealthStatus::Amber
    } else {
        HealthStatus::Green
    };

    ConnectionHealth {
        connection: connection.to_string(),
        status,
        last_success_ms,
        lag_ms,
        recent_total,
        recent_failed,
        error_rate,
        dead_letters,
        rows_recent,
        throughput,
    }
}

/// One tenant's rolled-up health for the super-operator view.
#[derive(Debug, Clone, Serialize)]
pub struct TenantHealth {
    pub tenant: String,
    pub status: HealthStatus,
    pub connections: u32,
    /// Connections in amber or red.
    pub needs_attention: u32,
    pub dead_letters: u64,
    pub queue_depth: i64,
}

/// A connection needing attention, surfaced across tenants in the super-operator view.
#[derive(Debug, Clone, Serialize)]
pub struct AttentionItem {
    pub tenant: String,
    pub connection: String,
    pub status: HealthStatus,
}

/// The platform-wide health rollup (platform-admin only).
#[derive(Debug, Clone, Serialize)]
pub struct PlatformHealth {
    pub tenants: Vec<TenantHealth>,
    /// The amber/red connections across every tenant, worst-first.
    pub needs_attention: Vec<AttentionItem>,
    /// Tenants with due work right now (fleet signal).
    pub active_tenants: u32,
    pub total_queue_depth: i64,
}

/// The worst status across a set — for a tenant rollup (Red > Amber > Green > Unknown).
pub fn worst(statuses: impl IntoIterator<Item = HealthStatus>) -> HealthStatus {
    let mut acc = HealthStatus::Unknown;
    let rank = |s: HealthStatus| match s {
        HealthStatus::Red => 3,
        HealthStatus::Amber => 2,
        HealthStatus::Green => 1,
        HealthStatus::Unknown => 0,
    };
    for s in statuses {
        if rank(s) > rank(acc) {
            acc = s;
        }
    }
    acc
}

/// Live runner resource usage for the health surface ([[WEIR-I-0035]] F1.4): the runner's *current*
/// measured mem/cpu pressure + how many resident sources it holds. Distinct from per-connection
/// [`ConnectionHealth`] (that's run history) — this is what F1.4's claim-headroom gate reacts to,
/// surfaced for ops. Populated by sampling the runner's `UsageProbe` at the health endpoint.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct RunnerUsage {
    pub mem_fraction: f64,
    pub cpu_fraction: f64,
    pub residents: u32,
    /// Green under `high_water`, amber within 10% of it, red at/over it. With no `high_water`
    /// configured the gate is off → always Green.
    pub status: HealthStatus,
}

impl RunnerUsage {
    /// Roll a measured sample + resident count + the configured high-water into a status.
    pub fn from_sample(
        mem_fraction: f64,
        cpu_fraction: f64,
        residents: u32,
        high_water: Option<f64>,
    ) -> Self {
        let max = mem_fraction.max(cpu_fraction);
        let status = match high_water {
            Some(hw) if max >= hw => HealthStatus::Red,
            Some(hw) if max >= hw * 0.9 => HealthStatus::Amber,
            Some(_) | None => HealthStatus::Green,
        };
        Self {
            mem_fraction,
            cpu_fraction,
            residents,
            status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000_000_000_000;
    fn done(finished: i64, rows: i64) -> HealthRun {
        HealthRun {
            state: "done".into(),
            finished_ms: Some(finished),
            rows_written: rows,
        }
    }
    fn failed() -> HealthRun {
        HealthRun {
            state: "failed".into(),
            finished_ms: Some(NOW),
            rows_written: 0,
        }
    }
    fn th() -> HealthThresholds {
        HealthThresholds::default()
    }

    #[test]
    fn green_when_fresh_and_succeeding() {
        // Scheduled every 60s, last success just now, all done, no dead letters.
        let runs = vec![done(NOW - 1000, 10), done(NOW - 61_000, 8)];
        let h = compute("c", &runs, 0, Some(60_000), NOW, &th());
        assert_eq!(h.status, HealthStatus::Green);
        assert_eq!(h.rows_recent, 18);
        assert_eq!(h.throughput, vec![8, 10]); // oldest→newest
        assert_eq!(h.lag_ms, Some(1000));
    }

    #[test]
    fn amber_on_dead_letters_or_staleness() {
        // Fresh + succeeding, but has dead letters → amber.
        let runs = vec![done(NOW - 1000, 5)];
        assert_eq!(
            compute("c", &runs, 3, Some(60_000), NOW, &th()).status,
            HealthStatus::Amber
        );
        // Stale: last success older than schedule + grace → amber.
        let stale = vec![done(NOW - 200_000, 5)];
        assert_eq!(
            compute("c", &stale, 0, Some(60_000), NOW, &th()).status,
            HealthStatus::Amber
        );
    }

    #[test]
    fn red_on_high_error_rate_or_no_success() {
        // 2 of 3 terminal runs failed → error_rate 0.66 ≥ red.
        let runs = vec![failed(), failed(), done(NOW - 1000, 1)];
        assert_eq!(
            compute("c", &runs, 0, Some(60_000), NOW, &th()).status,
            HealthStatus::Red
        );
        // Has runs but none succeeded → red.
        let none_ok = vec![failed(), failed()];
        assert_eq!(
            compute("c", &none_ok, 0, None, NOW, &th()).status,
            HealthStatus::Red
        );
    }

    #[test]
    fn unknown_when_no_terminal_runs() {
        let inflight = vec![HealthRun {
            state: "leased".into(),
            finished_ms: None,
            rows_written: 0,
        }];
        assert_eq!(
            compute("c", &inflight, 0, None, NOW, &th()).status,
            HealthStatus::Unknown
        );
        assert_eq!(
            compute("c", &[], 0, None, NOW, &th()).status,
            HealthStatus::Unknown
        );
    }

    #[test]
    fn worst_ranks_red_highest() {
        use HealthStatus::*;
        assert_eq!(worst([Green, Amber, Red]), Red);
        assert_eq!(worst([Green, Unknown]), Green);
        assert_eq!(worst([Unknown, Unknown]), Unknown);
    }

    #[test]
    fn runner_usage_status_tracks_high_water() {
        // No high-water configured → gate off → always green regardless of load.
        assert_eq!(
            RunnerUsage::from_sample(0.99, 0.99, 5, None).status,
            HealthStatus::Green
        );
        // Configured: green well under, amber within 10%, red at/over.
        assert_eq!(
            RunnerUsage::from_sample(0.50, 0.10, 3, Some(0.8)).status,
            HealthStatus::Green
        );
        assert_eq!(
            RunnerUsage::from_sample(0.74, 0.10, 3, Some(0.8)).status,
            HealthStatus::Amber
        );
        assert_eq!(
            RunnerUsage::from_sample(0.85, 0.10, 3, Some(0.8)).status,
            HealthStatus::Red
        );
        // cpu dominates when higher.
        assert_eq!(
            RunnerUsage::from_sample(0.10, 0.95, 3, Some(0.8)).status,
            HealthStatus::Red
        );
    }
}
