---
id: connector-onboarding-interfaces
level: specification
title: "Connector onboarding interfaces (backend + API + UI) — YAML-primary, build-where-you-execute"
short_code: "WEIR-S-0015"
created_at: 2026-06-24T12:44:39.580316+00:00
updated_at: 2026-06-24T12:44:39.580316+00:00
parent: WEIR-I-0012
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Connector onboarding interfaces (backend + API + UI) — YAML-primary, build-where-you-execute

## Overview **[REQUIRED]**

Defines the **onboarding** interfaces — backend ingress, control-plane API, and UI — for getting a connector
into an operator's catalog, per [[WEIR-A-0032]]: connectors distribute as **source/data**, never prebuilt
binaries. **Low-code = a `manifest.yaml` (data) that runs on a shared declarative runtime — no compile.**
**Full-code = a per-connector Rust crate, compiled at onboard.** Onboarding is **manual / human-initiated**
(phase 1) — a person selects or points at a connector and weir brings it in. This is distinct from
**composition** (building a connection from onboarded connectors) and **configuration** (per-connection
values against a connector's schema).

The interfaces today are **crate-centric and mismatched to the decided model**: the low-code path (register
a manifest → run it on the shared runtime) has no backend source, no API, and no UI, and the runtime
(`rest`) isn't wired as a manifest host. This spec fixes that contract end-to-end.

## Onboarding Model — Phase 1 (manual) **[REQUIRED]**

No automated registry crawl or background sync in phase 1. The two import gestures we already have widen
(from "crates only" to "crates + manifests"), plus a third for declarative connectors:

1. **Discover & select** — browse what's *available* and pick. "Available" widens from "crates.io connectors"
   → **crates.io connectors + YAML manifests vendored in the repo** (an in-repo `manifests/` set for now: the
   authored corpus, curated). One list, two artifact kinds.
2. **Point me to it** — direct import: a crate (path; later crates.io/git) **or** a manifest (yaml / path).
3. **Base runtime + your YAML** — pick a base declarative runtime (HTTP today = `rest`; DB/etc. later) and
   supply a manifest → a **named** catalog connector that runs on that runtime. (With one runtime today this
   collapses to "register a manifest"; the "base" choice generalizes as runtime families grow.)

**Low-code gestures register data → instant, no compile.** **Full-code gestures (a crate) compile at
onboard.** Deferred past phase 1: automated registry/hub sync, a manifest **registry service**, and
import-as-a-job (phase 1 import is synchronous — manifests are instant; a crate shows a `building…` state).

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Connector author** — writes a `manifest.yaml` (low-code) or a Rust guest crate (full-code).
- **Operator** — onboards into *their* weir instance, then composes connections.
- **Catalog** ([[WEIR-I-0010]]) — local registry of onboarded connectors (manifest entries + full-code packages).

### External Systems
- **Shared declarative runtime** — one wasm component (today's generalized `rest`, grown to the
  declarative surface under [[WEIR-I-0008]]) that **executes a manifest passed as its config**. Ships *with
  weir* (built in weir's own environment — not a per-connector binary).
- **`weir-importer`** — parse/validate a `manifest.yaml` + emit a **fidelity report** (tier / confidence / gaps).
- **cargo / wasm32-wasip2 toolchain** — compiles **full-code crates** (and the runtime) in the execution env.
- **Vendored manifests** (`manifests/`, phase 1) → a manifest registry (git / weir hub) later; crates.io for
  full-code source.

### Boundaries
- **In scope:** the three gestures + their backend/API/UI; manifest **registration** (data → runtime, no
  compile); the full-code compile path; the pre-commit **preview** (tier/confidence) gate.
- **Out of scope:** connection composition + config form (exist); the manifest registry/hub *service*
  (future); the runtime's declarative-coverage depth ([[WEIR-I-0008]] S3–S7); **codegen** (rejected, A-0032).

## Current State & The Mismatch **[REQUIRED]**

| Layer | Today | Gap vs [[WEIR-A-0032]] |
|---|---|---|
| **Ingress** (`weir-app::ingress::Source`) | `LocalCrate(PathBuf)`, `Folder{package}`. CratesIo/Git stubbed. | **No `Manifest` source** — can't register a manifest-as-data connector (the *primary* low-code path). |
| **Declarative runtime** | `rest` exists (config-driven HTTP). | Not wired as a **manifest host**: no catalog entry that means "the shared runtime + this bound manifest". Manifests run only via ad-hoc *per-connection* config (e.g. `rick-live`), not as a named connector. |
| **Preview** | none | No pre-commit **tier/confidence** report ([[WEIR-A-0020]]) — can't see what a manifest will/won't support before onboarding. |
| **API** | `POST /catalog/import {path\|package}`; `GET /catalog`, `/catalog/available`; `GET /connectors/{p}/spec\|discover` | No `manifest` import variant; **no `/catalog/preview`**; `/catalog/available` lists folders only, not manifests. |
| **UI** | "Add connectors" = folder-scan dropdown **or** a crate **path** textbox. | No manifest gesture (paste/upload/select); model is "pick a pre-built package", not "bring a manifest"; onboarding visually conflated with the connection form. |

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-1.1 | **Low-code onboarding (PRIMARY):** accept a `manifest.yaml`, validate it via `weir-importer`, and register a **named catalog connector bound to the shared declarative runtime** — **no compile, instant.** | [[WEIR-A-0032]] §1 (interpreter). |
| REQ-1.2 | **Full-code onboarding (manual):** accept a Rust guest crate (local path; later crates.io/git source) → `compile → stage → register`. | [[WEIR-A-0032]] §2. (Exists as `LocalCrate`.) |
| REQ-1.3 | **Folder re-import (dev):** upsert an already-staged package without recompiling. | Dev loop. (Exists.) |
| REQ-1.4 | **Preview (dry-run):** for a manifest, return **tier + confidence + unsupported-features** without running it. | [[WEIR-A-0020]]; "name the gap, don't onboard broken". |
| REQ-1.5 | Onboarding is **idempotent** on `(name, version)` (upsert) + invalidates the connector handle cache on re-onboard. | Re-onboard correctness ([[WEIR-T-0051]]). |
| REQ-1.6 | Onboarded connectors are selectable in the **connection-compose** source/dest dropdowns (role-filtered). | Onboarding feeds composition. |
| REQ-1.7 | All three gestures are available (discover&select / point-to-it / base-runtime+YAML); manifest gestures register data, crate gestures compile. | Phase-1 onboarding model. |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-1.1 | **No prebuilt connector binary crosses a machine boundary** — manifests are data; the runtime + full-code crates are built where they run. | [[WEIR-A-0032]] core principle. |
| NFR-1.2 | **Low-code onboarding is instant (no compile).** Only full-code compiles → a `building…`/spinner state; import-as-a-job is deferred past phase 1. | UX; the interpreter model's payoff. |
| NFR-1.3 | Onboarding records **provenance + origin** ([[WEIR-A-0018]]): source kind (manifest/crate/folder), origin (first-party/community/private), license where known. | Publication Contract. |
| NFR-1.4 | Failures are **surfaced, not swallowed** — a failed validation/compile reports to the operator; the catalog row reflects `failed`, never half-registered. | Trust. |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: Ingress `Source` (backend)
```rust
pub enum Source {
    Manifest { yaml: String, name: Option<String> },  // NEW — register data; runs on the shared runtime, NO compile
    LocalCrate(PathBuf),                                // full-code; compile (exists)
    Folder { package: String },                         // dev / re-import (exists)
    // Future: CratesIo { name, version }, Git { url, rev }, ManifestUrl { url }
}
```
- **Manifest pipeline (no compile):** `weir_importer::import_yaml(yaml)` → validate + tier/confidence →
  register a **catalog entry of kind `Manifest`** = `{ name, manifest_yaml, runtime: declarative }`. Done —
  no codegen, no cargo.
- **Crate pipeline (unchanged):** `compile_and_stage` → snapshot `spec()` → register (kind `Wasm`).
- **ADR:** [[WEIR-A-0032]] (codegen explicitly rejected as the low-code mechanism).

### Decision Area: Catalog & resolution
- Catalog entries gain a **kind**: `Wasm { package }` (full-code, compiled) | `Manifest { yaml }` (low-code).
- A connection selecting a `Manifest` connector **resolves to the shared declarative-runtime package + the
  manifest as the connector's bound spec/config**; per-connection config (e.g. credentials) layers on top.
  This generalizes today's `rick-live` (= `rest` + ad-hoc config) into a *named* catalog connector.

### Decision Area: Control-plane API
- `POST /catalog/preview { manifest }` → `ImportReport { tier, confidence, streams[], unsupported[] }` —
  **NEW**, synchronous, no run.
- `POST /catalog/import` — extend `ImportDto` with `manifest: Option<String>` (yaml) alongside `path`/`package`.
  **Manifest → instant `CatalogEntry`**; crate → compile (synchronous-with-spinner in phase 1).
- `GET /catalog/available` — **widen** to list vendored **manifests + crates** (the "discover & select" source).
- Unchanged: `GET /catalog`, `GET /connectors/{plugin}/spec|discover`, `POST /connections`.

### Decision Area: UI onboarding flow
- A dedicated **"Onboard a connector"** view exposing the three gestures:
  1. **Discover & select** — a list of *available* (manifests + crates) → pick → onboard.
  2. **Point me to it** — paste/upload a `manifest.yaml`, **or** a crate path.
  3. **Base runtime + YAML** — choose a runtime family + supply a manifest → a named connector.
- Manifest onboarding shows **Preview** (tier/confidence/gaps) before commit; **manifest commit is instant**,
  a crate commit shows `building…`.
- Onboarding is **visually separate** from the connection form; onboarded connectors populate the source/dest
  dropdowns (existing). The folder-scan list becomes a dev/debug affordance.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| [[WEIR-A-0032]] | Connector distribution: source-only; low-code on a shared runtime | decided | Manifests = data run on a shared declarative runtime (no compile); full-code crates compiled at onboard; **codegen + prebuilt-hub rejected**. |
| [[WEIR-A-0018]] | Publication Contract v1 | decided | Origin/provenance + ingress paths (refined by A-0032 to cover manifests). |
| [[WEIR-A-0030]] | WASM-always connectors | decided | Every connector — including the shared runtime — is a wasm component. |
| [[WEIR-A-0020]] | Tiered fidelity | decided | Tier/confidence reporting → the preview gate (REQ-1.4). |

## Constraints **[CONDITIONAL: Has Constraints]**

### Technical Constraints
- A manifest only does what the **shared runtime supports**; unsupported features surface via preview
  (REQ-1.4), and the runtime grows under [[WEIR-I-0008]]. The runtime is extended **once**, centrally — there
  is no per-connector compile to gain a feature (the interpreter advantage).
- **Full-code** onboarding needs the Rust + wasm32-wasip2 toolchain in the execution environment (or a
  weir-managed builder running *in* that environment) — scoped to the non-declarative tail only.

### Regulatory Constraints
- IP-clean ([[WEIR-V-0001]]): onboarding records license/provenance (NFR-1.3); the registry/hub gate
  ([[WEIR-A-0018]]) applies to *distributed* manifests; an operator's own manifest input is their call.

## Changelog **[REQUIRED after publication]**

| Date | Change | Rationale |
|------|--------|-----------|
| 2026-06-24 | Initial draft — codegen pipeline (`Source::Manifest → weir-codegen → compile`). | [[WEIR-A-0032]] v1. |
| 2026-06-24 | **Revised to the interpreter model + phase-1 manual gestures:** low-code = register a manifest that runs on the shared declarative runtime (no compile/codegen); added the discover/point/base-runtime gestures + vendored manifests; codegen removed from the pipeline. | [[WEIR-A-0032]] revised (codegen → interpreter). |
