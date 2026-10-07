# Changelog

All notable changes to this project will be documented in this file.
The API is **v0/unstable** during alpha (WEIR-A-0006): breaking changes may land
between releases and are called out here.

## [Unreleased]

### Added
- Runs API: each `GET /runs` row (and the admin mirror `/tenants/{id}/runs`) now
  carries `started_at` and `finished_at` — RFC 3339 UTC with millisecond
  precision (`2026-10-07T09:25:01.123Z`), `null` until the run starts / while it
  is in flight (WEIR-T-0224). Additive under WEIR-A-0006: existing fields are
  unchanged. `GET /runs/{id}` keeps its epoch-millisecond timestamps. The UI run
  feed shows a "started" column (relative time, UTC on hover).
- Runs API: `GET /runs` takes `?limit=` (default 50, max 500) and `?before=<id>`
  for cursor pagination (pass the smallest `id` of the previous page to walk
  history), and `GET /runs/{id}` returns one run in full — stream, timestamps,
  error, and a run-log tail. Tenant-scoped; admin mirrors under
  `/tenants/{id}/runs`.
- Retention: `weir serve` now prunes finished run history, run logs, and dead
  letters on the scheduler tick — age cap `WEIR_RETENTION_DAYS` (default 30)
  and per-tenant row cap `WEIR_RETENTION_MAX_ROWS` (default 10000); `0`
  disables. In-flight runs are never touched.
- Secret references: a secret field can hold `env:NAME` or `file:/abs/path` in
  place of the value. The runner host reads it on each run; the API never
  resolves it, and a read gives the reference text. A reference in a field that
  is not secret gets `400` (WEIR-T-0202).
- Web UI: each connection has an Edit action. A stored secret shows as
  "unchanged" and is kept unless you type a new value or clear it (WEIR-T-0216).
- Docs: a "Secrets posture" explanation page tells what redaction protects, what
  it does not (the database and its backups hold literal secrets in plaintext),
  and when to use references (WEIR-T-0205).

### Changed
- **Breaking (v0-unstable, WEIR-A-0006):** the connection API no longer returns
  secret values. `GET /connections`, `GET /connections/{name}` and the admin
  mirrors under `/tenants/{id}/connections` give the sentinel
  `__weir_secret_unchanged__` in place of each secret in `source_config` and
  `dest_config`. A field is secret when the connector's `config_schema` marks it
  `airbyte_secret: true` or `format: "password"`, or when it holds a baked auth
  credential (`api_key`, or the key that `basic_password_key`,
  `oauth_client_secret_key`, `oauth_refresh_token_key`, `google_sa_key_key`,
  `snowflake_private_key_key` or `aws_secret_access_key_key` names). On
  `POST /connections`, the sentinel or an omitted secret field keeps the stored
  value, a new value replaces it, and an empty string clears it. A new
  connection that sends the sentinel gets `400`.
- **Breaking (v0-unstable):** `DELETE /tenants/{id}` now **cascades** (WEIR-T-0210).
  It stops the runs of the tenant, then deletes its connections, schedules, runs,
  run logs, dead letters, stream state and schemas, catalog entries, API keys and
  staged connector files (the rows in one transaction). Before, it deleted only
  the tenant row and left the data and the keys of the tenant in place. The
  `default` tenant still cannot be deleted. Audit events are kept.
- The keys of a deleted tenant are refused (`401`) at once: the delete clears the
  API key cache, and key validation refuses a tenant key whose tenant does not
  exist. Minting a key for a tenant that does not exist (for example
  `weir auth token create --tenant acme`) now creates the tenant, and at open
  weir creates the missing tenant rows of keys stored before this change, so
  those keys keep working.
