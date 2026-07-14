---
id: close-the-partials-production
level: initiative
title: "Close the partials — production-fidelity hardening"
short_code: "WEIR-I-0022"
created_at: 2026-07-06T11:32:09.423473+00:00
updated_at: 2026-07-07T00:21:19.402003+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: close-the-partials-production
---

# Close the partials — production-fidelity hardening Initiative

## Context

The platform layer is complete (auth, tenancy, observability, k8s — [[WEIR-I-0017]]…[[WEIR-I-0021]]). A
capability audit (2026-07-06) surfaced **4 "partial" capabilities** — things that exist but aren't proven at
depth. This initiative closes them, moving each **partial → have**.

## Goals (the 4 partials → have)

1. **HA control plane** — `App::serve` loops `scheduler.tick()` on *every* replica (double-scheduling if HA).
   `Relay::try_acquire_lease` already exists ([[WEIR-T-0104]]) → **leader-elect the scheduler** so only the
   leader schedules. Small + concrete.
2. **Incremental / CDC fidelity** — `SyncMode::{FullRefresh, Incremental, Cdc}` + the Postgres connector exist,
   but correctness-at-scale is unproven. Build a **fidelity harness**: resume/cursor advance, **no dupes or
   gaps**, CDC ordering, partition checkpoints, crash-resume.
3. **Transforms / mapping** — `weir-engine/mapping.rs` exists; audit + **depth-test** the transform primitives,
   fill gaps, document the supported set.
4. **Connector breadth** — breadth is never *finished*; close the *partial* by **hardening the authoring path**
   (scaffold + docs + tests) and standing up **one new-category connector — a file/object-store source** (which
   also closes the file/object-store **gap**).

## Non-Goals

- Warehouse connectors, non-Postgres DB sources, bundled dashboards (the remaining *gaps* — separate work).
- A live k8s-cluster run (the other gap — [[WEIR-I-0021]] follow-up).

## Design decisions (2026-07-06, approved)

- **One initiative, 4 tasks.**
- **T-d (breadth)**: prove the authoring path + ship a **file/object-store source** exemplar (two birds: closes
  the breadth partial *and* the file gap).
- No new ADR — the work sits under existing decisions ([[WEIR-A-0010]] work distribution, [[WEIR-A-0028]]
  in-process default, [[WEIR-A-0036]] tenancy, [[WEIR-A-0030]] WASM-always).

## Proposed decomposition (for sign-off)

- **T-a — Scheduler leader-election (HA):** guard `serve`'s `sync_schedules`/`scheduler.tick()` with
  `Relay::try_acquire_lease("scheduler", …)`; only the leader schedules. Test: two servers on one store schedule
  exactly once. Closes **HA control plane**.
- **T-b — Incremental + CDC fidelity harness:** a rigorous engine/orchestrator test suite — incremental cursor
  advance + resume, no dupes/gaps across a restart, CDC event ordering, partition checkpoint isolation. Closes
  **incremental fidelity**.
- **T-c — Transform/mapping depth:** audit `mapping.rs` + the mapping primitives; depth tests (rename/cast/
  nested/drop/const); document the supported set; fill the obvious gaps. Closes **transforms/mapping**.
- **T-d — Connector-authoring + file source:** an `angreal`/scaffold path + docs for authoring a connector, and
  a **file/object-store source** connector (local dir / S3-compatible) proving a non-REST/non-DB category end to
  end (onboard → run → rows). Closes **connector breadth** (partial) + the **file gap**.

## Exit Criteria (draft — refine in design)

- [ ] Two control-plane replicas on one store schedule each connection exactly once (leader-elected scheduler).
- [ ] A fidelity harness proves incremental + CDC correctness (resume, no dupes/gaps, ordering) — green in CI.
- [ ] The transform/mapping supported set is tested at depth + documented.
- [ ] A file/object-store source onboards + runs end-to-end; the authoring path is scaffolded + documented.
- [ ] The capability matrix's 4 partials read **have**; workspace + clippy + e2e green.
