---
id: slim-the-demo-compose-profile-weir
level: task
title: "Slim the demo compose profile — weir+Postgres only, test deps behind their own profile"
short_code: "WEIR-T-0173"
created_at: 2026-08-16T15:24:13.659578+00:00
updated_at: 2026-08-16T15:24:13.659578+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/todo"


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

- [ ] `docker compose --profile demo up --build` starts only weir + its Postgres
- [ ] MSSQL/Dex/MinIO (+ seed jobs) live behind an integration/test profile; `angreal integration up`, `angreal test e2e`, and `angreal soak` still pass unmodified from the operator's point of view
- [ ] Host-port override env vars are documented user-facing (not only compose comments)
- [ ] The quickstart path in README/docs points at the slim profile (coordinates with [[WEIR-T-0163]])

## Implementation Notes

Compose profiles are additive — assign the integration services a `profiles: [integration]` key and have `angreal integration up` pass `--profile integration`; check whether the demo MSSQL pipeline path (`task_demo.py` notes it needs `angreal integration up` separately) keeps its documented behavior. Watch `angreal docker up`'s current semantics (it runs the demo profile — after this change it must not silently lose Dex for anyone using it to drive e2e by hand).

## Status Updates **[REQUIRED]**

*To be added during implementation*
