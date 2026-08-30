---
id: postgres-guest-tls-pgstream-enum
level: task
title: "Postgres guest TLS — PgStream enum, SSLRequest, sslmode/sslrootcert, SCRAM binding fix"
short_code: "WEIR-T-0176"
created_at: 2026-08-29T14:26:21.670492+00:00
updated_at: 2026-08-29T15:27:02.168049+00:00
parent: WEIR-I-0043
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0043
---

# Postgres guest TLS — PgStream enum, SSLRequest, sslmode/sslrootcert, SCRAM binding fix

## Parent Initiative

[[WEIR-I-0043]]

## Objective **[REQUIRED]**

Give the postgres connector guest-side TLS per [[WEIR-A-0041]] so TLS-requiring managed Postgres (RDS, Cloud SQL, Azure) can connect. Today `PgConn` speaks plain TCP with no SSLRequest path — "point weir at your database" fails against every TLS-required host.

## Design (settled by [[WEIR-A-0041]])

- **Stack**: rustls 0.23 (default-features off) + the pure-Rust `rustls-rustcrypto` provider + compiled-in webpki-roots. Compile-proven for wasm32-wasip2 (ring/aws-lc need a wasi-sdk clang); expect ≈ +1–1.2 MB guest size. Provider is alpha/unaudited — a recorded trade-off with review triggers on the ADR.
- **Wire**: insert the SSLRequest/'S' exchange in `PgConn::connect` (`crates/connectors/postgres/src/lib.rs`); the stream becomes a `Plain|Tls` enum over `fidius_guest::sockets::tcp::TcpStream` (rustls::Stream layered on it is fidius's own worked example).
- **Config**: `sslmode = disable | require | verify-full` (default **require**; `prefer` rejected as a dishonest silent fallback) + `sslrootcert` as inline PEM ([[WEIR-A-0037]]: read lazily per connect, never cached).
- **SCRAM channel-binding fix in the same change**: over TLS the GS2 flag must switch to `unsupported()` — servers advertising SCRAM-SHA-256-PLUS reject the current `unrequested()`.
- **Productize from the ADR's experiment artifacts** (2026-08-17 scratchpad `exp3-pg-guest-tls`); if the scratchpad has expired, the ADR's design notes above are sufficient to re-derive.

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [x] `sslmode=require` completes the SSLRequest upgrade and syncs against an `ssl=on` Postgres; `sslmode=disable` preserves today's plaintext path byte-for-byte
- [x] `verify-full` verifies hostname + chain against `sslrootcert` (inline PEM) or webpki-roots; the negative-SAN test in [[WEIR-T-0178]] fails it (proves verification is real, not `trust_cert` theater)
- [x] SCRAM works against a SCRAM-SHA-256-PLUS-advertising server over TLS (channel-binding GS2 flag `unsupported()`)
- [x] config_schema documents the new keys; wasm32-wasip2 build green; existing postgres suites (unit + docker-gated) green; wire-proof gates land with [[WEIR-T-0178]]

## Status Updates **[REQUIRED]**

**2026-08-29 — implemented + wire-proven ad hoc (ralph run); [[WEIR-T-0178]] productizes the gates.**

- **Implementation** (`crates/connectors/postgres`): `PgStream` enum (`Plain(TcpStream) | Tls(Box<rustls::StreamOwned>)`) with Read/Write delegation; SSLRequest (hand-encoded 8-byte 80877103) + one-byte 'S'/'N' answer in `PgConn::connect` — 'N' under require/verify-full is a hard, explanatory error (no silent fallback); `tls_connection()` builds rustls 0.23 (default-features off, std+tls12) on the `rustls-rustcrypto` provider — verify-full uses `sslrootcert` inline PEM (parsed per connect, [[WEIR-A-0037]]) else compiled-in webpki-roots; require uses an explicit `AcceptAnyCert` danger verifier (libpq semantics). Config: `sslmode` (default **require**) + `sslrootcert` via JSON fields or libpq-style `?sslmode=` URL query (JSON overrides URL); unknown sslmode errors at connect. config_schema documents both keys.
- **SCRAM fix**: GS2 flag `ChannelBinding::unsupported()` ("n") unconditionally — the old `unrequested()` ("y") is rejected as downgrade protection by any PLUS-advertising (= TLS) server; "n" is honest in both modes.
- **Wire proof (all ad hoc against real servers; [[WEIR-T-0178]] turns these into compose/CI gates):** `sslmode=disable` → 11/11 docker-gated suite vs plaintext postgres:16 (plaintext path + the new "n" SCRAM flag proven against scram-sha-256 auth); `sslmode=require` → **11/11 vs ssl=on postgres:16** (SSLRequest upgrade, rustls-rustcrypto handshake, SCRAM vs a SCRAM-SHA-256-PLUS-advertising server, full source/dest/CDC CRUD through TLS); `verify-full` + inline-PEM CA → pass; `verify-full` vs wrong-SAN leaf under the same CA → exact rustls hostname rejection ("certificate not valid for name \"localhost\" … only valid for DnsName(\"wrong.example.net\")"); `verify-full` vs untrusted self-signed → chain rejection; `require` vs plaintext-only server → clear refusal message.
- **Harness hooks** (`wasm_postgres_engine.rs`): `raw_pg_url()` (query-stripped URL for the NoTls seed/verify client) and `WEIR_TEST_PG_SSLROOTCERT` (PEM file path → inline sslrootcert in the connector config) — what T-0178's harness drives. Default test URL carries `?sslmode=disable` (the integration postgres is plaintext).
- **Docs**: first-sync tutorial + cdc-deletes guide updated for the require default (`?sslmode=disable` for local plaintext; verify-full/sslrootcert pointers).
- **Size**: guest 506 KB → 1.49 MB (inside the ADR's predicted band). `angreal check all` clean. Dep notes: rustls-pki-types ≥1.11 (pem API rides std — no "pem" cargo feature exists); rustls-rustcrypto 0.0.2-alpha (unaudited — the ADR review trigger stands).
