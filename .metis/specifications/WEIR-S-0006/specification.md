---
id: connector-contract-sdk
level: specification
title: "Connector Contract & SDK"
short_code: "WEIR-S-0006"
created_at: 2026-06-17T02:06:24.624851+00:00
updated_at: 2026-06-17T02:06:24.624851+00:00
parent: WEIR-I-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Connector Contract & SDK

*Component spec under [[WEIR-I-0001]]. Source: component PRD §5. The contract is the most important interface in the system — simultaneously the community contribution surface, the moat boundary, and the migration target.*

## Overview **[REQUIRED]**

Defines the connector contract that the Runtime ([[WEIR-S-0005]]) executes and the SDK targets, plus the authoring tooling: SDK, low-code builder, AI-assisted authoring, packaging. It exists to make authoring trivial while keeping the contract stable and versioned.

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Connector author (engineer)**: writes connectors with the SDK (Python first-class).
- **No-code author**: uses the declarative/low-code builder.
- **AI agent**: authors connectors with AI assistance.

### External Systems
- **Connector Runtime ([[WEIR-S-0005]])**: executes connectors against the contract.
- **Connector Catalog ([[WEIR-S-0007]])**: stores packaged, versioned connectors.
- **Migration Importer ([[WEIR-S-0008]])**: targets this contract as its translation output.

### Boundaries
Inside: the typed contract, SDK, builder, AI-assist, packaging, conformance harness. Outside: execution/isolation (Runtime), storage (Catalog).

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-CT-1 | Define a typed contract: spec/config, schema discovery, sync modes, state/checkpoint, structured errors, reverse-ETL write semantics, partitioning declaration. | The load-bearing interface (§A–§D). |
| REQ-CT-2 | Provide an SDK to author connectors (Python first-class; native optional). | Authoring (§E). |
| REQ-CT-3 | Provide a low-code/declarative builder. | Parity (§E). |
| REQ-CT-4 | Support AI-assisted connector authoring. | Differentiator (§E). |
| REQ-CT-5 | Package and version connectors against a contract version. | Packaging/versioning (§E). |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-CT-1 | Explicit contract versioning with a stated backward-compatibility policy. | Stable contract; ecosystem trust. |
| NFR-CT-2 | Authoring ergonomics: the long tail must be trivial to write. | Community contribution surface. |
| NFR-CT-3 | Transport-neutral contract (expressible over ABI / WASM / subprocess). | Sealed from ADR-0002. |
| NFR-CT-4 | A connector acceptance-test harness for conformance. | Migration fidelity + quality. |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: Connector contract design (FIRST-WAVE)
- **Context**: The typed contract; transport-neutral. **ADR**: WEIR-A-0014. Concrete design below.

### Decision Area: Native connectors as first-class
- **Context**: Native (Rust) first-class alongside Python. **ADR**: WEIR-A-0016 (decided).

### Decision Area: AI-assisted authoring approach
- **ADR**: WEIR-A-0017.

### Decision Area: Execution-model coupling / Migration target
- **ADR**: WEIR-A-0002 (decided), WEIR-A-0003.

## Detailed Design — Connector Contract v0 **[design draft for WEIR-A-0014]**

*Concrete realization of the shape decided in [[WEIR-A-0014]]. This is the artifact that takes 0014 toward `decided`.*

### The `Connector` interface (one capability-gated `fidius` `#[plugin_interface]`)

**fidius is request/response, not streaming** (confirmed against fidius 0.3.0). So the data path is a **chunked pull model**, not a stream: `read` returns one chunk + the next state + `has_more`; the Engine calls repeatedly, committing state per chunk. `read`/`write` use `#[wire(raw)]` (one `Vec<u8>` in/out carrying a bincode envelope with Arrow IPC bytes inside) — required for efficiency on the WASM path, where typed `Value` marshalling would explode a byte buffer into a per-element `list<u8>` while `#[wire(raw)]` does a bulk memcpy. `spec`/`check`/`discover` stay typed (small payloads).

