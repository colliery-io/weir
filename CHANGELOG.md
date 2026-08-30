# Changelog

All notable changes to this project will be documented in this file.
The API is **v0/unstable** during alpha (WEIR-A-0006): breaking changes may land
between releases and are called out here.

## [Unreleased]

### Changed
- **Breaking (v0-unstable):** the `postgres` destination now lands **typed
  relational columns** inferred from the records (bigint/double
  precision/boolean/timestamptz/text, per-column jsonb fallback) instead of a
  single `data JSONB` column; set `typed_columns: false` to restore the legacy
  layout. Existing tables gain typed columns additively on the next write.
- The `postgres` and `mssql` connectors default to **TLS**
  (`sslmode=require` / `tls_mode=require`, guest-side rustls per WEIR-A-0041)
  with real verification available (`verify-full` + inline-PEM roots); set
  `disable` for plaintext-only dev servers. mssql TLS rides the vendored
  `weir-tiberius` fork (TDS 7.x in-PRELOGIN handshake).

### Fixed
- Incremental cursors on typed (numeric/timestamp) columns compared
  lexicographically and could re-deliver rows; the predicate now compares in
  the column's native type.
- SigV4 signing double-encoded already-encoded query values (S3 listing
  continuation tokens, escaped prefixes) → 403s; canonicalization now encodes
  exactly once.
- The s3 source follows ListObjectsV2 continuation tokens — buckets over 1000
  objects no longer truncate silently.
- Postgres `discover()` introspects real tables (information_schema, with
  primary keys) instead of returning a stub stream; connection failures
  surface as discover errors.

## [0.0.1-alpha] - 2026-08-29

The first published alpha.

### Added
- First-run bootstrap: `weir api` / `weir serve` on a fresh store mint the admin
  API key and print it once (previously only `weir init` did, so the Docker demo
  booted to an unpassable sign-in gate).
- Release pipeline: tarballs now contain the embedded web UI, the staged WASM
  connectors, and the vendored manifests; a multi-arch container image is
  published to `ghcr.io/colliery-io/weir` (the Helm charts' default image).
- A `WEIR_REQUIRE_UI` guard makes UI-less release/image builds fail loudly
  instead of silently shipping a headless binary.
- Connection creation validates up front: an unknown source/sink connector or a
  config missing required fields is rejected with a clear error instead of
  failing at first run.
- Record-cursor pagination for declarative REST connectors
  (`page_cursor_record_field` + `page_stop_on_false_path`) — the construct
  Stripe-style APIs need; the Stripe, HubSpot, and Airtable manifests now
  paginate.
- Retry knobs: `WEIR_MAX_ATTEMPTS` (default 3) and `WEIR_RETRY_BASE_MS`
  (default 1000) tune scheduled-run retry behavior.
- UI error surfacing: rejected mutations show the server's reason, a banner
  appears when the control plane is unreachable, and toasts auto-dismiss.
- A "first-hour triage" guide, plus refreshed installation, CLI, and manifest
  docs.

### Changed
- The Docker demo profile (`docker compose --profile demo up --build`) now
  starts only weir + Postgres; the test-only services (MSSQL, Dex, MinIO) moved
  behind the `integration` compose profile (`angreal integration up`).
- README rewritten as a real front door (what weir is, Docker + from-source
  quickstarts, actual repository layout).
- Schedules are tenant-scoped and re-register on any config change, so an
  edited connection's next scheduled run uses the new config.
- Host ports for the Docker stacks are overridable (`WEIR_HTTP_HOST_PORT`,
  `WEIR_PG_HOST_PORT`, `WEIR_MSSQL_HOST_PORT`, `WEIR_MINIO_HOST_PORT`).

### Fixed
- Long-lived (resident) sources are stopped cleanly on server shutdown instead
  of wedging the process.
- Scheduled connections owned by non-default tenants now fire.
- The s3 connector is staged into release artifacts and the Docker image.
