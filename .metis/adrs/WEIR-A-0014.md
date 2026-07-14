---
id: 001-connector-contract-design
level: adr
title: "Connector contract design"
number: 1
short_code: "WEIR-A-0014"
created_at: 2026-06-17T02:12:09.844161+00:00
updated_at: 2026-06-18T13:00:43.536624+00:00
decision_date: 2026-06-18
decision_maker: Dylan Storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0014: Connector contract design

**Status:** **Decided (2026-06-18, Dylan Storey).** Ratified by the walking-skeleton E2E ([[WEIR-T-0006]]): a connector → Sync Engine → native Arrow destination round-trip, with the source running over **both** execution primitives (native dylib + WASM component), committing checkpoint+outbox transactionally. The contract design is proven end-to-end. *Raised by: [[WEIR-S-0006]], [[WEIR-S-0005]], [[WEIR-S-0004]].*

> **Ratification note:** the E2E used the `echo` reference connector as the source (it exercises the full Source contract — `discover`/`read` — over both primitives). The **declarative-REST-source-via-codegen** authoring path ([[WEIR-T-0003]]) is connector-authoring breadth, not a contract-design risk, and remains in progress; it does not block this decision.

**Concrete v0 (implemented in `weir-connector` / `weir-connector-types` / `weir-runtime`):** one capability-gated `fidius` `Connector` interface (`spec`/`check`/`discover`/`read`/`write`, reverse-ETL as a `write` flavor); **chunked-pull** `ReadChunk`/`WriteAck` raw envelopes (fidius is request/response, not streaming); **Arrow-canonical type system** (JSON rows conform to the Arrow schema); records cross as **Arrow IPC bytes** (cross-primitive: dylib + WASM); opaque `StreamState` committed by the Engine.

## Context **[REQUIRED]**

The connector contract is simultaneously the community contribution surface, the moat boundary, and the migration target ([[WEIR-A-0003]]). It constrains the Runtime, SDK, Catalog, and Importer together, so it is first-wave.

[[WEIR-A-0002]] reframed what this decision *is*: connectors are `fidius` plugins, so the contract is not a new wire protocol — it is a **versioned `fidius` interface**. That hands us, for free: `interface_hash` drift detection, capability bits for optional methods, interface/ABI versioning (feeds [[WEIR-A-0019]]), and transport-neutrality via the `fidius` `PluginExecutor` seam (native / PyO3 / WASM all satisfy the same interface). So the open design is **what the interface looks like and what data travels across it**, not how it is transported.

## Decision **[REQUIRED]**

### 1. One capability-gated `Connector` interface (not three)
A single versioned `fidius` `Connector` interface with capability-gated methods, rather than separate Source/Destination/Spec interfaces:
- **`spec` + `check`** — always present (config schema; validate config/connectivity).
- **`discover` + `read`** — present ⇒ the connector is a **Source** (`discover` returns streams, schemas, supported sync modes, and the partitioning declaration per [[WEIR-A-0012]]).
- **`write`** — present ⇒ the connector is a **Destination**; **reverse-ETL is a `write` capability flavor** (upsert/dedup on business keys, field mapping, rate-limit-aware/idempotent writes), not a separate method or type.

One connector = one trait = **one signed artifact**. Capability bits advertise what it actually does; a pure source simply leaves `write` unimplemented. (This supersedes the earlier three-interface sketch, which would have fragmented a connector across artifacts.)

### 2. Declarative-first, realized by **codegen** (not a runtime interpreter)
The long-tail API source is authored as a **declarative manifest** (auth, pagination, streams, schemas, incremental cursor). The manifest **codegens a strongly-typed, signed `fidius` plugin** (yaml → code) rather than being interpreted at runtime by a generic component. This gives:
- a **single execution path** — everything is a compiled, signed plugin (uniform signing/distribution per [[WEIR-A-0002]]); no separate interpreter to build or harden;
- **strongly-typed, fast** connectors instead of a dynamic interpreter;
- one artifact that serves three goals — the **trivial/long-tail authoring surface**, the **AI-authoring** target ([[WEIR-A-0017]]: agent emits manifest → codegen → signed plugin), and the **Airbyte migration** target ([[WEIR-A-0003]], [[WEIR-A-0020]]: Airbyte YAML → our manifest → plugin).