- The declarative `rest` runtime now **streams checkpoints per page** instead of
  buffering the whole paginated read: a run that dies at page N keeps pages
  1..N-1 committed and the next run resumes from the saved position (carried in
  the stream's opaque state), memory is bounded by page size, and the page cap
  (`max_pages`, default 1000, now per run and configurable) logs a loud warning
  and checkpoints resumably instead of silently truncating. The incremental
  cursor query param is now snapshotted for the whole read rather than
  advancing between pages of the same run.
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
- **Security:** a connection with an unknown `auth_scheme` is refused at
  create (`400`) and fails at run time. Before, its config, secrets included,
  went into the guest (WEIR-T-0203).
- **Security:** the first-party connectors mark their credential fields secret
  in `config_schema` (`mssql.password`, `postgres.password` and `postgres.url`),
  so the API redacts them. A test fails when a credential field is not marked
  (WEIR-T-0222).
- **Security:** the host-side credential caches no longer hold secret text in
  their keys, and a changed credential no longer reuses a cached token. The
  connector-handle cache is keyed by a salted SHA-256 digest of the connector,
  the stored config and the fingerprint of resolved `env:`/`file:` values, not
  by the config JSON. Google service-account and Snowflake key-pair tokens are
  cached per identity, together with a digest of the private key they were
  minted from: when the key changes, by an edited config or by a rotated
  `env:`/`file:` reference, the next run mints a new token instead of using the
  old one until it expires.
- **Security:** tenant ids must be safe slugs (`^[a-z0-9][a-z0-9-]{0,62}$`).
  `POST /tenants` refuses any other id (for example `../x`) with a 400, and
  every filesystem path built from a tenant id goes through the validated
  type, so a tenant id cannot escape the connectors directory.
- **Security:** stopping a resident connection is scoped by (tenant, name).
  A stop by tenant A no longer cancels a same-named connection of tenant B,
  and the no-double-start guard is tenant-scoped everywhere.
  **Breaking (v0-unstable, Rust API):** `Relay::cancel` and `Relay::has_active`
  now take the tenant; the name-only `has_active` and `has_active_in` are gone.
- Key revoke is scoped to one tenant (WEIR-T-0221). Before, `DELETE
  /tenants/{id}/keys/{kid}` ignored the tenant and matched on the key name only:
  revoking key `ci` of one tenant revoked every key named `ci` in every tenant,
  and revoking by key id (as the UI does) did nothing. Now `{kid}` is a key id
  or a key name, only the keys of tenant `{id}` match, the revoked key is
  refused (`401`) on the next request (it is purged from the API key cache),
  and the route returns `404` when no live key of the tenant matches.
  `weir auth token revoke <ident>` takes `--tenant <id>` and matches only the
  keys of that tenant (without it, only global keys); `weir auth token list`
  now shows the key id.
- Orchestrator hardening for long continuous operation: work-unit completion is
  now owner-guarded (a lease-expired worker finishing late can no longer
  clobber a re-claimed unit's state, and a worker's exit can no longer
  resurrect a cancelled run); one run's executor error — including a panicking
  connector run — fails only that run instead of aborting the whole drain pass
  and stranding its lease; one broken schedule no longer stops the remaining
  schedules from firing; work-unit ids are nonce-seeded per process so a
  restart can't re-mint an id; resident restart backoff is capped (5 min) and
  resets after a minute of healthy uptime; the connector handle cache is
  LRU-bounded; and a perpetually-requeuing unit fails the drain loudly instead
  of spinning it forever.
- Outbound query values were never URL-encoded: an ISO datetime cursor with a
  `+02:00` offset arrived server-side with a space, and reserved characters
  (`& = #`) in a cursor, opaque page token, or query-param API key corrupted
  the query string. Values (rest runtime + host-injected query credentials) are
  now percent-encoded exactly once at build time; param names go verbatim.
- The `rest` runtime treated ANY error page past page 1 as normal end-of-data,
  so a mid-sync auth expiry or rate limit ended the run "successfully" with
  partial data. Stops are now status-aware: an error status fails the run
  naming the status + page (safe — prior pages are checkpointed and the retry
  resumes), a 401 is transient so the worker re-runs with a freshly resolved
  credential, and only a 2xx empty page or a 404 one-past-the-end still count
  as the legitimate end.
- Incremental cursors on typed (numeric/timestamp) columns compared
  lexicographically and could re-deliver rows; the postgres predicate now
  compares in the column's native type, and the client-side cursor advance in
  the rest/mssql/snowflake connectors uses a shared numeric-aware compare
  (`"9" < "12"`; strings/timestamps unchanged).
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
