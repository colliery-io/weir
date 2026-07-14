---
id: executor-wiring-dylib-wasm
level: task
title: "Executor wiring: dylib + WASM primitives behind the seam"
short_code: "WEIR-T-0002"
created_at: 2026-06-17T21:25:13.010405+00:00
updated_at: 2026-06-18T13:11:04.595163+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Executor wiring: dylib + WASM primitives behind the seam

## Parent Initiative
[[WEIR-I-0002]]

## Objective **[REQUIRED]**

Host both `fidius` execution primitives — native **dylib** (trusted) and **WASM** (open) — behind the executor seam, so the Runtime ([[WEIR-S-0005]]) can load and invoke a `Connector` ([[WEIR-T-0001]]) without callers knowing which primitive runs it. Proves [[WEIR-A-0002]].

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [x] Runtime loads a native (in-process) connector **and** a WASM-component connector against the same `Connector` interface (`ConnectorHandle::native_in_process` + `::from_wasm_package`). *(Loading is proven; the dylib-**file** path + signature gating below are the remaining hardening.)*
- [x] A connector invoked over **both** primitives returns **identical** results through the seam — `tests/wasm_seam.rs` asserts native `echo` == WASM `echo` for spec/discover/read/write. **This is the WEIR-A-0002 proof.**
- [x] WASM path uses the sandboxed wasmtime/component host with capability-gated (deny-all) WASI imports ([[WEIR-A-0002]] Path 1) — via `PluginHost::load_wasm` + `fidius-host` `wasm` feature.
- [x] Signature verification (ed25519 via `fidius`) gates load ([[WEIR-A-0015]]) — `ConnectorHandle::from_wasm_package_signed` (`require_signature` + trusted Ed25519 keys). `tests/wasm_signing.rs`: signed+trusted loads; unsigned is rejected. (Native is in-process/linked; signing applies to the WASM-package + dylib-file paths.)

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
Leans on `fidius`'s `PluginExecutor`-style backend abstraction (the WASM branch shipped). Credentials injected via the host-controlled import surface for WASM ([[WEIR-A-0013]]).

### Dependencies
[[WEIR-T-0001]] (contract crate). `fidius` dylib + WASM branches.

## Status Updates **[REQUIRED]**

