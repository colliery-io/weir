# Deploy to Kubernetes

weir runs on Kubernetes as a **control plane** plus **per-tenant runner** pods that an autoscaler scales on queue
depth. Two Helm charts ship in `charts/`.

**Goal:** install the control plane and let it scale tenant runners.

## Prerequisites

- A cluster + `helm`, and a **Postgres** the control plane and runners share (the system of record).
- A weir image built **with the `kubernetes` feature** (`--features kubernetes`) if you want the built-in
  autoscaler; the default image runs everything else.

## 1. Install the control plane

`weir-server` is `weir api` + the scheduler, a Service, the store Secret, and the RBAC the actuator needs to
manage runner Deployments:

```sh
helm upgrade --install weir charts/weir-server -n weir --create-namespace \
  --set image.repository=<repo> --set image.tag=<tag> \
  --set store.url="postgres://weir:weir@postgres:5432/weir"
```

Enable OIDC with `--set oidc.enabled=true --set oidc.issuer=… --set oidc.clientId=…` (see
[Secure the control plane](secure-control-plane.md)).

## 2. Scale tenant runners

Each tenant drains in its own `weir-runner-<tenant>` Deployment. The **autoscaler** (leader-elected, safe in
every replica) sizes them on queue depth — up under load, to zero when idle:

```sh
weir --db "postgres://…" autoscaler \
  --namespace weir --image <repo>:<tag> --min 0 --max 8 --per-replica 20
```

Prefer HPA/KEDA? Don't run `weir autoscaler`; set `--set autoscaler.enabled=false` and put your own scalers on
the `weir-runner-<tenant>` Deployments.

## 3. Verify

Plan work for a tenant, then watch its runner:

```sh
kubectl -n weir get deploy -l app=weir-runner
```

**Done** when a tenant's `weir-runner-<tenant>` Deployment scales up as work is queued, drains it, and scales
back to zero when idle. `scripts/kind-smoke.sh` performs exactly this end to end on a local kind cluster.

## Notes

- The control plane is horizontally safe — scheduling is behind a leader lease, draining is claim-based, so you
  can run several `weir-server` replicas.
- Use **Postgres** as the store in production, not SQLite (SQLite is the single-node/dev path).