The code escape hatch is a hand-written `fidius` `Connector` impl for what the manifest cannot express.

### 3. Dual record encoding
- **Long tail:** JSON-Schema-style logical types with row encoding.
- **Bulk native path:** **Arrow** batches for the high-throughput DB/warehouse connectors ([[WEIR-A-0016]]), where throughput dominates.

### 4. Streaming + state
Records stream over `fidius`'s `#[wire(raw)]` path. Per-stream **opaque state blob + cursor** is emitted inline and **committed by the Engine** ([[WEIR-A-0011]] at-least-once + idempotent); the connector never owns durability.

### 5. Structured error taxonomy
Layered on `fidius`'s `PluginError`: **config** (do not retry) / **transient** (retry+backoff) / **record-level** (dead-letter) / **fatal**.

### Out of scope (delegated)
**In-flight transform/mapping is NOT a connector method.** Light mapping (filter/rename/cast/field-shape) is a connection-level stage owned by the Engine — see **[[WEIR-A-0026]]**. Connectors stay pure extract/load.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Decision point | Chosen | Rejected alternative | Why |
|---|---|---|---|
| Interface shape | One capability-gated `Connector` | Three interfaces (Source/Dest/Spec) | Avoids fragmenting one connector across artifacts; capability bits are `fidius`-idiomatic |
| Long-tail authoring | Declarative manifest → **codegen** plugin | Generic runtime **interpreter** of manifests | One execution path; strongly-typed + fast; no interpreter to maintain/harden |
| Contract substrate | Versioned `fidius` interface | New bespoke IDL/wire protocol | Reuses `interface_hash`, capability bits, versioning, transport-neutral executor |
| Record encoding | Dual (JSON-logical + Arrow bulk) | Single encoding | Single leaves native throughput on the table |
| Base lineage | Clean contract from scratch | Adopt Airbyte/Singer protocol | Inherits constraints we explicitly reject ([[WEIR-A-0003]]); no native reverse-ETL/partitioning |

## Rationale **[REQUIRED]**

- Expressing the contract as a `fidius` interface collapses "design a protocol" into "design a trait," and inherits drift-detection, versioning, optionality, and transport-neutrality from prior art.
- Codegen keeps a single signed-plugin execution path (no interpreter), is faster and strongly typed, and unifies the long-tail/AI/migration authoring surfaces onto one manifest.
- A capability-gated single interface matches how connectors actually exist (mostly source-only or destination-only) without multiplying artifacts.

## Consequences **[REQUIRED]**

### Positive
- Unblocks Runtime, SDK, Catalog, Importer with one stable interface.
- Manifest-as-codegen makes the long tail trivial, AI-authorable, and migration-friendly in one stroke.
- Native reverse-ETL and partitioning are first-class.

### Negative
- Codegen toolchain (manifest → `fidius` plugin) is net-new and must be maintained alongside the macro.
- Dual encoding means the contract and harness must handle both row-JSON and Arrow paths.
- Requires a connector **acceptance-test harness** for conformance (shared with migration fidelity, [[WEIR-A-0020]]).

### Neutral
- Transport-neutral by construction ([[WEIR-A-0002]]); versioned via `fidius` `interface_hash`/`interface_version` ([[WEIR-A-0019]]); partitioning declared in `discover` ([[WEIR-A-0012]]); transform delegated to [[WEIR-A-0026]].

## Review Schedule **[CONDITIONAL: Temporary Decision]**

### To move discussion → decided:
- A concrete `Connector` interface definition (methods, capability bits, error enum) as a `fidius` interface.
- A **manifest schema v0** + a working codegen path producing a signed plugin from it.
- A reference connector (one declarative long-tail source + one native Arrow destination) passing the acceptance-test harness.
