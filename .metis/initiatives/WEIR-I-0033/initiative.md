---
id: tech-debt-a-manifest-compiler
level: initiative
title: "Tech-debt: a manifest compiler module"
short_code: "WEIR-I-0033"
created_at: 2026-07-08T14:58:59.615540+00:00
updated_at: 2026-07-08T14:58:59.615540+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: M
initiative_id: tech-debt-a-manifest-compiler
---

# Tech-debt: a manifest compiler module Initiative

> **Tech-debt ticket** (2026-07-08 architecture review, "Worth exploring"). Parked in discovery on purpose — its
> exit criterion is a **decision to promote for fix**, not the fix itself.

## Context

Turning a declarative manifest into what the guest actually receives is a chain of small JSON transforms spread
across four crates: `weir-importer` builds the `MappingSpec` (`map_transforms`); `weir-app`'s
`manifest_stream_to_config` flattens a stream into ~30 flat string keys and **smuggles the transform mapping under a
magic `__mapping` key**, with `dest_object_to_config` as a near-identical destination twin (the two auth `match`
blocks are duplicated); `merge_config` layers user config over the baked base; then `work_spec` → `extract_mapping`
*strips* `__mapping` back out and re-attaches it as a typed `ConfiguredStream.mapping`.

So the mapping is serialized into a string as a magic key by one function purely to be stripped out by another — an
implicit side-channel through stringly-typed config, its field names existing only as untyped string literals that
must match the runtime parser and the codegen'd guest. Explore verdict: **scattered**; the `__mapping` round-trip is
a locality smell; deletion test *concentrates*. This is one instance of the review's cross-cutting theme —
*stringly-typed contracts crossing crate boundaries with no shared schema* — and it's coupled to [[WEIR-I-0034]]:
`from_auth_config` is the **reader** of the very auth-scheme vocabulary this baker **writes**.

## Goals & Non-Goals

**Goals:**
- Reach a **decision**: is a single "manifest compiler" module worth extracting now?

**Non-Goals:**
- Building it. That belongs to the promoted initiative *if* the decision is yes.

## Detailed Design

*Decision input.* If promoted: a `manifest::compile(manifest, stream, user_cfg) -> (GuestConfig, MappingSpec,
credential-scheme)` returning **typed** outputs — eliminating the `__mapping` embed/extract dance, giving the two
duplicated auth `match` blocks one home, and turning the stringly-typed hop into a checked interface. Best sequenced
alongside [[WEIR-I-0034]] since they share the auth-scheme vocabulary.

## Alternatives Considered

- **Do nothing.** The chain works; the cost is locality + the untyped side-channel. A valid decision outcome.
- **Only de-duplicate the two auth `match` blocks.** Smaller win, doesn't remove the `__mapping` round-trip.

## Implementation Plan

Single step — **decide**:
- [x] **Make a decision to promote for fix**: either (a) promote to a fix initiative (extract the manifest compiler,
  likely with [[WEIR-I-0034]]), or (b) close, recording the reason in an ADR so future reviews don't re-suggest it.

## Decision (2026-08-17)

**Promote — post-alpha, combined with [[WEIR-I-0034]]'s auth-vocabulary consolidation** as one typed-contract refactor: `manifest::compile(manifest, stream, user_cfg) -> (GuestConfig, MappingSpec, credential scheme)` with typed outputs, killing the `__mapping` side-channel and giving the duplicated auth `match` blocks one home. The 2026-08-16 alpha review independently re-identified this stringly-typed plumbing as the root cause of the silent-typo failure class — the third time it has surfaced, confirming promotion. **Not alpha-gating:** the alpha mitigation is creation-time validation ([[WEIR-T-0166]], which rejects unknown connectors and config-shape errors at POST /connections). Sequence the refactor after the alpha-cut initiatives, before opening manifest authoring to outside contributors. This ticket's exit criterion is met.
