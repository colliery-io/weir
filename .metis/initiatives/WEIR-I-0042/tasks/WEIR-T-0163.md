---
id: readme-rewrite-what-weir-is-the
level: task
title: "README rewrite — what weir is + the real quickstart"
short_code: "WEIR-T-0163"
created_at: 2026-08-16T15:23:58.851367+00:00
updated_at: 2026-08-25T02:21:26.909518+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# README rewrite — what weir is + the real quickstart

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

Replace the stub README.md with a real front door: what weir is, and the shortest working path to a running pipeline. Today README.md says "An early-stage Rust project" and documents a nonexistent `crates/weir-core` layout, with no pointer to the docs or any install path — every visitor's first impression is wrong.

## Evidence (2026-08-16 alpha review)

- `README.md` — stub; references nonexistent `weir-core` (verified).
- The real fastest path (`docker compose --profile demo up --build`) exists only as a comment inside `compose.yml`.
- `docs/tutorials/first-sync.md` was verified line-by-line accurate against the current CLI — the README just needs to route to it.
- `mkdocs.yml` site_description also says "An early-stage Rust project".

## Acceptance Criteria **[REQUIRED]**

- [x] README states what weir is (open-source no-code ingestion + reverse-ETL platform; Rust control plane, WASM connectors, embedded web UI; Apache-2.0, ASF-bound) and describes the actual workspace layout
- [x] Quickstart section with the docker compose demo path and links to `docs/tutorials/first-sync.md` and the docs site *(docs site is 404 until publishing lands, so the link targets the in-repo `docs/` tree)*
- [x] No references to nonexistent crates; `mkdocs.yml` site_description updated to match
- [x] License / contributing pointers appropriate for an Apache-2.0 project
- [x] Quickstart depends on [[WEIR-T-0164]] (bootstrap key) and [[WEIR-T-0173]] (slim demo profile) landing for its steps to be literally true — both landed first; the quickstart was verified by running it (below)

## Implementation Notes

Pure docs/markdown work; no code changes. Keep the README short — the docs site carries the depth. Verify the quickstart commands by running them, not by pattern-matching the compose file.

## Status Updates **[REQUIRED]**

**2026-08-18 — writing done; quickstart verification in flight (ralph run).**

- `README.md` rewritten: what weir is (no-code ingestion + reverse-ETL; Rust control plane, WASM-sandboxed connectors, host-side credentials; Apache-2.0/ASF-bound), honest pre-alpha status note with the [[WEIR-A-0006]] v0-unstable statement, Docker quickstart (slim `--profile demo` from [[WEIR-T-0173]], first-run key banner from [[WEIR-T-0164]] incl. a logs-recovery one-liner), from-source pointers to installation.md + first-sync tutorial, the ACTUAL repository layout (weir-core is gone), the angreal dev commands, license/contributing.
- Docs site is 404 (publish workflow is manual-only) — README links to the in-repo `docs/` tree instead of the dead URL; swap to the site link when docs publishing lands.
- `mkdocs.yml` site_description and the workspace `Cargo.toml` description both updated off "An early-stage Rust project".
- Pending: `docker compose --profile demo up --build` cold-start verification running in the background (full-workspace image build); on completion I verify :8080 + the printed key, which also closes the deferred cold-start boxes on [[WEIR-T-0164]]/[[WEIR-T-0173]]. Overlapping with [[WEIR-T-0165]] while it builds.

**2026-08-25 — quickstart verified end-to-end; task complete.**

- The cold-start verification caught a REAL quickstart-breaking bug: the Docker image build failed at `trunk build` with `E0463 can't find crate for core` — the Dockerfile's `rustup target add` targeted the image's default toolchain, but `rust-toolchain.toml` pins 1.96.1, so cargo switched to a toolchain without the wasm targets. Fix: the wasm targets (`wasm32-wasip2`, `wasm32-unknown-unknown`) now live in `rust-toolchain.toml` itself (fixes Docker, CI, and dev machines uniformly), and the Dockerfile copies the toolchain file first and runs `rustup toolchain install` as a cacheable pre-source layer.
- Second find: the weir service's host port was hardcoded `8080:8080` — collides with anything local (this machine: kairos). Added `WEIR_HTTP_HOST_PORT` override to compose.yml + docs (extends [[WEIR-T-0173]]'s override set).
- Verified on the rebuilt image (fresh store, ports 18080/25432): weir + postgres only came up healthy; the admin key was minted + printed in the weir logs; `/health` → ok; unauthenticated `/connections` → 401; the minted key → `/auth/me` as admin and `/connections` → `[]`; the embedded UI served (`<title>weir · control plane</title>`). Stack torn down with `down -v` afterwards.
