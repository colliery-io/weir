---
id: connector-developer-experience
level: specification
title: "Connector Developer Experience"
short_code: "WEIR-S-0014"
created_at: 2026-06-22T19:03:31.998202+00:00
updated_at: 2026-06-22T19:03:31.998202+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Connector Developer Experience

## Overview **[REQUIRED]**

Owns the **end-to-end connector lifecycle as a designed experience** — what it means to *write → test →
package → version → publish → obtain → install → pin → run → upgrade/hold-back* a connector. Designed
**outside-in**: the author's and operator's lived journey drives weir's internals, not the reverse.

It sits **above** the Connector Contract & SDK ([[WEIR-S-0006]]) and the Catalog ([[WEIR-S-0007]] /
[[WEIR-I-0010]]) — those are the *implementation* of the journey this spec defines.

**Unifying principle — many front doors, one artifact.** Every authoring tier converges on the **same
package** ([[WEIR-A-0015]]), the **same contract** ([[WEIR-A-0014]] / [[WEIR-A-0029]]), and the **same
catalog**. An author picks the tier that fits; an operator sees one consistent connector regardless of how
it was built. The native-vs-wasm question is resolved by [[WEIR-A-0030]] (**WASM-always**): there is **one
packaging target** — every connector is a wasm package; "first-party/trusted" is an *origin* attribute, not
a packaging kind.

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Connector Author** — writes a connector. Sub-personas by tier: *declarative* (manifest), *code*
  (Rust/Python SDK), *AI-assisted*, *importer* (Airbyte). Wants: a fast start, a clear contract, and
  confidence it's correct before sharing.
- **Maintainer / Publisher** — versions + publishes a connector for others; owns its semver +
  contract-range. Often the same person as the author.
- **Operator** — runs weir: browses/obtains connectors, installs versions into the catalog, builds + pins
  connections, decides upgrades and hold-backs.

### External Systems
- **Airbyte** — an import source for the parity long tail ([[WEIR-S-0008]] / [[WEIR-I-0008]]).
- **Artifact channel** — where packages are published/obtained: local dir now, hub/OCI later
  ([[WEIR-A-0018]]).
- **fidius** — the plugin contract/runtime substrate the SDK + packages target.

### Boundaries
**In:** the author/operator journey and the surfaces they touch — CLI (`weir connector new|test|build|
publish|install`), manifest, SDKs, the conformance test kit, publish/install, and the catalog UX. **Out:**
contract internals ([[WEIR-S-0006]]), catalog storage schema ([[WEIR-I-0010]]), runtime execution
([[WEIR-S-0005]]), importer internals ([[WEIR-S-0008]]) — this spec *drives* those but doesn't redefine
them.

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-1.1 | Support multiple authoring **tiers** (declarative manifest, Rust SDK, Python SDK, AI-assisted, Airbyte-import) as first-class peers, **all emitting the same package + contract**. | Meet authors where they are without fragmenting the runtime/catalog. |
| REQ-1.2 | `weir connector new <tier>` scaffolds a working starting point in the chosen tier. | A real "hello-world connector" on-ramp. |
| REQ-1.3 | A **conformance test kit** (`weir connector test`) validates a connector against the contract (check/discover/read/write) + fixtures/golden output + contract-version conformance — **before** distribution. | Confidence-before-ship; the current gap. |
| REQ-1.4 | `weir connector build` produces a **versioned** (semver), signable **package** declaring its contract-range. | A-0015 artifact + A-0019 versioning, one command. |
| REQ-1.5 | `publish` (author) + browse/`install` (operator) over the channel. | The distribution half of the journey. |
| REQ-1.6 | Operator installs a connector **version** into the catalog; a connection **pins `(name, version)`**. | Catalog (I-0010) + per-connection pinning (A-0019). |
| REQ-1.7 | Upgrade/hold-back is a **deliberate per-connection re-pin**; never automatic. | A-0019; operational hold-back. |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-1.1 | An author need not understand weir internals; each tier surface is self-contained. | Adoption / contributor ecosystem. |
| NFR-1.2 | A connector behaves identically regardless of authoring tier (same contract conformance). | "One consistent connector." |
| NFR-1.3 | A published `(name, version)` is **immutable**. | Reproducibility (A-0019). |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: Authoring tiers + decision matrix *(this spec defines)*

| Tier | Surface | Best for | Emits | Fidelity |
|------|---------|----------|-------|----------|
| **Declarative manifest** | YAML (`weir-manifest`) → `weir-codegen` | REST/SQL-ish sources expressible declaratively; fastest start | wasm pkg | n/a |
| **Rust SDK** | `Connector` trait, hand-written | first-party, perf-critical, logic beyond the manifest | wasm pkg | full |
| **Python SDK** | Python connector (compiled to wasm) | Python authors; Airbyte-CDK familiarity | wasm pkg | full |
| **AI-assisted** ([[WEIR-A-0017]]) | NL/spec → generated manifest or SDK | accelerate scaffolding; not a runtime tier | feeds manifest/SDK | varies |
| **Airbyte import** ([[WEIR-S-0008]]) | translate Airbyte manifest/CDK | the parity long tail | manifest → wasm | tiered ([[WEIR-A-0020]]) |

Constraints: language split [[WEIR-A-0001]]; all tiers conform to [[WEIR-A-0014]] and emit [[WEIR-A-0015]].

### Decision Area: Conformance test kit *(NEW — needs an ADR)*
- **Context**: how an author gains confidence a connector is correct before publishing (cf. Airbyte's
  Connector Acceptance Tests).
- **Required capabilities**: run check/discover/read/write against fixtures; golden-output comparison;
  contract-version conformance; runnable as `weir connector test` + in CI.
- **ADR**: *TBD.*

### Decision Area: Native vs wasm — RESOLVED ([[WEIR-A-0030]])
- **Decided: WASM-always.** One packaging + execution target — every connector is a wasm package
  ([[WEIR-A-0015]]); native cdylib is retired (it was *slower* on the v1 streaming surface). "First-party/
  trusted" becomes an **origin** attribute + isolation tier ([[WEIR-A-0002]]), not a packaging kind. The
  catalog `kind` collapses to `origin`. Supersedes [[WEIR-A-0016]].

### Decision Area: Distribution channel
- **Context**: publish → browse → obtain. **ADR**: [[WEIR-A-0018]] (local dir MVP → hub/OCI later).

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| [[WEIR-A-0014]] | Connector contract design | decided | The contract all tiers conform to. |
| [[WEIR-A-0015]] | Packaging & distribution format | decided | The one artifact all tiers emit. |
| [[WEIR-A-0030]] | WASM-always connector packaging | decided | One target: every connector is wasm; native retired; trust = origin. |
| [[WEIR-A-0016]] | Native connectors as first-class | superseded | by A-0030. |
| [[WEIR-A-0019]] | Connector versioning & compatibility | decided | Semver + per-connection pinning + contract-range gating. |
| [[WEIR-A-0017]] | AI-assisted authoring approach | draft | The AI tier. |
| [[WEIR-A-0018]] | Catalog artifact storage backend | draft | The distribution channel. |
| *TBD* | Connector conformance test kit | — | `weir connector test` + golden tests. |
