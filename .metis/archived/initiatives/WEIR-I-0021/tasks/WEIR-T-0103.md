---
id: kubernetes-actuator-per-tenant
level: task
title: "Kubernetes actuator — per-tenant runner Deployments"
short_code: "WEIR-T-0103"
created_at: 2026-07-06T10:43:02.868216+00:00
updated_at: 2026-07-06T10:56:01.188997+00:00
parent: WEIR-I-0021
blocked_by: [WEIR-T-0102]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0021
---

# Kubernetes actuator — per-tenant runner Deployments

## Parent Initiative

[[WEIR-I-0021]]. Governed by [[WEIR-A-0023]] (per-tenant Deployments, direct — no CRD, `kubernetes` feature).

## Objective

The control plane provisions a **per-tenant runner Deployment** on Kubernetes ([[WEIR-I-0018]] isolation):
create/scale/delete a `weir-runner` Deployment for each active tenant, via a `kube`-rs client.

## Reference

- cloacina `~/Desktop/cloacina/crates/cloacina-server/src/actuator/{kubernetes,guard,mod}.rs` — the provisioner
  to mirror. `kube = "2.0"` (client, rustls) + `k8s-openapi`.
- The runner image + args from [[WEIR-T-0102]] (`weir runner --tenant <id>`).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] An `Actuator` trait + a `KubernetesActuator` (behind the **`kubernetes` cargo feature** — default build
  compiles no `kube`): `ensure_runner(tenant, replicas)` creates/updates the tenant's `weir-runner` Deployment
  (image, `--tenant`, env: store URL, connectors), `scale(tenant, n)`, `remove(tenant)`.
- [ ] Deployment shape mirrors `charts/weir-runner` ([[WEIR-T-0105]]) — labels/selectors keyed by tenant, the
  store secret + connectors wired.
- [ ] Feature-gated so the default (non-k8s) build is unaffected; a `NoopActuator` for in-process/single-node.
- [ ] Unit tests on the Deployment manifest builder (correct name/labels/args per tenant); the live create/scale
  is covered by the kind smoke ([[WEIR-T-0105]]). Workspace builds with + without `--features kubernetes`; clippy clean.

## Status Updates

### 2026-07-06 — done (`3dcbebd`)

`weir-orchestrator/src/actuator.rs`: an `Actuator` trait (`scale(tenant, replicas)` / `remove(tenant)`) +
`NoopActuator` (single-node) — **always available**; `KubernetesActuator` + the `kube`/`k8s-openapi` deps behind
the **`kubernetes` cargo feature** (default build compiles no kube). `scale` **server-side-applies** a per-tenant
`weir-runner-<tenant>` Deployment (image, `runner --tenant <id>`, `WEIR_DB`/`WEIR_CONNECTORS_DIR` env,
`weir/tenant` labels); `remove` deletes it (best-effort). `runner_deployment_json` is the one manifest source
(the actuator + `charts/weir-runner` [[WEIR-T-0105]] share it).

Builds + tests **with and without `--features kubernetes`** — the manifest deserializes to a real k8s
`Deployment` (kube 2.0 / k8s-openapi 0.26, mirroring cloacina); unit tests on name-sanitization + the manifest.
clippy clean. Live create/scale is the kind smoke ([[WEIR-T-0105]]). **Complete.**
