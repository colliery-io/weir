# Connection config

A **connection** is the unit weir runs: a source, a destination, a stream, and config. This page is the field
reference; to create one, see the [CLI](cli.md) (`weir connection add`) or `POST /connections`.

## Fields

| Field | Meaning |
| --- | --- |
| `name` | Unique (per tenant) identifier. |
| `source` | Source connector name (e.g. `postgres`, `slow`, or an onboarded manifest like `exchangerate`). |
| `dest` | Destination connector name (e.g. `postgres`, `arrow-sink`, `rest-dest`). |
| `stream` | The stream to move; for table-shaped destinations this is the target table. |
| `config` | Convenience: JSON applied to **both** sides when a per-side config isn't given. |
| `source_config` | Source connector config (JSON); overrides `config` for the source. |
| `dest_config` | Destination connector config (JSON); overrides `config` for the destination. |
| `every_secs` | Optional schedule interval in seconds (the daemon fires it). |
| `cron` | Optional cron expression (6/7-field, seconds-first); takes precedence over `every_secs`. |
| `sync_mode` | How the source reads: `full_refresh` (default) \| `incremental` \| `cdc`. |
| `write_mode` | How the destination applies: `append` (default) \| `upsert` \| `overwrite`. |
| `business_keys` | Key set for `upsert` (required when `write_mode = upsert`). |
| `cursor_field` | Cursor field for `incremental` (required when `sync_mode = incremental`). |

The source and destination resolve with **independent** config ([[WEIR-I-0029]]) — `source_config` /
`dest_config` each override the shared `config` for their side, so a `postgres → postgres` connection can read
one table and write another. weir strips its own reserved keys (e.g. an embedded `__mapping`) before the config
reaches the guest.

## Host-side auth (`auth_scheme`)

Credentials are resolved and attached **on the host** — secret fields are stripped before the config reaches
the sandboxed connector. `auth_scheme` selects the scheme: `bearer` / `header` / `query` (an `api_key`),
`basic`, `oauth2` (client-credentials or refresh-token grants), `aws_sigv4`, `google_service_account`,
`snowflake_keypair_jwt`, and `session`. The **session** scheme performs a login request and injects the
extracted token per request:

| Key | Meaning |
| --- | --- |
| `session_login_url` | The login endpoint. |
| `session_login_method` | Login request method (default `POST`). |
| `session_login_body` | JSON object template for the login body. String values of the form `"{{key}}"` are substituted from the config — and those keys are stripped as secrets (e.g. `{"username": "{{admin_user}}", "password": "{{admin_pass}}"}`). |
| `session_token_path` | Dot-path (or array) into the login response holding the token. |
| `session_inject_header` | Header the token is injected as (default `Authorization`). |
| `session_ttl_secs` | Proactive re-login interval: a cached token older than this is re-minted **before** use, so long syncs survive expiring tokens. `0`/absent = cache for the run. A mid-run `401` fails the attempt; the automatic retry re-logs-in and resumes from the checkpoint. |

## In-flight mapping

An optional ordered `MappingSpec` reshapes each record **between** read and write — `select` / `drop` / `rename`
/ `cast` / `filter` / `compute`. It is bounded per-record shaping, not a transform engine. The operator table and
edge-case semantics are in [Map fields](../guides/field-mapping.md).

## Typed columns (postgres destination)

The `postgres` destination lands **typed relational columns** inferred from the records — `{"n": 1, "tag":
"a"}` becomes `n bigint, tag text`, with upsert keys as a typed primary key and new fields ALTERed in
additively. Unmapped shapes (objects/arrays, mixed types) land as `jsonb` **per column**. Set
`typed_columns: false` in the destination config to restore the legacy single-`data JSONB` layout (tables
created before this default keep working — new typed columns are added alongside).

## Write modes

How the destination applies records — `Append`, `Upsert { business_keys }`, `Overwrite` (see the
[connector contract](connector-contract.md#write-modes)). For CDC destinations, deletes apply per the Postgres
destination's delete config:

| Key | Default | Meaning |
| --- | --- | --- |
| `on_delete` | `hard` | `hard` → `DELETE` the row by key; `tombstone` → set a tombstone column instead. |
| `tombstone_column` | `_deleted_at` | The column stamped with the delete time when `on_delete = tombstone`. |

Set the sync/write mode per connection with `--sync-mode` / `--write-mode` / `--business-keys` (or the API
fields); `on_delete` / `tombstone_column` go in the connector `config`. See
[Capture changes and propagate deletes](../guides/cdc-deletes.md).

## Typed schemas

Each stream carries a typed schema — a list of `{ name, type, nullable }` fields, with `type` one of `str`,
`integer`, `float`, `boolean`, `timestamp`, `json`. weir captures it on the first run (from a connector-declared
schema, else inferred from a record sample), persists it, and on later runs **coerces each record to it** — an
uncoercible value or a missing required field dead-letters that record. On re-inference it detects **drift**:
additive fields merge; a breaking type change flags the stream (visible in the UI, cleared by accepting the new
schema). `GET /connections/{name}/schema` returns the captured schema + any drift flag.
