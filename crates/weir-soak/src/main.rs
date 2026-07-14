//! `weir-soak` ([[WEIR-I-0023]]) — provision a connector **fleet** on a running weir and soak it,
//! asserting health invariants. Drives everything over the HTTP API against a `--base-url`, so it
//! points at compose today and kind/k8s later. This file is [[WEIR-T-0122]] (the provisioner); the
//! soak loop + invariants land in [[WEIR-T-0123]].

use anyhow::Result;
use clap::Parser;
use serde_json::{Value, json};

#[derive(Parser, Debug)]
#[command(
    name = "weir-soak",
    about = "Provision a connector fleet + soak a running weir"
)]
struct Cli {
    /// Base URL of the running weir control plane.
    #[arg(
        long,
        env = "WEIR_SOAK_BASE_URL",
        default_value = "http://localhost:8080"
    )]
    base_url: String,
    /// Admin API key (bearer) — e.g. the server's bootstrap key.
    #[arg(long, env = "WEIR_SOAK_ADMIN_KEY")]
    admin_key: String,
    /// Number of local echo/slow connections to provision (the deterministic volume).
    #[arg(long, default_value_t = 8)]
    fleet: usize,
    /// Number of resident (long-lived, F1) connections to provision + start. 0 = none.
    #[arg(long, default_value_t = 0)]
    resident: usize,
    /// Postgres URL the provisioned pg connections use (as the weir *server* reaches it).
    #[arg(
        long,
        env = "WEIR_SOAK_PG_URL",
        default_value = "postgres://weir:weir@postgres:5432/weir"
    )]
    pg_url: String,
    /// Tight schedule interval (seconds) for the local fleet.
    #[arg(long, default_value_t = 1.0)]
    every_secs: f64,
    /// Skip the best-effort live REST corpus.
    #[arg(long)]
    no_live_rest: bool,
    /// Skip the postgres source/dest pair (a fully local, dependency-free fleet — used by the CI smoke).
    #[arg(long)]
    no_postgres: bool,
    /// Total soak duration in seconds (0 = provision only, no soak loop).
    #[arg(long, default_value_t = 90.0)]
    duration: f64,
    /// Poll window in seconds.
    #[arg(long, default_value_t = 5.0)]
    window: f64,
    /// Min run completions per window (after warmup) for the throughput invariant.
    #[arg(long, default_value_t = 1)]
    min_throughput: u64,
    /// Max in-flight (pending/running) runs before the queue is deemed unbounded.
    #[arg(long, default_value_t = 40)]
    max_queue: u64,
    /// Max new dead-letters per window (gated connections only) before a DL blowup.
    #[arg(long, default_value_t = 50)]
    max_dl_delta: u64,
    /// Windows to skip before enforcing throughput/DL (schedules warming up).
    #[arg(long, default_value_t = 2)]
    warmup: usize,
    /// Consecutive sub-throughput windows tolerated before it counts as a stall.
    #[arg(long, default_value_t = 3)]
    max_stall: usize,
}

/// One connection to provision.
#[derive(Debug, Clone)]
struct ConnSpec {
    name: String,
    source: String,
    dest: String,
    stream: String,
    config: Value,
    every_secs: f64,
    /// `"run_once"` (scheduled) or `"resident"` (long-lived, started explicitly — F1).
    execution_mode: String,
}

/// Resident (F1) connections: long-lived echo/slow sources, `execution_mode=resident`, launched via
/// `POST /connections/{name}/start` (the scheduler deliberately does not fire them). Named
/// `soak-resident-*` so the run accounting can segregate them from the scheduled gates.
fn resident_plan(n: usize) -> Vec<ConnSpec> {
    (0..n)
        .map(|i| {
            let (source, config) = if i % 2 == 0 {
                ("Echo", json!({}))
            } else {
                ("Slow", json!({ "rows": 5, "batch": true, "sleep_ms": 0 }))
            };
            ConnSpec {
                name: format!("soak-resident-{i}"),
                source: source.to_string(),
                dest: "ArrowSink".to_string(),
                stream: format!("r{i}"),
                config,
                every_secs: 1.0,
                execution_mode: "resident".to_string(),
            }
        })
        .collect()
}

/// True for the resident fleet (`soak-resident-*`) — excluded from the scheduled throughput/queue/DL
/// gates (a resident unit sits perpetually pending/leased and completes no discrete runs).
fn is_resident(connection: &str) -> bool {
    connection.starts_with("soak-resident-")
}

