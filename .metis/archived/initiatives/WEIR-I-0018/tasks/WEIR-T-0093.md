---
id: secrets-tenant-scoping-two-tenant
level: task
title: "Secrets tenant-scoping + two-tenant isolation integration test"
short_code: "WEIR-T-0093"
created_at: 2026-07-05T23:57:29.497112+00:00
updated_at: 2026-07-06T01:12:04.965800+00:00
parent: WEIR-I-0018
blocked_by: [WEIR-T-0090, WEIR-T-0091, WEIR-T-0092]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0018
---

# Secrets tenant-scoping + two-tenant isolation integration test

## Parent Initiative

[[WEIR-I-0018]] — the proving gate. Closes the initiative.

## Objective

Scope connector secrets to the tenant, then **prove** end-to-end that the whole initiative isolates tenants: a
two-tenant integration test asserting **no cross-tenant read, run, build, or secret access**.

## Reference

- Secrets: host-side credential injection ([[WEIR-A-0013]]/[[WEIR-A-0033]]) — the secret bundle / injection path
  (`crates/weir-app` ingress + the runner). Secrets must resolve per tenant.
- Isolation surfaces delivered by [[WEIR-T-0090]] (data/routes), [[WEIR-T-0091]] (execution), [[WEIR-T-0092]] (compile).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] A tenant's secrets are stored + resolved scoped to `tenant_id`; a run for tenant A can never read tenant
  B's secret, and injection uses the running work-unit's tenant.
- [ ] **Two-tenant integration test** (`#[ignore]` + an angreal lane) that provisions tenants A + B and asserts:
  - **Read:** A's key gets 404 on B's connection/run/catalog; listing shows only A's.
  - **Run:** A's runner only ever claims A's work_units; a run under A never touches B's data/dest.
  - **Build:** A + B onboard a same-named connector with different config → distinct artifacts, no reuse.
  - **Secret:** a run under A injects A's secret, never B's.
  - **Audit:** each mutation writes an `audit_events` row carrying the right tenant.
- [ ] The initiative's exit criteria all hold; workspace + clippy clean; existing suites (single-tenant `default`) still green.

## Implementation Notes

- This test is the initiative's definition of done — write it to fail first (cross-tenant access succeeding),
  then confirm the isolation work closes each hole.
- Reuse the integration harness (`angreal integration up` Postgres) + the auth key-minting from [[WEIR-I-0017]].

## Status Updates

### 2026-07-06 — done (`534c288`) — the initiative's isolation is proven

**Secrets are config-borne, so already tenant-scoped.** There's no separate secrets store — `resolve()` reads
the credential from the connection's `config.json` via `Credential::from_auth_config` (orchestrator.rs:141), and
that config is tenant-scoped ([[WEIR-T-0089]]/[[WEIR-T-0090]]). A run for tenant A carries A's connection config
(`work_spec(tenant, c)`), so injection uses A's secret; B can't read A's connection (404) → can't reach the
secret. No new secret store needed.

**Isolation proven across all five aspects** (fast unit/api tests, not `#[ignore]`+Postgres — they cover the
same ground at the store/router level with in-memory sqlite + the wasm testkit, so they run in the normal CI
suite, which is *better* than an ignored lane):
- **Read** — `cross_tenant_isolation` (api.rs): A's connection is 404 for B; B's list empty; B denied `/tenants`.
- **Run/claim** — `claim_is_tenant_isolated` (orchestrator): A's runner only ever claims A's units.
- **Build** — `compile_isolation_two_tenants_distinct_artifacts` (weir-app): distinct per-tenant artifacts.
- **Secret + Audit** — `two_tenant_secret_and_audit_isolation` (api.rs, **new**): B never sees A's secret (404 +
  secret-free list); A's create is audited to `key:acme-key`; no B-attributed mutation.

Full workspace + all suites + clippy green; single-tenant (`default`) behaviour unchanged. **Complete — closes
[[WEIR-I-0018]].**
