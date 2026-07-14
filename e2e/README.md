# weir UI end-to-end tests (Playwright)

Browser tests for the embedded Leptos + Aurora UI ([[WEIR-I-0016]]) behind the
auth gate ([[WEIR-I-0017]]): reverse-ETL destination onboarding ([[WEIR-T-0077]]),
the shell/operations views, the sign-in gate, and the **OIDC login round-trip via
Dex** ([[WEIR-T-0088]]).

## Run it all (recommended)

```sh
angreal test e2e
```

Builds the UI + connectors, brings up **Dex** (OIDC provider) via `compose.yml`,
mints an admin key, starts the **locked** weir server with the OIDC env, seeds a
demo connection, runs the full Playwright suite (both auth doors), and tears down.
Needs Docker running and port **5557** free (cloacina's Dex uses 5556).

## Run manually

```sh
# 1. build the UI into the binary + stage connectors (incl rest-dest)
angreal ui build && bash scripts/stage-connectors.sh /tmp/weir-e2e-connectors && cargo build -p weir-cli

# 2. (optional, for the OIDC spec) bring up Dex
docker compose up -d dex --wait

# 3. mint a key (the API is authenticated) + start the server
env="WEIR_CONNECTORS_DIR=/tmp/weir-e2e-connectors WEIR_MANIFESTS_DIR=$PWD/manifests WEIR_DEST_MANIFESTS_DIR=$PWD/dest-manifests"
env $env ./target/debug/weir --db /tmp/weir-e2e.db init
KEY=$(env $env ./target/debug/weir --db /tmp/weir-e2e.db auth token create --name e2e --admin | grep -oE 'weirk_[A-Za-z0-9_-]+')
env $env \
  WEIR_OIDC_ISSUER=http://localhost:5557/dex WEIR_OIDC_CLIENT_ID=weir \
  WEIR_OIDC_CLIENT_SECRET=weir-secret WEIR_OIDC_REDIRECT_URI=http://localhost:8787/auth/callback \
  ./target/debug/weir --db /tmp/weir-e2e.db api --port 8787 &

# 4. run the tests (first time: npm install && npx playwright install chromium)
cd e2e && WEIR_E2E_KEY="$KEY" npx playwright test
```

## Auth notes

- The API is **locked** ([[WEIR-A-0008]]) — every spec authenticates. `tests/fixtures.ts`
  seeds `localStorage["weir_api_key"]` from `WEIR_E2E_KEY` (the key door); the specs import
  `test` from it.
- `oidc.spec.ts` (the human door) drives the real Dex login and is **skipped** unless
  `WEIR_OIDC_ISSUER` is set. Dex user: `admin@weir.test` / `password`.
- `gate.spec.ts` needs no credential — it asserts the unauthenticated sign-in card renders.

Point at a different server with `WEIR_UI_URL`.
