# Secure the control plane

weir's API and UI are **authenticated by default**. Two doors let people and machines in: **API keys** (bearer
tokens) for programmatic access, and **OIDC** (single sign-on) for the web UI. This guide sets up both.

## API keys

The first key is minted at `init` (save it — it's shown once):

```bash
weir --db weir.db init
#   admin API key — save this, it is not shown again:
#     weirk_…
```

Mint more with the CLI — scope them by **role** and **tenant**:

```bash
# an admin key (full access)
weir --db weir.db auth token create --name ci --admin

# a non-admin key scoped to one tenant
weir --db weir.db auth token create --name acme-ro --role reader --tenant acme

weir --db weir.db auth token list
weir --db weir.db auth token revoke <ident>
```

Every API call carries the key: `Authorization: Bearer weirk_…`. Access is a **default-deny** route table — a
key is allowed a route only if its role meets the route's requirement, and platform (cross-tenant) routes are
admin-only.

## OIDC single sign-on (web UI)

Point weir at your identity provider with environment variables, then start the API:

```bash
export WEIR_OIDC_ISSUER="https://your-idp.example/realms/main"
export WEIR_OIDC_CLIENT_ID="weir"
export WEIR_OIDC_CLIENT_SECRET="…"
export WEIR_OIDC_REDIRECT_URI="http://localhost:8080/auth/callback"
export WEIR_OIDC_SCOPES="openid profile email"   # optional; sensible default otherwise

weir --db weir.db api --port 8080
```

The UI now offers an OIDC sign-in; the callback exchanges the code and issues the browser a session. Without the
OIDC variables, the UI falls back to the API-key sign-in.

**Done** when a `curl` with a valid `Bearer` key succeeds and one without gets `401`/`403`, and the UI shows the
sign-in gate.

## Notes

- **Secrets are off-platform** — weir *consumes* rotated connector secrets, it does not store or manage them.
  Provision them out of band; the host injects them into a connector's egress at run time and they never enter
  the WASM guest.
- Per-tenant keys pair with [tenant management](manage-tenants.md) for multi-tenant isolation.
