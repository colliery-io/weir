# CLI reference

The `weir` binary is the single-node control plane and operator tool. A global `--db <url>` selects the store
(a path/`:memory:` for SQLite, or a `postgres://` URL); it defaults to a local database. Run `weir <command>
--help` for the authoritative flags.

## Store + connections

| Command | Purpose |
| --- | --- |
| `weir init` | Initialise the store and mint the first **admin API key** (printed once). |
| `weir connection add --name --source --dest --stream [--config] [--every\|--cron] [--sync-mode] [--write-mode] [--business-keys] [--cursor-field] [--execution-mode]` | Add or replace a connection. `--source`/`--dest` are connector names; `--config` is JSON applied to both sides, with `--source-config`/`--dest-config` overriding each. `--sync-mode` = `full_refresh`\|`incremental`\|`cdc`; `--write-mode` = `append`\|`upsert`\|`overwrite`; `--business-keys` is comma-separated (for upsert); `--cursor-field` (for incremental); `--execution-mode` = `run_once` (default) \| `resident` (long-lived source). |
| `weir connection list` | List connections. |
| `weir run --name <name>` | Run a connection once (plan + drain, synchronous). |
| `weir start --name <name>` | Start a **resident** (long-lived) source: enqueue-once; a worker runs it indefinitely. |
| `weir stop --name <name>` | Stop a resident source (durable — ends its supervised restart loop). |

## Daemons

| Command | Purpose |
| --- | --- |
| `weir serve [--poll 0.5] [--concurrency 16]` | Run the scheduler **and** worker fleet until ctrl-c. `--poll` bounds schedule resolution; `--concurrency` caps parallel syncs. On a **fresh store**, mints + prints the admin API key once. |
| `weir api [--port 8080] [--concurrency 16]` | Serve the control-plane HTTP API + embedded UI (also runs the scheduler + workers in-process). On a **fresh store**, mints + prints the admin API key once. |
| `weir runner [--poll 0.5] [--concurrency 16] [--tenant <id>]` | A standalone worker — claim + execute from the shared store, no scheduler/HTTP. `--tenant` pins it to one tenant (pod-per-tenant). |
| `weir autoscaler --image <img> [--namespace default] [--min 0] [--max 8] [--per-replica 20] [--poll 5]` | Scale per-tenant runner Deployments on queue depth (needs a `--features kubernetes` build). |

## Auth

| Command | Purpose |
| --- | --- |
| `weir auth token create --name <n> [--role admin] [--tenant <id>] [--admin]` | Mint an API key, scoped by role/tenant. `--admin` grants full access. |
| `weir auth token list` | List issued keys. |
| `weir auth token revoke <ident>` | Revoke a key. |

## Misc

| Command | Purpose |
| --- | --- |
| `weir version` | Print the version. |

## See also

- [Connection config](connection-config.md) — the fields and config a connection accepts.
- [HTTP API](../api/index.md) — the same operations over HTTP, for automation and the UI.
