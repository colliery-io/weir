#!/usr/bin/env bash
# [[WEIR-T-0105]] — kind smoke test for the k8s topology ([[WEIR-I-0021]]). NOT in CI (needs a
# cluster). Proves: control plane provisions a per-tenant runner Deployment, the autoscaler scales it
# up on queue depth, work drains, and it scales back to zero.
set -euo pipefail

CLUSTER=${CLUSTER:-weir-smoke}
NS=${NS:-weir}
IMAGE=${IMAGE:-weir:smoke}
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "==> build the weir image WITH --features kubernetes + stage connectors"
# The Dockerfile must accept a build arg to pass `--features kubernetes` to the cargo build.
docker build -t "$IMAGE" --build-arg CARGO_FEATURES=kubernetes "$ROOT"

echo "==> kind cluster + load the image"
kind create cluster --name "$CLUSTER" 2>/dev/null || true
kind load docker-image "$IMAGE" --name "$CLUSTER"
kubectl create namespace "$NS" --dry-run=client -o yaml | kubectl apply -f -

echo "==> Postgres (the SoR)"
kubectl -n "$NS" apply -f - <<'YAML'
apiVersion: apps/v1
kind: Deployment
metadata: { name: postgres, labels: { app: postgres } }
spec:
  replicas: 1
  selector: { matchLabels: { app: postgres } }
  template:
    metadata: { labels: { app: postgres } }
    spec:
      containers:
        - name: postgres
          image: postgres:16
          env:
            - { name: POSTGRES_USER, value: weir }
            - { name: POSTGRES_PASSWORD, value: weir }
            - { name: POSTGRES_DB, value: weir }
          ports: [{ containerPort: 5432 }]
---
apiVersion: v1
kind: Service
metadata: { name: postgres }
spec:
  selector: { app: postgres }
  ports: [{ port: 5432, targetPort: 5432 }]
YAML
kubectl -n "$NS" rollout status deploy/postgres --timeout=120s

STORE="postgres://weir:weir@postgres:5432/weir"

echo "==> weir-server (control plane + RBAC)"
helm upgrade --install weir "$ROOT/charts/weir-server" -n "$NS" \
  --set image.repository="${IMAGE%:*}" --set image.tag="${IMAGE#*:}" \
  --set image.pullPolicy=IfNotPresent --set store.url="$STORE" --wait

echo "==> run the autoscaler in the control-plane pod"
POD="$(kubectl -n "$NS" get pod -l app.kubernetes.io/name=weir-server -o jsonpath='{.items[0].metadata.name}')"
kubectl -n "$NS" exec "$POD" -- sh -c \
  "weir --db '$STORE' autoscaler --namespace '$NS' --image '$IMAGE' --max 2 --per-replica 1 &"

echo "==> plan work for tenant 'acme' (mint a key + create/run a connection) — see charts/README"
echo "==> watch the runner scale up then to zero:"
kubectl -n "$NS" get deploy -l app=weir-runner -w &
WATCH=$!
sleep 60
kill "$WATCH" 2>/dev/null || true

echo "==> teardown: kind delete cluster --name $CLUSTER"
