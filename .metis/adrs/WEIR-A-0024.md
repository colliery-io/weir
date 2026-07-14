---
id: 001-web-ui-architecture
level: adr
title: "Web UI architecture"
number: 24
short_code: "WEIR-A-0024"
created_at: 2026-06-17T02:12:32.601898+00:00
updated_at: 2026-07-05T00:00:00.000000+00:00
decision_date: 2026-07-05
decision_maker: dylan.storey@gmail.com
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: true
initiative_id: NULL
---

# ADR-0024: Web UI architecture

**Status:** Decided (realized). *Raised by: [[WEIR-S-0003]] Web UI.* The SPA posture set here is
implemented; the stack it left open was resolved by [[WEIR-A-0035]].

## Context **[REQUIRED]**

The UI is a strict client of the Control Plane API ([[WEIR-A-0006]]) with no privileged backdoor. It must handle async/long-running syncs and streaming status, meet WCAG AA, and be i18n-ready. We must pick SPA vs. server-driven and the stack.

## Decision **[REQUIRED]**

An **SPA** consuming the REST API, chosen for rich async/streaming status UX — strictly API-client posture,
exposing nothing the API doesn't. The **stack** (deferred here) was subsequently selected in
[[WEIR-A-0035]]: **Leptos (CSR/WASM)** built from the **Colliery Aurora Dark** design system, compiled by
`trunk` and embedded in the `weir-api` binary. The UI is shipped (`weir-ui`, initiative [[WEIR-I-0016]]),
which realizes this ADR: a single WASM SPA, no server-driven rendering, no backdoor.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| SPA (chosen) | Rich async UX; clean API-client boundary | SEO/initial-load (irrelevant for an app) | Low | Medium |
| Server-driven (HTMX/SSR) | Simpler; less JS | Harder for streaming/long-run UX | Medium | Medium |

## Rationale **[REQUIRED]**

The app is behind auth and async-heavy; an SPA fits the streaming-status UX and the strict API-client invariant. The posture was fixed here; the stack was chosen in [[WEIR-A-0035]] (Leptos + Aurora) to build the UI from Colliery's shared design system rather than bespoke chrome.

## Consequences **[REQUIRED]**

### Positive
- Clean UI≡API-client boundary; good async UX. Delivered as an embedded WASM SPA.

### Negative
- Frontend toolchain weight; accessibility/i18n discipline required.

### Neutral
- Depends on the API protocol ([[WEIR-A-0006]]).

## Relationships

- **Stack selected by** [[WEIR-A-0035]] (Web UI on Leptos + the Aurora Dark design system).
- **Realized by** [[WEIR-I-0016]] (weir-ui → Leptos + Aurora Dark; shipped).
- Client of [[WEIR-A-0006]] (API protocol & versioning).
