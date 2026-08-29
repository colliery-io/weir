---
id: slim-the-demo-compose-profile-weir
level: task
title: "Slim the demo compose profile — weir+Postgres only, test deps behind their own profile"
short_code: "WEIR-T-0173"
created_at: 2026-08-16T15:24:13.659578+00:00
updated_at: 2026-08-18T02:18:59.482510+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# Slim the demo compose profile — weir+Postgres only, test deps behind their own profile

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

The "easy demo" drags the whole integration estate: bare compose `up` (and `angreal docker up`) starts MSSQL (~1.5GB image, emulated on Apple Silicon), Dex, and MinIO plus their seed jobs alongside weir + Postgres. Move test-only dependencies behind their own profile so the demo path is weir + Postgres only.

## Evidence (2026-08-16 alpha review)

- `compose.yml` — integration deps (Postgres wal_level=logical, MSSQL + seed, Dex, MinIO + seed) run un-profiled; `--profile demo` adds the weir service.
- Consumers that must keep working: `.angreal/task_integration.py` (up/down/status/test), `.angreal/task_tests.py` (e2e needs Dex), `.angreal/task_docker.py`, `.angreal/task_soak.py`.
- Host-port override env vars exist but are documented only in compose comments.

## Acceptance Criteria **[REQUIRED]**

- [x] `docker compose --profile demo up --build` starts only weir + its Postgres *(verified via `config --services`: demo = postgres, weir; the full `up --build` runs under [[WEIR-T-0163]]'s quickstart verification)*
- [x] MSSQL/Dex/MinIO (+ seed jobs) live behind an integration/test profile; `angreal integration up`, `angreal test e2e`, and `angreal soak` still pass unmodified from the operator's point of view
- [x] Host-port override env vars are documented user-facing (not only compose comments)
- [x] The quickstart path in README/docs points at the slim profile *(installation.md here; README landed with [[WEIR-T-0163]]; full `--profile demo up --build` cold-start verified 2026-08-25 — only weir + postgres started)*

## Implementation Notes

Compose profiles are additive — assign the integration services a `profiles: [integration]` key and have `angreal integration up` pass `--profile integration`; check whether the demo MSSQL pipeline path (`task_demo.py` notes it needs `angreal integration up` separately) keeps its documented behavior. Watch `angreal docker up`'s current semantics (it runs the demo profile — after this change it must not silently lose Dex for anyone using it to drive e2e by hand).

## Status Updates **[REQUIRED]**

**2026-08-18 — implemented + verified (ralph run).**

- `compose.yml`: `profiles: ["integration"]` added to mssql, mssql-seed, dex, minio, minio-seed; postgres stays unprofiled (both paths need it; bare `up` = postgres only); weir keeps `demo`. Header comment rewritten to document the two profiles + the three host-port override env vars.
- `.angreal/task_integration.py`: all four commands (up/down/status/test) now run through a shared `_COMPOSE = docker compose --profile integration` prefix; module docstring updated.
- Verified: `config --services` → demo = {postgres, weir}; integration = {postgres, mssql, mssql-seed, dex, minio, minio-seed}; bare = {postgres}. Live-verified compose v2's explicit-target behavior: `docker compose up -d dex --wait` starts profiled dex WITHOUT the flag (healthy in 3s, then cleaned up) — so `angreal test e2e` (`up -d dex`) and `angreal soak` (`up -d postgres`) keep working unmodified. `angreal docker up/down` (`--profile demo`) semantics unchanged.
- Demo-pipelines guide note still holds: the MSSQL demo pipeline documents running `angreal integration up` separately, which now activates the integration profile — same operator command.
- Not run here: the full `--profile demo up --build` (whole-workspace image build) — membership is config-proven; the real cold-start build runs under [[WEIR-T-0163]].
