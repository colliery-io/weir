---
id: fidius-0-5-adoption-wasm-http-weir
level: initiative
title: "fidius 0.5 adoption: WASM HTTP (WEIR-T-0023)"
short_code: "WEIR-I-0005"
created_at: 2026-06-19T23:47:44.764532+00:00
updated_at: 2026-06-21T03:21:31.254236+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: S
initiative_id: fidius-0-5-adoption-wasm-http-weir
---

# fidius 0.5 adoption: WASM HTTP (WEIR-T-0023)

## Context **[REQUIRED]**

Tracks weir's adoption of the fidius 0.4→0.5 line. **The dependency bump is DONE** — weir is on **fidius 0.5.0** (`feat/fidius-0.5-upgrade`, 0.3.0→0.5.0, breaking ABI 400→500 recompiled cleanly; workspace + WASM + all 8 Postgres integration tests green). **The streaming-contract question is resolved** — [[WEIR-A-0029]] is ratified (streaming + configured-instance contract v1), with implementation in [[WEIR-I-0006]]. So this initiative now narrows to its remaining piece: **[[WEIR-T-0023]]** (WASM outbound HTTP).

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- **[[WEIR-T-0023]]**: codegen'd WASM connectors do **live outbound HTTP** via `fidius_guest::http`, so manifest REST connectors run over the sandboxed/community primitive (not just dylib). The host `EgressPolicy` doubles as weir's **credential injection** ([[WEIR-A-0013]]) + SSRF/allow-list guard.

**Non-Goals:**
- The streaming/configured-instance contract — ratified separately ([[WEIR-A-0029]] → [[WEIR-I-0006]]).
- New connector breadth — later.

## Detailed Design **[REQUIRED]**

**T-0023 (now cleanly buildable on 0.5.0).** fidius 0.5 (FIDIUS-I-0028) added `fidius_guest::http` — a connector written **entirely with the macros** can fetch (`http::get(url)`, `Request::get(url).timeout(..)`, `resp.text()`), and `#[plugin_impl]` + `fidius_guest::http` **auto-compose the `wasi:http` import** (the fixture `tests/wasm-fixtures/macro-fetcher` is documented as "exactly what an adopter's codegen emits"). This **supersedes** the 0.4.2 R&D path (raw `wit_bindgen` + hand-vendored wit/, which my spike found couldn't compose with the fidius-guest world).

- Codegen (`weir-codegen/src/wasm.rs`): emit a macro connector whose `read` calls `fidius_guest::http` instead of the stub; declare the `http` capability in the package.
- weir's WASM load path (`ConnectorHandle::from_wasm_package*`): supply an `EgressPolicy` via `PluginHost::builder().egress(..)` — authorizes per request, injects credentials, enforces an allow-list.
- E2E: a manifest REST source runs over the WASM primitive against a mock and matches the dylib path.

## Alternatives Considered **[REQUIRED]**

- **Raw `wit_bindgen` guest + hand-vendored `wasi:http` wit/ (the 0.4.2-era path).** Rejected — my spike found the fidius-guest world (export-only) couldn't compose with a second `generate!`. fidius 0.5's `fidius_guest::http` solves this at the macro level; use it.

## Implementation Plan **[REQUIRED]**

1. ~~Bump fidius (→ 0.5.0)~~ **DONE** (`feat/fidius-0.5-upgrade`).
2. **[[WEIR-T-0023]]** — codegen `fidius_guest::http` `read` + `http` capability + `EgressPolicy` on the load path + E2E.

## Exit Criteria

- [x] weir on the fidius 0.5 line (0.3.0 → 0.5.0, verified — workspace + WASM + Postgres integration green).
- [x] Streaming-contract decision made ([[WEIR-A-0029]] ratified; implementation in [[WEIR-I-0006]]).
- [x] A codegen'd WASM REST connector performs live HTTP via `fidius_guest::http` (EgressPolicy-brokered), matching the dylib path. — [[WEIR-T-0023]] *(verified E2E 2026-06-21: `tests/wasm_http.rs`)*
