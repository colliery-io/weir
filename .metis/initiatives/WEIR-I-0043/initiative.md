---
id: real-source-reach-tls-to-managed
level: initiative
title: "Real-source reach — TLS to managed databases, real discover, OAuth, verified catalog"
short_code: "WEIR-I-0043"
created_at: 2026-08-18T01:57:39.186020+00:00
updated_at: 2026-08-30T11:43:11.381887+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: real-source-reach-tls-to-managed
---

# Real-source reach — TLS to managed databases, real discover, OAuth, verified catalog Initiative

## Context **[REQUIRED]**

The 2026-08-16 alpha review found the "point weir at YOUR database / SaaS account" promise fails at three walls: no TLS on either DB connector (managed databases — RDS, Cloud SQL, Azure — simply cannot connect), postgres `discover()` returns a hardcoded stub stream (users hand-type table names blind), and no OAuth authorization-code flow exists (SaaS "connect your account" is impossible; only hand-obtained refresh tokens and client_credentials work). Beyond that, 31 of 36 vendored manifests have never run against their real API, and coverage holes remain (s3 truncates buckets >1000 objects; the postgres destination lands JSONB blobs, not typed columns).

The TLS design was investigated and decided 2026-08-17: **[[WEIR-A-0041]] — TLS terminates guest-side** (rustls + pure-Rust provider in the guest; the host-side capability was found unimplementable and is rejected). That decision removed the feared cross-repo fidius dependency from this initiative's critical path: all TLS work is in weir, compile-proven, with experiment artifacts and a working tiberius-fork overlay already in hand. The one fidius FR filed (hostname-carrying TCP egress, 2026-08-17) is orthogonal — it fixes allow-list honesty, not TLS.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- A user can point weir at a TLS-requiring managed Postgres or SQL Server and sync — with real certificate verification available, not `trust_cert()` theater ([[WEIR-A-0041]] wire-proof gates).
- Postgres stream discovery introspects real tables (the mssql connector already does; port the pattern).
- OAuth authorization-code/PKCE exists for SaaS connectors, proven with HubSpot + Salesforce (the rETL pair that already exists).
- Keyed manifests get live verification as [[WEIR-T-0067]] secret bundles land, with a durable record of what passed when.
- s3 paginates via ContinuationToken; the postgres destination can land typed relational columns.

**Non-Goals:**
- New source families (MySQL/MariaDB, MongoDB, Kafka/webhooks) and new destinations (BigQuery, DuckDB, files) — post-alpha.
- TDS 8.0 "strict" support — no Rust client exists; documented limitation per [[WEIR-A-0041]].
- A fidius brokered-TCP/host-TLS surface — rejected by [[WEIR-A-0041]].
- Session-provider hardening beyond credentialed login bodies + 401 re-login.
- Flagship manifest pagination ([[WEIR-T-0168]]) and s3 staging ([[WEIR-T-0172]]) — owned by [[WEIR-I-0042]].

## Detailed Design **[REQUIRED]**

TLS design is settled by [[WEIR-A-0041]] (guest-side rustls; postgres `Plain|Tls` stream enum + SSLRequest exchange + SCRAM channel-binding fix; mssql via the minimal `weir-tiberius` fork; wire-proof integration gates; experiment artifacts in the 2026-08-17 session scratchpad). **Design questions closed 2026-08-29 (Dylan):**

1. **OAuth — DEFERRED.** Dylan: "glad to wait on these for now." No OAuth task is created at decompose; the alpha fallback posture (hand-obtained refresh tokens + client_credentials, documented) stands. When picked up: storage form belongs to [[WEIR-I-0047]]; this initiative owns the flow (authorize-redirect + callback — reuse the PKCE *pattern* from `crates/weir-api/src/oidc.rs`, not the code) and the manifest construct; recommended first providers were HubSpot + Salesforce.
2. **Verification-badge record — catalog row column.** A verified-at record on the connector catalog row, written by the live suite — queryable, surfaces in API/UI, survives doc rewrites; coordinate with [[WEIR-I-0014]]'s nightly live suite.

Additional prior work this initiative inherits: the fidius hostname-egress wiring landed 2026-08-29 as [[WEIR-T-0175]] (name-keyed TCP allow-lists + wire proof), so TLS tasks can assume hostname-carrying egress.

