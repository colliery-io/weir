---
id: 001-reverse-etl-destinations-on-a
level: adr
title: "Reverse-ETL destinations on a shared declarative runtime"
number: 34
short_code: "WEIR-A-0034"
created_at: 2026-07-04T03:12:37.299999+00:00
updated_at: 2026-07-04T03:12:37.299999+00:00
decision_date: 2026-07-04
decision_maker: dylan.storey@gmail.com
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-34: Reverse-ETL destinations on a shared declarative runtime

## Context **[REQUIRED]**

[[WEIR-I-0007]] (first-class reverse ETL) needs operational-SaaS **destinations** — HubSpot and
Salesforce in the open core — so a sync can be `warehouse → SaaS upsert`, not just `API → warehouse`.
Its S1 (the in-flight mapping stage, [[WEIR-A-0026]]) is now built (shipped as [[WEIR-T-0071]]); the open
question is **how the destination connectors themselves are built.**

The initiative was written 2026-06-21 and its S2 specifies *"declarative destination **codegen** … mirroring
the existing source codegen"* — i.e. manifest → generated Rust → a compiled wasm crate per destination.
That mirror target is now **stale**: [[WEIR-A-0032]] pivoted **sources** away from per-connector codegen to
a **shared declarative runtime** (the `rest` crate — one wasm guest that interprets a manifest as config at
run time). All ~34 sources run on that one runtime; `weir-codegen` is the legacy path. Continuing the
destination plan as codegen would point reverse-ETL at an architecture the project deliberately left.

## Decision **[REQUIRED]**

**SaaS destinations are built as a shared declarative destination runtime**, mirroring the source model of
[[WEIR-A-0032]]: **one** wasm `wasi:http` destination guest that **interprets a destination manifest at run
time** (per-object endpoint + method, upsert/business key, field mapping, batching, auth scheme) and does
the client-streaming `write`. HubSpot, Salesforce, and the SaaS long tail become **manifests** — no
per-connector codegen. The `weir-codegen` destination path is not pursued.

The **full-code crate escape hatch** of [[WEIR-A-0032]] remains for a genuinely bespoke destination that the
declarative schema can't express (e.g. Salesforce Bulk API 2.0 with async job polling) — shared runtime for
the common shape, full-code crate for the rare exception, never silent breakage ([[WEIR-A-0020]]).

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| **Shared declarative destination runtime** (chosen) | Ship a destination = a text manifest, no build; one runtime fix improves every destination (as retry/backoff did for all sources this cycle); reuses host-auth ([[WEIR-A-0033]]), retry/backoff, importer, mapping stage; one product mental model; SaaS writes are uniform (POST/PATCH object + upsert key + batch + per-record dead-letter) | Runtime can only do what its manifest schema expresses; truly bespoke write logic needs a runtime extension or the full-code hatch | Low | Medium (one new write-side guest + a manifest dest schema) |
| **Per-connector destination codegen** (original June plan) | Arbitrary per-connector logic; no interpretation layer | Reintroduces build-per-connector (toolchain, compile, binary distribution) that [[WEIR-A-0032]] removed for sources; two mental models (interpreted sources, compiled dests); template fixes require regen+recompile across all dests | Medium | High (codegen generator + per-connector build pipeline) |
| **Hand-write each SaaS destination** | Fastest for the first one | Doesn't scale to the SaaS long tail; every new dest is engineering, not config | Low | High (per connector, forever) |

## Rationale **[REQUIRED]**

The tradeoff is lopsided given where sources already landed. The decisive properties:

1. **"Add a connector = ship a manifest, no release"** — the demo's headline and the ecosystem cold-start
   lever — is a property of the shared-runtime choice. Codegen for destinations would forfeit it on the
   write side.
2. **Leverage compounds.** This cycle's Basic auth / Link-header / retry+backoff landed **once** and every
   source inherited them. The same holds for destinations: one runtime, universal improvement. Codegen
   fragments that into N regenerations.
3. **Direct reuse of what already exists.** Host-side credential injection ([[WEIR-A-0033]] — secrets never
   enter the guest, critical for Salesforce OAuth), transient retry/backoff (SaaS APIs rate-limit hard), the
   importer, and the mapping stage all carry over to the write side.
4. **SaaS writes are uniform** — arguably more regular than reads (no pagination/partition variety), so they
   express cleanly as config.
5. **One architecture** for source and destination halves of the product is cheaper to build, teach, and
   maintain than two.

## Consequences **[REQUIRED]**

### Positive
- HubSpot, Salesforce, and future SaaS destinations are manifests on one runtime — the reverse-ETL analogue
  of the 34-source catalog.
- Reverse-ETL inherits host-auth, retry/backoff, and mapping immediately; less new surface than the June plan.
- Consistent onboarding/importer/UI story across sources and destinations.

### Negative
- A destination whose write semantics exceed the manifest schema needs a runtime extension (reported per
  [[WEIR-A-0020]]) or the full-code crate hatch — not silent failure.
- The manifest **destination schema** is new design surface to get right (objects, upsert keys, batching).

### Neutral
- The destination runtime is a **new** wasm guest, not a reuse of the `rest` **source** guest — the write
  side is different plumbing (client-streaming `write`, upsert-by-business-key, per-record dead-letters,
  batching). It is built the *same way* (interpret a manifest), reusing the host-side machinery.
- Salesforce OAuth refresh extends the existing host-side token-provider pattern ([[WEIR-A-0033]]); it is not
  a new in-guest secret path.

## Relationships

- **Builds on:** [[WEIR-A-0032]] (source-only distribution / shared runtime — this extends it to the write
  side), [[WEIR-A-0033]] (host-side credential injection), [[WEIR-A-0026]] (in-flight mapping), [[WEIR-A-0011]]
  (upsert / checkpoint semantics), [[WEIR-A-0029]] (streaming `write` contract).
- **Updates:** [[WEIR-I-0007]] S2 — replaces "declarative destination codegen" with the shared declarative
  destination runtime; S1 already delivered by [[WEIR-T-0071]].
