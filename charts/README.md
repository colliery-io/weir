# weir on Kubernetes ([[WEIR-I-0021]])

Two charts + the control-plane actuator/autoscaler ([[WEIR-A-0023]]):

- **`weir-server`** — the control plane (`weir api` + scheduler), a Service, the store Secret, and **RBAC**
  (a Role letting the actuator manage runner Deployments in its namespace).
- **`weir-runner`** — the per-tenant runner Deployment (`weir runner --tenant <id>`). Its shape matches the
  actuator's `runner_deployment_json` ([[WEIR-T-0103]]) so the actuator's server-side-apply lands on the same
  object; the **autoscaler** ([[WEIR-T-0104]]) scales it on queue depth (to zero when idle).

The k8s deps (`kube`/`k8s-openapi`) are behind the **`kubernetes` cargo feature** — build the image with
`--features kubernetes` to get `weir autoscaler`. The default build is unaffected.

## Install

```sh
helm upgrade --install weir charts/weir-server -n weir --create-namespace \
  --set image.repository=<repo> --set image.tag=<tag> \
  --set store.url="postgres://weir:weir@postgres:5432/weir"
```

Run the autoscaler (leader-elected; safe to run in every control-plane replica):

```sh
weir --db "postgres://…" autoscaler --namespace weir --image <repo>:<tag> --min 0 --max 8 --per-replica 20
```

To **defer scaling to HPA/KEDA** instead, simply don't run `weir autoscaler` and set your own scalers on the
`weir-runner-<tenant>` Deployments (`--set autoscaler.enabled=false` on `weir-server`).

## Values (highlights)

| key | default | meaning |
|---|---|---|
| `image.repository`/`tag` | `ghcr.io/colliery-io/weir`/`latest` | the weir image (build `--features kubernetes` for the autoscaler) |
| `store.url` | `postgres://…` | the Postgres SoR the control plane + runners share |
| `oidc.enabled`/`issuer`/`clientId` | `false` | OIDC login ([[WEIR-I-0017]]) |
| `autoscaler.min/max/perReplica` | `0`/`8`/`20` | the depth→replicas policy |
| `rbac.create` | `true` | the actuator's Role + RoleBinding |

## kind smoke test (not CI — needs a cluster)

`scripts/kind-smoke.sh` proves the topology end-to-end: build the image (`--features kubernetes`), load it into
kind, install Postgres + `weir-server`, run `weir autoscaler`, plan work for a tenant → its `weir-runner-<tenant>`
Deployment **scales up**, work **drains**, then it **scales to zero**. `helm lint`/`template` run in CI; the
live scale is this smoke.
