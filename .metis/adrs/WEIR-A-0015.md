---
id: 001-packaging-distribution-format
level: adr
title: "Packaging & distribution format"
number: 1
short_code: "WEIR-A-0015"
created_at: 2026-06-17T02:12:11.414062+00:00
updated_at: 2026-06-17T21:03:52.967968+00:00
decision_date: 2026-06-17
decision_maker: Dylan Storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0015: Packaging & distribution format

**Status:** Decided (determined by [[WEIR-A-0002]] + [[WEIR-A-0014]]). *Raised by: [[WEIR-S-0006]] Connector Contract & SDK, [[WEIR-S-0007]] Catalog, [[WEIR-S-0005]] Connector Runtime.* *Decision-maker: Dylan Storey, 2026-06-17.*

## Context **[REQUIRED]**

Packaging determines what the Catalog stores and the Runtime loads. It was deferred until the execution model was settled; with [[WEIR-A-0002]] decided (dylib for trusted, WASM-first for open) and [[WEIR-A-0014]] decided (declarative manifest → codegen), the format set follows directly.

## Decision **[REQUIRED]**

**Plural artifact formats keyed to the execution primitive, with the declarative manifest as a source that codegens to them:**

| Artifact | For | Notes |
|----------|-----|-------|
| **Signed native dylib** (`fidius`) | trusted / first-party connectors ([[WEIR-A-0002]] dylib path) | ed25519-signed (`fidius` signing); includes the PyO3 bundle for first-party Python |
| **WASM component** (WASI/WIT) | "open" / community connectors ([[WEIR-A-0002]] WASM-first) | capability-isolated; signed |
| **Declarative manifest** (source, not runtime artifact) | long-tail authoring | **codegens** to a dylib or WASM artifact ([[WEIR-A-0014]]) — it is the *source*, not a separately-hosted format |

- **OCI image is rejected as a connector packaging format** (the OS-image weight `WEIR-A-0002` rejects). OCI may still appear at the *agent/deployment* layer ([[WEIR-A-0023]]), not as a connector artifact.
- All artifacts are **signed** (ed25519 via `fidius`) and the **trust→format mapping** is enforced by the catalog/signing policy ([[WEIR-A-0002]] governance hook): dylib gated to first-party/trusted-signed, open defaults to WASM.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Verdict |
|--------|---------|
| dylib (trusted) + WASM component (open) + manifest-as-source (chosen) | **Chosen** — matches the two execution primitives + the codegen authoring model |
| Single uniform format | Rejected — can't serve both the trusted-native and open-isolated primitives |
| OCI image as connector format | Rejected — OS-image tax (NFR-RT-2); reserved for the deployment layer |

## Rationale **[REQUIRED]**

Packaging is a direct consequence of [[WEIR-A-0002]]/[[WEIR-A-0014]]: one artifact form per execution primitive, the manifest as the codegen source. Nothing new to decide; this ADR records the determined outcome and the signing/trust mapping.

## Consequences **[REQUIRED]**

### Positive
- Catalog/Runtime handle exactly two artifact kinds (dylib, WASM) + a manifest source; signing is uniform.

### Negative
- Plural formats add some Catalog/Runtime handling (two loaders, already implied by [[WEIR-A-0002]]'s two primitives).

### Neutral
- Storage backend for these artifacts is [[WEIR-A-0018]]; versioning/compat is [[WEIR-A-0019]].
