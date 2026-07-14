---
id: f1-follow-up-multi-runner-soak
level: task
title: "F1 follow-up: multi-runner soak — batch+resident co-existence + scale-out"
short_code: "WEIR-T-0152"
created_at: 2026-07-14T02:24:33.732458+00:00
updated_at: 2026-07-14T02:37:56.180727+00:00
parent: WEIR-I-0035
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# F1 follow-up: multi-runner co-existence

## Parent Initiative

[[WEIR-I-0035]] (F1). Followup identified during development. The single-node soak showed batch work starves behind
residents on ONE worker; the point here is that the **lease/claim model** lets MULTIPLE runners co-claim from one
store so batch + residents co-exist safely — which is what a real multi-runner fleet relies on.

## Objective

Prove, in-process, that ≥2 concurrent runners sharing one store co-exist: run-once units all drain, each executed
**exactly once** (atomic claim → no double-run), while a perpetual resident runs on one runner.

## What was added

`weir-orchestrator/tests/scheduler.rs::two_runners_share_one_store_no_double_run` — two `Worker`s (distinct owners
`runner-1`/`runner-2`) drive ONE shared store concurrently (`tokio::join!`), with 1 perpetual resident + 6 run-once.
A recording executor logs every `execute(id)`; asserts: all 6 run-once reach `done`; **each ran exactly once**
across both runners (the atomic state-guarded claim prevents double-run); the resident stays active (its heartbeat
holds the lease so the other runner never reclaims it).

## Scope / honesty

- This proves the **claim/lease safety mechanism** that underpins a multi-runner fleet.
- **TRUE multi-node** (k8s pods + the autoscaler/actuator scaling real replicas) is beyond an in-process test and is
  not exercised here; that lives with deployment (ADR-0023/0036) and F4 (fan-out at scale). A `weir-soak --runners N`
  flag was intentionally NOT added: the soak is an HTTP client against ONE server, so it can't spawn real runner
  processes — multiple `weir runner` processes / pods are the real path. Documented rather than faked.

## Acceptance Criteria

- [x] ≥2 concurrent runners on one shared store; run-once all drain to `done`.
- [x] No double-run — each unit executes exactly once across the runners (asserted).
- [x] A perpetual resident co-runs (stays active) throughout.
- [x] Honest note that true multi-node scaling is out of in-process scope (→ F4 / deployment).

## Status Updates

**2026-07-14 — COMPLETE.** Added `two_runners_share_one_store_no_double_run`. In-process multi-runner claim-sharing
proven; true multi-node deferred to deployment/F4 (documented, not faked). Not committed.
