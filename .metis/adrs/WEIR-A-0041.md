---
id: 001-db-connector-tls-terminates-guest
level: adr
title: "DB connector TLS terminates guest-side (no host TLS capability)"
number: 1
short_code: "WEIR-A-0041"
created_at: 2026-08-18T01:57:37.838463+00:00
updated_at: 2026-08-18T01:59:19.666112+00:00
decision_date:
decision_maker:
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-1: DB connector TLS terminates guest-side (no host TLS capability)

**Status:** Decided (2026-08-17). *Raised by: the alpha-readiness review (2026-08-16) via [[WEIR-I-0043]] Real-source reach. Investigated 2026-08-17 (five-agent deep-dive over fidius + weir + compile experiments; scratchpad experiments referenced below).*

## Context **[REQUIRED]**

Managed databases (RDS, Cloud SQL, Azure SQL/PostgreSQL) require TLS, and neither DB connector supports it: postgres speaks plain TCP with no SSLRequest path (`crates/connectors/postgres/src/lib.rs`, PgConn), mssql pins `tiberius::EncryptionLevel::NotSupported` (`crates/connectors/mssql/src/lib.rs:239-242`). "Point weir at your database" fails against every TLS-requiring host. Connectors are wasm32-wasip2 fidius guests whose only network path is policy-gated egress ([[WEIR-A-0033]] host-side credentials, [[WEIR-A-0039]] brokered ws precedent), so the question is **where TLS terminates**: in the guest, or in a new host capability.

Constraints discovered during the investigation (all verified against the fidius checkout and by compile experiments):

- **fidius does not own the TCP byte path.** Guest `TcpStream` is a newtype over `std::net::TcpStream`, which on wasm32-wasip2 *is* wasi:sockets, served by wasmtime-wasi; fidius's only hook is a connect-time `socket_addr_check` calling `EgressPolicy::authorize_tcp(SocketAddr)` (fidius `executor/wasm.rs:460-490`). The connected socket is a private `Arc<tokio::net::TcpStream>` inside wasmtime-wasi — no replacement seam.
- **A guest cannot name its socket across an import** (the wasi fd table is guest-internal), so `start_tls(handle)` implies a whole new fidius-owned brokered-TCP subsystem (~700-1200 lines, 2-4 weeks), not an incremental FR.
- **Classic TDS 7.x tunnels the TLS handshake inside PRELOGIN packets** — a socket-level host wrap corrupts TDS framing, and the upgrade moment is unreachable inside `tiberius::Client::connect`. TDS 8.0 "strict" (TLS-from-byte-0) has no published Rust client (tiberius tops out at TDS 7.4).
- **fidius's documented design intent for this tier is guest-side rustls** (`fidius-guest/src/sockets.rs:21-23, 57-62` shows `rustls::Stream` layered over the guest TcpStream as the worked example).
- **Compile experiments (2026-08-17):** rustls 0.23 + pure-Rust `rustls-rustcrypto` provider + webpki-roots compiles cleanly to wasm32-wasip2 (ring and aws-lc-rs both fail without a wasi-sdk clang); a realistic postgres mix builds at ~1.25 MB (~506 KB → ~1.5-1.7 MB connector); a minimal tiberius fork (2 files + 1 pre-existing 32-bit literal fix, rebased to futures-rustls 0.26/rustls 0.23/rustls-rustcrypto) compiles and links as a wasip2 cdylib with `EncryptionLevel::Required`.
- **Host-side TLS would buy no secret confinement for DB connectors** — the DB password is already in-guest (SCRAM/LOGIN7 run guest-side); the only stake was verification placement.

## Decision **[REQUIRED]**

**TLS for DB/wire-protocol connectors terminates in the guest**, composed over the existing brokered TCP stream:

