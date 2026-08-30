---
id: live-verify-keyed-manifests
level: task
title: "Live-verify keyed manifests + catalog verification record"
short_code: "WEIR-T-0183"
created_at: 2026-08-29T14:26:50.154249+00:00
updated_at: 2026-08-30T02:04:00.634018+00:00
parent: WEIR-I-0043
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0043
---

# Live-verify keyed manifests + catalog verification record

## Parent Initiative

[[WEIR-I-0043]]

## Objective **[REQUIRED]**

Turn "31 of 36 vendored manifests have never run against their real API" into a visible, durable quality signal: run keyed manifests live as [[WEIR-T-0067]] secret bundles land, and record what passed when as a **catalog row verification record** (decided 2026-08-29 — catalog column, not a docs table: queryable, surfaces in API/UI, survives doc rewrites).

## Design

- **Record**: verification fields on the connector catalog row (e.g. `verified_at` timestamp + `verified_ref` for what ran) written by the live suite on a pass; absent/stale values are honest "unverified". Schema change goes through `angreal schema gen` (diesel-dualdb logical DDL).
- **Writer**: the live connector suite (`angreal test connectors-live`, [[WEIR-I-0014]]) records a pass per keyed manifest; coordinate with the nightly CI wiring so the record refreshes without manual runs.
- **Surface**: catalog listings (API; UI if cheap) show the verification state; `manifests/README.md`'s honesty section points at the catalog as the source of truth instead of hand-maintained claims.
- **GATED on [[WEIR-T-0067]] / [[WEIR-S-0018]]** (human: accounts + SOPS bundles). Pipelines whose bundles are absent skip cleanly and stay unverified — the mechanism lands regardless; coverage grows as bundles arrive. Sequence GA4 last (its data window).

## Acceptance Criteria **[REQUIRED]**

*(Amended 2026-08-29 during implementation: (a) a per-deployment DB row can't be written by CI's live suite — the durable record is the VENDORED ledger `manifests/verified.json`, copied onto the catalog row (`verified_at`) at registration, so deployments carry it; (b) zero [[WEIR-T-0067]] bundles have landed, so "already-bundled" = the no-auth live tier, verified for real; (c) UI surfacing skipped as not-cheap — the API carries the field; (d) automated nightly commit-back of the ledger is left to [[WEIR-I-0014]]'s CI wiring — the refresh flow is documented and one command.)*

- [x] Catalog rows carry `verified_at` (schema migration `0004_verification` via `angreal schema gen`); the live suite writes the ledger ONLY on a pass (skip/fail arms untouched)
- [x] The available live tier shows genuine records end-to-end: the no-auth suite ran live against the 5 real APIs and wrote the ledger; registration copies it onto rows (unit-tested); absent bundles skip cleanly (pre-existing suite behavior)
- [x] The API surfaces `verified_at` (CatalogEntry serializes straight through `GET /catalog`/connectors listings); manifests/README defers to `verified.json` + the catalog row as the source of truth
- [x] Refresh flow documented (`WEIR_WRITE_VERIFIED=1` + commit; nightly automation with [[WEIR-I-0014]]); `angreal check all` + unit wall green

## Status Updates **[REQUIRED]**

**2026-08-30 — implemented + seeded with REAL live verification (ralph run).**

- **Propagation design resolved**: CI/live runs can't write user deployments' DBs, so the durable record is the vendored ledger **`manifests/verified.json`** (`{name: {verified_at, ref}}`, sorted for stable diffs) — travels with the manifests; `register_connector` copies the date onto the catalog row's new `verified_at` column (explicit value on the entry wins; unlisted = honest NULL).
- **Schema**: logical migration `0004_verification` (`ALTER TABLE connectors ADD COLUMN verified_at TEXT`), regenerated per-backend via `angreal schema gen` + registered in weir-schema's MIGRATIONS table (the include_str! list must be appended by hand — gotcha noted in that file's comment).
- **Writer**: `manifest_corpus.rs` — both live pass arms (no-auth + keyed) call `record_verified` when `WEIR_WRITE_VERIFIED=1`; default runs never touch the repo.
- **Seeded for real**: ran `no_auth_manifests_run_live` with the writer against the live APIs — coinpaprika (61,216 rows), frankfurter (165), jsonplaceholder (100), rickandmorty (826), xkcd (1) all verified 2026-08-30 and committed to the ledger. Keyed connectors join as [[WEIR-T-0067]] bundles land — the suite already skips them cleanly.
- **Surfaces**: `CatalogEntry.verified_at` flows through every catalog API endpoint unchanged (Json-serialized directly); manifests/README's honesty section now points at the ledger + row instead of a hand-maintained list.
- Tests: `registration_fills_verified_at_from_ledger` (ledger → row; unlisted stays None) — weir-app lib 29/29; corpus non-live 1/1; `angreal check all` clean; unit wall 12/12.
