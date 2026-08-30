---
id: weir-tiberius-fork-productization
level: task
title: "weir-tiberius fork productization + mssql TLS config"
short_code: "WEIR-T-0177"
created_at: 2026-08-29T14:26:26.088518+00:00
updated_at: 2026-08-29T19:44:34.185991+00:00
parent: WEIR-I-0043
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0043
---

# weir-tiberius fork productization + mssql TLS config

## Parent Initiative

[[WEIR-I-0043]]

## Objective **[REQUIRED]**

Enable TLS for the mssql connector by productizing the minimal `weir-tiberius` fork ([[WEIR-A-0041]]) and dropping the `EncryptionLevel::NotSupported` pin (`crates/connectors/mssql/src/lib.rs:239-242`), so Force-Encryption SQL Server (and Azure SQL) can sync.

## Design (settled by [[WEIR-A-0041]])

- **Why a fork**: classic TDS 7.x tunnels the TLS handshake INSIDE PRELOGIN packets — only tiberius's own `TlsPreloginWrapper` can express it, and upstream (dormant at 0.12.3) hardwires native-tls/rustls stacks that don't build for wasm32-wasip2. TDS 8.0 "strict" has no Rust client — documented unsupported (watch Microsoft `mssql-tds-preview`).
- **Fork delta stays tiny** (compile-proven overlay from the ADR's `exp-tib-fork` / `fork-overlay/` artifacts): rewritten `tls_stream` module rebased to futures-rustls 0.26 / rustls 0.23 / rustls-rustcrypto, an inline-PEM trust API, and the pre-existing 32-bit PLP sentinel literal fix. `TlsPreloginWrapper` handles the TDS 7.x tunneling over the existing SyncSock adapter.
- **Config**: `tls_mode = require | prefer | disable` + `tls_ca_pem` (inline PEM) + explicit `tls_insecure_skip_verify` opt-out; default verifies against webpki-roots.
- **Open sub-decision (settle with Dylan at pickup)**: where the fork lives — a `colliery-io/weir-tiberius` repo (git dep) vs vendored in-tree. Either way weir owns it indefinitely; keep the delta reviewable.

## Acceptance Criteria **[REQUIRED]**

- [x] `weir-tiberius` builds as part of the mssql guest for wasm32-wasip2 with `EncryptionLevel::Required` available; the NotSupported pin is gone
- [x] `tls_mode=require` syncs against a Force-Encryption SQL Server ([[WEIR-T-0178]] harness); `disable` preserves today's plaintext demo path
- [x] Verification is real: default checks against webpki-roots/`tls_ca_pem`; `tls_insecure_skip_verify` is an explicit, logged opt-out; the negative verification gate in [[WEIR-T-0178]] fails a bad chain
- [x] TDS 8.0 strict-only servers documented as unsupported; fork location + delta documented (repo or vendored, per the sub-decision)

## Status Updates **[REQUIRED]**

**2026-08-29 — implemented + wire-proven (ralph run). Fork location: VENDORED in-tree (Dylan's call at pickup).**

- **The fork** (`vendor/weir-tiberius/`, upstream tiberius 0.12.3 + README documenting the delta; lib target still named `tiberius` so connector imports are unchanged): (1) `rustls_tls_stream.rs` fully rewritten on **rustls 0.23 driven directly** with rustls-rustcrypto + webpki-roots; (2) `Config::trust_cert_pem` / `TrustConfig::CaCertificatePem` inline-PEM trust (guests have no fs); (3) the 32-bit PLP sentinel literals made an explicit truncating-cast constant (upstream only compiles on wasm32 because dependency lint-capping silences the overflow; the truncated value still routes to the PLP decode arm — invariant documented); (4) manifest trimmed to tds73/rustls + value-type optionals.
- **The bug the ADR's overlay never hit** (scratchpad artifacts had expired; re-derivation found it): futures-rustls's handshake future drains `wants_read()` and only exits that loop via `Poll::Pending` — on the guest's always-`Ready` blocking SyncSock it deadlocks after the handshake completes, blocked reading a TDS PRELOGIN header the server never sends (server waits for LOGIN7 → mutual deadlock → ~110 s server-side close). Diagnosed with a native repro + a TDS-framing proxy + a process stack sample; fixed by dropping futures-rustls entirely and hand-driving rustls with a handshake loop that terminates on `!is_handshaking()` (correct for blocking AND async transports). Upstream `TlsPreloginWrapper` is untouched.
- **Connector** (`crates/connectors/mssql`): NotSupported pin GONE; `tls_mode = require (default) | prefer | disable` → EncryptionLevel Required/On/NotSupported; `tls_ca_pem` inline PEM; `tls_insecure_skip_verify` explicit opt-out (driver logs it loudly); unknown mode errors at connect; config_schema documents all three keys. Guest builds wasm32-wasip2 with tds73+rustls.
- **Harness additions**: `mssql-tls` Force-Encryption compose service hardened (root-staged certs → `/var/opt/mssql/certs` with daemon-owned 0600 — the image runs as non-root, so the first attempt silently booted UNconfigured with its own self-signed **v1** cert and FE off; caught because the plaintext gate wrongly passed) + `mssql-tls-seed` (sqlcmd -N -C).
- **Wire gates green** (`wasm_mssql_engine.rs`, docker-gated): `mssql_tls_require_with_ca_reads_rows` (in-PRELOGIN TDS 7.x handshake + chain/hostname verification + rows through Force-Encryption TLS), `mssql_tls_skip_verify_reads_rows`, `mssql_tls_wrong_name_fails_verification` (127.0.0.1 vs DNS:localhost-only cert), `mssql_tls_plaintext_mode_is_rejected`. Full suite 7/7 (3 plain regression w/ `tls_mode=disable` + 4 TLS) in one run.
- **Sundries**: demo `pipelines.toml` mssql source gains `tls_mode = "disable"` (plain compose service); testkit freshness cache now watches `vendor/` so fork edits rebuild guests (real trap hit during debugging); fork README carries the TDS 8.0-strict unsupported note + delta inventory. `angreal check all` clean; unit wall 12/12.
