---
id: 001-secrets-are-off-platform-consumed
level: adr
title: "Secrets are off-platform: consumed, not managed"
number: 1
short_code: "WEIR-A-0037"
created_at: 2026-07-07T04:00:33.228349+00:00
updated_at: 2026-07-07T04:00:33.228349+00:00
decision_date: 2026-07-07
decision_maker: dylan
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-37: Secrets are off-platform: consumed, not managed

## Context

A capability review flagged "production secret lifecycle" (rotation, a Vault/KMS backend) as a gap. weir already
resolves + **injects credentials host-side** ([[WEIR-A-0033]]) — bearer / OAuth2 / session / basic / AWS SigV4 —
so the secret never enters the guest sandbox. The open question was whether weir should *own* the secret
lifecycle: storage, rotation, a managed backend.

## Decision

**No. Secrets live off-platform; weir consumes them, it does not manage them.** weir reads a secret from where
the operator's secret system already puts it (env / mounted file / injected connection config), uses it to sign
or authenticate an outbound request, and re-reads the current value on each run so **rotation is transparent** —
the operator rotates in their system of record and weir simply picks up the new value. weir builds **no** secret
store, no rotation scheduler, and no Vault/KMS integration of its own.

## Rationale

- **Separation of concerns / smaller blast radius.** A secret manager is a whole security product; owning one
  makes weir a higher-value target and duplicates what operators already run (Vault, cloud secret managers,
  k8s Secrets, SOPS). Consuming rotated values is strictly less to get wrong.
- **The hard part is already done right** — host-side injection keeps secrets out of the guest ([[WEIR-A-0033]]),
  and config sanitization strips them before the guest ever sees the config. That's the security-critical piece.
- **Rotation "just works"** when we re-read per run and never cache a secret across its rotation window.

## Consequences

### Positive
- weir stays a data-movement platform, not a secrets platform; less attack surface, less to operate.
- Integrates with whatever the operator already uses (env, files, k8s Secrets, SOPS, external-secrets operator).

### Negative
- weir provides no rotation UX or audit of secret *access* beyond the request it makes; that's the operator's
  secret system's job.

### Neutral
- The test-only SOPS bundles ([[WEIR-I-0014]]) remain a *test* convenience, not a product feature.
- Requirement on connectors/host: **re-read the secret per run; never cache across rotation.**
