//! Leader-elected autoscaler ([[WEIR-T-0104]] / [[WEIR-A-0023]]) — scales each tenant's runner
//! Deployment on **queue depth** (up under load, **to zero** when idle). Only the store-lease leader
//! acts, so multiple control-plane replicas are safe; the election needs no k8s (portable).

use crate::{Actuator, Relay};
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

/// Fleet-saturation signal ([[WEIR-I-0035]] F1.4): `true` when the given tenant's fleet is at its
/// high-water mark (runners gating new resident claims), so the autoscaler adds one instance.
pub type SaturationFn = std::sync::Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// depth → replicas: `0` when idle (scale-to-zero), else `ceil(depth / per_replica)` clamped `[min, max]`.
#[derive(Clone, Copy, Debug)]
pub struct ScalePolicy {
    pub min: i32,
    pub max: i32,
    /// Queued units one replica is sized to handle.
    pub per_replica: i64,
}

impl Default for ScalePolicy {
    fn default() -> Self {
        Self {
            min: 1,
            max: 8,
            per_replica: 20,
        }
    }
}

impl ScalePolicy {
    pub fn target(&self, depth: i64) -> i32 {
        if depth <= 0 {
            return 0;
        }
        let per = self.per_replica.max(1);
        let n = ((depth + per - 1) / per) as i32; // ceil
        let lo = self.min.max(1);
        n.clamp(lo, self.max.max(lo))
    }
}

/// Target replicas for `depth`, nudged **+1** (clamped to `max`) when the fleet is saturated
/// ([[WEIR-I-0035]] F1.4): a fully-loaded fleet — runners at their high-water mark, gating new
/// resident claims — gets another instance beyond what queue depth alone would request. Only
/// nudges when there is actually work (`depth > 0`), so scale-to-zero is preserved.
fn scaled_target(policy: &ScalePolicy, depth: i64, saturated: bool) -> i32 {
    let mut t = policy.target(depth);
    if saturated && depth > 0 && t < policy.max {
        t = (t + 1).min(policy.max);
    }
    t
}

/// The autoscaler: a store-lease **leader** that scales per-tenant runners each `tick`.
pub struct Autoscaler<A: Actuator> {
    relay: Relay,
    actuator: A,
    owner: String,
    policy: ScalePolicy,
    lease_ttl: Duration,
    scaled: Mutex<HashSet<String>>,
    /// Optional fleet-saturation signal ([[WEIR-I-0035]] F1.4): `saturated(tenant) == true` nudges
    /// that tenant's target up one replica. `None` = depth-only scaling (default, unchanged).
    saturation: Option<SaturationFn>,
}

impl<A: Actuator> Autoscaler<A> {
    pub fn new(
        relay: Relay,
        actuator: A,
        owner: String,
        policy: ScalePolicy,
        lease_ttl: Duration,
    ) -> Self {
        Self {
            relay,
            actuator,
            owner,
            policy,
            lease_ttl,
            scaled: Mutex::new(HashSet::new()),
            saturation: None,
        }
    }

    /// Wire a fleet-saturation signal ([[WEIR-I-0035]] F1.4): when `saturated(tenant)` is true and
    /// the tenant has queued work, nudge that tenant's target up one replica (clamped to `max`) so
    /// a fully-loaded fleet scales out beyond queue depth alone. Default: no signal (unchanged).
    pub fn with_saturation(mut self, saturated: SaturationFn) -> Self {
        self.saturation = Some(saturated);
        self
    }

