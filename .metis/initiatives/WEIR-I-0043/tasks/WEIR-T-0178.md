---
id: tls-test-harness-compose-postgres
level: task
title: "TLS test harness — compose postgres-tls + mssql forceencryption + cert-gen + negative SAN gate"
short_code: "WEIR-T-0178"
created_at: 2026-08-29T14:26:30.232790+00:00
updated_at: 2026-08-29T15:35:01.191646+00:00
parent: WEIR-I-0043
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0043
---

# TLS test harness — compose postgres-tls + mssql forceencryption + cert-gen + negative SAN gate

## Parent Initiative

[[WEIR-I-0043]]

## Objective **[REQUIRED]**

Build the wire-proof gates [[WEIR-A-0041]] requires before ANY TLS claim ships: integration infrastructure that makes "TLS claimed but not engaged" and "verification claimed but not real" testable failures, not review opinions.

## Design (per [[WEIR-A-0041]] §5)

- **postgres-tls compose service** (integration profile): Postgres with `ssl=on` and a **hostssl-only pg_hba** — a plaintext connection is REJECTED, so a passing sync proves TLS actually engaged (not silently fell back).
- **mssql Force-Encryption variant**: the mssql service (or a second one) with Force Encryption on — gates [[WEIR-T-0177]].
- **Cert generation**: scripted, ephemeral test CA + server certs (SANs controlled) — never committed key material; generated into the compose volume at up.
- **Negative verify-full SAN test**: a server cert with the WRONG SAN must fail `verify-full` (postgres) and default verification (mssql) — proves verification is real.
- **integration.yml switches to compose** (`docker compose --profile integration up -d --wait`): CI currently provisions a Postgres-only service container (`.angreal/task_integration.py` docstring records the gap), so none of these gates would run in CI without the switch.
- CA/root material read lazily per connect, never cached across runs ([[WEIR-A-0037]]).

## Acceptance Criteria **[REQUIRED]**

- [x] `postgres-tls` service up: hostssl-only pg_hba rejects plaintext; the [[WEIR-T-0176]] `sslmode=require` sync passes against it and a plaintext attempt demonstrably fails
- [x] `verify-full` negative gate: wrong-SAN server cert → sync fails with a verification error (and the mssql equivalent for [[WEIR-T-0177]])
- [x] Force-Encryption SQL Server service available for [[WEIR-T-0177]]'s gates
- [x] Cert-gen scripted + ephemeral (no committed keys); `angreal integration up/test` covers the new services; integration.yml runs the compose estate so the gates run in CI
- [x] Ordering honored: this task's harness lands WITH or BEFORE the first TLS claim from [[WEIR-T-0176]]/[[WEIR-T-0177]]

## Status Updates **[REQUIRED]**

**2026-08-29 — implemented + all four gates run green locally (ralph run).**

- **Cert-gen** (`scripts/gen-test-tls.sh`): ephemeral CA + localhost-SAN leaf + hostssl-only pg_hba + Force-Encryption mssql.conf into `target/weir-tls-certs/` (gitignored via target/; day-fresh idempotence). Wired into `angreal integration up`/`test`.
- **Compose services** (integration profile): `postgres-tls` (ssl=on, `hba_file` = hostssl-only, wal_level=logical, port `WEIR_PG_TLS_HOST_PORT`:5434) and `mssql-tls` (mssql.conf forceencryption=1, healthcheck via `sqlcmd -N -C`, port `WEIR_MSSQL_TLS_HOST_PORT`:11433). Both use an entrypoint wrapper copying the bind-mounted material so the key gets daemon-owned 0600 perms regardless of host uid — the classic bind-mount ownership trap on Linux CI.
- **Wire gates** (`wasm_postgres_engine.rs`, docker-gated, all green): `tls_require_syncs_against_hostssl_only_server` (engagement proof: plaintext is impossible there), `tls_plaintext_is_rejected_by_hostssl_only_server` (the proof's other half), `tls_verify_full_with_ca_syncs` (inline-PEM sslrootcert), `tls_verify_full_wrong_name_fails` (negative SAN: dialing 127.0.0.1 against a DNS:localhost-only cert → rustls hostname rejection — same server, no extra service). 4/4 in 11s.
- **mssql negative equivalent**: deferred to [[WEIR-T-0177]] by design — no client can speak TDS-TLS until the fork lands; the Force-Encryption server (its gate) is up and healthchecked.
- **CI**: integration.yml rewritten from the Postgres-only service container to the full compose estate (cert-gen step → `docker compose --profile integration up -d --wait` → `cargo test --workspace -- --ignored`, estate logs on failure, wasm32-wasip2 target for guest builds). Side benefit: CI postgres now has `wal_level=logical`, so the CDC ignored tests can actually pass there (the old service container never set it).
- `docker compose --profile integration config -q` valid; `angreal check all` clean; estate torn down after the run.
