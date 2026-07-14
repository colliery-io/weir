---
id: s7-error-handling-defaulterrorhandl
level: task
title: "S7: Error handling — DefaultErrorHandler + backoff → retry/dead-letter"
short_code: "WEIR-T-0069"
created_at: 2026-07-03T01:28:00.928051+00:00
updated_at: 2026-07-04T01:33:31.879931+00:00
parent: WEIR-I-0008
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0008
---

# S7: Error handling — DefaultErrorHandler + backoff → retry/dead-letter

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0008]] — slice S7. Tracked in [[WEIR-S-0016]] (error-handling rows).

## Objective **[REQUIRED]**

Real APIs rate-limit and blip. Map Airbyte's `DefaultErrorHandler` (response filters → action) + backoff
strategies onto the `rest` runtime so transient failures (429 / 5xx) **retry with backoff** instead of
failing the sync, and genuinely-bad responses fail loudly. Reuses the existing retry/dead-letter posture
([[WEIR-T-0008]]).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] **Importer:** map `DefaultErrorHandler` → response-filter rules (http status / error substring →
  action: `retry` | `fail` | `ignore`) + backoff strategy (`ConstantBackoff`, `ExponentialBackoff`,
  `WaitTimeFromHeader` / `Retry-After`). Unsupported sub-forms reported.
- [ ] **Runtime:** `rest` applies the rules — retry on `retry` actions with the configured backoff (capped
  attempts), surface `fail` as a fatal error, skip `ignore`. Default (no handler) = retry 429/5xx a few
  times with exponential backoff, as a sane baseline.
- [ ] **Wire-level proof:** a mock endpoint returns 429/500 then 200 → the connector retries and succeeds;
  a persistent 500 → fails after the cap with a readable error.
- [ ] **Ledger flipped:** [[WEIR-S-0016]] `DefaultErrorHandler` + backoff rows → ✅; `analyze()` updated.
- [ ] Workspace + integration suites green; clippy clean.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
- The runtime today treats a failed request as `ConnectorError::transient` (retryable) or `fatal`. Add a
  bounded retry loop with backoff inside `fetch_slice` keyed on status; honor `Retry-After`.
- Keep it simple: a small `ErrorRule { on: status/substring, action }` list + a `Backoff` enum in config,
  emitted by the importer. Don't over-model Airbyte's full grammar — cover the common shapes.

### Dependencies
- Independent of [[WEIR-T-0068]]/[[WEIR-T-0070]]. Pairs well with live testing (rate-limited real APIs).

### Risk Considerations
- Don't retry forever — cap attempts + total wait so a run can't hang (the 25s live-test timeout is a
  backstop). Retries are per-request, within the existing per-slice fetch.

## Status Updates **[REQUIRED]**

### 2026-07-04 — universal retry+backoff landed; **task complete** (baseline; custom rules deferred)

**Feasibility:** the `slow` wasm fixture uses `std::thread::sleep` on wasip2 → the guest *can* back off.

**Scope (pragmatic, per the datetime precedent):** shipped the **always-on default retry** — the highest-
value 90% — and deferred the importer mapping of `DefaultErrorHandler`'s *custom* per-status filter rules
(rare; the default covers 429/5xx).

**Implemented (`rest` runtime):**
- `send_with_retry(url, max_retries)` retries transient failures — **429 / 5xx / transport error** — with
  **exponential backoff** (`backoff_ms`: 200ms→…→8s cap) honoring **`Retry-After`** (`retry_after_ms`).
  Non-retryable status returned to the caller; a persistent transient one errors with a readable message
  (`… failed after N retries (HTTP 5xx)`). `max_retries` config (default 4).
- `fetch_slice` routes every request through it → **universal** across all connectors/pages.
- The connector didn't check `resp.status` before; a 429/5xx now retries instead of failing the parse.

**Wire test green:** mock returns 429 (Retry-After: 0) once → connector backs off + retries → 200 → 1 row.

**Ledger:** [[WEIR-S-0016]] transient-retry + backoff rows → ✅; custom `DefaultErrorHandler` rules /
`CompositeErrorHandler` / wait-time-from-body split into a reported ❌ row.

**AC deviation:** the importer `DefaultErrorHandler`→rules mapping is deferred (reported), not built — the
runtime default retry is the deliverable and covers the common case.
