---
id: connector-contract-crate-interface
level: task
title: "Connector contract crate (interface + boundary types)"
short_code: "WEIR-T-0001"
created_at: 2026-06-17T21:25:11.299325+00:00
updated_at: 2026-06-17T22:24:48.779270+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Connector contract crate (interface + boundary types)

## Parent Initiative
[[WEIR-I-0002]]

## Objective **[REQUIRED]**

Implement the connector contract v0 ([[WEIR-A-0014]] / [[WEIR-S-0006]]) as a `fidius` `#[plugin_interface]` plus its boundary types, in a dedicated crate. This is the foundation every other Slice-1 task builds on.

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [x] `Connector` interface defined as a `fidius` `#[plugin_interface]`: `spec`, `check`, `discover` (typed) + `read`, `write` (`#[wire(raw)]`); roles `Source`/`Destination`/`ReverseEtl` via `#[optional]` capability bits.
- [x] Boundary types implemented: `ConnectorSpec`, `Catalog`/`Stream` (Arrow-canonical schema), `PartitionScheme`, `ConfiguredCatalog`/`ConfiguredStream`, `WriteMode`, `ReadContext`/`ReadChunk`, `WriteContext`/`WriteRequest`/`WriteAck`, `RecordBatch` (`Rows` | `Arrow(ipc)`), `StreamState`, `LogEntry`, `ConnectorError` (Config/Transient/Record/Fatal). (Chunked-pull model replaced the streamed `Message` enum — see Status.)
- [x] `interface_hash`/`interface_version` wired via fidius (`version = 1`; fidius derives the hash — feeds [[WEIR-A-0019]]).
- [x] Compiles against fidius 0.3.0; 3 unit tests green (typed JSON round-trip + `read`/`write` bincode-envelope round-trips, incl. Arrow IPC byte payloads). Whole workspace builds + tests clean.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
Arrow type system is canonical ([[WEIR-S-0006]]); JSON `Rows` conform to the Arrow schema. Records cross as Arrow IPC bytes in v0.

### Dependencies
`fidius` (dependency, [[WEIR-A-0027]]); the Arrow crate. No other Slice-1 task; this is first.

## Status Updates **[REQUIRED]**

- **2026-06-17 — fidius 0.3.0 API grounding (design change).** fidius is **request/response, not streaming**. Revised the contract data path from "MessageStream" to a **chunked pull model**: `read(ReadContext) -> ReadChunk { batch, next_state, has_more }` called repeatedly (Engine commits `next_state` per chunk); `write(WriteContext, batch) -> WriteAck { state }` per batch. `read`/`write` use `#[wire(raw)]` (bincode envelope w/ Arrow IPC bytes inside) — required because WASM typed `Value` marshalling would explode a byte buffer into per-element `list<u8>`; `#[wire(raw)]` does a bulk memcpy. `spec`/`check`/`discover` stay typed. Capability gating via fidius `#[optional]` bits. Updated [[WEIR-S-0006]] accordingly.
- Imports/paths confirmed: interface via `fidius::{plugin_interface, plugin_impl}`; guest/WASM crate uses `crate = "fidius_guest"`; `#[derive(WitType)]` for boundary structs so they project to WIT; build via `fidius_build::emit_wit()` + `cargo component build --target wasm32-wasip2`.
- **2026-06-17 — DONE.** Crate `crates/weir-connector` implemented (`error.rs`, `types.rs`, `lib.rs`) on branch `feat/weir-t-0001-connector-contract`; builds + 3 tests green; workspace clean. **Two bincode lessons (bincode is fidius's typed wire):** (1) `serde_json::Value` can't cross bincode (`deserialize_any` unsupported) → free-form JSON carried as **JSON text (`String`)**, which is also WIT-`string`-friendly; (2) `#[serde(skip_serializing_if)]`/`default` desync bincode's positional stream → **no field-level serde attrs**; every field always serializes. **Open follow-ups for [[WEIR-T-0002]]/[[WEIR-T-0003]]:** schema/records are opaque `ArrowSchemaIpc`/`ArrowIpc` bytes (the `arrow` crate enters with the real connectors); the interface-definition `crate=` path (`fidius` host/dylib vs `fidius_guest` for WASM) needs reconciling (likely a feature flag) when the WASM guest is built; `#[derive(WitType)]` on boundary types is needed for the WASM typed methods (`spec`/`check`/`discover`).
