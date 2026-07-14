---
id: named-declarative-gaps-basic-auth
level: task
title: "Named declarative gaps — Basic auth, Link-header pagination, richer datetime cursor"
short_code: "WEIR-T-0070"
created_at: 2026-07-03T01:28:02.093207+00:00
updated_at: 2026-07-04T00:59:41.524773+00:00
parent: WEIR-I-0008
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0008
---

# Named declarative gaps — Basic auth, Link-header pagination, richer datetime cursor

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0008]]. Tracked in [[WEIR-S-0016]] (auth / pagination / incremental rows).

## Objective **[REQUIRED]**

Close the three remaining small named gaps in the ledger, each a self-contained construct:
- **`BasicHttpAuthenticator`** — host-side `Authorization: Basic base64(user:pass)` ([[WEIR-A-0033]]).
- **`Link`-header pagination** — follow RFC 5988 `Link: <…>; rel="next"` (GitHub-style) — the runtime reads
  the *response header*, not a body token.
- **Richer `DatetimeBasedCursor`** — start/end datetime, step window, datetime format, lookback — beyond
  the current basic cursor-field/param.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] **Basic auth:** new `Credential::Basic { user, pass }` (host-side, base64); importer maps
  `BasicHttpAuthenticator`; `from_auth_config` builds it; wire test asserts the `Authorization: Basic …`
  header; secret strippped from the guest config.
- [ ] **Link-header pagination:** `rest` reads `Link` `rel="next"` from the response headers and follows it
  until absent; importer maps the Airbyte header-based cursor paginator; wire test (2–3 pages via `Link`).
  (Requires the runtime to expose response headers to the pagination step.)
- [ ] **Richer datetime cursor:** importer maps `DatetimeBasedCursor` start/end/step/format; runtime honors
  them; wire/unit test. Keep it pragmatic — cover the common shapes, report the rest.
- [ ] **Ledger flipped:** [[WEIR-S-0016]] Basic-auth, `Link`-header, and richer-datetime rows → ✅ (or ⚠️→✅);
  `analyze()` updated.
- [ ] Workspace + integration suites green; clippy clean.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
- Basic auth mirrors the existing host-side `Credential` variants ([[WEIR-A-0033]] / [[WEIR-T-0063]]) —
  smallest of the three.
- Link-header needs the guest to surface response headers to the fetch loop (today it reads the body only);
  check what `fidius_guest::http` exposes for response headers.
- These are independent; land + flip each ledger row as it goes. Can be split if one balloons.

### Dependencies
- Independent of [[WEIR-T-0068]]/[[WEIR-T-0069]]. GitHub key ([[WEIR-T-0067]]) enables a live Link-header
  check if GitHub's vendored manifest uses it.

### Risk Considerations
- Response-header access may be the gating unknown for Link-header — if `fidius_guest::http` doesn't expose
  headers, that sub-item needs a runtime/contract touch (note + split if so).

## Status Updates **[REQUIRED]**

### 2026-07-03 — Basic auth done (1 of 3)

**Basic auth ✅ landed + tested.** Simpler than specced — it's a *static* header, so it reuses
`Credential::Header` (no new variant / `apply` change); only `from_auth_config` gained a `"basic"` arm
that base64-encodes `user:pass`.
- `weir-manifest`: `Auth::Basic { username_key, password_key }`.
- `weir-importer`: `BasicHttpAuthenticator` → `Auth::Basic`; `analyze()` treats it supported. Unit test.
- `weir-app`: emits `auth_scheme=basic` + `basic_username_key`/`basic_password_key`.
- `weir-runtime`: `from_auth_config` `"basic"` arm → `Authorization: Basic base64(user:pass)`, strips the
  creds from the guest config. Added `base64` dep.
- **Wire test green:** host injects `Authorization: Basic YWxpY2U6czNjcjN0` over real wasi:http; secret
  never reaches the guest. `weir-importer` 10/10.
- **Ledger:** [[WEIR-S-0016]] `BasicHttpAuthenticator` ❌→✅.

### 2026-07-03 — Link-header pagination done (2 of 3)

**Unknown resolved:** `fidius_guest::http::Response` *does* expose `headers: Vec<(String,String)>` (the rest
connector just ignored them via `.text()`). So Link-header is viable.

**Link-header ✅ landed + tested.**
- `weir-manifest`: `Pagination::LinkHeader`.
- `weir-importer`: a `CursorPagination` whose `cursor_value` references the response `headers`/`link` maps
  to `LinkHeader` (else a body cursor). Unit test.
- `weir-app`: emits `page_link_header: true`.
- `rest` runtime: `fetch_slice` now captures the full `Response`; new `next_link_url` parses RFC 5988
  `Link: <url>; rel="next"`; the loop follows the **absolute** next URL until absent.
- `weir-codegen` (legacy) matches updated for the new variant.
- **Wire test green:** page 1 → 2 records + `Link` next → page 2 → 1 record, no Link → stop (3 rows).
  importer 11/11, wire suite 12/12.
- **Ledger:** [[WEIR-S-0016]] `Link`-header row ❌→✅.

### 2026-07-03 — richer datetime done (3 of 3); **task complete**

**Richer `DatetimeBasedCursor` ✅ landed + tested** — the high-value subset (start + end bounds):
- `weir-manifest`: `Incremental` gains `start_value` (initial lower bound), `end_param`/`end_value` (upper
  bound).
- `weir-importer`: `DatetimeBasedCursor` now reads `start_datetime` / `end_datetime` (literal or a
  `MinMaxDatetime.datetime`, via `datetime_str`) + `end_time_option`. Unit test. (Boxed the raw
  `serde_yaml::Value` fields to satisfy clippy `large_enum_variant`.)
- `weir-app`: emits `cursor_start` / `cursor_end_param` / `cursor_end`.
- `rest` runtime: first run (no state) seeds the cursor from `cursor_start`; every request sends
  `?<end_param>=<end>`; both render `{{ config[...] }}`.
- **Wire test green:** request carries `?since=2020-01-01…&until=2026-12-31…`. importer 12/12.
- **Ledger:** [[WEIR-S-0016]] richer-datetime row ⚠️→✅; **step-windowing / lookback / custom formats split
  into a new ❌ row** (reported, deferred — pragmatic per the AC).

**All three named gaps done.** Basic auth + Link-header + richer datetime — importer 12/12, wire suite
13/13, clippy clean. Nothing left in this task.
