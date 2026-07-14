---
id: f1-1-executionmode-model-threading
level: task
title: "F1.1 — ExecutionMode model + threading (connection → work_spec → executor)"
short_code: "WEIR-T-0137"
created_at: 2026-07-09T15:34:25.007561+00:00
updated_at: 2026-07-09T16:25:42.377935+00:00
parent: WEIR-I-0035
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# F1.1 — ExecutionMode model + threading (connection → work_spec → executor)

## Parent Initiative

[[WEIR-I-0035]] (F1 — Long-lived source runtime)

## Objective

Introduce the **execution mode** dimension end-to-end so a connection can be declared *resident* vs *run-once*, and
thread it from the persisted model into the `WorkSpec` the executor sees. This is the foundational task — F1.2/F1.3
branch on it. No behavior change yet for run-once; resident is declarable but not yet executed.

## Acceptance Criteria

## Acceptance Criteria

- [ ] `Connection` (weir-app) gains an `execution_mode` field; persisted + round-trips through the store.
- [ ] `work_spec()` parses it into a `WorkSpec` `ExecutionMode` enum: `RunOnce` | `Resident { cadence_ms:
  Option<u64>, restart_backoff_ms, high_water hints optional }`.
- [ ] Connector `spec()` advertises a **resident capability** (can-be-resident / is-event-reader); `validate_connection_modes`
  rejects declaring resident on a connector that doesn't advertise it, and rejects incoherent mode combinations.
- [ ] Default is `RunOnce`; every existing connection/test continues unchanged (backward-compatible).
- [ ] CLI/API surface accepts and reports the mode.

## Implementation Notes

### Technical Approach
- Model + store: `weir-app/src/lib.rs:Connection` (~:70), `validate_connection_modes`; `weir-app/src/store.rs`.
- WorkSpec: `weir-app/src/lib.rs:work_spec` (~:1746) — add parse alongside `parse_sync_mode`/`parse_write_mode`.
- Enum: `weir-connector-types/src/types.rs` (`ExecutionMode`), consumed by `weir-orchestrator` `WorkSpec`.
- Capability: connector `spec()` in `weir-connector-types` / guest contract (mind the canonical guest-contract block
  / drift guard — regenerate via `angreal connectors sync-contract`).

### Dependencies
None — first task. F1.2/F1.3 depend on this.

### Risk Considerations
- Guest-contract change touches every connector's `weir_guest_types.rs`; use the canonical-block regen path, don't
  hand-edit. Keep the capability field optional/defaulted so existing guests still onboard.

## Status Updates

**2026-07-09 — COMPLETED (build-verified green).**
- `ExecutionMode` enum landed **host-side in `weir-orchestrator`** (internally-tagged, snake_case): `RunOnce`
  (`#[default]`) | `Resident { cadence_ms: Option<u64>, restart_backoff_ms: u64 }`. Threaded onto `WorkSpec`
  (`#[serde(default)]`), persisted through `work_units` as JSON. `Connection.execution_mode: String` (default
  `"run_once"`) persisted like `sync_mode`; `parse_execution_mode` + `work_spec` threading; `weir-api` DTO +
  `weir-cli` carry it.
- Schema: new `0003_resident_runtime` migration + `generated/schema.rs` update. **Caveat:** the `angreal schema gen`
  generator wasn't available in-sandbox, so the generated `schema.rs` + per-backend migrations were **hand-written
  to match**; a reviewer with the generator should re-run `angreal schema gen` to confirm byte-identical output.
  Postgres migration not runtime-verified (no PG in sandbox); **sqlite path is green** via unit tests.
- **Verify:** `cargo fmt --check` clean; `angreal check clippy` clean; `cargo build --workspace` green;
  lib tests green for `weir-connector-types` / `weir-orchestrator` / `weir-app` / `weir-schema` (re-run and
  confirmed by the orchestrator). One flaky full-suite failure (`weir-runtime` oauth loopback) is **unrelated**
  (crate untouched; passes in isolation). Spurious `weir_slow_wasm.wasm` rebuild churn reverted.
- **DEFERRED to F3 ([[WEIR-I-0037]]):** connector `spec()` capability fields (`resident_capable` / `event_reader`).
  `ConnectorSpec` is a bincode-positional `WitType`; adding fields is a breaking wire change best bundled with F3's
  contract amendment + guest-contract regen. **Consequently `validate_connection_modes` validates the mode string
  but does NOT yet reject `resident` on an incapable connector** — F3 closes that gate. Acceptance criterion #3 is
  therefore **partially** met by design; re-homed, not dropped.
- Per-connection cadence/backoff surfacing + a `--execution-mode` CLI flag deferred to F1.3/F1.4 (defaults hold).