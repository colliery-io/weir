---
id: 001-connector-distribution-source-only
level: adr
title: "Connector distribution: source-only — low-code manifests run on a shared declarative runtime, full-code crates built where they execute"
number: 1
short_code: "WEIR-A-0032"
created_at: 2026-06-24T12:41:54.466962+00:00
updated_at: 2026-06-24T12:43:19.024600+00:00
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

# ADR-1: Connector distribution: source-only — low-code manifests run on a shared declarative runtime, full-code crates built where they execute

## Context **[REQUIRED]**

weir has two kinds of connector and needed a single, coherent answer to **how connectors are distributed
and onboarded**:
- **Full-code** — a Rust crate compiled to a WASM component ([[WEIR-A-0030]] WASM-always), e.g. `rest`,
  `postgres` (now `crates/connectors/*`).
- **Low-code** — a declarative `manifest.yaml` (auth / pagination / streams / cursor), the Airbyte-style DSL
  the parity arc ([[WEIR-I-0008]]) translates. A manifest is **data, not code**.

The Publication Contract ([[WEIR-A-0018]]) framed ingress around **crates** (curated allowlist / community
crates.io crawl / direct import). That leaves the low-code channel unspecified, and the catalog ingress
([[WEIR-I-0010]]) currently supports only `LocalCrate` (compile-on-import) and `Folder` (already-staged
package). Two candidate distribution models were on the table:
1. **Prebuilt-binary hub** — codegen + compile centrally, ship the `.wasm`/`.cwasm` to operators.
2. **Interpreter runtime** — one generic "declarative runtime" wasm; a connector is a manifest fed to it as
   config (Airbyte's `source-declarative-manifest` model). Our generalized `rest` (config-driven HTTP)
   is the embryo of this.

The crux: a prebuilt component — especially the AOT `.cwasm` — **pins the consumer to the build-time host
stack** (wasmtime version, fidius version, contract hash). Portable in theory, coupled in practice: ship it
to a host on a different stack and you get silent build/execute mismatches.

## Decision **[REQUIRED]**

**Connectors distribute as SOURCE/DATA; no prebuilt connector binary crosses a machine boundary.** The
governing principle is **build where you execute** — so build-env always equals run-env and the mismatch
class is eliminated by construction. **Low-code mirrors Airbyte: a manifest is data, run by a shared
runtime — no per-connector compile. Full-code is a per-connector crate, compiled at onboard.**

1. **Low-code runs on a shared declarative runtime (interpreter).** A low-code connector *is* its
   `manifest.yaml` — pure data. Onboarding = register the manifest; at run time a single shared
   **declarative-runtime wasm component** (today's generalized `rest`, grown to the full declarative
   surface) reads the manifest and executes it. **No per-connector compile.** The manifest is the distributed
   unit (git / a weir hub). This is exactly Airbyte's `source-declarative-manifest` model.
2. **Full-code is a per-connector Rust crate** — for what the DSL can't express. Distributed as **source via
   crates.io**, compiled at onboard in the execution env (`crates/connectors/*`). weir may offer a *one-time*
   **scaffold export** from a manifest (you own the emitted crate after — **no regeneration, no patching**).
3. **Codegen-to-crate is REJECTED as the low-code mechanism.** It buys nothing over the interpreter: same
   declarative-surface ceiling (codegen can't express anything a manifest can't), **no editability** (a
   generated-then-compiled crate isn't operator-editable; editing means regenerate-vs-patch, which we won't
   build), and it **forces a compile at every onboard** — the opposite of what we want. The generated-crate
   idea survives only as the optional one-time scaffold export in §2.
4. **The shared runtime ships *with weir*** (built in weir's own environment as part of the platform), so it
   is not a per-connector binary crossing machines — consistent with build-where-you-execute. The *only*
   onboard compile is the full-code crate path.
5. **The catalog ([[WEIR-I-0010]])** registers both kinds: a **manifest entry** (runs on the declarative
   runtime, no compile) and a **full-code package** (compiled). It gains manifest-aware ingress.
6. **No prebuilt-binary hub.**

This **refines [[WEIR-A-0018]]**: ingress paths now cover *manifests* (low-code, primary) in addition to
crates (full-code); the distributed artifact is always source/data.

> **Revision (2026-06-24):** this ADR originally chose *codegen-to-crate* for low-code. Revised to the
> **interpreter** model after establishing that (a) codegen and the interpreter share the same capability
> ceiling, (b) codegen-at-import yields no editable artifact, and (c) the interpreter uniquely satisfies
> *both* original goals — no onboard compile **and** no build/execute coupling — which codegen does not.
> The distribution principle (source/data, no prebuilt binaries) is unchanged; only the low-code *mechanism*
> flipped from codegen to interpreter.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| **Interpreter runtime for low-code** (chosen) | No onboard compile (manifest is data); one runtime to build/harden; manifest = trivially distributable data; Airbyte-proven; builds on existing `rest` | Per-run interpretation cost (negligible — connectors are I/O-bound); the runtime must grow to cover the declarative surface | Low | Med (= I-0008, but in the runtime) |
| **Codegen-to-crate for low-code** | Per-connector "real" artifact | Same capability ceiling as interpreter (no gain); not editable (regenerate-vs-patch); **forces a compile at every onboard**; generator + templates to maintain | Med | High |
| **Prebuilt-binary hub** | No onboard compile | Pins consumer to build-time wasmtime/fidius/contract; silent build/execute mismatch; binary-hosting infra | High | High |
| **Full-code per-connector crate** (chosen, for the non-declarative tail) | Unbounded capability; real signable/versioned crate | Compile at onboard; author effort per connector | Low | — |

## Rationale **[REQUIRED]**

"Build where you execute" makes the portability problem disappear rather than managing it: the only things
that travel are **data** (a manifest) or **source** (crate), never a binary tied to a build-time host stack.

For *low-code*, the interpreter wins decisively once two myths are dropped: codegen does **not** raise the
capability ceiling (both are bounded by the declarative surface a manifest can express), and codegen-at-
import does **not** produce something editable (you'd need regenerate-vs-patch machinery we refuse to build).
With those gone, the interpreter uniquely delivers *both* original goals — **no compile at onboard** (the
manifest is data) **and no build/execute coupling** (the shared runtime ships as part of weir, built in its
own environment) — which codegen achieves neither of. It's also Airbyte's battle-tested model
(`source-declarative-manifest`) and it builds directly on our generalized `rest`. The parity work
([[WEIR-I-0008]]) is unchanged in substance — implement auth / pagination / partition-routers / transforms —
it simply lives **in the shared runtime** instead of in a code generator (one runtime vs generator+templates
= *less* code). Full-code remains the escape hatch for the non-declarative tail, as a hand-written crate.

## Consequences **[REQUIRED]**

### Positive
- No build/execute arch/version mismatch — ever (the failure class is designed out).
- **Low-code onboarding is instant** (register a manifest; no compile) — the best operator/onboarding UX.
- Distribution artifacts are tiny + portable (a YAML; or crate source) — trivial to register, sign, audit.
- The declarative runtime (grown `rest`) is the center of gravity; [[WEIR-I-0008]]'s coverage targets it.

### Negative
- The shared runtime must be grown to the full declarative surface ([[WEIR-I-0008]]); a manifest only does
  what the runtime supports (gaps are surfaced via the preview/tier report, not silently emitted).
- Full-code onboarding still does a local `cargo build` (the exec env needs the toolchain, or a weir-managed
  builder running *in* that env) — accepted, and scoped to the non-declarative tail only.

### Neutral
- A manifest registry (git / weir hub) is a new distribution surface to define; crates.io covers full-code
  source. The catalog stays the local registry (manifest entries + full-code packages).
- Today's `LocalCrate`/`Folder` ingress remains valid (full-code / dev); a **manifest ingress path** (data →
  runtime, no compile) must be added — current backend/UI onboarding interfaces are crate-centric and need
  reconciling (see [[WEIR-S-0015]]).
- The earlier "codegen is the center of gravity" framing is retired; the shared runtime is.

## Review Schedule **[CONDITIONAL: Temporary Decision]**

### Review Triggers
- A weir-managed remote build/execution plane appears where build-env == run-env can be *guaranteed* server-
  side (could reopen a prebuilt-artifact cache **scoped to a pinned host stack**, never cross-stack).
- Onboard compile time becomes a real operator pain point (would motivate a per-host compile cache, not
  cross-host binaries).
