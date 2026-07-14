---
id: soak-loop-invariants-report
level: task
title: "Soak loop + invariants + report"
short_code: "WEIR-T-0123"
created_at: 2026-07-08T00:08:06.765090+00:00
updated_at: 2026-07-08T00:37:40.988604+00:00
parent: WEIR-I-0023
blocked_by: [WEIR-T-0122]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0023
---

# Soak loop + invariants + report

## Parent Initiative

[[WEIR-I-0023]] — the soak actually *gates*.

## Objective

`weir-soak` runs for a bounded `--duration`, polling the ops API each window, and **asserts health invariants**;
a final summary + a **non-zero exit** on any breach.

## Reference

- The provisioned fleet from [[WEIR-T-0122]]; the ops API ([[WEIR-I-0024]]): `GET /overview`, `/runs`,
  `/connections`, `/connections/{name}/dead-letters` — the observation surface (dogfood it).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `--duration`/`--window` poll loop over `/runs`, folding each window into a `Sample` (cumulative
  completions, in-flight depth, cumulative gated dead-letters, api-ok).
- [x] Four invariants in a pure `evaluate`: liveness + bounded-queue every window; throughput + bounded-DL past
  `--warmup`; live-REST (`soak-rest-*`) excluded from the DL gate via `is_gated`.
- [x] End summary (windows / completions / max in-flight / gated DLs / per-invariant PASS·FAIL) + `exit(1)` on
  any breach.
- [x] Thresholds + window/duration/warmup are flags; unit tests (healthy series + each breach kind + gate
  split). Workspace + clippy clean.

## Status Updates

### 2026-07-08 — done (`6080a3c`)

Soak loop after provisioning: poll `/runs` per window → `Sample` series → pure `evaluate(samples, thresholds)`
→ breaches. Live-REST excluded from the DL gate. Summary + non-zero exit on breach. Note: queue depth is a
proxy from the recent-50 `/runs` window (an unbounded backlog saturates it) — a dedicated pending-count endpoint
could sharpen it later. 4 unit tests green; workspace + clippy clean. **Complete.** Next: [[WEIR-T-0124]] wraps
it in `angreal soak` + a CI smoke.