```rust
#[plugin_interface(version = 0)]   // fidius interface; #[optional] methods carry capability bits
pub trait Connector: Send + Sync {
    // --- always present (typed) ---
    fn spec(&self) -> ConnectorSpec;                                   // config schema + capabilities + sync modes
    fn check(&self, config: Config) -> CheckResult;                    // validate config + connectivity

    // --- SOURCE capability (#[optional] => capability bit) ---
    #[optional(since = 1)]
    fn discover(&self, config: Config) -> Result<Catalog, ConnectorError>;   // typed
    #[optional(since = 1)] #[wire(raw)]
    fn read(&self, request: Vec<u8>) -> Vec<u8>;   // raw envelope: ReadRequest(ReadContext) -> ReadChunk{ batch, next_state, has_more }

    // --- DESTINATION capability (REVERSE_ETL is a WriteMode flavor, not a method) ---
    #[optional(since = 1)] #[wire(raw)]
    fn write(&self, request: Vec<u8>) -> Vec<u8>;  // raw envelope: WriteRequest(WriteContext + batch) -> WriteAck{ state }
}
```

**Capability gating:** fidius `#[optional]` sets a per-method capability bit the host can query. `SOURCE` = `discover`+`read` implemented; `DESTINATION` = `write` implemented; `REVERSE_ETL` is declared in `ConnectorSpec` (a `write` flavor via `WriteMode::Upsert`). A pure source leaves `write` unimplemented → its bit is off → the host never dispatches writes to it. (The SDK wraps the raw `read`/`write` so authors still write typed `ReadChunk`/`WriteAck` code.)