/// The deterministic local + postgres fleet (pure — unit-testable without a server): `fleet` local
/// echo/slow → arrow connections (alternating, tight cadence) + a postgres write/read pair for real
/// DB load. Live-REST connections are added separately (best-effort, excluded from the hard gate).
fn fleet_plan(fleet: usize, pg_url: &str, every_secs: f64, postgres: bool) -> Vec<ConnSpec> {
    let mut v = Vec::new();
    for i in 0..fleet {
        let (source, config) = if i % 2 == 0 {
            ("Echo", json!({}))
        } else {
            ("Slow", json!({ "rows": 5, "batch": true, "sleep_ms": 0 }))
        };
        v.push(ConnSpec {
            name: format!("soak-local-{i}"),
            source: source.to_string(),
            dest: "ArrowSink".to_string(),
            stream: format!("s{i}"),
            config,
            every_secs,
            execution_mode: "run_once".to_string(),
        });
    }
    if !postgres {
        return v;
    }
    // Real DB load: write into a table, then read it back.
    v.push(ConnSpec {
        name: "soak-pg-dst".to_string(),
        source: "Slow".to_string(),
        dest: "Postgres".to_string(),
        stream: "soak_pg".to_string(),
        config: json!({ "url": pg_url, "rows": 5, "batch": true, "sleep_ms": 0 }),
        every_secs: every_secs.max(2.0),
        execution_mode: "run_once".to_string(),
    });
    v.push(ConnSpec {
        name: "soak-pg-src".to_string(),
        source: "Postgres".to_string(),
        dest: "ArrowSink".to_string(),
        stream: "soak_pg".to_string(),
        config: json!({ "url": pg_url, "table": "soak_pg" }),
        every_secs: every_secs.max(2.0),
        execution_mode: "run_once".to_string(),
    });
    v
}

/// A tiny bearer-authed HTTP client for the weir API.
struct Client {
    base: String,
    key: String,
    http: reqwest::Client,
}

impl Client {
    fn new(base: &str, key: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            key: key.to_string(),
            http: reqwest::Client::new(),
        }
    }

    async fn get_json(&self, path: &str) -> Result<Value> {
        Ok(self
            .http
            .get(format!("{}{path}", self.base))
            .bearer_auth(&self.key)
            .send()
            .await?
            .json()
            .await?)
    }

    async fn post(&self, path: &str, body: Value) -> Result<reqwest::Response> {
        Ok(self
            .http
            .post(format!("{}{path}", self.base))
            .bearer_auth(&self.key)
            .json(&body)
            .send()
            .await?)
    }

    /// Create a connection. `Ok(true)` = created; `Ok(false)` = already exists / rejected (non-fatal
    /// so the soak is re-runnable); `Err` = an unexpected transport/status failure.
    async fn create_connection(&self, c: &ConnSpec) -> Result<bool> {
        let body = json!({
            "name": c.name, "source": c.source, "dest": c.dest, "stream": c.stream,
            "config": c.config, "every_secs": c.every_secs,
            "execution_mode": c.execution_mode,
        });
        let resp = self
            .http
            .post(format!("{}/connections", self.base))
            .bearer_auth(&self.key)
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            Ok(true)
        } else if matches!(status.as_u16(), 400 | 409) {
            Ok(false) // already exists / rejected — non-fatal for a re-run
        } else {
            anyhow::bail!("create {} → HTTP {status}", c.name)
        }
    }

    /// Launch a resident connection (`POST /connections/{name}/start`). Idempotent server-side.
    async fn start(&self, name: &str) -> Result<bool> {
        let resp = self
            .post(&format!("/connections/{name}/start"), json!({}))
            .await?;
        let status = resp.status();
        if status.is_success() {
            Ok(true)
        } else {
            anyhow::bail!("start {name} → HTTP {status}")
        }
    }

    /// Active (pending/leased/running) unit count for one connection, via its own run list —
    /// robust to the global `/runs` recent-window limit. 0 on any transport/parse error.
    async fn active_unit_count(&self, name: &str) -> u32 {
        match self.get_json(&format!("/connections/{name}/runs")).await {
            Ok(v) => {
                let runs: Vec<SoakRun> = serde_json::from_value(v).unwrap_or_default();
                runs.iter()
                    .filter(|r| matches!(r.state.as_str(), "pending" | "running" | "leased"))
                    .count() as u32
            }
            Err(_) => 0,
        }
    }
}

