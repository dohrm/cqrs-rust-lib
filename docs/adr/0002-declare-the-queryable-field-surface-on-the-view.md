# ADR-0002 — `_q` and the typed query params are the same set

- **Status**: Accepted
- **Date**: 2026-08-20

## Context

`CqrsHttpQuery<Q>` gives an endpoint two ways to filter: the typed params of `Q`
(`?category=famille`) and the RSQL string `_q`. They are not two features. They are one
set of filterable fields in two syntaxes — RSQL exists because a flat `?field=value`
cannot express `>=`, `=in=`, `or`, or a range.

But `_q` is compiled with `RestSql::new`, which validates operators and syntax and **not
field names**. So the two syntaxes reach different sets: the typed params reach `Q`'s
fields, while `_q` reaches anything a caller can name in the stored document. `GameView`
holds `borrower`; `Movement` holds `account_id` while `MovementQuery` declared no field at
all. `?_q=borrower==alice` filters on a field no query param offers, which is how a
listing becomes a lookup by borrower.

## Decision

We will make the two syntaxes name the same set.

- **A field is filterable from `_q` if and only if it is a field of `Q`.** If it is not
  reachable as a query param, it has no reason to be reachable from `_q`. The set is
  derived from `Q`'s `Deserialize` impl (`rest_sql::dsl::serde_fields`) — no second list,
  no phantom names, and every filterable field is a *typed* parameter in the OpenAPI
  document by construction. Anything else → `422` naming it. Not opt-in.
- **A field whose name the extractor consumes is in neither set.** `_q`, `skip`, `limit`,
  `page`, `page_size`, `pageSize` and `sort` are subtracted from the derived set and from
  the published params: `?limit=5` is pagination, so `Q::limit` is unreachable both ways.
- **An empty derived set means no filter can be derived**, and `_q` is refused wholesale.

## Consequences

**Breaking, deliberately.** `?_q=title==Catan` against a `GameQuery` with no `title` field
now answers `422`. The remedy is to add the field — which is the point, since it gains a
typed OpenAPI parameter with it. Needs a version bump; see
`docs/migration_guide/queryable_fields.md`.

The extractor now requires `Q: Query`, breaking a hand-written handler that reads only
`typed()`. One `impl Query for Q {}` fixes it.

**`#[serde(flatten)]` breaks the equivalence.** serde emits no `deserialize_struct` for a
flattened field, nor for a unit/newtype/tuple struct, so such a type derives no field
while `IntoParams` still publishes its params — the two syntaxes diverge, which is the one
thing this decision forbids. Logged once per type at `warn`; a query type must keep its
fields on itself.

**Nested paths are lost.** The derivation returns top-level names only, while an RSQL
selector may be dotted and the backends are built for it (`DataPrefixMapper`). So
`_q=amount.value>=10` now answers `422`. Nested filtering is out of scope: add a flat
field and let the storage's `FieldMapper` point it at the nested path.

**`#[serde(alias)]` is unsupported on a query type, and the check cannot enforce that.**
serde builds `FIELDS` alias-expanded, so the derived set admits the alias — but `_q` never
passes through serde: the name goes straight into the AST and out through the
`FieldMapper` as written. `?label=x` therefore filters on `name` while `_q=label==x`
filters on `label`, which is the divergence this record exists to remove. `rename` is
fine — it moves both sides. Fixing it means an allowlist built from the *serialize* names
(what `derive_filter_from_serde` emits) rather than the deserialize names, which
`serde_fields` cannot give; that is a follow-up decision, not this one. Pinned by
`a_serde_alias_reaches_q_under_its_own_name_which_is_why_it_is_unsupported`.

## Alternatives considered

- **A `queryable_fields()` list of strings** — the first draft. Rejected: it permits
  fields with nothing behind them, and a name known only as a string cannot become a
  typed OpenAPI parameter.
- **`RestSql::new_for::<Q>()`** — same derivation and blind spot, but its error renders as
  a `Debug` dump of a rest-sql-internal enum, which ADR-0001 exists to avoid.
- **Leave `_q` unbounded and document it** — keeps two syntaxes with two different sets,
  which is the defect.
