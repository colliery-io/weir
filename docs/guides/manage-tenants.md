# Manage tenants

weir is **multi-tenant**. Every connection, key, run, and schedule belongs to a tenant, and work executes in a
**per-tenant runner** so one tenant's load and failures stay isolated. Single-tenant deploys never notice this —
everything lives under the implicit `default` tenant. To host several, create tenants and scope keys to them.

Tenant administration is a **platform** (cross-tenant) capability, so it runs over the HTTP API with an **admin**
key.

**Goal:** create a tenant and give it its own scoped key.

## 1. Create the tenant

```bash
curl -s -X POST http://localhost:8080/tenants \
  -H "authorization: Bearer $ADMIN_KEY" -H "content-type: application/json" \
  -d '{"id":"acme","name":"Acme Inc"}'
```

`GET /tenants` lists them.

## 2. Issue a tenant-scoped key

```bash
curl -s -X POST http://localhost:8080/tenants/acme/keys \
  -H "authorization: Bearer $ADMIN_KEY" -H "content-type: application/json" \
  -d '{"name":"acme-app","role":"admin"}'
```

The returned key is scoped to `acme` — it can only see and act on that tenant's connections. (The same is
available from the CLI: `weir auth token create --tenant acme --name acme-app --admin`.)

## 3. Work as the tenant

Calls made with a tenant-scoped key are implicitly scoped — `GET /connections` returns only that tenant's
connections, a run only touches its data. Admins can also address a tenant explicitly via the
`/tenants/{id}/connections/…` routes.

**Done** when the tenant-scoped key lists only `acme`'s connections and is refused the platform routes
(`GET /tenants` → `403`).

## Notes

- Isolation is enforced at execution too: the orchestrator runs a **separate worker per active tenant**, and in
  Kubernetes you can run a **runner pod per tenant** (see [Deploy to Kubernetes](deploy-kubernetes.md)).
- The cross-tenant **Platform** health view in the UI is the admin's window into every tenant at once.