### Candidate decomposition (tasks created at decompose, after design questions close)

| # | Task | Effort | Notes |
|---|---|---|---|
| 1 | Postgres guest TLS (PgStream enum, SSLRequest, sslmode/sslrootcert, SCRAM binding fix) | 2-4 days | per [[WEIR-A-0041]]; productize from scratchpad exp3 |
| 2 | weir-tiberius fork productization + mssql TLS config (drop the NotSupported pin) | 1-2 weeks | fork overlay exists; incl. inline-PEM trust API + 32-bit PLP fix |
| 3 | TLS test harness: compose postgres-tls (hostssl-reject) + mssql forceencryption + cert-gen + negative verify-full SAN test + integration.yml switch to compose | 1-2 days | gates any TLS claim |
| 4 | Postgres real discover() via information_schema (port mssql's pattern) | days | kills blind stream typing |
| 5 | OAuth authorization-code/PKCE: control-plane authorize+callback routes + manifest construct + token refresh | weeks | depends on [[WEIR-I-0047]] storage decision |
| 6 | Session provider: credentialed login bodies + 401-driven re-login | days | `weir-runtime` L1104 follow-up |
| 7 | s3 ListObjectsV2 ContinuationToken loop | days | closes silent >1000-object truncation |
| 8 | Postgres destination typed columns (schema-driven DDL instead of JSONB blob) | week | warehouse-user expectation |
| 9 | Live-verify keyed manifests + verification record | week, gated | on [[WEIR-T-0067]]/[[WEIR-S-0018]] human provisioning |

## Alternatives Considered **[REQUIRED]**

- **Host-side TLS capability in fidius** — rejected with full analysis in [[WEIR-A-0041]] (no implementation point; can't serve TDS 7.x; 2-4 weeks of fidius work whose only clean customer was postgres).
- **Defer TLS entirely, ship "local/self-hosted DBs only"** — rejected: "point at your database" means managed databases for most real users; the guest-side path is cheap enough (~2-3 weeks total) that deferral buys little.
- **Skip OAuth, document refresh-token-by-hand** — retained as the *alpha fallback posture* (task 5 may trail the alpha), but the flow is designed here so it lands right after.

## Implementation Plan **[REQUIRED]**

**Decomposed 2026-08-29** (OAuth task 5 NOT created — deferred per Dylan): 1 → [[WEIR-T-0176]], 2 → [[WEIR-T-0177]], 3 → [[WEIR-T-0178]], 4 → [[WEIR-T-0179]], 6 → [[WEIR-T-0180]], 7 → [[WEIR-T-0181]], 8 → [[WEIR-T-0182]], 9 → [[WEIR-T-0183]]. (The hostname-egress dependency landed early as [[WEIR-T-0175]].)

Order: [[WEIR-T-0176]] + [[WEIR-T-0178]] (TLS core + gates) first — they unblock the flagship "managed database" claim; [[WEIR-T-0179]], [[WEIR-T-0180]], [[WEIR-T-0181]] are independent fillers; [[WEIR-T-0182]] after [[WEIR-T-0176]] (same connector); [[WEIR-T-0183]] whenever the human provisioning ([[WEIR-S-0018]]/[[WEIR-T-0067]]) lands. Alpha cut: [[WEIR-T-0176]], [[WEIR-T-0178]], [[WEIR-T-0179]] are must; [[WEIR-T-0177]] strongly-should; [[WEIR-T-0182]]/[[WEIR-T-0183]] may trail with documented posture.

Dependencies: [[WEIR-A-0041]] (decided), [[WEIR-I-0047]] (OAuth token storage form), [[WEIR-T-0067]]/[[WEIR-S-0018]] (human: accounts + bundles), fidius hostname-egress FR (orthogonal; wire into `HostAllowList` when it lands).

## Exit Criteria

- [ ] A TLS-required managed Postgres (RDS or equivalent) syncs end-to-end with `sslmode=require`, and `verify-full` passes with a supplied CA bundle; the hostssl-reject + negative-SAN integration tests are green
- [ ] A Force-Encryption SQL Server syncs via the forked tiberius path
- [ ] Postgres discover shows real tables in the UI stream dropdown
- [ ] The alpha's connector-support claims (what works, what needs a CA, what's unsupported) are documented and honest
