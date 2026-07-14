---
id: 001-secret-resolution-path
level: adr
title: "Secret resolution path"
number: 1
short_code: "WEIR-A-0013"
created_at: 2026-06-17T02:12:07.628339+00:00
updated_at: 2026-06-17T21:03:51.145652+00:00
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

# ADR-0013: Secret resolution path

**Status:** Decided. *Raised by: [[WEIR-S-0004]] Sync Engine, [[WEIR-S-0010]] Secrets Manager, [[WEIR-S-0005]] Connector Runtime.* *Decision-maker: Dylan Storey, 2026-06-17.*

## Context **[REQUIRED]**

Connector runs need credentials, but plaintext must never touch logs, config, or the control plane. Where in the path are secrets resolved? The execution model ([[WEIR-A-0002]]) is now decided (dylib + WASM on a topologically-partitioned agent fleet), which fixes *where* "the runtime" is: on the agent, inside the tenant's domain.

## Decision **[REQUIRED]**

**Handle-based, redeemed on the agent.** The Engine passes only a **short-lived secret handle** with the work unit; the **runtime host on the agent redeems the handle** from the Secrets Manager ([[WEIR-A-0021]]) at run time and injects credentials into the connector. The Engine/control plane never sees plaintext.

- Redemption happens **on the agent, within the tenant domain** ([[WEIR-A-0002]] topological isolation) — plaintext never crosses back to the control plane.
- Applies to **both execution primitives**: the runtime host redeems and injects regardless of whether the connector runs as a signed dylib or a capability-isolated WASM component (for WASM, credentials are passed in via the host-controlled, capability-gated import surface).

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| Runtime redeems handle on the agent (chosen) | Minimal blast radius; control plane plaintext-free; same model for dylib + WASM | Agent needs a Secrets Manager access path | **Chosen** |
| Engine resolves and passes plaintext | Simpler dispatch | Plaintext transits the control plane — larger blast radius | Rejected |

## Rationale **[REQUIRED]**

Keeping plaintext out of the control plane shrinks the blast radius to the isolated, tenant-scoped agent — consistent with the isolation goals of [[WEIR-A-0002]] and the secrets guarantees of [[WEIR-S-0010]].

## Consequences **[REQUIRED]**

### Positive
- Control plane is plaintext-free; tight, tenant-scoped secret blast radius; uniform across dylib and WASM.

### Negative
- Requires an agent→Secrets Manager handle-redemption path, and (for WASM) credential injection via the capability-gated import surface.

### Neutral
- Coupled to the execution/isolation model ([[WEIR-A-0002]]) and the secrets backend abstraction ([[WEIR-A-0021]]).