/// The staged catalog package backing a friendly connector name (for `/catalog/import`).
fn pkg_for(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "echo" => Some("weir-echo-pkg"),
        "slow" => Some("weir-slow-pkg"),
        "arrowsink" | "arrow-sink" => Some("weir-arrow-sink-pkg"),
        "postgres" => Some("weir-postgres-pkg"),
        _ => None,
    }
}

/// Register the connector packages a plan needs into the catalog ([[WEIR-T-0124]]) — a connection
/// can't resolve its connector at run time until its package is imported. Best-effort/idempotent.
async fn import_packages(client: &Client, plan: &[ConnSpec]) {
    let mut pkgs: Vec<&str> = Vec::new();
    for c in plan {
        for name in [c.source.as_str(), c.dest.as_str()] {
            if let Some(pkg) = pkg_for(name)
                && !pkgs.contains(&pkg)
            {
                pkgs.push(pkg);
            }
        }
    }
    for pkg in pkgs {
        let _ = client
            .post("/catalog/import", json!({ "package": pkg }))
            .await;
        println!("  registered {pkg}");
    }
}

/// Provision the deterministic fleet, then (optionally) the best-effort live REST corpus.
async fn provision(client: &Client, plan: &[ConnSpec], live_rest: bool) -> Result<usize> {
    let mut created = 0;
    for c in plan {
        match client.create_connection(c).await {
            Ok(true) => {
                created += 1;
                println!("  + {} ({} → {})", c.name, c.source, c.dest);
            }
            Ok(false) => println!("  · {} (exists/skipped)", c.name),
            Err(e) => println!("  ! {} — {e}", c.name),
        }
    }
    if live_rest {
        created += provision_live_rest(client).await.unwrap_or(0);
    }
    Ok(created)
}

/// A small allowlist of **no-auth REST source** manifests to soak (must be importable + need no
/// secrets). Not the whole `/catalog/available` — that includes dests + authed connectors that just
/// fail. External, so best-effort + excluded from the DL gate ([[WEIR-I-0023]]).
const LIVE_REST_SOURCES: &[&str] = &["frankfurter", "exchangerate"];

/// Best-effort: import + connect a handful of no-auth REST sources. Never fatal.
async fn provision_live_rest(client: &Client) -> Result<usize> {
    let mut created = 0;
    for m in LIVE_REST_SOURCES {
        // Onboard the manifest, then wire a connection off it.
        let _ = client
            .post("/catalog/import", json!({ "manifest_name": m }))
            .await;
        let spec = ConnSpec {
            name: format!("soak-rest-{m}"),
            source: (*m).to_string(),
            dest: "ArrowSink".to_string(),
            stream: (*m).to_string(),
            config: json!({}),
            every_secs: 5.0,
            execution_mode: "run_once".to_string(),
        };
        if let Ok(true) = client.create_connection(&spec).await {
            created += 1;
            println!("  + {} (live REST: {m})", spec.name);
        }
    }
    Ok(created)
}

/// Whether a connection is on the **hard gate** — the deterministic local/pg fleet. Live-REST
/// connections (`soak-rest-*`) are excluded (external flakiness is expected, [[WEIR-I-0023]]).
fn is_gated(connection: &str) -> bool {
    !connection.starts_with("soak-rest-") && !is_resident(connection)
}

/// A minimal view of a `/runs` row.
#[derive(serde::Deserialize)]
struct SoakRun {
    id: i64,
    #[serde(default)]
    connection: String,
    state: String,
    #[serde(default)]
    dead_lettered: i64,
}

/// One observation window: cumulative completed runs, current in-flight depth, cumulative
/// gated dead-letters, and whether the API answered.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sample {
    cum_done: u64,
    queue: u64,
    cum_dl: u64,
    api_ok: bool,
    /// Resident (F1) conns with ≥1 unit in pending/leased/running this window.
    residents_alive: usize,
    /// Resident conns with >1 active unit — an enqueue-once violation.
    residents_overrun: usize,
}

struct Thresholds {
    min_throughput: u64,
    max_queue: u64,
    max_dl_delta: u64,
    warmup: usize,
    /// Consecutive sub-throughput windows tolerated before it's a stall (an isolated 0-window under
    /// single-node lock contention is fine; a *sustained* stall is the failure).
    max_stall: usize,
    /// Number of resident (F1) connections expected alive each window (0 = no resident checks).
    resident_expected: usize,
}

