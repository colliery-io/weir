---
id: provision-authed-connector
level: task
title: "Provision authed connector accounts + encrypt secret bundles"
short_code: "WEIR-T-0067"
created_at: 2026-07-03T01:17:47.546721+00:00
updated_at: 2026-07-03T01:17:47.546721+00:00
parent: WEIR-I-0014
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/todo"


exit_criteria_met: false
initiative_id: WEIR-I-0014
---

# Provision authed connector accounts + encrypt secret bundles

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0014]] — the operator provisioning that makes the authed half of the [[WEIR-T-0066]] live suite
real. (Machinery — SOPS + `angreal test connectors-live` — is done in [[WEIR-T-0065]]/[[WEIR-T-0066]].)

## Objective **[REQUIRED]**

Sign up for accounts / mint API keys for the authed connectors in the vendored corpus and drop each into a
sops-encrypted `secrets/<slug>.enc.json`, so the live suite exercises real auth end-to-end. All corpus
connectors use **static keys** (no OAuth in the corpus yet), so each is just a key (+ occasionally a
resource id / tenant).

## Per-connector process

For each connector below:
```sh
angreal secrets edit <slug>          # opens secrets/<slug>.enc.json in $EDITOR via sops
# put {"<field>": "<key>", ...any extra config...}   (field name per the tables below)
git add secrets/<slug>.enc.json      # ciphertext — safe to commit
angreal test connectors-live         # confirm it goes green (rows > 0)
```

**The secret field name differs per connector** — using the wrong one silently fails. It's one of
`api_key`, `api_token`, or `access_token`, listed below. Optional `"__stream": "<name>"` picks a stream
(default = the manifest's first).

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

### Starter set — free + instant, covers all three host-side injection paths (do these first)

- [x] **openweather** — `api_key` · header api-key · openweathermap.org (done; committed `d55cdf1`)
- [ ] **github** — `api_key` · bearer header · github.com → Settings → Developer settings → PAT (read-only)
- [ ] **stripe** — `api_key` · bearer header · dashboard.stripe.com → **test mode** → Secret key (`sk_test_…`)
- [ ] **todoist** — `api_key` · bearer header · Todoist → Settings → Integrations → API token
- [ ] **nasa** — `api_key` · **query param** · api.nasa.gov (exercises host-side URL rewrite)
- [ ] **newsapi** — `api_key` · header api-key · newsapi.org/register

### Easy free keys, no extra config

- [ ] **tmdb** — `access_token` · themoviedb.org (v4 read token)
- [ ] **ticketmaster** — `api_key` · query · developer.ticketmaster.com
- [ ] **omdb** — `api_key` · omdbapi.com (key by email)
- [ ] **coingecko** — `api_key` · demo key
- [ ] **nps** — `api_key` · query · nps.gov developer portal
- [ ] **pagerduty** — `api_key` · REST API key
- [ ] **exchangerate** — `api_key` · exchangerate-api.com
- [ ] **asana** — `api_token` · personal access token
- [ ] **notion** — `api_token` · create an internal integration + share a page with it
- [ ] **hubspot** — `access_token` · private-app token

### Needs an extra config value (key + a resource/tenant)

- [ ] **airtable** — `api_key` + `base_id` + `table_name` (create a base/table first)
- [ ] **gitlab** — `access_token` + `base_url` (e.g. `https://gitlab.com`)
- [ ] **jira** — `api_token` + `domain` (`your-site.atlassian.net`)
- [ ] **zendesk** — `api_token` + `subdomain`
- [ ] **openaq** — `api_key` + `sensors_id`
- [ ] **slack** — `api_token` (+ a workspace; injected as query)
- [ ] **square** — `api_key` (+ a Square sandbox account)
- [ ] **intercom** — `api_key` (+ a workspace)

### Deferred

- **spotify** — corpus manifest uses a static bearer (`access_token`), but Spotify tokens expire hourly →
  a static key won't persist. Skip until an OAuth-refresh manifest exists (see note below).

### CI enablement (operator, once a batch of keys is in)

- [ ] Add the CI age private key as repo secret **`SOPS_AGE_KEY`** (its pubkey must be a `.sops.yaml`
  recipient).
- [ ] Re-enable GitHub Actions (disabled while private) so `connectors-live.yml` runs nightly.

## Implementation Notes

- **No-auth connectors** (coinpaprika, frankfurter, jsonplaceholder, mediawiki, openfda, openlibrary,
  pokeapi, rickandmorty, xkcd) need no key — already asserted by the no-auth live run.
- **No OAuth connector in the corpus**, so the host-side OAuth path ([[WEIR-T-0063]]) isn't exercised live.
  If we want that covered, file a small follow-up to add one client-credentials manifest — not part of this
  task.
- Don't chase 100%: even the starter set proves all three injection paths. Add connectors opportunistically.

## Status Updates **[REQUIRED]**

*openweather done (2026-07-02). Remainder is operator work as accounts are provisioned.*

**2026-08-30 — CANCELLED by decision (archived).** Dylan: "I won't be provisioning those accounts." No further account provisioning will happen. What exists stays: three encrypted bundles are in-tree and usable (`openweather`, `github`, `nasa` — covering header, bearer, and query-param injection paths), plus the five no-auth live connectors. The live-verification story is therefore: no-auth tier + the three existing bundles + wire-level integration gates (compose estate) — keyed cloud connectors (snowflake/hubspot/stripe/ga4/sheets) remain honestly unverified in the [[WEIR-T-0183]] ledger, which is exactly what the ledger is for. The SOPS machinery remains a shipped user-facing feature — anyone can bring their own accounts via `angreal secrets edit`.