- **2026-06-17 — native half DONE.** `crates/weir-runtime` `ConnectorHandle` seam implemented + `crates/weir-connector-echo` reference connector; in-process seam test green (spec/check/discover typed + read/write raw envelopes, capability-bit role detection). Method indices 0–4 and optional-capability bits 0–2 confirmed correct.
- **White-label pattern adopted (refines WEIR-T-0001).** Per fidius `docs/how-to/white-label-interface.md`: `weir-connector` now does `pub use fidius;` + `pub use fidius::{plugin_impl, PluginError};` and declares the interface with **`crate = "crate::fidius"`** (the doc's `crate = "crate"` is wrong for 0.3.0 — the macro emits `<crate_path>::descriptor`, so the path must point at fidius's modules). Connectors author against **`weir-connector` only, no direct `fidius` dep** (`#[plugin_impl(Connector, crate = "weir_connector::fidius")]` + `weir_connector::fidius::fidius_plugin_registry!()`). Great for the ecosystem/moat (fidius is hidden).
- Gotcha: `#[optional]` is interface-only; on the impl, methods are plain (only `#[wire(raw)]` is repeated).
- **WASM half — findings + plan (2026-06-17).** Got the full fidius wasm recipe: a guest uses `fidius-guest` + `fidius-macro` + `wit-bindgen` (crate-type cdylib), re-declares the interface with `crate = "fidius_guest"` (same method signatures → same `interface_hash` → host-compatible), builds with `cargo build --target wasm32-wasip2`; typed-method types need `#[derive(WitType)]` (raw read/write don't — they're `list<u8>`); host loads via `WasmComponentExecutor::from_component_bytes(bytes, "fidius:connector/connector@0.1.0", methods, caps, info)` or `PluginHost::load_wasm(pkg, &WASM_DESCRIPTOR)`; signing is package-level (ed25519 over `package_digest`).
  - **Blocker found:** `weir-connector` **does not build for wasm32** — depends on `fidius` (host facade), which pulls `bzip2-sys`/`tar` (C, no wasm stdlib). So the guest cannot share `weir-connector` directly.
  - **Required refactor (the WASM-unblock):** split boundary types + error + envelope codec into a new **`weir-connector-types`** crate with **no `fidius` dep** (serde + thiserror + bincode only; `fidius_macro::WitType` derives gated behind a `wit` feature). `weir-connector` (host/dylib interface) depends on it + `fidius`; the **wasm guest** depends on `weir-connector-types` + `fidius-guest` and re-declares the `Connector` interface with `crate = "fidius_guest"`.
  - **Then:** WitType-reshape any types WitType rejects (likely `ConnectorError.context` map → `Vec<(String,String)>`; check newtypes `Config`/`ArrowSchemaIpc`), build echo→wasm, add `Backend::Wasm(WasmComponentExecutor)` to `ConnectorHandle`, assert identical results to native, then signature-gated load on both paths. → WEIR-T-0002 completed.
  - **2026-06-17 — split DONE.** `weir-connector-types` crate created (serde + thiserror + bincode; optional `wit` feature for `fidius_macro::WitType`); types/error/codec/DiscoverOutcome moved there; `weir-connector` now re-exports them + keeps the fidius interface. Native workspace green; **`weir-connector-types` builds clean for `wasm32-wasip2`**.
  - **2026-06-18 — WitType-ready DONE.** Added `#[cfg_attr(feature = "wit", derive(fidius_macro::WitType))]` to the typed-method types (ConnectorSpec/Config/CheckResult/Catalog/Stream/ArrowSchemaIpc/PartitionScheme/SyncMode/ConnectorRole/DiscoverOutcome/ConnectorError/ErrorKind). One reshape: `ConnectorError.context` `BTreeMap` → `Vec<ErrorContext>` (maps don't project to WIT). **`cargo build -p weir-connector-types --target wasm32-wasip2 --features wit` compiles** — the full contract is WIT-projectable (newtypes + struct-variant enums all OK). Native still green.
  - **ARCHITECTURE FINDING (gates the guest):** `fidius_build::emit_wit()` parses **only the guest crate's own `src/`** for `#[derive(WitType)]` types — it **cannot** read them from a dependency. So a WASM connector's interface + WitType types must live in its **own source**; it can't `use` them from `weir-connector-types`. This is exactly why [[WEIR-A-0014]] chose **codegen**: the WASM guest (interface + types + impl) is *generated per-connector* from the contract (same signatures ⇒ same `interface_hash`; same struct shapes ⇒ bincode-compatible raw envelopes). `weir-connector-types` remains the source-of-truth for host + native/dylib; WASM guests get a generated self-contained copy.
  - **Remaining for T-0002 WASM proof (now coupled to codegen / a self-contained guest):** author a self-contained WASM echo guest (interface + needed types + impl, `crate = "fidius_guest"`, `build.rs` `emit_wit`), `cargo build --target wasm32-wasip2`; add `Backend::Wasm(WasmComponentExecutor)` to `ConnectorHandle` (typed via `ValueExecutor::call` + `to_value`/`from_value`; raw via `call_raw`); assert identical results to native; signature-gated load. **Recommendation:** do this as part of / right before [[WEIR-T-0003]] (the codegen task), since codegen *is* the guest-authoring mechanism.
  - **2026-06-18 — WASM PROOF DONE (WEIR-A-0002 demonstrated).** Built `wasm-fixtures/echo` (self-contained guest) to a validated WASM component. More WIT-keyword reshapes needed beyond `record`: renamed `Stream`→`StreamInfo` (`stream` reserved), `ErrorContext`→`ContextPair` (`error-context` is a component-model built-in), `ErrorKind::Record`→`RecordLevel`, `PartitionScheme::None`→`Unpartitioned`, and `Config`/`ArrowSchemaIpc` to **named-field** structs (WitType rejects tuple structs). Host wiring turned out trivial: `PluginHost::load_wasm` returns a normal `PluginHandle`, so the seam needed only a second constructor (`from_wasm_package`) — no manual `WasmComponentExecutor`/Value plumbing. Needed `fidius-host` `wasm` feature (facade doesn't forward it). Capability bits are cdylib-only → role detection now reads `spec().roles` (backend-agnostic). `tests/wasm_seam.rs`: native `echo` and WASM `echo` return **identical** spec/discover/read/write. **Only signature-gating remains** (criterion 4) — a small follow-up.
