---
id: first-class-reverse-etl-in-flight
level: initiative
title: "First-class reverse ETL (in-flight mapping + declarative SaaS destinations)"
short_code: "WEIR-I-0007"
created_at: 2026-06-21T23:02:32.124292+00:00
updated_at: 2026-07-05T01:18:55.966389+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: XL
initiative_id: first-class-reverse-etl-in-flight
---

# First-class reverse ETL (in-flight mapping + declarative SaaS destinations) Initiative

*Phase 2 completion of the weir vision ([[WEIR-V-0001]]): reverse ETL is a **headline open-core capability** — first-class, including the Salesforce destination. The connector contract is already symmetric after [[WEIR-I-0006]] — this initiative cashes that in.*

> **RESUMED (2026-07-04).** The parity arc ([[WEIR-I-0008]]) is complete, so this un-parks. Two updates
> since the June plan:
> - **S1 (in-flight mapping) is already built** — it migrated to the parity arc as shared infrastructure and
>   shipped as [[WEIR-T-0071]] (engine applies `ConfiguredStream.mapping`; importer lowers Airbyte transforms).
> - **Destinations are now built as a shared declarative runtime, NOT codegen** — see [[WEIR-A-0034]]. The
>   June plan's "destination codegen mirroring source codegen" is superseded: [[WEIR-A-0032]] moved sources to
>   the shared `rest` runtime, so destinations mirror *that*. S2 below is re-scoped accordingly; S3–S5 hold.

## Context **[REQUIRED]**

The vision makes reverse ETL (data activation) a **first-class, symmetric primitive**: a sync is `source → destination` where the destination may be a warehouse **or** an operational SaaS system. After [[WEIR-I-0006]] the contract already supports the write side — **client-streaming `write`**, `WriteMode::Upsert` (idempotent activation, [[WEIR-A-0011]]), WASM `wasi:http` egress with **credential injection** ([[WEIR-A-0013]]) — but two pieces are missing:

1. **In-flight mapping** ([[WEIR-A-0026]]) is ratified but only stubbed (the engine passes records through unmapped). Activation needs field shaping: warehouse columns → SaaS object properties.
2. **No operational-system destinations exist.** The open core must ship **HubSpot and Salesforce** ([[WEIR-V-0001]] Major Features / Success Criteria).

Per direction (2026-06-21): build **both** SaaS destinations, and build them via **declarative destination codegen** (manifest → wasm-http dest), mirroring the existing source codegen ([[WEIR-T-0003]]) rather than hand-writing one-offs.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- Implement the **engine-owned in-flight mapping stage** ([[WEIR-A-0026]] v0): `select`/`drop`/`rename`/`cast`/`filter` + computed fields via the bounded expression language, over the row-JSON path (and Arrow projection/filter where cheap).
- Add **declarative-destination codegen**: a manifest describing a SaaS REST destination → a WASM `wasi:http` connector doing client-streaming `write` with idempotent upsert + brokered credential injection.
- Ship **HubSpot** and **Salesforce** destinations generated from manifests, in the open core.
- Prove the reverse-ETL flow end-to-end: **warehouse (Postgres) → mapping → SaaS upsert**, idempotent under replay, against mock SaaS servers.