    /// One cycle: **if leader**, scale each active tenant to its target + scale-to-zero the tenants that
    /// went idle. Returns whether we led this cycle (a non-leader is a no-op).
    pub async fn tick(&self) -> Result<bool, String> {
        if !self
            .relay
            .try_acquire_lease("autoscaler", &self.owner, self.lease_ttl)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        let active = self.relay.active_tenants().map_err(|e| e.to_string())?;
        let active_set: HashSet<String> = active.iter().cloned().collect();
        for t in &active {
            let depth = self.relay.pending_depth(t).map_err(|e| e.to_string())?;
            metrics::gauge!("weir_queue_depth", "tenant" => t.clone()).set(depth as f64);
            let saturated = self.saturation.as_ref().map(|f| f(t)).unwrap_or(false);
            let _ = self
                .actuator
                .scale(t, scaled_target(&self.policy, depth, saturated))
                .await;
        }
        // Scale-to-zero the tenants we'd scaled that are no longer active (lock dropped before await).
        let idle: Vec<String> = {
            let scaled = self.scaled.lock().unwrap();
            scaled.difference(&active_set).cloned().collect()
        };
        for t in &idle {
            let _ = self.actuator.scale(t, 0).await;
        }
        {
            let mut scaled = self.scaled.lock().unwrap();
            for t in &idle {
                scaled.remove(t);
            }
            for t in &active {
                scaled.insert(t.clone());
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn relay() -> (tempfile::TempDir, Relay) {
        let tmp = tempfile::TempDir::new().unwrap();
        let store =
            Arc::new(weir_engine::Store::open(tmp.path().join("w.db").to_str().unwrap()).unwrap());
        (tmp, Relay::new(store).unwrap())
    }

    #[test]
    fn policy_scales_to_zero_and_ramps_bounded() {
        let p = ScalePolicy {
            min: 1,
            max: 4,
            per_replica: 10,
        };
        assert_eq!(p.target(0), 0); // idle → scale-to-zero
        assert_eq!(p.target(1), 1); // any work → at least min
        assert_eq!(p.target(10), 1);
        assert_eq!(p.target(11), 2); // ceil(11/10)
        assert_eq!(p.target(1000), 4); // clamped to max
    }

    #[test]
    fn saturation_nudges_target_up_one_within_max() {
        let p = ScalePolicy {
            min: 1,
            max: 4,
            per_replica: 10,
        };
        assert_eq!(scaled_target(&p, 0, true), 0); // no work → no nudge (scale-to-zero preserved)
        assert_eq!(scaled_target(&p, 1, false), 1); // depth alone → min
        assert_eq!(scaled_target(&p, 1, true), 2); // saturated fleet → +1 instance
        assert_eq!(scaled_target(&p, 1000, true), 4); // already at max → clamped, no runaway
    }

    /// [[WEIR-I-0035]] F1 **Goal 3** (end-to-end): a saturated fleet with queued resident work makes
    /// the leader autoscaler **request another instance via the actuator** — the "hit high-water →
    /// launch another instance, not an OOM" path, driven through a real `tick()` + a fake actuator.
    #[tokio::test]
    async fn saturated_fleet_scales_out_through_the_actuator() {
        use crate::{ConnectorRef, ExecutionMode, Origin, WorkSpec};
        use std::sync::Mutex as StdMutex;
        use weir_connector::{Config, ConfiguredStream, MappingSpec, SyncMode, WriteMode};

        fn resident_ws(conn: &str) -> WorkSpec {
            let wasm = ConnectorRef::Wasm {
                search_path: ".".into(),
                package: "p".into(),
                version: "0".into(),
                origin: Origin::FirstParty,
            };
            WorkSpec {
                connection: conn.into(),
                tenant: "default".into(),
                stream: ConfiguredStream {
                    stream: "s".into(),
                    sync_mode: SyncMode::FullRefresh,
                    cursor_field: None,
                    primary_key: None,
                    write_mode: WriteMode::Append,
                    mapping: MappingSpec::default(),
                },
                source: wasm.clone(),
                dest: wasm,
                source_config: Config { json: "{}".into() },
                dest_config: Config { json: "{}".into() },
                state_key: None,
                seed_cursor: None,
                partition: None,
                execution_mode: ExecutionMode::Resident {
                    cadence_ms: Some(50),
                    restart_backoff_ms: 100,
                },
            }
        }

        // Records every scale request the autoscaler issues.
        #[derive(Clone, Default)]
        struct Rec(Arc<StdMutex<Vec<(String, i32)>>>);
        #[async_trait::async_trait]
        impl Actuator for Rec {
            async fn scale(&self, tenant: &str, replicas: i32) -> Result<(), String> {
                self.0.lock().unwrap().push((tenant.to_string(), replicas));
                Ok(())
            }
            async fn remove(&self, _tenant: &str) -> Result<(), String> {
                Ok(())
            }
        }

        let (_t, relay) = relay();
        // Queued resident work for `default` → depth 2 (target 1 at per_replica 20).
        relay.plan(&resident_ws("c1")).unwrap();
        relay.plan(&resident_ws("c2")).unwrap();

        let rec = Rec::default();
        let auto = Autoscaler::new(
            relay,
            rec.clone(),
            "owner".into(),
            ScalePolicy {
                min: 1,
                max: 8,
                per_replica: 20,
            },
            Duration::from_secs(60),
        )
        .with_saturation(Arc::new(|_tenant: &str| true)); // fleet at high-water

        assert!(auto.tick().await.unwrap(), "fresh relay → autoscaler leads");
        let calls = rec.0.lock().unwrap().clone();
        assert!(
            calls.iter().any(|(t, r)| t == "default" && *r == 2),
            "saturated fleet (depth 2 → target 1) nudged +1 → scale-out to 2 replicas; got {calls:?}"
        );
    }

    #[test]
    fn resident_gate_decision_and_usage() {
        use crate::{Usage, gate_new_resident};
        assert!(!gate_new_resident(None, 0.99, true)); // gate off unless configured
        assert!(!gate_new_resident(Some(0.8), 0.99, false)); // run-once never gated
        assert!(!gate_new_resident(Some(0.8), 0.79, true)); // resident, below high-water
        assert!(gate_new_resident(Some(0.8), 0.80, true)); // resident, at high-water → gate
        assert_eq!(
            Usage {
                mem_fraction: 0.3,
                cpu_fraction: 0.9
            }
            .max_fraction(),
            0.9
        );
    }

    #[test]
    fn leader_lease_is_single_holder() {
        let (_t, relay) = relay();
        // A takes the lease; B can't take a live one; A renews.
        assert!(
            relay
                .try_acquire_lease("autoscaler", "A", Duration::from_secs(60))
                .unwrap()
        );
        assert!(
            !relay
                .try_acquire_lease("autoscaler", "B", Duration::from_secs(60))
                .unwrap()
        );
        assert!(
            relay
                .try_acquire_lease("autoscaler", "A", Duration::from_secs(60))
                .unwrap()
        );
        // A's lease expires immediately → B can take over.
        assert!(
            relay
                .try_acquire_lease("autoscaler", "A", Duration::from_millis(0))
                .unwrap()
        );
        std::thread::sleep(Duration::from_millis(5));
        assert!(
            relay
                .try_acquire_lease("autoscaler", "B", Duration::from_secs(60))
                .unwrap()
        );
        assert!(
            !relay
                .try_acquire_lease("autoscaler", "A", Duration::from_secs(60))
                .unwrap()
        );
    }
}
