---
id: 001-wasm-always-connector-packaging
level: adr
title: "WASM-always connector packaging"
number: 1
short_code: "WEIR-A-0030"
created_at: 2026-06-22T19:15:44.303963+00:00
updated_at: 2026-06-22T19:16:39.604542+00:00
decision_date:
decision_maker:
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0030: WASM-always connector packaging

**Status:** Decided (2026-06-22, Dylan Storey). **Supersedes [[WEIR-A-0016]]** (native connectors as
first-class); **revises [[WEIR-A-0002]]** (its "native dylib is the trusted-default execution path"
conclusion). *Raised by: [[WEIR-S-0014]] Connector Developer Experience.*

## Context **[REQUIRED]**

[[WEIR-A-0002]] / [[WEIR-A-0016]] made the **native (cdylib) FFI path the default for trusted/first-party
connectors**, justified by bulk/high-throughput (DB/warehouse, Arrow) performance, with WASM reserved for
open/untrusted connectors. Two things changed that premise:

1. The **v1 streaming contract** ([[WEIR-A-0029]]) makes *all* data movement — incremental streaming **and**
   bulk Arrow record-batches — ride the **same streaming surface** (`read → Stream<ReadMessage>`,
   `write` client-streams `RecordBatch`). There is no longer a separate "bulk path" that only cdylib serves.
2. On that streaming surface, **WASM is measurably *faster* than cdylib** for streaming plugins. The
   throughput argument that justified native cdylib no longer holds — wasm wins on the hot path too.

With one packaging target we also collapse the native-vs-wasm split that was complicating the catalog
([[WEIR-I-0010]]) and the SDK build targets ([[WEIR-S-0014]]).

## Decision **[REQUIRED]**

**WASM is the single connector packaging + execution target. There is no native (cdylib) connector path.**

- Every connector — first-party **and** community — is a **wasm-component package** ([[WEIR-A-0015]]),
  loaded via `from_wasm_package` and executed in the wasm runtime.
- "First-party / trusted" is now an **origin/trust attribute** (bundled-with-weir vs external), **not** a
  packaging kind. The catalog records origin; the isolation tier ([[WEIR-A-0002]]) keys off it.
- The host/engine/orchestrator remain native Rust; **only connectors** are wasm.
- All authoring tiers ([[WEIR-S-0014]]: manifest / Rust SDK / Python SDK / AI / import) emit **wasm**.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk Level | Implementation Cost |
|--------|------|------|------------|-------------------|
| **WASM-always (chosen)** | One packaging path; faster streaming surface; uniform sandbox/isolation; collapses catalog `kind`; one SDK target | Migrate existing native connectors; lose in-process cdylib | Medium | Medium-High (migration) |
| Native cdylib default + wasm for untrusted (A-0016, superseded) | In-process speed *was* assumed | Two paths; dual SDK surface; cdylib slower on the streaming surface; trust bifurcation in packaging | — | — |
| Dual-target, author chooses | Flexibility | Worst of both: two build/test/dist paths forever | High | High |

## Rationale **[REQUIRED]**

Once the streaming contract unified bulk + incremental onto one surface, the only remaining reason to keep
cdylib was raw throughput — and wasm is *faster* there for streaming plugins. So native cdylib has no
advantage left, while WASM gives a single packaging/distribution path, uniform isolation, and a much simpler
catalog + SDK. Trust/first-party — the legitimate concern A-0016 raised — is preserved as an origin
attribute + isolation tier, not a separate packaging mechanism.

## Consequences **[REQUIRED]**

### Positive
- One artifact, one build, one distribution path, one SDK target — the [[WEIR-S-0014]] "many front doors,
  one artifact" principle becomes literally true.
- Faster streaming; uniform wasm sandbox for every connector; catalog `kind` collapses to `origin/trust`.

### Negative
- **Migration:** the existing in-process/native connectors (echo, slow, faulty, arrow-sink, rest,
  postgres) and the **dylib codegen path** (`weir-codegen/dylib.rs`, `ConnectorHandle::native_in_process`,
  the cdylib fidius backend) must move to wasm. Sizable, sequenced separately.
- `ConnectorRef::Native` is retired; `ConnectorRef` becomes wasm-package + origin. Connection rows migrate.

### Neutral
- **Supersedes [[WEIR-A-0016]]**; **[[WEIR-A-0002]] needs a follow-up revision** to restate its execution
  default as wasm (its isolation *model* stands). Reframes [[WEIR-I-0010]] catalog `kind` → `origin`.
- Python connectors compile to/run as wasm rather than via the PyO3 in-process boundary.

## Review Schedule **[CONDITIONAL: Temporary Decision]**

{Delete if decision is permanent}

### Review Triggers
- {Condition that would trigger review 1}
- {Condition that would trigger review 2}

### Scheduled Review
- **Next Review Date**: {Date}
- **Review Criteria**: {What to evaluate}
- **Sunset Date**: {When this decision expires if not renewed}
