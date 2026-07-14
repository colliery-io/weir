---
id: connector-catalog-registry
level: specification
title: "Connector Catalog / Registry"
short_code: "WEIR-S-0007"
created_at: 2026-06-17T02:06:26.185079+00:00
updated_at: 2026-06-17T02:06:26.185079+00:00
parent: WEIR-I-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Connector Catalog / Registry

*Component spec under [[WEIR-I-0001]]. Source: component PRD §6.*

## Overview **[REQUIRED]**

Stores, versions, discovers, and serves connector definitions and artifacts. It exists so the Runtime ([[WEIR-S-0005]]) has one authoritative place to fetch a versioned connector and users have one place to find and install them. It is fed by the SDK build pipeline ([[WEIR-S-0006]]) and the Migration Importer ([[WEIR-S-0008]]), and read by the Runtime at dispatch.

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **User**: discovers, searches, installs/enables connectors into a workspace.

### External Systems
- **Connector SDK pipeline ([[WEIR-S-0006]])** and **Migration Importer ([[WEIR-S-0008]])**: feed connectors in.
- **Connector Runtime ([[WEIR-S-0005]])**: fetches a specific version at dispatch.
- **Control Plane ([[WEIR-S-0002]])**: proxies browse/install operations.

### Boundaries
Inside: artifact storage, versioning, discovery, install. Outside: execution (Runtime), authoring (SDK).

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-CA-1 | Register, store, and version connector artifacts and metadata. | Authoritative catalog. |
| REQ-CA-2 | Discovery/search and install/enable into a workspace. | Marketplace (§E). |
| REQ-CA-3 | Serve a specific connector version to the Runtime at dispatch. | Deterministic runs. |
| REQ-CA-4 | Track contract-version compatibility per connector. | Contract gating (ADR-0019). |
| REQ-CA-5 | Accept connectors from the SDK pipeline and the Migration Importer. | Two supply paths. |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-CA-1 | Artifact integrity and provenance (signing/verification target). | Supply-chain trust. |
| NFR-CA-2 | Catalog reads do not block runs (cache-friendly). | Throughput. |
| NFR-CA-3 | Scales to thousands of connector versions. | Ecosystem scale. |
| NFR-CA-4 | Supports first-party, community, and private catalogs. | Vendor + community. |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: Catalog artifact storage backend
- **Context**: OCI registry vs. object store vs. DB-backed blobs. **ADR**: WEIR-A-0018.

### Decision Area: Connector versioning & compatibility policy
- **Context**: Semver, contract-version gating, deprecation. **ADR**: WEIR-A-0019.

### Decision Area: Packaging format (shared)
- **ADR**: WEIR-A-0015.

## Catalog Sync — availability + ingress (design 2026-06-23)

"Sync" = keeping the catalog aligned with the publication universe ([[WEIR-A-0018]] Publication Contract
v1). *Distinct from data-sync (connection execution).* Two engines over that contract:

### 1. Availability refresh (discovery; metadata-only, NO compile)
Produces the **"available to register"** view — what *could* be registered — keyed `(name, version,
origin, source)`, read straight from manifests (`[package.metadata.weir]` → `contract` + `roles`), so it
is cheap and compiles nothing. Three sources:
- **first-party** — the curated **allowlist**; for each entry, query crates.io for published versions +
  metadata. `origin=first-party`.
- **community** — crates.io **crawl** (keyword `weir-connector` / prefix). `origin=community`, view-only.
- **local (MVP)** — **folder scan** of `connectors/` (local crates / built packages). `origin=private`.

Refresh is **on-demand** (a "refresh catalog" action) ± periodic. Output is a cache/listing, **not** the
registered table. **MVP ships only the folder-scan source**; allowlist + crates.io crawl are post-MVP
([[WEIR-A-0018]]), plugging into the same view.

### 2. Ingress pipeline (register/upsert; the "make it real" path)
One shared pipeline for **all** paths — *resolve source → fetch → compile `wasm32-wasip2` → load + snapshot
`spec()` → upsert the `connectors` row*:
- **source** = `{ allowlisted crates.io (name,ver) | community crates.io (name,ver) | git URL | local
  path | folder package }`.
- **fetch** = cargo fetch / git clone / copy the source.
- **compile** = the [[WEIR-I-0011]] build seam (`weir-wasm-testkit`) elevated to an app capability →
  cached wasm in `connectors/`.
- **snapshot** = load wasm, call `spec()` → roles, `config_schema`, contract, supported_sync_modes.
- **upsert** = write/replace the `(name, version)` row (identity = `Cargo.toml`, [[WEIR-A-0018]]): origin,
  source-ref, spec snapshot, contract_range, artifact path, status, timestamps. Idempotent.
- **gates** — incompatible `contract` or compile/spec failure → surfaced + `status=failed`, not usable.
- **status lifecycle** — `importing` → `ready` → `failed`.

MVP entry = **local/folder import**; crates.io + git sources plug into the same pipeline post-MVP.

### Relationship
Availability feeds the **browse** UI; ingress is the **explicit register/import** action over a chosen
(or directly-provided) source. The folder-scan MVP collapses both (the folder is the availability source
*and* a local-import ingress). Connections then **pin `(name, version)`** from the registered table
([[WEIR-A-0019]]); dispatch gates the pinned `contract_range` against the running engine.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| WEIR-A-0018 | Catalog artifact storage backend | **Publication Contract v1 decided** (storage/OCI open) | 3 ingress paths; `[package.metadata.weir]`; fetch→compile→snapshot→upsert. |
| WEIR-A-0019 | Connector versioning & compatibility | decided | Semver + per-connection pin + contract gating. |
| WEIR-A-0015 | Packaging & distribution format | decided | wasm-component package (shared with Runtime/SDK). |
