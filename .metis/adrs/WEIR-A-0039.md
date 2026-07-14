---
id: 001-brokered-websocket-egress-for
level: adr
title: "Brokered websocket egress for connectors"
number: 39
short_code: "WEIR-A-0039"
created_at: 2026-07-10T13:37:56.329692+00:00
updated_at: 2026-07-10T13:37:56.329692+00:00
decision_date: 2026-07-10
decision_maker: Dylan Storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0039: Brokered websocket egress for connectors

**Status:** Decided (2026-07-10, Dylan Storey). Arises from F1 ([[WEIR-I-0035]], task [[WEIR-T-0143]]): a
**websocket→websocket resident consumer** — a non-poll, event-reader source tailing a live ws upstream and a sink
pushing to a live ws downstream. Amends the egress model of [[WEIR-A-0013]] / [[WEIR-A-0033]]; distinguished from
[[WEIR-A-0038]]. *Raised by: the ws→ws consumer.*

## Context **[REQUIRED]**

A WASM connector guest today gets exactly one host-brokered egress: **`wasi:http`**, allow-listed and with
host-side credential injection (secrets never enter the guest — [[WEIR-A-0033]]). There is **no** websocket / raw
socket capability anywhere. A ws→ws consumer needs a **persistent outbound** websocket on each end.

Two facts shape the decision:
1. These are **outbound client** connections (the connector *dials out* to an upstream and a downstream ws) — the
   same direction as the existing `wasi:http` egress. They are **not** inbound *listening* sockets. So the
   [[WEIR-A-0038]] ruling that put the pub/sub broker host-side (because it *accepts* many inbound subscriber
   sockets, which a sandboxed per-run guest cannot) does **not** bind here.
2. Under [[WEIR-A-0029]] streaming + [[WEIR-I-0035]] `run_resident`, a guest already holds its configured instance
   open for the run's life — so it can naturally hold a long-lived outbound ws for a resident source.

## Decision **[REQUIRED]**

**Extend the host egress broker from `wasi:http` to also broker `ws`/`wss`.** The host opens and owns the outbound
websocket — applies the egress **allow-list**, injects credentials/tokens **host-side** ([[WEIR-A-0033]]) — and
exposes it to the guest as a **duplex message stream** over the runtime seam. The **connector stays a normal WASM
guest** ([[WEIR-A-0030]]): a ws **source** uses the brokered ws in `read` (yield a `ReadMessage` per inbound frame),
a ws **sink** uses it in `write` (send frames). Under `run_resident` the guest holds the ws for the resident run's
life; **disconnect → stream error → supervised reconnect** (requeue + backoff, resume from checkpoint — the F1
supervision path). The guest is **never** granted raw `wasi:sockets`; it only ever sees a policy-scoped, brokered
websocket handle.

**Mechanism (corrected 2026-07-10, feasibility pass).** The decision (guest-side, host-brokered, connector stays
wasm) stands — but it needs **no new host capability for `ws://`**. The guest **already** has host-brokered raw TCP:
`fidius_guest::sockets::tcp::connect(host, port)` returns a sync `Read + Write` stream, gated host-side by
`EgressPolicy::authorize_tcp` (allow-list already implemented; used by the postgres connector). A websocket is
RFC6455 **over TCP**, so the ws connector rides this existing egress and does the ws framing **in the guest**
(a small RFC6455 client — masked text frames — or a wasm-buildable ws lib). The only genuinely new work is: (1) a
**scheme tag** (http vs ws/wss) on the egress policy for clarity, and (2) for **`wss`/authenticated** ws, a
**host-brokered handshake** so TLS + credentials stay host-side ([[WEIR-A-0033]]) — that part is a follow-on;
`ws://` no-auth is fully expressible today. This is *cheaper* than the "new host ws handle API" this ADR first
assumed.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| **A. Brokered ws/wss egress (decided)** | Mirrors the `wasi:http` egress model; connectors stay wasm; secrets stay host-side; no raw-socket sandbox surface; fits `run_resident` | New host egress surface + a guest ws-handle shim to build | Medium | M |
| **B. Host-side native ws connector** | Consistent with [[WEIR-A-0038]] | Outbound doesn't need host-side (A-0038's inbound justification doesn't apply); breaks "all connectors are wasm"; not a portable package | Medium | M-H |
| **C. Grant the guest raw `wasi:sockets`** | Least host code | Broad sandbox attack surface; policy coarser than an HTTP/ws allow-list; secrets would risk entering the guest; against the host-brokered-egress principle | High | M |
| **D. External ws bridge weir doesn't own** | No weir code | Undercuts "the fabric IS weir"; external dep in the signal path ([[WEIR-A-0005]]) | High | — |

## Rationale **[REQUIRED]**

Option **A** is the smallest, most consistent extension: websocket egress is the streaming sibling of the HTTP
egress weir already brokers, so it inherits the allow-list + host-side credential model and keeps connectors as
portable wasm packages. Because the connections are **outbound clients**, none of the inbound-listening constraints
that forced the broker host-side ([[WEIR-A-0038]]) apply. **C** is rejected on security (raw sockets + secret
exposure); **B** is unwarranted host-side infrastructure for an outbound client; **D** breaks the open-core /
fabric-is-weir line.

## Consequences **[REQUIRED]**

### Positive
- A live **event-reader** consumer (ws, and later SSE/other duplex) is expressible as an ordinary wasm connector.
- Reconnect/resilience come for free from F1 supervision (`run_resident` + requeue/backoff).
- Secrets/tokens for the ws stay host-side ([[WEIR-A-0033]]); the guest gets no raw socket.

### Negative
- ~~New host surface: a guest-facing brokered-ws handle API over fidius~~ **(corrected: not needed for `ws://` —
  the guest does RFC6455 over the existing brokered TCP egress).** Real new work: a scheme tag on the egress policy;
  and, for **`wss`/auth**, a host-brokered handshake so TLS/creds stay host-side (follow-on).
- A guest-side RFC6455 client (or a wasm-buildable ws lib) in the connector.

### Neutral
- Distinct from the **inbound** delivery plane ([[WEIR-S-0017]] / [[WEIR-A-0038]]); that stays host-side.
- Backpressure/bounded-memory on the ws stream follow the [[WEIR-A-0029]] streaming semantics.

## Review Schedule **[CONDITIONAL: Temporary Decision]**

Permanent once ratified. Implementation tracked in [[WEIR-T-0143]] under [[WEIR-I-0035]].
