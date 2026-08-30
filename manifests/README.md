# Vendored declarative connectors

The phase-1 "discover & select" corpus for low-code onboarding ([[WEIR-I-0012]] /
[[WEIR-A-0032]]). `weir-app::available_packages` lists each `*.yaml` here (kind
`manifest`); onboarding registers one as a named connector that runs on the shared
declarative runtime (`rest`) — **no compile**.

## Provenance

Every connector here is authored **solely** from the target API's own official
documentation — no third-party connector implementation was consulted. Each `*.yaml`
carries a header naming the docs it was written from. License: Apache-2.0 (this repo).

## The set

36 connectors (the count is `ls *.yaml | wc -l` — keep this line honest), each importing
cleanly through `weir-importer` onto the shared runtime:

`airtable` `asana` `coingecko` `coinpaprika` `exchangerate` `frankfurter` `github`
`gitlab` `google-analytics` `google-sheets` `hubspot` `intercom` `jira`
`jsonplaceholder` `mediawiki` `nasa` `newsapi` `notion` `nps` `omdb` `openaq`
`openfda` `openlibrary` `openweather` `pagerduty` `pokeapi` `rickandmorty` `slack`
`spotify` `square` `stripe` `ticketmaster` `tmdb` `todoist` `xkcd` `zendesk`

Run any through the **preview** (`POST /catalog/preview`) to see its tier / confidence
/ runtime gaps before onboarding. **Auth** (bearer, header + query-param api-key,
OAuth2 refresh/client-credentials, Google service-account + Snowflake key-pair JWT) is
applied **host-side** ([[WEIR-A-0033]]) — the manifest declares the scheme, and the
secret is supplied per-connection, never entering the connector sandbox. **Templated
`url_base`** (`{{ config[…] }}` tenant subdomain / account id), **page / offset /
opaque-cursor pagination** — carried in the query string or **injected into the POST
body** ([[WEIR-T-0154]], e.g. Notion's `start_cursor`) — **cursor-from-last-record +
`has_more` stop** ([[WEIR-T-0168]], Stripe's `starting_after`), **POST-with-body
requests**, **`Link`-header pagination**, and **single-object** (non-array) responses
are all handled.

## Verification honesty

Importing cleanly is not the same as verified against the live API. The durable record
is **[`verified.json`](verified.json)** — one entry per connector that last PASSED the
live suite, with the date and what ran. It is written by
`angreal test connectors-live` (or `angreal test manifests --live`) when
`WEIR_WRITE_VERIFIED=1` is set, and committed like any vendored change; at connector
registration the date lands on the catalog row as `verified_at` (API/UI-visible), so
deployments carry the record. Keyed connectors join the ledger as their secret bundles
land. **Absent from the ledger = well-founded-but-unverified** — treat it that way.
