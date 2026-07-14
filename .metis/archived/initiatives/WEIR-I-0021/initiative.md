---
id: kubernetes-native-runner
level: initiative
title: "Kubernetes-native runner provisioning + autoscaling"
short_code: "WEIR-I-0021"
created_at: 2026-07-05T23:46:45.047197+00:00
updated_at: 2026-07-06T11:12:32.207709+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: kubernetes-native-runner
---

# Kubernetes-native runner provisioning + autoscaling Initiative

## Context

Today weir runs the scheduler + worker **in-process** in the single binary (`weir api` spawns `App::serve`,
[[WEIR-A-0028]]/[[WEIR-A-0010]]). To scale — and to isolate tenants at the pod level ([[WEIR-I-0018]]) — the
control plane needs to **provision + autoscale connector runners on Kubernetes**: a control plane that watches
queue depth and spins runner pods up/down, deployable via Helm. This mirrors cloacina's **actuator** (Docker +
Kubernetes provisioners), **autoscaler** (leader-elected), and **Helm charts**.

## Goals & Non-Goals

**Goals**
- **Runner as a separable process** — extract the worker into a standalone `weir runner` that claims work from
  the shared store (the orchestrator's lease model, [[WEIR-A-0010]]/[[WEIR-A-0011]], already supports multiple
  claimants) and executes it, so N runners can run against one control plane. (In-process stays the default for
  local/single-node.)
- **Kubernetes actuator** — the control plane provisions runner **Deployments/Pods** on k8s (a `kube`-rs
  client), gated behind a feature/config. Tenant-scoped runners ([[WEIR-I-0018]]): a runner pod serves one
  tenant's work. Mirror cloacina `actuator/kubernetes.rs` (+ `docker.rs` for local).
- **Autoscaler** — a leader-elected controller that scales runner replicas on **queue depth / lease latency**
  (from the observability metrics, [[WEIR-I-0020]]): scale up when work backs up, down to zero when idle.
  Mirror cloacina `autoscaler/{leader,mod}.rs`.
- **Helm charts** — `charts/weir-server` + `charts/weir-runner` (mirror cloacina's `cloacina-server`/`-agent`):
  deploy the control plane + runners, wire Postgres (SoR), secrets, the OIDC config, and RBAC.
- Decide [[WEIR-A-0023]] (Deployment topology & operator scope) as part of this.

**Non-Goals**
- A full CRD/operator (Custom Resources + a reconcile controller) unless design shows it's needed — start with
  the control plane provisioning Deployments directly (cloacina's model).
- Cloud-managed autoscaling (KEDA/HPA) as the *only* path — we own the scale signal (queue depth), but a
  values.yaml switch to defer to HPA is fine.
- Multi-cluster / federation.

## Prior art (cloacina)

`~/Desktop/cloacina/crates/cloacina-server/src/actuator/{kubernetes,docker,guard,mod}.rs` (provisioners),
`autoscaler/{leader,mod}.rs` (leader-elected scaling), `fleet_coordinator.rs`/`fleet_executor.rs` (the runner
fleet), and `charts/{cloacina-server,cloacina-agent}` (Helm). The `cloacina-agent` chart is the runner analog.

## Weir surfaces to change

- `crates/weir-cli` — a `weir runner` subcommand (worker-only, claims from the store).
- New crate (e.g. `weir-actuator` / in `weir-orchestrator`) — the k8s provisioner (`kube` client) + autoscaler
  (leader election via the store or k8s lease).
- `crates/weir-orchestrator` — ensure the lease/claim model is safe for many external runners (it is by design;
  verify + test at scale).
- `charts/` — `weir-server` + `weir-runner` Helm charts; the `Dockerfile` may split server/runner images.
- ADR: finalize [[WEIR-A-0023]].

## Design decisions (2026-07-06, approved)

1. **Pod-per-tenant Deployment.** One runner Deployment per active tenant, scaled by that tenant's queue depth —
   strong isolation (a pod only ever touches one tenant's work), 1:1 with the [[WEIR-I-0018]] per-tenant `Fleet`
   + cloacina's per-tenant agent.
2. **weir owns the autoscaler.** A **leader-elected** controller (leader via a **store row** — portable, no k8s
   dependency) scales each tenant's runner replicas on **queue depth** (`Relay::pending_depth` per tenant),
   scale-to-zero when idle. A `values.yaml` switch can defer to HPA/KEDA. Direct **Deployments** (no CRD).
3. **All four pieces**, verified via `helm template`/lint + an **optional kind smoke test** (documented, not
   blocking CI — there's no cluster in CI today). k8s deps (`kube`, `k8s-openapi`) live behind a `kubernetes`
   cargo feature so the default build stays light.

## Proposed decomposition (for sign-off)

- **[[WEIR-A-0023]]** (decide): deployment topology — control plane + per-tenant runner Deployments; weir-owned
  store-leader autoscaler on queue depth; direct Deployments (no CRD); Helm.
- **T-a — Extract `weir runner`:** a standalone `weir runner` subcommand (worker-only) that runs the `Fleet`
  claiming from the shared store; in-process stays the single-node default. Test: a runner drains work planned
  by another process.
- **T-b — Kubernetes actuator (`kubernetes` feature):** a `kube`-rs provisioner that ensures a per-tenant runner
  Deployment (create/scale/delete) for each active tenant. Mirror cloacina `actuator/kubernetes.rs`.
- **T-c — Autoscaler:** leader-elected (store row) controller scaling each tenant's Deployment on
  `Relay::pending_depth` (this also lands the deferred `weir_queue_depth` gauge, [[WEIR-T-0099]]); scale-to-zero.
- **T-d — Helm charts + kind smoke:** `charts/weir-server` + `charts/weir-runner` (Postgres, secrets, OIDC,
  RBAC); `helm lint`/`template` in CI + a documented kind smoke test (scale a runner, drain work).

## Exit Criteria (draft — refine in design)

- [ ] `weir runner` runs standalone, claims work from the shared store, and executes it; many runners coexist.
- [ ] The control plane provisions runner Deployments on k8s (feature/config-gated); tenant-scoped per [[WEIR-I-0018]].
- [ ] An autoscaler scales runners on queue depth (up under load, to zero when idle), leader-elected.
- [ ] `charts/weir-server` + `charts/weir-runner` deploy the stack (Postgres, secrets, OIDC, RBAC); documented.
- [ ] [[WEIR-A-0023]] decided; a kind/minikube smoke test scales a runner up + drains work; clippy green.

## Dependencies

- Benefits from [[WEIR-I-0020]] (the queue-depth/latency metrics drive the autoscaler) + [[WEIR-I-0018]]
  (tenant-scoped runners).