### Boundary data types
- **ConnectorSpec** `{ name, connector_version, contract_version: u32, config_schema: JsonSchema, capabilities: u64, supported_sync_modes: [FullRefresh|Incremental|Cdc] }`
- **Catalog** `{ streams: [Stream] }`
- **Stream** `{ name, namespace?, schema: ArrowSchema (canonical), supported_sync_modes, source_defined_cursor: bool, default_cursor_field?, source_defined_primary_key?, partitioning: PartitionScheme }` — **Arrow's type system is canonical**; long-tail JSON rows conform to (project from) the Arrow schema, so both encodings share one type system.
- **PartitionScheme** = `None | ByCursorRange { granularity } | ByKeyShards { key, count } | ByParent { parent_stream, key }` — connector **declares** how it can be sliced; the Engine **plans** the slices ([[WEIR-A-0012]]).
- **ConfiguredCatalog** → **ConfiguredStream** `{ stream_ref, sync_mode, cursor_field?, primary_key?, write_mode: WriteMode, mapping: MappingSpec → [[WEIR-A-0026]] }`
- **WriteMode** = `Append | Upsert { business_keys: [field] } | Overwrite` (reverse-ETL = `Upsert` to an operational destination).
- **ReadContext** `{ config, configured_stream, partition: Partition, state: StreamState }`; **WriteContext** `{ config, configured_stream }`.
- **ReadChunk** (the `read` raw-envelope return) `{ batch: RecordBatch, next_state: StreamState, has_more: bool, diagnostics: [LogEntry] }` — one pull; `has_more=false` ends the work unit.
- **WriteAck** (the `write` raw-envelope return) `{ state: StreamState, accepted: u64, diagnostics: [LogEntry] }`.
- **RecordBatch** (dual encoding, [[WEIR-A-0014]] §3) = `Rows(Vec<JsonRow>)` (long tail; rows conform to the stream's Arrow schema) `| Arrow(ipc_bytes)` (bulk native path). **v0 crosses the boundary as Arrow IPC bytes** inside the raw envelope (cross-primitive: dylib + WASM); a shared-memory handle is a possible later dylib-only optimization.
- **StreamState** `{ cursor?: Value, opaque: bytes }` — returned in each `ReadChunk.next_state`; the **Engine commits** it transactionally ([[WEIR-A-0011]]) and persists via the store ([[WEIR-A-0007]]/[[WEIR-A-0009]]). The connector never owns durability.
- **ConnectorError** `{ kind: Config | Transient | RecordLevel | Fatal, message, retryable, context }` ([[WEIR-A-0014]] §5): Config→surface, don't retry; Transient→Engine retries w/ backoff; RecordLevel→dead-letter record, continue; Fatal→abort run. (Carried in the raw envelope as `Result`.)

### Read/write flow (chunked, request/response)
1. Engine plans work units from `ConfiguredCatalog` + last `StreamState` + `PartitionScheme` ([[WEIR-A-0012]]) — one unit per `(stream, partition)`.
2. Runtime calls `read(ReadContext)` → `ReadChunk`; **loops** while `has_more`, feeding `next_state` back in. Each chunk's `next_state` is a checkpoint.
3. Engine applies the in-flight mapping stage ([[WEIR-A-0026]]) to each batch, then calls `write(WriteContext, batch)` → `WriteAck` on the destination.
4. Engine commits checkpoint + outbox + run-state in **one transaction** per chunk ([[WEIR-A-0010]]/[[WEIR-A-0011]]).

### Manifest schema v0 (declarative source → codegen)
Long-tail authoring surface; **codegens a `fidius` connector** implementing the interface above ([[WEIR-A-0014]] §2). v0 targets REST/HTTP API sources (bulk of the long tail + the Airbyte migration target, [[WEIR-A-0020]]).

```yaml
connector: { name: example, version: 0.1.0, contract_version: 0 }
spec:
  config:
    - { name: api_key, type: string, secret: true, required: true }
    - { name: start_date, type: date, required: false }
auth: { type: bearer, token: "{{ config.api_key }}" }   # api_key | basic | bearer | oauth2
rate_limit: { requests: 5, per: 1s }
streams:
  - name: users
    http: { method: GET, path: /v1/users }
    record_selector: "$.data[*]"            # JSONPath to records
    primary_key: [id]
    schema: { id: {type: string}, email: {type: string}, created_at: {type: datetime} }
    incremental: { cursor_field: created_at, request_param: { since: "{{ state.cursor }}" } }
    pagination: { type: cursor, cursor_path: "$.next_cursor", request_param: { cursor: "{{ page.cursor }}" } }
    partitioning: { type: cursor_range, granularity: P1D }   # → PartitionScheme::ByCursorRange
```
Codegen lowers this to a `fidius` `Connector` impl: `spec`/`check`/`discover` from the static manifest; `read` drives the HTTP + pagination + cursor loop, emitting `Rows`. Destinations and bulk/Arrow sources are **hand-written native** connectors in v0 (not manifest-driven).

### Resolved design decisions (2026-06-17)
- **Message framing:** ✅ typed `Message` enum (`Record`/`Checkpoint`/`Log`/`Trace`) over `#[wire(raw)]` — mirrors Airbyte's protocol (aids migration) but typed.
- **Encoding handoff:** ✅ **Arrow IPC bytes** is the v0 cross-primitive form (dylib + WASM); shared-memory handle deferred as a dylib-only optimization.
- **Type system:** ✅ **Arrow-canonical** — Arrow types are the source of truth; JSON rows conform to/project from the stream's Arrow schema. (The manifest `schema:` field declares Arrow logical types.)

### Implementation status — contract dimensions (WEIR-I-0004, 2026-06-20)
The engine + the `weir-connector-postgres` conformance vehicle now **honor** these contract dimensions, each with a live Postgres integration test:
- **WriteMode** `Append` / `Upsert{business_keys}` / `Overwrite` — [[WEIR-T-0025]].
- **SyncMode** `FullRefresh` / `Incremental` (resume-from-cursor) — [[WEIR-T-0026]].
- **Partitioned reads** — source advertises `PartitionScheme`; the engine fans out one work unit per `Partition` with independent per-partition checkpoints (`ByKeyShards` v0) — [[WEIR-T-0027]].
- **SyncMode `Cdc`** — Postgres logical replication; the WAL LSN / slot is carried in `StreamState.opaque` — [[WEIR-T-0028]].

These were *implementing the already-declared types*, not redesigning the contract → no new ADR. The `Cdc` (and all) data path remains the chunked-pull model (fidius 0.3.0 request/response); a streaming `read` is a future revisit ([[WEIR-I-0005]]) now that fidius 0.4 supports streaming.

### Remaining gate to WEIR-A-0014 `decided`
- **Reference connector (the gate):** build one declarative REST source + one native Arrow destination and prove the contract round-trips over **both** execution primitives ([[WEIR-A-0002]]) — this is the agreed bar before ratifying `0014`.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| WEIR-A-0014 | Connector contract design | discussion | Shape decided; concrete v0 above pending ratification. |
| WEIR-A-0016 | Native connectors as first-class | decided | Native dylib is the trusted default; Python first-class too. |
| WEIR-A-0002 | Execution & isolation model | decided | dylib (trusted) + WASM-first (open); transport-neutral. |
| WEIR-A-0015 | Packaging & distribution format | decided | dylib + WASM + manifest-as-codegen-source. |
| WEIR-A-0017 | AI-assisted authoring approach | draft | Agent emits manifest → codegen → signed plugin. |
| WEIR-A-0003 | Airbyte compatibility strategy | draft | Contract is the migration target. |