#[derive(Debug, PartialEq)]
enum BreachKind {
    ApiDown,
    NoThroughput,
    QueueUnbounded,
    DlBlowup,
    /// A resident connection has no active unit — it silently died (F1 supervision failure).
    ResidentDead,
    /// A resident connection has >1 active unit — enqueue-once violated.
    ResidentOverrun,
}

#[derive(Debug, PartialEq)]
struct Breach {
    window: usize,
    kind: BreachKind,
}

/// Evaluate the invariants over the window series (pure). Liveness + bounded-queue every window;
/// throughput + bounded-DL only once past warmup (and with a prior window to delta against).
fn evaluate(samples: &[Sample], t: &Thresholds) -> Vec<Breach> {
    let mut breaches = Vec::new();
    let mut stall = 0usize;
    for (i, s) in samples.iter().enumerate() {
        if !s.api_ok {
            breaches.push(Breach {
                window: i,
                kind: BreachKind::ApiDown,
            });
            continue; // no throughput/queue signal from a failed poll
        }
        if s.queue > t.max_queue {
            breaches.push(Breach {
                window: i,
                kind: BreachKind::QueueUnbounded,
            });
        }
        if i >= t.warmup.max(1) {
            let prev = &samples[i - 1];
            if s.cum_done.saturating_sub(prev.cum_done) < t.min_throughput {
                stall += 1;
                if stall >= t.max_stall.max(1) {
                    breaches.push(Breach {
                        window: i,
                        kind: BreachKind::NoThroughput,
                    });
                }
            } else {
                stall = 0;
            }
            if s.cum_dl.saturating_sub(prev.cum_dl) > t.max_dl_delta {
                breaches.push(Breach {
                    window: i,
                    kind: BreachKind::DlBlowup,
                });
            }
        }
        // Resident (F1) invariants: every resident conn stays alive (≥1 active unit) and never
        // accumulates >1 (enqueue-once). Gated by warmup so residents have time to be claimed.
        if t.resident_expected > 0 && i >= t.warmup.max(1) {
            if s.residents_alive < t.resident_expected {
                breaches.push(Breach {
                    window: i,
                    kind: BreachKind::ResidentDead,
                });
            }
            if s.residents_overrun > 0 {
                breaches.push(Breach {
                    window: i,
                    kind: BreachKind::ResidentOverrun,
                });
            }
        }
    }
    breaches
}

/// Observe one window: fold `/runs` into a `Sample`, updating the seen-done set + cumulative gated DL.
async fn observe(
    client: &Client,
    seen_done: &mut std::collections::HashSet<i64>,
    cum_dl: &mut u64,
    resident_names: &[String],
) -> Sample {
    // Scheduled gates from the global recent-runs view (resident units excluded from `queue`).
    let (cum_done, queue, cum_dl_v, api_ok) = match client.get_json("/runs").await {
        Ok(v) => {
            let runs: Vec<SoakRun> = serde_json::from_value(v).unwrap_or_default();
            let mut queue = 0u64;
            for r in &runs {
                match r.state.as_str() {
                    "done" => {
                        if seen_done.insert(r.id) && is_gated(&r.connection) {
                            *cum_dl += r.dead_lettered.max(0) as u64;
                        }
                    }
                    "pending" | "running" | "leased" if !is_resident(&r.connection) => queue += 1,
                    _ => {}
                }
            }
            (seen_done.len() as u64, queue, *cum_dl, true)
        }
        Err(_) => (seen_done.len() as u64, 0, *cum_dl, false),
    };
    // Resident (F1) liveness via each connection's own run list (robust to the recent-window limit).
    let mut residents_alive = 0usize;
    let mut residents_overrun = 0usize;
    if api_ok {
        for name in resident_names {
            let active = client.active_unit_count(name).await;
            if active >= 1 {
                residents_alive += 1;
            }
            if active > 1 {
                residents_overrun += 1;
            }
        }
    }
    Sample {
        cum_done,
        queue,
        cum_dl: cum_dl_v,
        api_ok,
        residents_alive,
        residents_overrun,
    }
}