1. **rustls 0.23 (default-features off) with the pure-Rust `rustls-rustcrypto` provider and compiled-in webpki-roots** as the guest TLS stack. The provider's alpha maturity is an explicit, recorded trade-off (see Review Triggers); the audited fallback (ring provider + wasi-sdk clang in CI) stays open.
2. **postgres:** SSLRequest/'S' exchange inserted in `PgConn::connect`, stream becomes a `Plain|Tls` enum; config surface `sslmode = disable | require | verify-full` (default `require`; `prefer` rejected as a dishonest silent fallback) + `sslrootcert` as inline PEM. The SCRAM channel-binding flag switches to `unsupported()` over TLS in the same change (servers advertising SCRAM-SHA-256-PLUS reject the current `unrequested()` GS2 flag).
3. **mssql:** a minimal maintained fork (`weir-tiberius`: rewritten `tls_stream` module + inline-PEM trust API + the 32-bit PLP sentinel fix), tiberius's own `TlsPreloginWrapper` handles TDS 7.x tunneling over the existing SyncSock. Config `tls_mode = require | prefer | disable` + `tls_ca_pem` + explicit `tls_insecure_skip_verify` opt-out; default verifies against webpki-roots. TDS 8.0 strict-only servers are documented as unsupported until a TDS 8.0 client exists (watch Microsoft's `mssql-tds-preview`).
4. **No host TLS capability is requested from fidius.** The one fidius FR filed is orthogonal: hostname-carrying TCP egress (`TcpTarget { host, addr }` + resolve-and-pin) so allow-lists can speak names — needed for egress honesty regardless of TLS placement.
5. **Wire-proof gates before any TLS claim ships:** integration tests against `ssl=on` Postgres with hostssl-only pg_hba (plaintext rejected — proves TLS engaged), a Force-Encryption SQL Server, and a negative verify-full SAN test (proves verification is real). CA/root material is read lazily per connect, never cached across runs ([[WEIR-A-0037]]).

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk Level | Implementation Cost |
|--------|------|------|------------|-------------------|
| **Guest-side rustls (chosen)** | fidius's documented design for this tier; no fidius change; compile-proven for both connectors; guest already holds the hostname for SNI; only option that covers TDS 7.x PRELOGIN tunneling | ~+1-1.2 MB per guest, TLS stack duplicated per connector; verification runs in the guest where the host can't observe it; rustls-rustcrypto is 0.0.2-alpha/unaudited; mssql needs a maintained tiberius fork | Medium | pg ~2-4 days; mssql ~1-2 weeks (weir repo only) |
| Host `start_tls` capability (new fidius brokered-TCP subsystem) | Host-owned roots + verification, one TLS implementation | No implementation point today — fidius doesn't own the byte path and guests can't name sockets across imports; duplicates wasi:sockets semantics; still cannot express TDS 7.x (host would need TDS framing), so a tiberius fork is needed anyway; guest-claimed hostname must be cross-checked | High | 2-4 weeks in fidius + connector migration; only clean customer is postgres |
| Host TLS from byte 0 | Trivial where it applies | Postgres is STARTTLS-style (server speaks cleartext first) — impossible; TDS 8.0 strict has no Rust client | High (unusable) | n/a |

## Rationale **[REQUIRED]**

The investigation reframed the choice: the host-side capability was priced as an FR but is actually a subsystem fidius's architecture deliberately avoids (the tcp tier is a dumb byte pipe; [[WEIR-A-0039]] validated that philosophy), and it cannot serve the harder of the two connectors regardless. Guest-side TLS is the framework's own worked example, requires zero cross-repo dependency on the critical path, and every load-bearing claim was verified empirically (compiles, sizes, the tiberius fork) rather than assumed. The security trade — verification inside the guest, on an alpha-maturity provider — is real but bounded: the wire-proof integration gates make "TLS claimed but not engaged/verified" testable, and the provider choice is reversible per connector via the ring+wasi-sdk fallback without touching the design.

## Consequences **[REQUIRED]**

### Positive
- No fidius work on the alpha critical path; DB TLS is ~2-3 weeks entirely in weir ([[WEIR-I-0043]]).
- Azure SQL/PostgreSQL work out of the box (public roots in webpki); RDS/Cloud SQL work with a user-supplied CA PEM in connection config — consistent with [[WEIR-A-0037]] consumed-not-managed.
- The postgres pattern (Plain|Tls stream enum + rustls) becomes the template for every future wire-protocol connector (MySQL etc.).

### Negative
- weir owns a tiberius fork indefinitely (upstream dormant at 0.12.3); delta kept deliberately tiny (2 files + 1 line + one trust API).
- Per-guest binary cost ~3x (≈506 KB → ≈1.5-1.7 MB) and a duplicated TLS stack per DB connector.
- rustls-rustcrypto is unaudited alpha — the TLS trust root for customer databases until reviewed or replaced (see Review Triggers).
- The host cannot observe whether a guest verified the peer; connector review/conformance ([[WEIR-A-0031]]) is the compensating control.

### Neutral
- TDS 8.0 strict-only servers unsupported for now; documented limitation.
- The fidius hostname-egress FR (filed 2026-08-17) proceeds independently; when it lands, `HostAllowList` grows name matching (~1 hour, `crates/weir-runtime/src/lib.rs:246`).
- Compile-experiment artifacts and the fork overlay live in the session scratchpad (`exp1a/1b/1c`, `exp2`, `exp3-pg-guest-tls`, `exp-tib-fork`, `fork-overlay/`) — productize from there, don't rediscover.

## Review Schedule

### Review Triggers
- rustls-rustcrypto publishes a non-alpha release or an audit → re-pin and record.
- A TDS 8.0-capable Rust client matures (Microsoft `mssql-tds-preview` / `mssql-tiberius-bridge`) → add strict mode, revisit the fork.
- A third wire-protocol connector needs TLS → reassess whether the per-guest duplication justifies a shared fidius surface.
- The fidius hostname-egress FR lands → wire `authorize_tcp_target` into weir's allow-list.
