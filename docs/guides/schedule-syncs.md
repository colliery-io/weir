# Schedule syncs

`weir run` executes a connection once. To keep a connection syncing on a cadence, give it a **schedule** and run
the **daemon** — weir's scheduler enqueues due connections and its workers drain them.

**Goal:** run a connection every 30 seconds.

## 1. Give the connection a schedule

Add `--every` (seconds) when you create the connection — or `--cron` for a cron expression (6/7-field,
seconds-first), which takes precedence:

```bash
weir --db weir.db connection add \
  --name rates --source slow --dest arrow-sink --stream demo \
  --config '{"rows":5,"batch":true}' \
  --every 30
```

## 2. Start the daemon

```bash
weir --db weir.db serve --poll 0.5 --concurrency 16
```

`serve` runs the scheduler **and** the worker fleet until you stop it. `--poll` bounds how often it checks for
due work (so it also bounds schedule resolution); `--concurrency` caps how many syncs run at once.

Only one process should schedule against a store; weir gates scheduling behind a **leader lease**, so you can run
several `serve`/API replicas safely — only the leader enqueues, everyone drains. Set `WEIR_DISABLE_SCHEDULER` to
make a replica API/worker-only.

## 3. Confirm it's firing

```bash
weir --db weir.db connection list          # shows the connection
# or, against the API:
curl -s http://localhost:8080/runs -H "authorization: Bearer $KEY"
```

**Done** when new runs appear roughly every 30s in the run history, each ending `done`.

## Notes

- The `weir api` server runs the same scheduler + workers in-process, so a connection with `every_secs` set will
  sync on its own once the API is up — you don't need a separate `serve`.
- For a fleet of connections under sustained schedule load, see [Soak testing](soak-testing.md).
