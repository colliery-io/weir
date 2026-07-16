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

34 connectors, each importing cleanly through `weir-importer` onto the shared runtime:

`airtable` `asana` `coingecko` `coinpaprika` `exchangerate` `frankfurter` `github`
`gitlab` `hubspot` `intercom` `jira` `jsonplaceholder` `mediawiki` `nasa` `newsapi`
`notion` `nps` `omdb` `openaq` `openfda` `openlibrary` `openweather` `pagerduty`
`pokeapi` `rickandmorty` `slack` `spotify` `square` `stripe` `ticketmaster` `tmdb`
`todoist` `xkcd` `zendesk`

Run any through the **preview** (`POST /catalog/preview`) to see its tier / confidence
/ runtime gaps before onboarding. **Auth** (bearer, header + query-param api-key) is
applied by the runtime ([[WEIR-I-0008]]) — the manifest declares the scheme, and the
secret (`api_key`) is supplied per-connection. **Templated `url_base`** (`{{ config[…] }}`
tenant subdomain / account id), **page / offset / opaque-cursor pagination** — carried in
the query string or **injected into the POST body** ([[WEIR-T-0154]], e.g. Notion's
`start_cursor`) — **POST-with-body requests**, **`Link`-header pagination**, and
**single-object** (non-array) responses are all handled.

## Known gaps (authored, but beyond the runtime today)

Manifests also exist for `restcountries`, `youtube`, `usgs`, `weatherapi`, and
`dogceo`, but they don't yet import — they need features the shared runtime doesn't
cover (response envelopes, cursor pagination, etc.). They get vendored here as the
runtime grows.
