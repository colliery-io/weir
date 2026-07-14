---
id: 001-connector-execution-isolation-model
level: adr
title: "Connector execution & isolation model"
number: 1
short_code: "WEIR-A-0002"
created_at: 2026-06-17T02:11:46.037266+00:00
updated_at: 2026-06-17T17:00:04.678450+00:00
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

# ADR-0002: Connector execution & isolation model

**Status:** Decided (2026-06-17). **Execution-primitive conclusion REVISED by [[WEIR-A-0030]]** (WASM-always,
2026-06-22): the *Review trigger* below **fired** — WASM is faster on the v1 streaming surface
([[WEIR-A-0029]]), so weir collapses to a **single WASM primitive**; the native dylib path is retired. What
**stands**: the **isolation model** (topological tenancy + WASM capability isolation, defense-in-depth) and
the **trust governance hook** — now selecting *isolation posture + origin*, not a different execution
primitive. *Raised by: [[WEIR-S-0005]] Connector Runtime, [[WEIR-S-0006]] Connector Contract & SDK.*
*Decision-maker: Dylan Storey, 2026-06-17.*

## Context **[REQUIRED]**

Connectors run as orchestrated services against a transport-neutral contract ([[WEIR-A-0014]]). The decision is the *execution/isolation primitive* — how connector code is hosted and isolated when it runs. It gates the Runtime, SDK, Catalog, packaging ([[WEIR-A-0015]]), and migration together, so it is first-wave.

Ratified constraints that bind it:
- **NFR-RT-1**: untrusted connector code *cannot crash the host or read another tenant's secrets.*
- **NFR-RT-2**: lean footprint — per-connector overhead negligible vs. transfer time; *no OS-image tax.*
- **NFR-RT-4**: fast cold start.
- **NFR-CT-3**: the contract is *transport-neutral*.
- **NFR-SM-1 / [[WEIR-A-0013]]**: plaintext secrets must not leak; redeemed inside the isolation boundary.

Two reframings settled the design:
1. **Tenancy is topological.** Tenancy lives at the data plane; we do not co-locate two tenants in one OS space. Tenant↔tenant isolation comes from *placement*, not from sandboxing every call.
2. **fidius now ships a language-agnostic WASM branch** (sandboxed wasmtime/component host, capability-gated imports) alongside the native dylib path. So weir has *two* real execution primitives behind one contract, and can pick per **trust tier**.

## Decision **[REQUIRED]**

> **⚠️ Revised by [[WEIR-A-0030]] (WASM-always, 2026-06-22).** The *two-primitive* design below collapsed to
> **one**: WASM for **all** connectors; the native dylib FFI path (and first-party PyO3-in-process) is
> retired. Read the dylib-specific bullets as **historical**. Trust tiers now select **isolation posture +
> origin**, not a different execution primitive. The **isolation model** (topological tenancy + WASM
> capability isolation) and the **governance hook** (trust → posture) remain in force.

**Two fidius execution primitives, selected by trust tier, behind one transport-neutral contract; isolation = WASM capability-isolation (open connectors) + topological partitioning (tenancy), as defense-in-depth.**

- **Trusted / first-party path — native dylib FFI (default for code we own).** Native (Rust) connectors and the high-throughput DB/warehouse core ([[WEIR-A-0016]], the Arrow bulk path) run as signature-verified `fidius` dylib plugins in-process. First-party Python runs here via the **PyO3 boundary**. Leanest, fastest; reserved for code whose provenance we control.
- **Open / community path — WASM-first (strongly recommended for third-party).** "Open" connectors run on `fidius`'s **language-agnostic WASM branch** (sandboxed wasmtime + WASI/WIT, capability-gated host imports) — capability-isolated *at the primitive*, so a faulting/hostile connector traps instead of compromising the agent. Any source language that compiles to WASM (Rust, Python via componentize-py, Go, …) is supported.
- **One contract over both.** Both primitives satisfy the same `fidius` `Connector` contract ([[WEIR-A-0014]]) via the executor abstraction; transport is a hosting detail.
- **Isolation, defense-in-depth:** topological partitioning (the agent fleet — tenancy at the data plane; protects the in-process dylib path and cross-tenant secrets) **plus** WASM capability isolation for open connectors. The fleet **collapses to a single in-process agent on single-node** (NFR-DP-1 / NFR-SE-5) and fans out for the multi-tenant periphery.
- **Container hardening is now optional ops, not a required mechanism** — the fidius WASM branch supersedes it as the primary agent-host hardening layer (the earlier "Path 2"). Container deployment of agents remains available as an operational choice.