/// Run the soak loop: poll each window until `duration` elapses, then evaluate + summarize.
/// Returns `true` if all invariants held.
async fn soak(client: &Client, cli: &Cli) -> bool {
    let t = Thresholds {
        min_throughput: cli.min_throughput,
        max_queue: cli.max_queue,
        max_dl_delta: cli.max_dl_delta,
        warmup: cli.warmup,
        max_stall: cli.max_stall,
        resident_expected: cli.resident,
    };
    let resident_names: Vec<String> = (0..cli.resident)
        .map(|i| format!("soak-resident-{i}"))
        .collect();
    let mut seen_done = std::collections::HashSet::new();
    let mut cum_dl = 0u64;
    let mut samples = Vec::new();
    let start = tokio::time::Instant::now();
    let total = std::time::Duration::from_secs_f64(cli.duration);
    let window = std::time::Duration::from_secs_f64(cli.window.max(0.5));
    let mut w = 0;
    println!(
        "weir-soak: soaking for {:.0}s (window {:.0}s)…",
        cli.duration, cli.window
    );
    while start.elapsed() < total {
        tokio::time::sleep(window).await;
        let s = observe(client, &mut seen_done, &mut cum_dl, &resident_names).await;
        println!(
            "  window {w}: done={} queue={} dl={} api={}{}",
            s.cum_done,
            s.queue,
            s.cum_dl,
            if s.api_ok { "ok" } else { "DOWN" },
            if cli.resident > 0 {
                format!(
                    " resident={}/{} (overrun={})",
                    s.residents_alive, cli.resident, s.residents_overrun
                )
            } else {
                String::new()
            }
        );
        samples.push(s);
        w += 1;
    }

    let breaches = evaluate(&samples, &t);
    let max_queue = samples.iter().map(|s| s.queue).max().unwrap_or(0);
    let total_done = samples.last().map(|s| s.cum_done).unwrap_or(0);
    let total_dl = samples.last().map(|s| s.cum_dl).unwrap_or(0);
    println!("\nweir-soak summary:");
    println!("  windows       {}", samples.len());
    println!("  runs completed {total_done}");
    println!("  max in-flight  {max_queue}");
    println!("  gated dead-letters {total_dl}");
    let mut invariants = vec!["liveness", "throughput", "bounded-queue", "bounded-dl"];
    if cli.resident > 0 {
        invariants.push("resident");
    }
    for inv in invariants {
        let failed = breaches.iter().any(|b| invariant_name(&b.kind) == inv);
        println!("  [{}] {inv}", if failed { "FAIL" } else { "PASS" });
    }
    if breaches.is_empty() {
        println!("weir-soak: PASS");
        true
    } else {
        for b in &breaches {
            println!("  ! window {} — {:?}", b.window, b.kind);
        }
        println!("weir-soak: FAIL ({} breach(es))", breaches.len());
        false
    }
}

