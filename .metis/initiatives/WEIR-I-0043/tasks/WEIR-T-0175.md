---
id: wire-fidius-hostname-carrying-tcp
level: task
title: "Wire fidius hostname-carrying TCP egress into HostAllowList"
short_code: "WEIR-T-0175"
created_at: 2026-08-29T13:52:33.826087+00:00
updated_at: 2026-08-29T14:00:05.138262+00:00
parent: WEIR-I-0043
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0043
---

# Wire fidius hostname-carrying TCP egress into HostAllowList

## Parent Initiative

[[WEIR-I-0043]]

## Objective **[REQUIRED]**

Close the [[WEIR-A-0041]] review trigger that FIRED 2026-08-25: fidius 0.5.8 ships the hostname-carrying TCP egress FR (FIDIUS-I-0034 — resolve-and-pin, `TcpTarget { host, addr }`, `EgressPolicy::authorize_tcp_target`, `authorize_dns`). Wire it into weir's `HostAllowList` so TCP allow-lists can speak hostnames — IP allow-lists are operationally broken for managed endpoints (RDS/Cloud SQL/Azure) that rotate IPs.

## Context

- fidius 0.5.8 (published; local checkout `~/Desktop/fidius`, FR merged in PR #9): `authorize_tcp_target(&TcpTarget)` defaults to delegating to `authorize_tcp(&addr)`; `target.host` is `Some(lowercased-name)` only for pinned by-name dials, `None` for IP-literal dials (name-keyed policies should deny `None`).
- weir side: `HostAllowList` (`crates/weir-runtime/src/lib.rs`) currently matches TCP by resolved IP/`ip:port` only (`authorize_tcp`). The fidius surface is reached white-label via `weir_connector::fidius` (whole-crate re-export, so `TcpTarget` is already reachable).
- Production currently runs `allow_all()` (orchestrator resolve site) — this task lands the *capability*; config-driven tightening stays with [[WEIR-I-0043]]'s TLS/allow-list tasks.

## Acceptance Criteria **[REQUIRED]**

- [x] fidius host-side dependency floor raised to 0.5.8 (weir-connector `fidius`, weir-runtime `fidius-core`, weir-connector-types `fidius-macro`)
- [x] `HostAllowList` implements `authorize_tcp_target`: hostname entries match by-name dials (case-insensitive, `host` and `host:port` forms); IP/`ip:port` entries keep matching via the resolved addr; a hostname entry never authorizes an IP-literal dial; empty list stays allow-all
- [x] Unit tests cover: empty-list allow, name match, `name:port` match, case-insensitivity, IP-literal dial denied by name-only list, IP entry still authorizing, deny reason carries both name and addr
- [x] `angreal check all` + `angreal test unit` green; wasm engine suites still pass against fidius 0.5.8

## Status Updates **[REQUIRED]**

**2026-08-29 — implemented + verified.**

- Dep floors: `fidius = "0.5.8"` (weir-connector), `fidius-core = "0.5.8"` (weir-runtime), `fidius-macro = "0.5.8"` (weir-connector-types); `cargo tree` confirms the whole fidius family resolves at 0.5.8. `TcpTarget` reaches weir-runtime white-label via the existing `weir_connector::fidius` whole-crate re-export — no facade change needed.
- `HostAllowList::authorize_tcp_target` (`crates/weir-runtime/src/lib.rs`): hostname entries (`db.example.com` / `db.example.com:5432`) match the pinned dialed name case-insensitively; misses fall back to the resolved-addr check so IP/`ip:port` entries keep working for both dial styles; an IP-literal dial (`host: None`) can never satisfy a hostname entry (fail-closed, per fidius's "name-keyed policies deny None" guidance); empty list stays allow-all. Deny reasons carry the dialed name AND resolved addr (or an explicit IP-literal hint).
- Tests: new `tcp_egress_policy_tests` module, 5 tests covering all AC cases — weir-runtime lib 21/21.
- Verification against 0.5.8: `wasm_http_engine` 25/25, `wasm_resident_engine` 2/2, `wasm_resident_ws_engine` 3/3 (real guest TCP dial through the new resolve-and-pin host path), `angreal check all` clean, `angreal test unit` 12/12 binaries. The docker-gated `wasm_postgres_engine` wire tests remain ignored as usual (fidius's own `hostname_egress_e2e` covers the by-name plumbing upstream).
- Scope note: production still constructs `allow_all()` at the orchestrator resolve site — config-driven allow-list tightening (per-connection `allowed_hosts` speaking names) belongs to [[WEIR-I-0043]]'s TLS/allow-list tasks; this task landed the policy capability the ADR queued.
