# Multi-tenancy

weir is multi-tenant from the ground up, but single-tenant deploys never pay for it. Understanding the model
explains both how isolation works and why it stays invisible when you don't need it.

## Everything belongs to a tenant

Connections, API keys, runs, schedules, dead-letters, and schemas all carry a `tenant_id`. Connection identity is
composite — `(tenant, name)` — so two tenants can each own a connection called `orders` without collision. A key
is scoped to a tenant, and the ordinary API routes act on **that** tenant implicitly: a tenant-scoped key listing
connections sees only its own.

When you don't set up tenants, everything lives under an implicit **`default`** tenant. Nothing in the
single-tenant experience changes — the column is just always `default`.

## Isolation at execution, not just in queries

Tenancy that's only a `WHERE` clause isn't isolation — a noisy tenant would still starve the others through the
shared worker pool. weir isolates at the **execution** layer: the orchestrator runs a **separate worker per
active tenant**, each draining only that tenant's work units. One tenant's backlog or failures don't consume
another tenant's throughput. Idle tenants get no worker (natural reaping).

In Kubernetes this extends to the pod level: a **runner pod per tenant** (`weir runner --tenant <id>`), scaled
independently on that tenant's queue depth. A tenant can be scaled, paused, or drained without touching the rest.

## The operator's cross-tenant view

Because everything is tenant-scoped, an admin needs a way to see across tenants. The **Platform** health view is
that window: a cross-tenant rollup of per-tenant health, admin-only (it's a *platform* capability in the
default-deny authz table). A tenant admin sees only their own health; a platform admin sees everyone's.

## Why composite keys in the baseline, not a migration

weir folded tenancy into the baseline schema rather than bolting it on later: the store's primary keys recompose
to include `tenant_id`, and the claim path filters by it. Doing this up front — while pre-release, with no
deployed databases to preserve — avoided a painful PK-recompose migration and made isolation a property of the
data model rather than a convention layered on top.
