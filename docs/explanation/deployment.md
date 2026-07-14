# Deployment topology

weir is designed to run as a single process on a laptop and as a scaled, highly-available control plane on
Kubernetes — with the *same* binary and the same code paths. What changes between them is the store and how many
of each role you run.

## One binary, several roles

The `weir` binary is the control plane (`weir api`), the scheduler + worker daemon (`weir serve`), a standalone
worker (`weir runner`), and the autoscaler (`weir autoscaler`). A single `weir api` process is a complete node:
it serves the API/UI and runs the scheduler and workers in-process. Scaling is a matter of splitting those roles
across processes and pointing them at a shared store.

## Single-node

For development and small deployments, one `weir api` over **SQLite** is the whole system. It's self-contained —
no external database — and the checkpoint/lease machinery still applies, so it's correct, just not distributed.

**One caveat, learned the hard way.** SQLite's default rollback-journal mode lets any reader block the writer,
and under a *scheduled fleet* — the scheduler, several workers, and the API all hitting the store — that
contention can starve the workers entirely (a soak test found a small fleet completing zero runs). weir therefore
opens SQLite in **WAL** mode with `synchronous=NORMAL`, so readers and one writer proceed concurrently. It's a
good illustration of why the [soak test](../guides/soak-testing.md) exists: sustained load surfaces what a single
run never will.

## Scaled + highly available

For anything shared or scaled, use **Postgres** as the store and split the roles:

- Run several `weir-server` (control-plane) replicas behind a Service. Scheduling is gated by a **leader lease**,
  so only one replica enqueues; draining is claim-based, so every replica's workers pull safely. Replicas are
  therefore horizontally scalable and highly available — losing one loses no work.
- Run **per-tenant runner** Deployments (`weir runner --tenant <id>`), each draining one tenant's queue. This is
  where [multi-tenancy](multi-tenancy.md) becomes physical isolation.
- Let the **autoscaler** size runners on queue depth (up under load, to zero when idle), or defer to HPA/KEDA.

The [Deploy to Kubernetes](../guides/deploy-kubernetes.md) guide walks the Helm charts that encode this shape.

## Why the same code both ways

The async orchestration boundary — queue, leases, leader election — is identical whether it's coordinating
threads in one process or pods across a cluster. That's deliberate: the distributed behaviour is exercised (and
kept honest) by the single-node path every developer runs, so there's no separate, under-tested "production
mode."
