---
id: 001-catalog-artifact-storage-backend
level: adr
title: "Catalog artifact storage backend"
number: 1
short_code: "WEIR-A-0018"
created_at: 2026-06-17T02:12:19.718532+00:00
updated_at: 2026-06-22T20:33:11.530705+00:00
decision_date:
decision_maker:
parent:
archived: false

tags:
  - "#adr"
  - "#phase/discussion"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0018: Catalog artifact storage backend

**Status:** Discussion → **Publication Contract v1 decided (2026-06-23)**. The publication + ingress
contract is settled (three ingress paths, `[package.metadata.weir]`, the pull→compile→register pipeline —
see *Publication Contract (v1)* below). **Still open:** the prebuilt-artifact / non-Rust channel
(OCI/object-store). *Coupled to [[WEIR-A-0015]] (packaging), [[WEIR-A-0030]] (WASM-always),
[[WEIR-A-0019]] (versioning). Raised by: [[WEIR-S-0007]] Catalog; realized by [[WEIR-I-0010]].*

## Context **[REQUIRED]**

The Catalog stores and serves versioned connector artifacts to the Runtime at dispatch. The backend must support integrity/provenance, cache-friendly reads, and first-party/community/private catalogs.

## Decision **[REQUIRED]**

**Two channels; the first-party/Rust one is decided.**

**1. First-party / Rust tier — crates.io source distribution (decided).**
- Connectors are **workspace crates** published to crates.io as **`weir-connector-<name>`** at a semver
  ([[WEIR-A-0019]]) when ready — the workspace crate name *is* the namespace.
- crates.io carries the **source guest crate** (the `fidius-guest`-shaped crate, per [[WEIR-I-0011]]), **not**
  a built artifact.
- **Flow:** the operator **pins `(name, version)` in the UI** → **weir (the app) orchestrates
  pull-from-crates.io + compile-to-`wasm32-wasip2` + register in the catalog** ([[WEIR-I-0010]]) — *"making
  the pin real."* The build mechanism is the [[WEIR-I-0011]] S1 seam (`weir-wasm-testkit`) elevated to an
  app capability.
- **Catalog index is DB-backed** (the [[WEIR-I-0010]] `connectors` table: persisted `(name,version)` + spec
  + a pointer to the locally-built wasm in the `connectors/` dir).

**2. Prebuilt-artifact / non-Rust tier — OCI/object-store (open).**
- For non-Rust connectors (Python, arbitrary wasm), toolchain-less/minimal deployments, or signed
  prebuilt-`.wasm` mirroring: an **OCI registry or object store** of built, signed ([[WEIR-A-0015]]) wasm.
  Deferred until those needs are real.

**Requirement:** the pull→compile flow needs a **build toolchain** (cargo + `wasm32-wasip2`) in the weir
deployment; the prebuilt-artifact mode (channel 2) is the alternative for toolchain-less/minimal runtimes.

## Publication Contract (v1) — decided 2026-06-23

A connector enters the catalog by one of **three ingress paths**, all flowing through the **same pipeline**
— *fetch/pull source → compile to `wasm32-wasip2` → snapshot `spec()` → register + cache the wasm in
`connectors/`* (the [[WEIR-I-0011]] build seam elevated to an app capability). **Registration is always an
explicit act**; the allowlist governs *curation / auto-trust*, not whether ingress is possible.

**Unit & required metadata.** One `weir-connector-<name>` crate (a fidius guest, [[WEIR-A-0015]]) → one
wasm component, `cargo publish`ed at a semver ([[WEIR-A-0019]]). It MUST declare `[package.metadata.weir]`
with `contract = <n>` (targeted contract version) + `roles = [...]` — read straight from the manifest for a
cheap **pre-compile** listing + contract-range gate. The full `config_schema` comes from `spec()`
**post-compile** at registration (not duplicated in metadata).

**Ingress paths & `origin`:**
1. **Curated first-party** — a colliery-maintained **allowlist** of `weir-connector-*` crates.io crates →
   `origin = first-party`, trusted, registers normally. The official catalog.
