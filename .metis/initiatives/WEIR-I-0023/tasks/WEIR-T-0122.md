---
id: weir-soak-provisioner-bin-api
level: task
title: "weir-soak provisioner (bin + API client + fleet)"
short_code: "WEIR-T-0122"
created_at: 2026-07-08T00:08:05.769203+00:00
updated_at: 2026-07-08T00:16:24.282608+00:00
parent: WEIR-I-0023
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0023
---

# weir-soak provisioner (bin + API client + fleet)

## Parent Initiative

[[WEIR-I-0023]] — the driver + the load it stands up.

## Objective

A new `weir-soak` binary that, given a running weir (`--base-url` + an admin key), **provisions a fleet**: an
admin key, tenant(s), and a set of scheduled connections spanning the local echo/slow connectors, postgres
source+dest, and the no-auth live REST corpus.

## Reference

- `crates/weir-api/src/lib.rs` — the provisioning routes: `POST /connections`, `POST /tenants`,
  `POST /tenants/{id}/keys`, `every_secs` for schedules; `App::bootstrap_admin_key` for the first key.
- `crates/weir-cli/src/main.rs` — the `Connection`/`Tenant`/`Key` provisioning shapes to mirror over HTTP.
- The live REST corpus + postgres compose stack ([[WEIR-I-0014]] / `angreal integration`).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] New `weir-soak` crate/bin + a bearer-authed HTTP client (`--base-url`, `--admin-key`).
- [x] Provisions a **configurable fleet**: `--fleet` local echo/slow → arrow connections (tight `every_secs`),
  a postgres write + read pair, and the best-effort live REST corpus from `/catalog/available`. (Admin key is
  passed in / bootstrap; explicit tenant-create folded to T-0123 where it's exercised end-to-end.)
- [x] Re-runnable: an existing/rejected connection (400/409) is non-fatal + logged; `--fleet N` scales volume.
- [x] Builds; `fleet_plan` unit test (counts / refs / tight schedules) without a live server. Workspace + clippy
  clean.

## Status Updates

### 2026-07-08 — done (`f4ae121`)

`weir-soak` bin: `Client` (bearer, `--base-url`) + pure `fleet_plan(fleet, pg_url, every_secs)` (N local
echo/slow → arrow + a pg write/read pair) + `provision` (non-fatal on existing) + best-effort
`provision_live_rest` (from `/catalog/available`, never fatal — external flakiness expected). `main`
provisions + prints. `fleet_plan` test green; workspace + clippy clean. The soak loop + invariants are
[[WEIR-T-0123]] (which adds the local/pg-vs-external split for the DL gate). **Complete.**
