---
id: compile-isolation-per-tenant-build
level: task
title: "Compile isolation — per-tenant build namespace + cache keys"
short_code: "WEIR-T-0092"
created_at: 2026-07-05T23:57:26.763118+00:00
updated_at: 2026-07-06T01:09:04.220874+00:00
parent: WEIR-I-0018
blocked_by: [WEIR-T-0089]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0018
---

# Compile isolation — per-tenant build namespace + cache keys

## Parent Initiative

[[WEIR-I-0018]]. Governed by [[WEIR-A-0036]] (decision 5: per-tenant compile).

## Objective

Isolate connector **onboarding/build** per tenant: a tenant's onboarded manifests/crates and built wasm
artifacts live in its own namespace, and build cache keys include `tenant_id` — so no cross-tenant artifact
reuse can leak one tenant's manifest, config, or credentials into another's build.

## Reference

- `crates/weir-codegen/src/{lib,main,wasm}.rs` — the manifest→crate + wasm build.
- Artifacts resolve via `WEIR_CONNECTORS_DIR` (staged wasm guests, [[WEIR-A-0030]]); the catalog `location`
  column points at the built artifact.
- Onboarding flows through `weir-app` (catalog import → build → register).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] Built artifacts land under a per-tenant path (e.g. `<connectors_dir>/<tenant_id>/<pkg>`); the catalog
  `location` is tenant-scoped ([[WEIR-T-0089]] put `tenant_id` on `connectors`).
- [ ] The build cache key includes `tenant_id`; a rebuild for tenant B never returns tenant A's artifact.
- [ ] Onboarding builds under the caller's tenant; resolution at run time loads the tenant's artifact.
- [ ] A test: two tenants onboard a same-named connector with different config → distinct artifacts, no reuse.
- [ ] `angreal test ...` for codegen green; workspace + clippy clean.

## Implementation Notes

- Open sub-question (initiative): is any artifact safely shareable (a public manifest, no secrets)? Default to
  **all per-tenant** for simplicity + safety; revisit sharing only if cache size becomes a problem.
- Coordinate the artifact path convention with the runner ([[WEIR-T-0091]]) so a tenant's runner resolves its
  own tenant's artifacts.

## Status Updates

### 2026-07-06 — analysis + concrete plan (scope decided; touches the resolution hot path)

**Decision (approved):** per-tenant paths only for **compiled private crates** (`Source::LocalCrate`); shared
generic runtimes (rest/rest-dest/echo/arrow-sink) + manifest configs (already row-isolated by [[WEIR-T-0089]]/
[[WEIR-T-0090]]) stay shared. "All per-tenant" (the old lean) is wrong — it'd stage N copies of identical
platform wasm.

**How the artifact model works (read):**
- `location` = the package name; the runtime resolves `<connectors_dir>/<package>` (shared).
- `import(tenant, LocalCrate)` → `compile_and_stage(path, <dir>)` → `<dir>/<package>` (ingress.rs:56, 209).
- A connection's source is a `ConnectorRef::Wasm{search_path=connectors_dir(), package}` built by the free fn
  `connector_ref(name)` (lib.rs:78). `resolve()` (orchestrator.rs:122) keys the handle cache on
  `(search_path, package, config)` and loads via `from_wasm_package(search_path, package)`.
- **The tenant is NOT present at `resolve()`** — search_path/package are baked into the stored `ConnectorRef`.

**Concrete plan (3 changes — the 3rd is the hot-path risk):**
1. `compile_and_stage(tenant, path, root)` → stage private crates to `<dir>/<tenant>/<package>`; `import`
   threads the tenant. *(additive)*
2. `from_wasm_package` / `resolve`: **parent-fallback** — try `<search_path>/<package>`; if absent, try
   `<parent(search_path)>/<package>`. So a tenant search_path finds its private artifact, else the shared one.
   *(additive — can't break existing loads; only fires when primary is absent)*
3. `add_connection(tenant, c)`: rewrite the stored source/dest `ConnectorRef::Wasm.search_path` from `<dir>` →
   `<dir>/<tenant>`. **This is the behavior change on the resolution path every run takes** — the reason to do
   this carefully, not rushed. Cache key then includes the tenant via search_path (AC2). Shared runtimes resolve
   via the fallback (step 2).
4. Test: two tenants import a same-named private crate → distinct `<dir>/<tenant>/<pkg>` artifacts; each tenant's
   connection resolves its own; no cross-tenant reuse.

### 2026-07-06 — done (`98c2f39`), refined step 3

Implemented, with one refinement over the plan: **step 3 scopes the ref in `work_spec` (execution time), not
`add_connection` (storage)** — storing the scoped ref broke the source round-trip (`get_connection().source ==
connector_ref(name)`, cli.rs:31). So the stored connection stays un-scoped and `work_spec(tenant, c)` scopes
`source`/`dest` to `<dir>/<tenant>` on the way to the runner. Net:
- `import` LocalCrate → `<dir>/<tenant>/<pkg>`; Folder resolves tenant-first then shared; cache invalidated at
  the tenant path.
- `resolve()` parent-fallback: load `<search_path>/<pkg>`, else `<parent>/<pkg>` (the shared runtimes/guests).
- `work_spec` scopes the exec refs via `scope_wasm_to_tenant` (shared default → `<dir>/<tenant>`).

Test **`compile_isolation_two_tenants_distinct_artifacts`**: acme + globex import the same private crate → each
lands at `<dir>/<tenant>/weir-slow-pkg` (distinct), each cataloged per tenant, refs resolve distinct namespaces.
The full app/orchestrator/api/cli suite (which exercises the run/resolution hot path) stayed green — the
guardrail held; clippy clean. **Complete.**
