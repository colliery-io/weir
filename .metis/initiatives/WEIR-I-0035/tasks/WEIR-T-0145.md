---
id: f1-9-unified-configurable-resident
level: task
title: "F1.9 — Unified configurable resident connector (mode: poll|tail|ws) + correct warm-poll semantics"
short_code: "WEIR-T-0145"
created_at: 2026-07-10T17:10:55.228097+00:00
updated_at: 2026-07-10T17:11:37.476843+00:00
parent: WEIR-I-0035
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# F1.9 — Unified configurable resident connector + correct warm-poll semantics

## Parent Initiative

[[WEIR-I-0035]] (F1). **Corrects the resident *polling* model** exposed by the UI demo (the F1.6 `resident-poll`
fixture was an *infinite per-item stream*, which paces per-row and never reads-to-end — not real polling), and
consolidates the resident fixtures into one config-driven connector.

## Design (decided with the human, 2026-07-10)

Resident polling **stays** — it's the read-amplification clamp (one warm source runs the expensive read; many
consumers read the result). Going through the **scheduler** per cycle at high frequency is wasteful, so a resident
poll **keeps the instance warm and sleeps between polls**. Model:

- **Poll floor ~20ms (50Hz).** Faster than that → *"use triggers"* (event-reader), not polling.
- **One configurable connector**, `mode: poll | tail | ws` (yaml/json config), NOT three crates.
  - `poll`: warm loop — **read-to-end (bounded batch) → Checkpoint → sleep(cadence) → repeat**; cadence from config
    (≥ ~20ms); never exits (resident). *(Fixes the infinite-per-item bug.)*
  - `tail`: **emit-on-arrival** (block/wait for the next event) — the "while" shape (logs/changefeeds).
  - `ws`: websocket pass-through (F1.7 hand-rolled RFC6455 over brokered TCP) folded in under config.

## Acceptance Criteria

## Acceptance Criteria

- [ ] One resident connector fixture reads `mode` (+ per-mode config: `cadence_ms` for poll, upstream URL for ws,
  source for tail) and behaves per mode. Replaces `resident-poll` / `resident-events` / `resident-ws`.
- [ ] **poll**: a test asserts **bounded read-to-end per cycle** at the configured cadence — e.g. at 20ms over
  ~200ms, ~10 *polls* each emitting a bounded batch (NOT an unbounded row stream); sleep is **between polls**, not
  per row; instance stays warm (no re-schedule per cycle).
- [ ] **tail**: emits per arrival (K arrivals → K), idle → nothing (not clock-driven).
- [ ] **ws**: frame-arrival emission + supervised reconnect (carry over F1.7's tests).
- [ ] Cadence is wired from connection config → the poll loop (fix the F1.6 gap where `--every` didn't reach the
  resident cadence); enforce/clamp a sane floor (~20ms).
- [ ] **Demo reseed**: the UI `resident-demo` tile uses `mode: poll`, cadence 2s → row count advances by a bounded
  batch every 2s (a real poll), not a continuous stream.
- [ ] F1.6 Goal-4 poll claim + test updated to the real semantics; event/ws claims stand.

## Implementation Notes

- New/renamed fixture (e.g. `wasm-fixtures/resident/`) with a `mode` config; retire the 3 separate fixtures (keep
  their tests, re-pointed).
- **Poll sleep mechanism**: prefer guest-side wait (`wasi:clocks` monotonic) so the loop is `read→checkpoint→sleep`
  in the guest; if the guest can't sleep cleanly, host-pace **per-Checkpoint** (poll boundary) in `Engine::drive`
  (NOT per-Records as F1.6 did). Cadence value threaded from config.
- Update `.angreal/task_ui.py` demo seed + `crates/weir-engine/tests/wasm_resident_*` tests.
- Consider a short ADR ("resident modes + 20ms poll floor") if we want it formalized — optional.

### Verification
- `angreal check all`, `cargo build --workspace`, the new/updated resident tests, `angreal test connectors`,
  `angreal ui build`; a demo smoke showing the poll tile advancing by a batch every 2s. Paste real output.

## Status Updates

**2026-07-10 — COMPLETE (build + tests + live demo verified).** Fixes the F1.6 poll mis-model + consolidates.
- **NEW `wasm-fixtures/resident/`** (`weir-resident-pkg`, 247 KB wasm) — one connector, `mode: poll|tail|ws`;
  **removed** `resident-poll`/`resident-events`/`resident-ws`.
- **poll** = bounded read-to-end per cycle (`rows_per_poll` batch + Checkpoint), infinite cycles, warm, never
  exits. The cadence `thread::sleep` **moved from the per-`Records` arm to the per-`Checkpoint` arm** in
  `Engine::drive` → sleep is **between polls**, not per row. `parse_execution_mode` clamps `cadence_ms` to a **20ms
  floor** (faster → tail/triggers). **tail** = emit-per-arrival; **ws** = F1.7 RFC6455-over-brokered-TCP folded in.
- **Tests (my own run):** `resident_poll_reads_bounded_batch_per_cadence_cycle` ✅ (rows == polls×batch AND ~cadence-
  paced count, e.g. ~12 polls/250ms@20ms — NOT thousands), `resident_tail_emits_per_arrival` ✅ (3→3, 0→0),
  ws 3/3 ✅. Build green; orchestrator lib 8 / scheduler 8. fmt/clippy clean.
- **Demo reseeded + relaunched:** `resident-demo` = `resident` `{mode:poll, rows_per_poll:3}` `--every 2.0` → tile
  advances by a **bounded 3-row batch every ~2s** (verified live: leased, rows 24 = 8 polls), not a stream.

**Corrects F1.6 Goal-4:** the "polling emits on cadence" claim is now genuinely met (real read-to-end-per-cycle),
not host-pacing of an infinite stream. **Design recorded:** resident poll stays (read-amp clamp — warm + sleep, no
per-cycle scheduler overhead); **20ms floor**, faster → event triggers; **one configurable connector**
(poll|tail|ws).

**Gaps:** `tail` still a config-supplied arrival burst (real blocking log-tail only via `ws`); 20ms floor enforced
at config-parse, not inside `run_resident` (a direct caller can pass less); `angreal test connectors` full rebuild
not run (fixture wasm-build verified directly). Not committed.