**Governance hook (trust → primitive).** The catalog + signing policy encodes the mapping: the **dylib path is gated to first-party / trusted-signed** connectors; **open submissions default to WASM**. Carried into [[WEIR-A-0018]] (artifact storage), [[WEIR-A-0019]] (versioning/compat) and [[WEIR-A-0005]] (open-core boundary).

**Dependency hygiene.** `fidius` and `diesel-dual-db` are **dependencies** (standalone libraries). `cloacina` is **not** — its agent-fleet + outbox **patterns** are reimplemented in weir's Sync Engine ([[WEIR-A-0010]], [[WEIR-A-0027]]).

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Isolation | Footprint / throughput | Verdict |
|--------|-----------|------------------------|---------|
| Trust-tiered: dylib (trusted) + WASM-first (open), topological tenancy (chosen) | ✓ capability (open) + placement (tenancy) | ✓ native speed where trusted; lean | **Chosen** |
| Single native-FFI primitive, topological isolation only | ◑ placement only; in-process open code unisolated on its agent | ✓ leanest | Superseded — WASM now gives cheap capability isolation for open code |
| WASM as the *default for all* connectors | ✓ | ✗ pays WASM overhead on trusted bulk Arrow path for no trust benefit | Rejected — dylib stays default for trusted/bulk |
| Container per connector as default | ✓ | ✗ fails NFR-RT-2 | Rejected as default; container kept as optional agent-level ops |

## Rationale **[REQUIRED]**

- Matching primitive to trust tier gives the best of both: native speed where we own the code (and on the bulk Arrow path), capability isolation where we don't.
- fidius's WASM branch being **language-agnostic** means "open ⇒ WASM-first" is achievable across languages, not just Rust — the long tail (incl. Python compiled to WASM) gets primitive-level isolation.
- Topological partitioning still earns its keep: it isolates tenants for the in-process dylib path and bounds secret blast radius regardless of primitive. WASM + topology together is a strong, layered posture.
- One contract keeps the Runtime/SDK/Catalog oblivious to which primitive ran a connector.

## Consequences **[REQUIRED]**

### Positive
- Open connectors are capability-isolated at the primitive; trusted/bulk connectors keep native throughput. Defense-in-depth (capability + topological).
- The "two paths" defense-in-depth question is **resolved** — fidius WASM is the host-hardening mechanism; container hardening drops to optional.
- Single-node still collapses to one in-process binary.

### Negative
- Two execution backends to build/test behind the contract (dylib + WASM); the executor seam must keep connectors oblivious to which runs them.
- Tenant isolation still depends on **correct fleet placement** — an invariant the Engine ([[WEIR-S-0004]]) must enforce.
- **Python→WASM toolchain maturity** is the residual risk for native-dep-heavy Python *open* connectors; the **PyO3-native dylib path remains the first-party fallback** until the Python→WASM story is solid for a given connector.

### Neutral
- Couples to packaging ([[WEIR-A-0015]]): dylib artifact + WASM component (+ PyO3 bundle for first-party Python). Secret redemption ([[WEIR-A-0013]]) happens on the agent within the tenant domain.

## Implementation validation (follow-ups, not open decisions)
- Confirm the contract ([[WEIR-A-0014]]) round-trips over **both** primitives via the executor seam (one trusted dylib + one WASM open connector).
- Confirm secret-handle redemption ([[WEIR-A-0013]]) works inside the WASM/agent boundary, plaintext never reaching the control plane.
- Confirm single-node collapse to one in-process agent (NFR-DP-1 / NFR-SE-5).
- Validate Python→WASM for a representative C-extension-using connector; document where PyO3-native fallback applies.

### Review trigger
- Revisit if the WASM path matures/performs well enough to become the default even for trusted bulk connectors (would simplify to a single primitive). — **✅ FIRED 2026-06-22 → [[WEIR-A-0030]]** (WASM is faster on the streaming surface; collapsed to the single WASM primitive).