**Non-Goals:**
- Real warehouse **source** connectors (Snowflake/BigQuery) — Postgres stands in as the warehouse; dedicated warehouse sources are later initiatives.
- Heavy transforms (joins/aggregations/UDFs) — explicitly out of scope ([[WEIR-A-0026]]; dbt's job).
- The no-code mapping **UI** / connector builder ([[WEIR-A-0017]]/[[WEIR-A-0024]]) — separate initiative; this initiative is the engine + connector substrate (config-level).
- Vendor periphery (managed control plane, premium SLAs).

## Architecture **[CONDITIONAL: Technically Complex Initiative]**

### Overview
- **Mapping stage** lives in `weir-engine`, applied per `RecordBatch` between the read stream and the buffered client-streaming write — driven by `ConfiguredStream.mapping` (`MappingSpec`). Pure function `(MappingSpec, RecordBatch) -> RecordBatch` (+ dropped rows for `filter`), evaluated on row-JSON; Arrow path does column projection/filter natively.
- **Declarative destination codegen** extends `weir-manifest` + `weir-codegen`: a manifest's destination section (base URL, auth, per-object endpoint + method + upsert key + field mapping defaults + batching) generates a `wasm32-wasip2` guest implementing the v1 `write` (client-streaming): pull `RecordBatch`es, shape each record into the SaaS object JSON, POST/PATCH via `fidius_guest::http`, return `WriteOutcome` (accepted count + dead-letters for per-record rejects). Mirrors `crates/weir-codegen/src/wasm.rs`.
- **Credentials** never enter the guest: the host `EgressPolicy` (`HostAllowList.inject_headers`, [[WEIR-A-0013]]) injects the API token / OAuth bearer. **Salesforce OAuth** (token refresh) is the open sub-question — see Alternatives.

### Sequence (reverse-ETL run)
`Postgres source.read` → `Stream<ReadMessage>` → engine **maps** each `Records` batch (`MappingSpec`) → buffers → on `Checkpoint`, **client-streaming `write`** to the SaaS dest (guest upserts each record over `wasi:http`) → engine commits checkpoint+outbox atomically. Replay re-upserts (idempotent on the business key).

## Detailed Design **[REQUIRED]**

Decomposed into the slices below (decomposition pending human review before task creation).

**S1 — In-flight mapping stage ([[WEIR-A-0026]] v0). ✅ DONE ([[WEIR-T-0071]]).** `MappingOp`
Select/Drop/Rename/Cast/Filter/Compute applied by `weir-engine` between read and write over row-JSON; the
importer lowers Airbyte `AddFields`/`RemoveFields`/`record_filter` onto it; dbt boundary held by the grammar.

**S2 — Shared declarative destination runtime + manifest schema ([[WEIR-A-0034]]).** A **new** wasm
`wasi:http` destination guest (sibling of `crates/connectors/rest`, built the same interpreter way) that reads
a **destination manifest** as config and does the client-streaming `write`: per-object endpoint + method
(POST/PATCH), upsert/business key, field mapping, batching, auth scheme. Per-record dead-letters; host-side
credential injection reused ([[WEIR-A-0033]]); transient retry/backoff reused ([[WEIR-T-0069]] pattern).
Add the destination schema to `weir-manifest` (+ importer/authoring path). Wire test against a mock SaaS
server. **No `weir-codegen`.**

**S3 — Reverse-ETL flow + semantics.** Wire mapping + dest into the engine flow; idempotent activation via `Upsert(business_keys)`; warehouse(Postgres)→mapping→mock-SaaS E2E; replay/idempotency test.

**S4 — HubSpot destination (manifest → codegen).** HubSpot CRM manifest (private-app token via egress inject; create-or-update by unique property); E2E against a mock HubSpot.

**S5 — Salesforce destination (manifest → codegen).** Salesforce manifest (sObject upsert by External Id; Composite/Bulk); resolve **OAuth token handling** (S5a if it needs an auth/refresh seam beyond static header injection); E2E against a mock Salesforce.

## Alternatives Considered **[REQUIRED]**

- **Hand-write the first SaaS connector, codegen later.** Rejected per direction — invest in declarative-destination codegen now so both dests (and the long tail) come from manifests, mirroring the source path.
- **Mapping as a connector method.** Rejected by [[WEIR-A-0026]] — keeps connectors pure; mapping is reusable across any source→dest.
- **Salesforce OAuth in-guest.** Rejected — credentials/refresh stay host-side ([[WEIR-A-0013]]); if static header injection is insufficient for OAuth refresh, add a host-side token provider to the egress policy (sub-slice), not in-guest secrets.

## Implementation Plan **[REQUIRED]**

Slices S1 → S5 (S1 is the prerequisite; S2 enables S4/S5; S4/S5 can land independently once S2/S3 exist). Decompose into tasks after review. Each slice ends green (workspace + conformance; SaaS dests E2E against mock servers, mirroring the `wasm_http` pattern).

## Exit Criteria

- [ ] Engine applies `MappingSpec` (select/drop/rename/cast/filter/compute) on row-JSON + Arrow; dbt boundary enforced by grammar ([[WEIR-A-0026]]).
- [ ] Declarative destination codegen emits building, conformant wasm `wasi:http` upsert destinations.
- [ ] HubSpot **and** Salesforce destinations generated + E2E green against mock servers.
- [ ] Reverse-ETL flow (Postgres warehouse → mapping → SaaS upsert) E2E green + idempotent under replay.
- [ ] Workspace + integration suites green; clippy clean.
