# Field mapping

weir applies an optional **`MappingSpec`** to every record **between read and write** — in the engine, not the
connector. Mapping is for *bounded, per-record shaping*: rename, drop, cast, filter, and simple computed fields.
It is deliberately **not** a transform engine — no I/O, no joins, no aggregation, no arbitrary code. Heavier
transforms belong post-load (the dbt boundary, [[WEIR-A-0026]]).

A spec is an ordered list of ops applied left-to-right to each record's top-level JSON object. Records that
aren't JSON objects pass through untouched.

## Operators

| Op | Shape | Effect |
|---|---|---|
| **Select** | `{ fields: [..] }` | Keep only the listed fields (drop everything else). |
| **Drop** | `{ fields: [..] }` | Remove the listed fields; keep the rest. |
| **Rename** | `{ from, to }` | Rename a field. Absent `from` → no-op. |
| **Cast** | `{ field, to }` | Coerce to `str` \| `integer` \| `float` \| `boolean`. |
| **Filter** | `{ field, op, value }` | Keep the record only if the predicate holds; else it's *filtered* (dropped, counted). |
| **Compute** | `{ field, value }` | Set a field from an expression: `const` \| `field` \| `concat` \| `lower` \| `upper` (result is a string). |

`Filter` comparators: `eq`, `ne`, `lt`, `le`, `gt`, `ge`. A comparison is numeric when both sides parse as a
number, otherwise it's a string comparison.

## Defined behaviour on the edges

These are contract, covered by tests in `crates/weir-engine/src/mapping.rs`:

- **Absent field** — `Rename`, `Drop`, `Select`, `Cast`, `Compute`-source on a missing field are **no-ops**, never
  errors. (`Filter` on an absent field *excludes* the record — the predicate can't be satisfied.)
- **`null` values** — a `null` **passes through any cast unchanged** (SQL-nullable semantics); one empty optional
  field never dead-letters the whole record.
- **Uncastable value** — a value that can't be coerced (e.g. the string `"abc"` → `integer`, or an object →
  `integer`) **dead-letters** that record with a reason; the rest of the sync continues.
- **Unmapped fields** — anything an op doesn't touch **passes through** (only `Select` prunes).
- **Order matters** — ops compose in order; e.g. `cast n→integer` then `rename n→id` yields `id` as an integer.

## Example

Source record:

```json
{ "Id": "42", "Amount": "150.00", "note": "x", "status": "active" }
```

Spec — cast the id, keep only what matters, rename, and gate on status:

```json
[
  { "cast":   { "field": "Id", "to": "integer" } },
  { "cast":   { "field": "Amount", "to": "float" } },
  { "filter": { "field": "status", "op": "eq", "value": "active" } },
  { "rename": { "from": "Id", "to": "id" } },
  { "select": { "fields": ["id", "Amount"] } }
]
```

Result:

```json
{ "id": 42, "Amount": 150.0 }
```

## Not supported (by design)

- **Nested paths** — ops address **top-level** keys only (no `a.b.c` traversal). Reshape nested payloads
  post-load, or at the source.
- **Numeric compute** — `compute` results are strings; use it for keys/labels, not arithmetic.
- **Cross-record** anything — joins, lookups, aggregation, dedup. That's the transform layer's job, not mapping's.