2. **Open community discovery** — weir crawls crates.io (keyword `weir-connector` / `weir-connector-*`
   prefix) to surface community connectors for **awareness/visibility only** → `origin = community`,
   untrusted, **not registerable from the crawl**. To use one, import it (path 3).
3. **Direct import (bring-your-own)** — the user hands weir a crate from **`{ crates.io (name,version) |
   git URL | local path }`**; weir fetches → compiles → registers. **Import IS the explicit trust act** —
   the universal "make it real" verb. `origin = private` (or `community` for a crates.io crate pulled this
   way). The [[WEIR-I-0010]] **folder-scan MVP is the degenerate local-import case**.

**Identity & upsert.** A connector's identity is its **`Cargo.toml` `(name, version)`** — the *same key for
every ingress path* (crates.io publishes its manifest; git/local crates carry their own). **Import is an
upsert on `(name, version)`**: re-importing the same key re-compiles, re-snapshots `spec()`, and replaces
the row + cached artifact. No separate pin/SHA for private. Consequence to note: crates.io versions are
**immutable** (registry-enforced), so first-party/community rows are stable; **`private`/local versions are
mutable-by-reimport** — exactly the ergonomics wanted for *local connector dev* (edit → re-import → refresh
under the same version), but it means a connection that pins a `private` version is not a hard freeze the
way a crates.io pin is ([[WEIR-A-0019]]). Accepted trade: re-import is the operator's explicit, trusted act.

**Trust.** Allowlist = the auto-trusted curated set; community crawl = view-only; everything else enters by
explicit import. Built-wasm **signature/verification enforcement** ([[WEIR-A-0015]]) is deferred.

**Downstream "sync" ([[WEIR-I-0010]]).** = (a) **availability refresh** of the two discovery tracks
(allowlist + crates.io crawl) feeding the browse list, and (b) the **ingress pipeline** above. `origin` is
a 3-value enum (`first-party | community | private`) on the catalog row.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| **crates.io source + app pull/compile/register (chosen, Rust tier)** | Zero new infra; reuses Rust registry; source-auditable; clean pin→real flow; reuses the S1 build seam | Needs a build toolchain in the deployment; compile-on-install latency; Rust-only | Low | Low |
| OCI registry (prebuilt wasm) | Signing/provenance/mirroring; no toolchain at runtime; language-agnostic | New infra; ties artifact to OCI | Low | Low–Med |
| Object store + index | Flexible; cheap | Build provenance/signing ourselves | Medium | Medium |
| DB-backed blobs | Simple single-node | Poor at scale/large artifacts | Medium | Low |

## Rationale **[REQUIRED]**

For the first-party/Rust tier the connectors are *already* workspace crates, so crates.io is a zero-infra
distribution channel and `cargo publish` is the whole release step. Compiling on pull (reusing the
[[WEIR-I-0011]] build seam) keeps WASM-always ([[WEIR-A-0030]]) honest without an artifact store, and the
catalog index stays DB-backed (trivial single-node). Prebuilt-artifact/OCI is kept as the complementary
channel for the cases crates.io-source can't serve (non-Rust, no-toolchain, signed mirrors).

## Consequences **[REQUIRED]**

### Positive
- First-party distribution = `cargo publish weir-connector-<name>`; no artifact infra.
- A clean operator story: **pin in the UI → weir pulls + compiles + registers**; the S1 seam is reused.
- Source-auditable; catalog index trivial (DB-backed).

### Negative
- The pull→compile flow requires a **build toolchain** in the deployment (heavy for minimal/containerized
  runtimes) and adds **compile-on-install latency**. Mitigation: the prebuilt-artifact channel (2).
- Rust-only; non-Rust connectors need channel 2.

### Neutral
- Built-wasm **signing/trust** ([[WEIR-A-0015]]) + **origin** ([[WEIR-A-0030]]) still apply post-compile.
- Versioning/pinning per [[WEIR-A-0019]]; the prebuilt-artifact/non-Rust channel remains **open**.
