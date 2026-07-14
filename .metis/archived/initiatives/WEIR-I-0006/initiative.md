---
id: streaming-configured-instance
level: initiative
title: "Streaming + configured-instance connector contract (v1)"
short_code: "WEIR-I-0006"
created_at: 2026-06-21T00:43:14.061975+00:00
updated_at: 2026-06-21T20:49:29.719471+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: streaming-configured-instance
---

# Streaming + configured-instance connector contract (v1) Initiative

> **GATED — do not decompose/activate yet.** Implements the **ratified** [[WEIR-A-0029]] (decided 2026-06-20). Blocked on: (1) fidius 0.5.0 on crates.io, (2) the weir 0.4.2 → 0.5.0 bump + breaking-recompile verification ([[WEIR-I-0005]]). Decomposition + activation need human sign-off once those clear.

## Context **[REQUIRED]**

[[WEIR-I-0004]] made the engine **honor** the contract dimensions the types already declared (write modes, incremental, partitioned, CDC). This initiative **evolves the contract itself** per ratified [[WEIR-A-0029]]: chunked-pull / config-per-call (**v0**) → **streaming + configured instances (v1)**, now that fidius 0.5.0 removes the request/response constraint that forced the chunked-pull workaround ([[WEIR-S-0006]]).

## Goals & Non-Goals **[REQUIRED]**

**Goals (per [[WEIR-A-0029]]):**
- **Streaming `read`** → `read(ReadContext) -> Stream<ReadMessage>` (`Records | Checkpoint | Log | DeadLetter`); connector yields records + inline checkpoints; engine commits on `Checkpoint` transactionally ([[WEIR-A-0011]]); real backpressure + drop-to-cancel.
- **Configured instances** — bind typed config once at construction (`configure(Config) -> Self`); `ReadContext`/`WriteContext` drop per-call `config`; `read`/`write` run on the configured instance (host `load_wasm_configured` / `configure_in_process`).
- **Streaming `write`** (client-streaming) — destination consumes `Stream<RecordBatch>`. *(Phasable: read-first acceptable; symmetric is the target.)*
- Re-express partitioned reads (one configured stream per `Partition`) and CDC (a live change stream) over the new carrier; **conformance tests** per dimension over `weir-connector-postgres`.

**Non-Goals:**
- Re-litigating the *shape* — that's decided in [[WEIR-A-0029]].
- New connector breadth (HubSpot/Salesforce) — later initiative.
- The fidius 0.5.0 bump itself + [[WEIR-T-0023]] — tracked in [[WEIR-I-0005]] (this initiative's gate).

## Detailed Design **[REQUIRED — see WEIR-A-0029]**

The decision + alternatives + rationale live in [[WEIR-A-0029]]. Implementation touches: the contract crate (`weir-connector-types`/`weir-connector`), `weir-codegen` (emit streaming `read` over `fidius_guest::Stream`, drop the `has_more` loop), `weir-engine` (stream drive loop: pull → map → write → commit on `Checkpoint`), `weir-runtime` (configured-instance load + `call_streaming` seam), and **every connector** (echo/faulty/arrow-sink/postgres) rewritten to v1. Bumps contract version v0 → v1 (replace, not dual-path — pre-1.0, unshipped).

## Implementation Plan **[DRAFT — decompose after gates clear + sign-off]**

Proposed slice order (mirrors I-0004's "interface, then honor it"):
1. **Contract v1 types** — `ReadMessage` stream, configured-instance lifecycle, config off the contexts; bump contract version.
2. **Runtime seam** — configured-instance load (`load_wasm_configured` / `configure_in_process`) + `call_streaming` in `weir-runtime`.
3. **Engine stream loop** — pull `Stream<ReadMessage>`, map + write per batch, commit on `Checkpoint`; backpressure + cancel.
4. **Codegen** — emit streaming `read` + configured construction.
5. **Connectors** — port echo/faulty/arrow-sink + `weir-connector-postgres` (incl. CDC as a live stream); conformance per dimension.
6. *(phasable)* **Streaming `write`** — client-streaming destination.

## Exit Criteria **[DRAFT]**

- [ ] Contract is v1: `read -> Stream<ReadMessage>`, configured instances, config off `Read/WriteContext`.
- [ ] Engine drives the stream with transactional checkpoint-on-`Checkpoint`; resume-from-last-checkpoint on cancel/crash holds ([[WEIR-A-0011]]).
- [ ] All connectors ported; partitioned reads + CDC re-expressed over the stream; conformance green (incl. live Postgres).
- [ ] `weir-codegen` emits v1; [[WEIR-S-0006]]/[[WEIR-A-0014]] updated to reflect the ratified v1 shape.
