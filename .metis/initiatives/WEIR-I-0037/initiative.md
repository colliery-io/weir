---
id: f3-freshness-triple-on-the
level: initiative
title: "F3 — Freshness triple on the connector contract"
short_code: "WEIR-I-0037"
created_at: 2026-07-09T03:19:40.522488+00:00
updated_at: 2026-07-09T03:19:40.522488+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: M
initiative_id: f3-freshness-triple-on-the
---

# F3 — Freshness triple on the connector contract Initiative

> **Re-scoped 2026-08-04 ([[WEIR-A-0040]]).** Originally F3 of the six-part Signal Fabric program; the
> delivery-plane half of that program (F2/F4/F5/F6, the Signal Broker) was withdrawn. F3 is **retained as an
> F1-supporting connector-contract amendment** — the freshness triple serves **resident sources** ([[WEIR-I-0035]])
> directly (a heartbeat + value-timestamp is useful whether or not anything downstream subscribes) and carries the
> deferred F1 `spec()` resident-capability fields (below). The former "fabric shape" framing — snapshot+delta channels,
> subscribe-mid-stream, the [[WEIR-S-0017]] state/delta contract — is **dropped**; freshness is a property a source
> stamps on records and a destination/consumer reads, nothing more.
>
> **Feature request — connector-contract amendment.** **Home: open-core.** No hard dependency, but only fully
> meaningful together with [[WEIR-I-0035]] (F1, live sources). Amends the connector contract
> ([[WEIR-A-0014]] / [[WEIR-A-0029]] / [[WEIR-S-0006]]). Filed in **discovery**.
>
> **Inherited from F1.1 ([[WEIR-T-0137]]).** The connector `spec()` **resident-capability** fields
> (`resident_capable`, `event_reader`) were deferred from F1 into this contract amendment — `ConnectorSpec` is a
> bincode-positional `WitType`, so adding fields is a breaking wire change best bundled with the guest-contract regen
> done here. F3 must add them, and F1's `validate_connection_modes` should then **reject declaring `resident` on an
> incapable connector** (currently only the mode string is validated).

## Context

Extend the connector contract so every emitted record can carry a **freshness triple**, a **new dimension** distinct
from weir's existing sync-checkpoint / cursor state — which tracks *read progress*, not *value staleness*:

1. **Value timestamp** — when the value was true **at the source** (not when weir delivered it).
2. **Liveness heartbeat** — a signal that the producing source is alive and reading, emitted **even when the value
   has not changed**, so a consumer can distinguish "unchanged" from "silent/dead."
3. **Source-declared staleness expectation** — the cadence beyond which the source considers itself to have missed a
   beat (e.g. "I poll every 2.5s; older than ~5s means I've stalled"). Only the source knows its own cadence, so the
   **source declares it**.

These are contract fields a source populates and a destination/consumer reads uniformly across all connectors.

## Goals & Non-Goals

**Goals:**
- A source can emit all three fields; a destination/consumer can read them off any record **uniformly, regardless of
  source type**.
- The **heartbeat is emittable independently of value change** (liveness ≠ new data), and its **absence is
  detectable downstream**.
- **Backward-compatible:** an existing connector that does not populate freshness still runs (sensible defaults or
  explicit "freshness-unsupported").

**Non-Goals:**
- **weir does not interpret the triple.** No "weir decides stale." Mapping the triple to an action
  (`assume_worst` / `assume_pass` / `hold_last`) is the **consumer's** job — explicitly **not weir** (a Swish
  Policy-Engine / shared-client-library concern; see the "Explicitly NOT weir" note below).
- Repurposing checkpoint/cursor fields — this is a **new** dimension, carried alongside existing state semantics.

## Open-Core Boundary

**Wholly open-core.** Carrying and exposing a freshness triple makes weir a better platform for anyone wanting
freshness-aware records, independent of Swish. The line weir must not cross: **weir carries and exposes; it never
interprets.** Missing-signal *mapping* is a Swish concern and is not filed against weir.

## Detailed Design

*(Discovery-phase sketch.)*

- **Contract extension.** Add the triple to the record/message contract ([[WEIR-S-0006]]) — most naturally on the
  streaming `ReadMessage` path from [[WEIR-A-0029]]. The heartbeat likely surfaces as a message variant (or a
  populated-but-value-unchanged record) so liveness can flow without new data.
- **Versionable + defaulted.** Connectors that don't set freshness are valid; the contract signals
  "freshness-unsupported" explicitly rather than fabricating values.
- **Uniform read surface.** A destination/consumer reads freshness identically across connector types — the same
  fields regardless of source, read off the record like any other value.

## Alternatives Considered

- **Overload checkpoint/cursor state to carry staleness.** Rejected — value staleness and read progress are
  different dimensions; conflating them breaks resume semantics and the "unchanged vs. dead" distinction.
- **Let weir compute/interpret staleness.** Rejected — violates the "weir never decides stale" boundary; only the
  source knows its cadence and only the consumer owns the action.

## Acceptance Criteria

- [ ] A source can emit all three fields; a destination/consumer can read them off any record uniformly.
- [ ] A heartbeat is emitted on a live-but-unchanged source, and its absence is detectable downstream.
- [ ] An existing connector that doesn't populate freshness still runs (backward-compatible).

## Open Questions

- Contract carrier: extend `ReadMessage` variants vs. per-record metadata vs. a dedicated heartbeat message.
- Clock/skew semantics for the value timestamp (source clock vs. delivery clock) and how it's documented for
  consumers.
- How "freshness-unsupported" is represented so downstream can tell "no freshness" from "stale."

## Implementation Plan

Discovery deliverable — ratify the contract shape (an amendment to [[WEIR-A-0014]]/[[WEIR-A-0029]] may be warranted)
then decompose. Candidate seams: (1) contract-type + codegen changes for the triple; (2) heartbeat emission path;
(3) uniform consumer read + "unsupported" defaulting; (4) conformance coverage across connector types.

**Exit criteria:** contract shape ratified with the human (ADR amendment decided if needed); backward-compat plan
confirmed; decomposable into tasks.
