---
id: durable-continuous-operation
level: initiative
title: "Durable continuous operation — streaming checkpoints, cursor correctness, bounded growth"
short_code: "WEIR-I-0044"
created_at: 2026-08-18T01:57:40.600909+00:00
updated_at: 2026-08-18T02:04:32.936211+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/design"


exit_criteria_met: false
estimated_complexity: M
initiative_id: durable-continuous-operation
---

# Durable continuous operation — streaming checkpoints, cursor correctness, bounded growth Initiative

## Context **[REQUIRED]**

The 2026-08-16 alpha review's "leave it running a month" test fails on three axes. **Memory/progress:** every source buffers its whole read before checkpointing — the declarative REST runtime reads all pages into memory and emits ONE checkpoint (`crates/connectors/rest/src/lib.rs` fetch_records; failure at page 900 loses everything; MAX_PAGES=1000 silently truncates); postgres/snowflake/s3 buffer whole results similarly. **Correctness:** lexicographic cursor comparison drops rows for numeric cursors ('9' > '10' — rest L668, postgres L186, mssql L76, snowflake L505); query params are never URL-encoded (ISO timestamps with '+' corrupt requests); non-JSON responses past page 1 are treated as normal end-of-data, so a mid-sync auth expiry ends "successfully" with partial data. **Boundedness:** work_units/run_logs/dead_letters grow forever with no retention, and the orchestrator carries sharp edges (unowned terminal transitions, one corrupt schedule poisoning every tick, next_id cross-process collision, resident backoff that never decays, unbounded HANDLE_CACHE).

The key verified simplification (2026-08-16, engine read): **the engine already commits atomically per `Checkpoint` and handles any number per read** (`crates/weir-engine/src/lib.rs:855-959`) — so mid-read checkpointing is a guest-local change to the REST runtime, no contract or engine work.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- The declarative REST runtime streams: per-page-batch `Records`+`Checkpoint` emission with paginator state in the opaque state bytes — bounded memory, resumable progress, no silent page cap.
- Cursors compare correctly for numeric values; query params are URL-encoded; error pages stop a sync as *errors*, never as success-with-partial-data.
- Storage is bounded: retention/pruning for work_units, run_logs, dead_letters with operator knobs; runs feed paginates; a single run is fetchable.
- The orchestrator sharp-edges bundle is closed (owner-guarded terminal transitions, per-schedule tick isolation, next_id process nonce, run_until_idle containment, resident backoff decay/cap, HANDLE_CACHE bounding).

**Non-Goals:**
- Retries-on ([[WEIR-T-0169]]), shutdown/reclaim ([[WEIR-T-0170]]), schedule re-registration ([[WEIR-T-0171]]) — owned by [[WEIR-I-0042]].
- An outbox-relay design for append-mode dedup — the alpha posture is documented: **upsert is the crash-safe write mode**; revisit post-alpha.
- Contract-level typed cursors — the wire change folds into [[WEIR-I-0037]]'s bundled WitType change; here we fix comparison behavior with a shared helper.
- HANDLE_CACHE secret-key hygiene — the *secrets* aspect is [[WEIR-I-0047]]'s; here only size bounding.

## Detailed Design **[REQUIRED]**

**Open design questions:**

1. **Checkpoint granularity** — per page vs per N pages (transaction overhead vs resume granularity). Recommendation: per page-batch with paginator state (page number / offset / opaque token / partition-slice position) serialized into `StreamState.opaque`; MAX_PAGES becomes a loud warning, not a silent cap. Resume must be proven by a kill-at-page-N test.
2. **Cursor comparison** — a shared numeric-aware helper in `weir-connector-types` (compare as i128/f64 when both sides parse, else string) adopted by rest/postgres/mssql/snowflake, vs engine-side comparison. Recommendation: the helper — it keeps connector semantics local and needs no contract change; note postgres additionally compares in SQL (`cursor::text >` literal) and needs the predicate fixed too.
3. **Retention shape** — age + count caps (suggest defaults: 30d / 10k units per connection) enforced by a pruning pass on the scheduler tick, env-overridable. Dead-letter *purge* ships here; dead-letter *replay* is deliberately later.
4. **End-of-data honesty** — stop conditions become status-code-aware: 2xx-with-empty = end; 4xx/5xx past page 1 = run error carrying partial-progress info (the last committed checkpoint makes this safe once streaming lands).

### Candidate decomposition

| # | Task | Effort | Notes |
|---|---|---|---|
| 1 | REST runtime streaming: per-page-batch checkpoints + paginator state in opaque + kill/resume test | week | the headline fix; engine needs nothing |
| 2 | URL-encode query params (rest fetch_slice + weir-runtime append_query) | days | silent-corruption class |
| 3 | Status-aware end-of-data (error pages ≠ success) | days | with 1, partial data becomes resumable instead of wrong |
| 4 | Numeric-aware cursor helper + adoption in 4 connectors + pg SQL predicate | days | unit tests per connector |
| 5 | Retention/pruning + knobs + dead-letter purge | week | scheduler-tick pass |
| 6 | Runs feed pagination + GET single run | days | API + UI consumer note for [[WEIR-I-0045]] |
| 7 | Orchestrator sharp-edges bundle: owner-guarded mark_done/mark_failed, per-schedule tick isolation, next_id nonce, run_until_idle containment, resident backoff decay/cap, HANDLE_CACHE bound, per-run wasm isolation for the T-0066 JoinError wedge | week | one hardening PR-train |

## Alternatives Considered **[REQUIRED]**

- **Contract change for streaming** — unnecessary: the engine already commits per `Checkpoint` (verified `weir-engine/src/lib.rs:855-959`); the fix is guest-local.
- **Engine-side cursor typing** — rejected for now: it centralizes but requires the engine to know cursor semantics per stream; the shared helper gets correctness without a contract change, and real typing rides [[WEIR-I-0037]]'s wire change.
- **Outbox as a dedup relay for append mode** — deferred: today's outbox rows are an audit receipt (processed=1 at insert). Building the relay is real design work; the alpha posture "upsert is the crash-safe mode" is honest and documented.

## Implementation Plan **[REQUIRED]**

Tasks 1-4 are the alpha cut (the silent-wrong-data class); 5-6 alpha-should; 7 can trail the alpha but must precede any "leave it running for a month" claim. 1 and 3 land together (error-page handling changes once checkpoints stream). No dependency on other initiatives; [[WEIR-I-0045]] consumes task 6's API.

## Exit Criteria

- [ ] A sync killed mid-pagination resumes from the last committed page (test-proven); memory stays bounded on a 10k-page source; no silent truncation remains
- [ ] Integer-cursor incremental sync provably does not drop rows across digit-length boundaries (regression test)
- [ ] A mid-sync auth failure lands the run as *failed* with committed progress intact — never success-with-partial-data
- [ ] A month-scale soak (or accelerated equivalent) shows bounded store growth under retention defaults
