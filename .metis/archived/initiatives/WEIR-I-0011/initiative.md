---
id: wasm-always-connector-migration
level: initiative
title: "WASM-always connector migration"
short_code: "WEIR-I-0011"
created_at: 2026-06-22T20:00:02.420933+00:00
updated_at: 2026-06-23T22:16:59.610722+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: wasm-always-connector-migration
---

# WASM-always connector migration Initiative

Implements [[WEIR-A-0030]] (WASM-always). The prerequisite the catalog ([[WEIR-I-0010]]), parity
([[WEIR-I-0008]]), and the developer-experience tooling ([[WEIR-S-0014]]) sit on top of.

## Context **[REQUIRED]**

Today the **entire** working system is native in-process: every connector (`echo`, `slow`, `faulty`,
`arrow-sink`, `rest`, `postgres`) is a `#[plugin_impl]` cdylib loaded via
`ConnectorHandle::native_in_process`; `ConnectorRef::Native` is the only path the engine/orchestrator/app
exercise; the 57-group test suite + the `angreal ui demo` all run on it. [[WEIR-A-0030]] retires that path
— **WASM is the single packaging + execution target.**

The wasm path **already exists** (built in [[WEIR-I-0006]]): `ConnectorHandle::from_wasm_package`, the
`weir-codegen/wasm.rs` generator, the `wasm-fixtures/rest` package, and the `wasm_http` tests that
drive a wasm connector through the engine via `block_on`. So this is a **migration + default-flip**, not
greenfield: build each connector as a wasm component package, run the stack over wasm, then retire the
native path.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- Every first-party connector ships as a **wasm component package** ([[WEIR-A-0015]]); the engine/
  orchestrator/app/api/demo run over wasm; the **57-group suite is green on wasm**.
- **Retire the native path**: `weir-codegen/dylib.rs`, `ConnectorHandle::native_in_process`,
  `ConnectorRef::Native`. `ConnectorRef` becomes wasm-package + **origin** (first-party|community).
- A repeatable **connector→wasm build** (one connector → a versioned wasm package) — the seam
  [[WEIR-S-0014]] tooling + [[WEIR-I-0010]] registration build on.
- **Validate the premise**: streaming throughput on wasm ≥ the old cdylib (no regression; ideally faster).

**Non-Goals:**
- The **catalog** persistence/registry ([[WEIR-I-0010]]) and the **dev-experience CLI** (`weir connector
  new|test|build|publish`, [[WEIR-S-0014]]) — this initiative produces the wasm artifacts + load path they
  consume, not those surfaces.
- Python→wasm connector authoring (no Python connectors exist yet; [[WEIR-A-0001]]/[[WEIR-A-0002]] residual).
- Removing the wasm artifacts from `.gitignore` — built on demand, never committed (per project policy).

## Detailed Design **[REQUIRED]**

**Strategy: build the wasm path up beside native, prove it through the whole stack, then flip + retire** —
never a big-bang break. The engine already drives reads via `futures::executor::block_on` (chosen in
[[WEIR-I-0006]] precisely so the wasm guest's own runtime doesn't nest-panic), so the runtime is ready.

1. **Build seam.** A repeatable `connector → wasm component package` build (cargo `wasm32` + componentize
   + `package.toml`), modeled on `wasm-fixtures/rest`. Produces a versioned artifact; never committed.
2. **Dual-mode transition.** Connectors build as wasm while native still works; the engine/orchestrator
   load them via `from_wasm_package`. Keeps the suite green throughout.
3. **Flip + retire.** Once everything runs on wasm: delete `dylib.rs`, `native_in_process`,
   `ConnectorRef::Native`; `ConnectorRef` = `{ package, version, origin }`; resolution is wasm-only.
4. **Validate.** Benchmark streaming throughput wasm vs the retired cdylib to confirm [[WEIR-A-0030]]'s
   premise (no regression).

**Known risks / hard spots:**
- **Native-dependency connectors in wasm** — `postgres` needs a wasm-compatible driver over WASI sockets;
  this is the residual risk [[WEIR-A-0002]] already flagged. Sequence it **last/separately**; the demo +
  parity don't depend on it. May surface a real blocker (a fidius/WASI capability gap → a user-owned FR).
- **Test-suite build cost** — wasm fixtures build per run; migrating all tests naively could slow the
  suite badly. Mitigate with a **build-once, cached** wasm artifact for the test connectors (the
  `wasm_http` test already builds a fixture — generalize + cache it).
- **fidius wasm-backend coverage** — confirm every contract method (esp. client-streaming `write`,
  `discover`, logs/dead-letters) round-trips over wasm as it does native; gaps are user-owned fidius FRs.

## Implementation Plan (proposed slices)

- **S1 — Connector→wasm build seam + first connector e2e.** Generalize the package build; bring `echo`
  (and/or reuse the `rest` fixture) up as a wasm package; drive it through the engine via
  `from_wasm_package`. Cache the built artifact for tests. Proves the pipeline.
- **S2 — Migrate the demo connectors.** `slow`, `faulty`, `arrow-sink` as wasm packages; `angreal ui demo`
  + the api/orchestrator tests run over wasm. (Bulk of the suite green on wasm.)
- **S3 — Flip + retire the native path.** Remove `dylib.rs` / `native_in_process` / `ConnectorRef::Native`;
  `ConnectorRef` → `{package, version, origin}`; engine/orchestrator/app/api wasm-only; full suite green.
- **S4 — Postgres on wasm (the hard spot).** Wasm pg driver over WASI sockets; integration test. If
  blocked by a WASI/fidius gap, document it + raise the FR; postgres may lag behind the flip.
- **S5 — Throughput validation.** Benchmark streaming wasm vs the old cdylib; record the result against
  [[WEIR-A-0030]].

Sequencing: S1→S3 are the core flip; **S4 (postgres) runs in parallel / can trail**; S5 closes the loop.

## Alternatives Considered

- **Big-bang rewrite (flip all connectors + delete native at once)** — rejected: breaks the green suite +
  demo mid-flight; no incremental validation.
- **Keep native for first-party, wasm for community (the old A-0016 split)** — rejected by [[WEIR-A-0030]];
  perpetuates two build/test/distribution paths.
- **Block the whole migration on postgres-in-wasm** — rejected: postgres is the hardest, least-central
  case; don't let it gate echo/slow/faulty/rest + the demo. Sequence it separately.
