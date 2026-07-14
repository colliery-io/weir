# Soak testing

The soak test stands weir up, provisions a **fleet of connectors** on a tight schedule, and lets it
run continuously — surfacing the things a single run never will: memory/lease stability, scheduler and
queue behaviour, and lock contention under sustained load. It asserts health **invariants** over time
and fails (non-zero exit) if any is breached.

It has two pieces:

- **`weir-soak`** — a small binary that provisions + drives + asserts entirely over the HTTP API
  against a `--base-url`. Because it only speaks HTTP, it points at **any** running weir — compose
  today, kind/k8s later — so the harness is reusable.
- **`angreal soak`** — the one-command entry point: it builds the pieces, starts a local weir server,
  runs `weir-soak` against it, and tears everything down.

## Running it

```bash
# Full soak (default ~90s): local echo/slow fleet + a postgres write/read pair + a couple of
# no-auth live REST sources. Brings up the compose Postgres and tears it down.
angreal soak

# CI smoke: fast, fully local, no docker, no network. Exits non-zero on any invariant breach.
angreal soak --mode smoke

# Tune it.
angreal soak --duration 300 --fleet 32
```

`--mode smoke` runs a bounded, deterministic, dependency-free soak (local connectors only) — suitable
for a CI gate. `--mode full` (the default) adds the postgres pair and the live REST allowlist.

## The invariants

Each polling window, `weir-soak` reads the ops API (`/runs`) and checks:

| Invariant | Meaning |
| --- | --- |
| **liveness** | the API answered every poll |
| **throughput** | the fleet keeps completing runs — a *sustained* stall (N consecutive empty windows) fails, an isolated blip does not |
| **bounded-queue** | in-flight work doesn't grow without bound |
| **bounded-dead-letters** | the aggregate dead-letter rate stays under a threshold — the external live-REST connectors are **excluded** from this hard gate, since their flakiness is expected |

The run ends with a summary (windows, runs completed, max in-flight, dead-letters, and a PASS/FAIL per
invariant) and exits non-zero if anything breached. All thresholds (`--min-throughput`, `--max-queue`,
`--max-dl-delta`, `--warmup`, `--max-stall`, `--window`) are flags.

## Pointing it at a real cluster

`angreal soak` is just the compose-shaped wrapper. To soak a kind/k8s (or any) deployment, run the
binary directly against its base URL — the provisioning and invariants are identical:

```bash
weir-soak --base-url https://weir.my-cluster.example --admin-key "$KEY" --duration 600 --fleet 64
```

## What it has already caught

The soak is not theatre — standing a fleet up under sustained schedule load immediately surfaced that
single-node **SQLite in the default rollback-journal mode starved the worker fleet** (a 4-connection
fleet completed *zero* runs, because any reader blocked the writer). weir now opens SQLite in **WAL**
mode with `synchronous=NORMAL`, and the same fleet sustains full throughput with the queue drained to
zero. That is exactly the class of problem this test exists to find.
