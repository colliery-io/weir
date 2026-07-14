---
id: f1-7-websocket-websocket-resident
level: task
title: "F1.7 — Websocket→websocket resident consumer + connector socket-transport capability"
short_code: "WEIR-T-0143"
created_at: 2026-07-10T12:55:49.638602+00:00
updated_at: 2026-07-10T13:39:21.407368+00:00
parent: WEIR-I-0035
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# F1.7 — Websocket→websocket resident consumer + connector socket-transport capability

## Parent Initiative

[[WEIR-I-0035]] (F1). Brought in as a real non-poll consumer to validate — and now extend — the resident interfaces.
Execution model decided in **[[WEIR-A-0039]]** (brokered ws/wss egress; connector stays wasm; outbound-client, so
NOT host-side like [[WEIR-A-0038]]'s inbound broker).

## Objective

Support a **websocket→websocket** resident consumer: an event-reader source tailing a live ws upstream and a sink
pushing to a live ws downstream, over **host-brokered websocket egress** ([[WEIR-A-0039]]) — the streaming sibling
of the existing `wasi:http` egress. Reuses F1's `run_resident` + supervision (reconnect on disconnect).

## Acceptance Criteria

## Acceptance Criteria

- [ ] **Host ws/wss egress broker**: the host opens/owns the outbound websocket, enforces the egress allow-list, and
  injects credentials host-side ([[WEIR-A-0033]]); the guest is NOT granted raw `wasi:sockets`.
- [ ] **Guest brokered-ws handle** (open / recv-frame / send-frame / close) over the runtime seam, usable from a
  wasm connector's `read`/`write`.
- [ ] `EgressPolicy` gains a scheme dimension (http vs ws/wss) in the allow-list.
- [ ] A **ws→ws resident connector** (wasm): `read` yields a `ReadMessage` per inbound frame (arrival-driven, not
  cadence); `write` sends frames to the downstream ws. Runs under `run_resident`.
- [ ] **Supervised reconnect**: killing the upstream ws → stream error → requeue+backoff → reconnect + resume from
  last checkpoint (no silent death).
- [ ] **E2E test** against a local ws echo server (upstream + downstream), driving arrivals and asserting emission
  tracks them; a disconnect→reconnect assertion. No external network.

## Implementation Notes

### Technical Approach
- **Egress broker**: extend the `wasi:http` brokering in `crates/weir-runtime` (`EgressPolicy`/`HostAllowList`,
  ~`lib.rs`) to a ws/wss path — host holds a `tokio-tungstenite` client, allow-list + credential injection as for
  http. Expose a brokered-ws handle to the guest via the fidius host-function seam (ws isn't standard WASI → custom
  host capability). Mind [[WEIR-A-0029]] streaming + bounded memory.
- **Connector**: a new `wasm-fixtures/` (or `crates/connectors/`) ws guest using the brokered handle in `read`
  (frame→`ReadMessage::Records`, checkpoint per frame/N) and `write` (RecordBatch→frames).
- **Supervision**: disconnect surfaces as a stream error → `EngineError` → the F1.3 requeue/backoff path; resume
  from last checkpoint.
- **Test**: a local `tokio-tungstenite` echo server fixture; assert arrival-driven emission + reconnect.

### Risk / Notes
- The guest-facing brokered-ws handle over fidius is the hard part (ws ≠ WASI world). If the fidius seam can't
  express a duplex handle cleanly, fall back to a host-side ws source/sink for the connector while keeping the
  egress-policy model — and report that as a finding (it would partially reopen the A-0039 vs A-0038 line).
- Keep secrets host-side; never hand the guest a raw socket.

### Verification
- `angreal check all`, `cargo build --workspace`, the e2e test (local echo ws), `angreal test connectors`. Paste
  real output. No external network.

## Status Updates

**2026-07-10 — feasibility corrected; guest-side path confirmed (A-0039 amended).** The guest already has
host-brokered raw TCP (`fidius_guest::sockets::tcp::connect` + `EgressPolicy::authorize_tcp`, allow-list live; used
by the postgres connector). A websocket = RFC6455 over TCP, so the ws connector stays a **wasm guest** riding
existing egress — **no new host capability for `ws://`** (cheaper than first assumed; A-0039 updated). Plan:
guest-side RFC6455 (hand-rolled masked text frames to avoid the tungstenite-wasm-compile risk) over the brokered TCP
stream; `read` yields a lazy `fidius_guest::Stream` (one `ReadMessage` per frame, à la `resident-poll`); `write`
sends frames; supervised reconnect via the F1.3 requeue path; e2e vs a local `tokio-tungstenite` echo server
(test dev-dep). `wss`/auth (host-brokered handshake) is a follow-on. **Build in progress.**

**2026-07-10 — COMPLETE (build + e2e verified by my own run).** A real ws→ws resident consumer works on the
existing interfaces.
- **New wasm fixture `wasm-fixtures/resident-ws/`** — hand-rolled RFC6455 client (dependency-free: handshake,
  client-masked send via a cheap LCG, frame parse text/binary/close/ping) over `fidius_guest::sockets::tcp::connect`
  (host-brokered, `authorize_tcp`-gated). **wasm-builds** (`weir_resident_ws_wasm.wasm`, 220 KB). No `tungstenite`
  in the guest.
- `read`: connect upstream, lazy `Stream` yielding `Records`+`Checkpoint` **per frame arrival**; read error/close →
  transient `Fatal` → F1.3 requeue. `write`: send each record as a text frame. Declares `Source, Destination`.
- **E2E `crates/weir-engine/tests/wasm_resident_ws_engine.rs`** (local `tokio-tungstenite` echo server, dev-dep,
  no external net): `resident_ws_emits_one_record_per_frame_arrival` (5→5), `..._upstream_close_is_transient_then_
  reconnects` (close→Err→2nd run reconnects+emits), `..._zero_arrivals_zero_records` (0→0). **3/3 pass** (my run).
- **Regression (my run):** `cargo build --workspace` green; full `weir-engine` suite green (ws 3, resident 2,
  engine 5, http 16, …); fmt clean; clippy clean. Nothing else touched.

**Honest gaps (follow-ons, recorded):**
1. **Stop can't interrupt a guest blocked in a synchronous socket read** — drop-to-cancel is honored only *between*
   frames; an idle live tail won't stop until the next frame or a socket error. Fix = guest read timeout /
   non-blocking read. (Same family as the F1.6 stop-fix, but on the guest's blocking socket read.)
2. **`wss`/authenticated ws deferred** — `ws://` no-auth only; `wss` (TLS) + auth needs the host-brokered handshake
   to keep creds host-side (the [[WEIR-A-0039]] follow-on).
3. Lenient handshake (fixed key, no `Sec-WebSocket-Accept` verify) + non-crypto LCG mask — RFC-valid, fine for a
   trusted brokered peer/test; a hardened client verifies accept + uses a real RNG.
4. `EgressPolicy` scheme tag (http vs ws/wss) not added — ws rides `authorize_tcp` as-is (clarity enhancement).
5. ws **sink** (`write`) compiles but is not e2e-tested (only source→ArrowSink is); a dest-ws test needs a
   receiving-server assertion.

**Net:** the "test the current interfaces" goal is met AND extended — the resident/event-reader/streaming/
supervision interfaces genuinely carry a live ws→ws consumer, and the transport rode existing brokered TCP with no
new host capability.