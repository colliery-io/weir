---
id: helm-charts-weir-server-weir
level: task
title: "Helm charts (weir-server + weir-runner) + kind smoke"
short_code: "WEIR-T-0105"
created_at: 2026-07-06T10:43:17.636701+00:00
updated_at: 2026-07-06T11:12:28.097128+00:00
parent: WEIR-I-0021
blocked_by: [WEIR-T-0102]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0021
---

# Helm charts (weir-server + weir-runner) + kind smoke

## Parent Initiative

[[WEIR-I-0021]] — the proving gate. Closes the initiative.

## Objective

Ship weir on Kubernetes: `charts/weir-server` (control plane) + `charts/weir-runner` (the runner Deployment the
actuator/autoscaler manages), verified via `helm lint`/`template` + an optional kind smoke test.

## Reference

- cloacina `~/Desktop/cloacina/charts/{cloacina-server,cloacina-agent}` — the two-chart pattern; `cloacina-agent`
  = the runner analog (`deployment.yaml`, `secret.yaml`, `_helpers.tpl`).
- Runner image + `weir runner` args ([[WEIR-T-0102]]); the actuator's Deployment shape ([[WEIR-T-0103]]).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] `charts/weir-server`: the control-plane Deployment + Service, Postgres connection (SoR), the OIDC config,
  the SOPS/secret wiring, and **RBAC** (a ServiceAccount + Role allowing the actuator to manage runner
  Deployments in its namespace).
- [ ] `charts/weir-runner`: the runner Deployment template (image, `weir runner --tenant`, store URL + connectors),
  tenant-keyed labels/selectors matching the actuator ([[WEIR-T-0103]]) — the base the autoscaler scales.
- [ ] `helm lint` + `helm template` both charts pass (add to CI); values documented.
- [ ] A **documented kind/minikube smoke test** (script, not blocking CI): install the charts, plan work for a
  tenant → the autoscaler scales its runner up → work drains → scales to zero. Runbook in `charts/README`.
- [ ] Exit criteria of [[WEIR-I-0021]] all hold; workspace + clippy clean.

## Status Updates

### 2026-07-06 — done (`ee9722a`) — closes I-0021

- **`charts/weir-server`**: control-plane Deployment (`weir api`, `/health` probes) + Service + Secret
  (`WEIR_DB`, OIDC secret) + ServiceAccount + **RBAC** (a Role granting `apps/deployments` so the actuator
  manages runners in-namespace).
- **`charts/weir-runner`**: the per-tenant Deployment (`weir-runner-<tenant>`, `app`/`weir/tenant` labels,
  `runner --tenant <id>` args, `WEIR_DB`/`WEIR_CONNECTORS_DIR` env) — **byte-for-byte the actuator's
  `runner_deployment_json`** ([[WEIR-T-0103]]), so the server-side-apply lands on the same object.
- **Connective wiring**: `App::run_autoscaler` + a `weir autoscaler` command (behind the `kubernetes` feature
  chain: weir-cli → weir-app → weir-orchestrator; bails cleanly otherwise) run the leader-elected `Autoscaler`
  with the `KubernetesActuator`. Deferring to HPA/KEDA = don't run it (`autoscaler.enabled=false`).
- **Verified**: `helm lint` + `helm template` (incl OIDC) both charts clean → a `charts.yml` CI workflow. Builds
  with + without `--features kubernetes`; clippy clean. `scripts/kind-smoke.sh` + `charts/README` document the
  live scale-up→drain→scale-to-zero (needs a cluster; not CI). (pre-commit `check-yaml` now excludes the Go-
  templated chart templates.) **Complete — closes [[WEIR-I-0021]].**
