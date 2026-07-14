# Typed schemas and evolution

Records flow through weir as JSON, which is convenient until a source quietly changes shape and a destination
silently breaks. weir gives every stream a **typed schema** and a defined policy for what happens when that
schema **drifts**.

## A stream has a schema

A schema is a list of fields, each `{ name, type, nullable }`, where `type` is one of `str`, `integer`, `float`,
`boolean`, `timestamp`, `json`. weir gets it one of two ways: a connector can **declare** it, or — the common
case — weir **infers** it from a sample of the first run's records (numeric widening, mixed types fall back to
`json`, a field that's ever null or absent is nullable). Either way the schema is captured once and persisted per
stream.

## The schema is enforced, on the write path

On subsequent runs, weir **coerces** each record to the stored schema using the same machinery as the in-flight
`cast` mapping: a `"42"` becomes an integer where the schema says integer. A value that can't be coerced, or a
missing **non-nullable** field, **dead-letters** that record with a reason — it doesn't reach the destination.
Extra fields the schema doesn't mention pass through untouched. Enforcement runs on the *post-mapping* shape, so
what's checked is exactly what would be written.

## Drift: additive flows, breaking is surfaced

Sources evolve. Each run weir re-infers the current shape and **diffs** it against the stored schema:

- A **new field** is *additive* — it's merged into the stored schema, and the run proceeds.
- A **type change** on an existing field is *breaking* — weir flags the stream (a reason like "field `amount`
  changed integer → string") rather than silently applying it. The breaking-typed records dead-letter, so the
  destination is protected, and the flag is visible in the UI.

An operator resolves a breaking change by **accepting** the new schema — weir then re-baselines from the current
shape and records enforce against the new types. Nothing about a source's shape changes your destination behind
your back.

## Why infer, and why not exactly-once schema

Inference means every stream gets a usable schema without waiting on connector authors to declare one — and it's
good enough in practice, since a source's real types show up plainly in its records. The choice to *surface*
breaking drift rather than auto-migrate is deliberate: an automatic type change is exactly the kind of silent
corruption the schema exists to prevent. weir would rather stop and ask.
