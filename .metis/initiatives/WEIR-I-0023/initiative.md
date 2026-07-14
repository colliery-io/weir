---
id: soak-test-provisioned-stack-under
level: initiative
title: "Soak test — provisioned stack under sustained connector load"
short_code: "WEIR-I-0023"
created_at: 2026-07-07T04:00:24.038342+00:00
updated_at: 2026-07-08T01:55:30.554151+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: soak-test-provisioned-stack-under
---

# Soak test — provisioned stack under sustained connector load

## Context

The k8s deploy path is code + unit + helm-verified but never *run under load* — the only "live-cluster" proof
is a documented kind smoke ([[WEIR-T-0105]]). Reframe that gap honestly as a **soak test**: stand the service
stack up, provision realistic state, wire up **many connectors**, and let it run continuously — surfacing leaks,
lease churn, scheduler/queue behaviour, and drift that a single smoke run never would.

## Goal

An **`angreal` task** (e.g. `angreal soak`) that:
1. **Spins up the service stack** (compose today; the same shape ports to kind/k8s later).
2. Runs a **setup binary** that provisions what it can via the API/CLI — tenants, keys, connections across the
   available connector types (REST manifests, postgres, s3), schedules.
3. **Configures a fleet of connectors** running on a tight cadence, then lets the stack **soak** — asserting
   health invariants over time (runs keep completing, queue depth stays bounded, no dead-letter blowup, memory/
   lease stability), with a summary at the end.

## Non-Goals

- A full k8s/kind run (compose first; the setup binary + invariants are the reusable part).
- Perf benchmarking / SLO numbers (this is stability-under-load, not a benchmark).

### Design decisions (2026-07-07, approved)

- **Driver**: a dedicated **`weir-soak` binary** (new workspace crate) that provisions + drives + asserts
  entirely over the **HTTP API** against a `--base-url` — so it points at compose today and kind/k8s later
  (the reusable shape). Not a `weir-cli` subcommand (keeps the load/ops harness out of the no-code CLI).
- **Load mix**: **local fleet + postgres + live REST corpus**. Bulk deterministic volume from echo/slow →
  arrow-sink/echo on a tight cadence; a couple of postgres source/dest connections against the compose Postgres
  (real DB + CDC); plus the no-auth **live REST manifest corpus** ([[WEIR-I-0014]]) for connector variety.
  Because the live-REST endpoints are external, invariants must tolerate **per-connector** external flakiness
  (a single flaky external connector must not fail the soak — see invariants).
- **Run + report**: **bounded, configurable duration** (default ~90s; override for long soaks). Poll the ops
  API each window (`/overview`, `/runs`, `/connections`) asserting invariants; a final summary + **non-zero
  exit** on any breach.
  - **Invariants**: (1) *throughput* — the local/postgres fleet keeps completing runs each window (system-wide
    forward progress, not per-connector); (2) *bounded queue* — `work_units` pending doesn't grow without
    bound; (3) *dead-letter rate bounded* — the aggregate DL rate stays under a threshold, and external-REST
    connectors are excluded from a hard DL gate (their flakiness is expected); (4) *liveness* — the API stays
    responsive for the whole run.
- Reuses the ops API ([[WEIR-I-0024]]) as the observation surface (dogfoods it). No new ADR.

## Proposed decomposition (for sign-off)

- **T-a — `weir-soak` provisioner:** the new bin + its API client; provision an admin key, tenant(s), and a
  **fleet** of connections across connector types (echo/slow local, postgres source+dest, the live REST corpus)
  each on a tight `every_secs` schedule.
- **T-b — soak loop + invariants + report:** run for `--duration`, poll the ops API each window, assert the four
  invariants (throughput / bounded queue / bounded DL / liveness, external-REST excluded from the DL gate),
  print a summary + exit non-zero on breach.
- **T-c — `angreal soak` + docs + CI smoke:** an `angreal soak` task that brings the compose stack up, runs
  `weir-soak`, and tears it down; a short-duration smoke wired for CI; docs + the reusable kind/k8s note.

## Exit Criteria (draft)

- [ ] `angreal soak` brings up the stack, provisions tenants/connections/connectors, and soaks for a bounded run.
- [ ] Health invariants asserted over time; a clear pass/fail + summary.
- [ ] Documented; reusable shape for a later kind/k8s soak.