fn invariant_name(k: &BreachKind) -> &'static str {
    match k {
        BreachKind::ApiDown => "liveness",
        BreachKind::NoThroughput => "throughput",
        BreachKind::QueueUnbounded => "bounded-queue",
        BreachKind::DlBlowup => "bounded-dl",
        BreachKind::ResidentDead | BreachKind::ResidentOverrun => "resident",
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = Client::new(&cli.base_url, &cli.admin_key);
    let plan = fleet_plan(cli.fleet, &cli.pg_url, cli.every_secs, !cli.no_postgres);
    println!(
        "weir-soak: provisioning {} connections against {}",
        plan.len(),
        cli.base_url
    );
    import_packages(&client, &plan).await;
    let created = provision(&client, &plan, !cli.no_live_rest).await?;
    println!("weir-soak: provisioned {created} connections.");
    // Resident (F1) fleet: create + explicitly START (the scheduler skips resident specs, so nothing
    // fires them otherwise). Enqueue-once is enforced server-side (idempotent start).
    if cli.resident > 0 {
        let rplan = resident_plan(cli.resident);
        import_packages(&client, &rplan).await;
        let rcreated = provision(&client, &rplan, false).await?;
        println!("weir-soak: provisioned {rcreated} resident connections; starting…");
        let mut started = 0;
        for c in &rplan {
            match client.start(&c.name).await {
                Ok(_) => {
                    started += 1;
                    println!("  ▶ {} (resident started)", c.name);
                }
                Err(e) => println!("  ! start {} — {e}", c.name),
            }
        }
        println!(
            "weir-soak: started {started}/{} resident connections.",
            rplan.len()
        );
    }
    if cli.duration <= 0.0 {
        return Ok(()); // provision-only
    }
    if !soak(&client, &cli).await {
        std::process::exit(1); // an invariant breached
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fleet_plan_shape() {
        let p = fleet_plan(4, "postgres://x", 1.0, true);
        assert_eq!(p.len(), 6, "4 local + 2 pg");
        assert_eq!(
            p.iter()
                .filter(|c| c.name.starts_with("soak-local-"))
                .count(),
            4
        );
        assert!(
            p.iter()
                .any(|c| c.name == "soak-pg-dst" && c.dest == "Postgres")
        );
        assert!(
            p.iter()
                .any(|c| c.name == "soak-pg-src" && c.source == "Postgres")
        );
        // The local fleet alternates echo/slow, and every schedule is tight.
        assert!(p.iter().any(|c| c.source == "Echo"));
        assert!(p.iter().any(|c| c.source == "Slow"));
        assert!(p.iter().all(|c| c.every_secs >= 1.0));
        // Without postgres → local only (the CI-smoke shape).
        assert_eq!(fleet_plan(4, "u", 1.0, false).len(), 4);
    }

    fn thresholds() -> Thresholds {
        Thresholds {
            min_throughput: 1,
            max_queue: 40,
            max_dl_delta: 50,
            warmup: 1,
            max_stall: 1,
            resident_expected: 0,
        }
    }
    fn ok(cum_done: u64, queue: u64, cum_dl: u64) -> Sample {
        Sample {
            cum_done,
            queue,
            cum_dl,
            api_ok: true,
            residents_alive: 0,
            residents_overrun: 0,
        }
    }

    #[test]
    fn evaluate_passes_a_healthy_series() {
        let s = vec![ok(0, 3, 0), ok(4, 5, 0), ok(9, 4, 1), ok(15, 6, 1)];
        assert!(evaluate(&s, &thresholds()).is_empty());
    }

    #[test]
    fn evaluate_flags_each_breach() {
        // Stalled throughput: cum_done flat past warmup.
        let stalled = vec![ok(0, 1, 0), ok(3, 1, 0), ok(3, 1, 0)];
        assert!(
            evaluate(&stalled, &thresholds())
                .iter()
                .any(|b| b.kind == BreachKind::NoThroughput)
        );

        // Unbounded queue.
        let backed_up = vec![ok(0, 1, 0), ok(5, 99, 0)];
        assert!(
            evaluate(&backed_up, &thresholds())
                .iter()
                .any(|b| b.kind == BreachKind::QueueUnbounded)
        );

        // Dead-letter blowup (gated).
        let dl = vec![ok(0, 1, 0), ok(5, 1, 200)];
        assert!(
            evaluate(&dl, &thresholds())
                .iter()
                .any(|b| b.kind == BreachKind::DlBlowup)
        );

        // API down.
        let down = vec![
            ok(0, 1, 0),
            Sample {
                cum_done: 0,
                queue: 0,
                cum_dl: 0,
                api_ok: false,
                residents_alive: 0,
                residents_overrun: 0,
            },
        ];
        assert!(
            evaluate(&down, &thresholds())
                .iter()
                .any(|b| b.kind == BreachKind::ApiDown)
        );
    }

    #[test]
    fn evaluate_resident_invariants() {
        let t = Thresholds {
            resident_expected: 2,
            ..thresholds()
        };
        // Healthy throughput (cum_done climbs) so only the resident invariant is under test.
        let s = |done, alive, over| Sample {
            cum_done: done,
            queue: 1,
            cum_dl: 0,
            api_ok: true,
            residents_alive: alive,
            residents_overrun: over,
        };
        // Both residents alive, no overrun → clean.
        assert!(evaluate(&[s(0, 2, 0), s(5, 2, 0), s(10, 2, 0)], &t).is_empty());
        // A resident died past warmup → ResidentDead.
        assert!(
            evaluate(&[s(0, 2, 0), s(5, 2, 0), s(10, 1, 0)], &t)
                .iter()
                .any(|b| b.kind == BreachKind::ResidentDead)
        );
        // A resident accumulated a second active unit → ResidentOverrun.
        assert!(
            evaluate(&[s(0, 2, 0), s(5, 2, 0), s(10, 2, 1)], &t)
                .iter()
                .any(|b| b.kind == BreachKind::ResidentOverrun)
        );
    }

    #[test]
    fn gate_excludes_live_rest() {
        assert!(is_gated("soak-local-3"));
        assert!(is_gated("soak-pg-dst"));
        assert!(!is_gated("soak-rest-0"));
    }
}
