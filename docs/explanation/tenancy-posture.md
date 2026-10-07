# Tenancy posture in the alpha

This page tells you what weir tenants do and do not protect in the alpha. Read it before you give a tenant key
or an OIDC sign-in to a person.

## Summary

In the alpha, a tenant is a **workspace for trusted operators**. A tenant is **not** a hardened multi-tenant
security boundary. Tenants keep the data and the work of different teams apart. Do not use tenants to isolate
users that you do not trust from each other or from the control-plane host.

## What you must trust

### OIDC sign-in gives a write key on `default`

When you set the `WEIR_OIDC_*` variables (see [Secure the control plane](../guides/secure-control-plane.md)),
weir gives **each user that your identity provider (IdP) authenticates** a short-lived **write** key on the
**`default`** tenant. weir does not map the identity to a tenant or to a role. weir does not deny an
authenticated identity.

Thus, every user that can sign in to your IdP can read and change the connections, runs and schemas of the
`default` tenant. If you use a public IdP (for example Google, or GitHub through Dex), this is every person
with an account there.

Do these steps:

- Use OIDC only with an IdP that authenticates only the people that you trust.
- Do not keep data in `default` that these people must not see.

### Catalog crate import runs `cargo build` on the control-plane host

`POST /catalog/import` with a local crate path compiles that crate with `cargo build` on the host that runs
`weir api`. A Rust build runs the build scripts and the procedural macros of the crate and of its dependencies.
This code runs with the rights of the `weir api` process, outside the WASM sandbox.

The route needs a **Write** key of any tenant. Thus, a tenant Write key can run code on the control-plane host.
Give Write keys only to people that you trust with the host.

### Discover runs at Read level and can reach the network

`POST /connectors/{plugin}/discover` runs a connector with the config in the request. The route needs only a
**Read** key. The connector can connect to each host that its egress policy allows, from the network of the
control plane. Thus, a Read key can make the control plane send requests to the hosts that the egress policies
allow. Give Read keys only to people that you trust with that network access.

## What weir enforces

The alpha tenancy safety work (COLLIERY-I-0252) makes these rules true:

- **Tenant ids are safe slugs.** A tenant id must match `^[a-z0-9][a-z0-9-]{0,62}$`. weir refuses other ids.
  weir makes the file system paths of a tenant through one checked function, so a tenant id cannot point
  outside the directory of the tenant.
- **Run control is scoped to the tenant.** Stop, cancel, the "is a run active" check and run history find a
  connection by the pair (tenant, name). A tenant cannot stop or cancel the run of a different tenant that has a
  connection with the same name.
- **Tenant delete cascades.** `DELETE /tenants/{id}` stops the runs of the tenant. Then it deletes the
  connections, schedules, work units, runs, run logs, dead letters, catalog entries, API keys and staged
  connector files of the tenant. You cannot delete the `default` tenant. The steps are in
  [Manage tenants](../guides/manage-tenants.md#4-delete-a-tenant).
- **Keys of a deleted tenant are refused.** The keys of a deleted tenant get `401` on the next request to the
  API server that did the delete. Other API servers of the same deployment refuse them after 30 seconds at most
  (the key cache TTL). weir also refuses a tenant key when its tenant does not exist.
- **A key for a missing tenant creates the tenant.** When an admin makes a key for a tenant id that does not
  exist, weir creates the tenant.

The isolation of execution (one worker for each active tenant) is in [Multi-tenancy](multi-tenancy.md).

## The startup notice

When `weir api` starts, it writes one warning to the log if there is a tenant other than `default`, or if OIDC
is on:

```text
tenancy posture (alpha): tenants are workspaces for trusted operators, not a hardened isolation boundary; OIDC sign-in gives each IdP-authenticated user a write key on the `default` tenant; see docs/explanation/tenancy-posture.md
```

A deploy with only the `default` tenant and no OIDC does not write this line.

## What changes after the alpha

The decision record WEIR-A-0042 (decided 2026-10-07) is the plan for OIDC after the alpha:

- **Deny by default.** An OIDC sign-in gets a key only when an operator binding maps the identity to a tenant
  and a role. weir denies an identity that has no binding (`403`). Each OIDC key that is not an admin key
  belongs to one tenant, so the tenant cascade also removes it.
- **Platform admin only from a deploy-time list.** Only the identities in `WEIR_OIDC_PLATFORM_ADMINS` (exact
  `issuer|subject` pairs) get a platform-admin key from OIDC. No binding and no API call can give platform admin.

This is not built yet. Until it ships, the OIDC rules in [What you must trust](#what-you-must-trust) apply.
