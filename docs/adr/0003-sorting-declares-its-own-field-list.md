# ADR-0003 — Sorting is offered field by field, and nothing is offered by default

- **Status**: Accepted
- **Date**: 2026-08-21

## Context

[ADR-0002](0002-declare-the-queryable-field-surface-on-the-view.md) binds `_q` to the
query struct's fields, because filtering by RSQL and by typed param are one operation in
two syntaxes. Sorting is not that operation, and the same derivation does not fit it: a
caller routinely wants to order by a column of the *view* — `title`, `created_at`, `date`
— that has no business being a filter parameter. Deriving the sortable set from `Q` would
refuse exactly the common case, so there is nothing to derive it from.

Meanwhile `sort` reaches `ORDER BY` after `Sorter::validated_field` has checked the name
is an identifier, which says nothing about whether the view offers it. Any column of the
stored document is orderable by anyone who guesses its name.

## Decision

We will have the view declare what it sorts on:

```rust
fn sortable_fields(&self) -> Vec<&str> { vec![] }
```

- **Empty — the default — means the view offers no sort**, and every caller-supplied
  `sort` is refused with `422`. A field is sortable because the view says so, never by
  default. That is the same reading as an empty filterable set in ADR-0002: an empty
  offer is an offer of nothing, not an absence of rules.
- The list is the **whole** sortable set, not an addition to the query's own fields:
  declare `vec!["title"]` and `?sort=category` is refused even if `category` is a field.
- A field outside the list → `422` naming it.
- It constrains the *caller* only. `Query::default_sort()` is written in Rust and is not
  filtered by it, so a view can order its own results while offering the caller no say.
  `Sorter::validated_field` still gates every sort at the storage layer, asking the
  different question of whether the name is an identifier at all.
- `?sort=` with an empty value stays an *absent* parameter, as everywhere else in the
  extractor — it asks for no sort rather than for an empty one.

## Consequences

**Breaking.** Every view served by `CQRSCodexReadRouter` loses `sort` until it declares a
list; `?sort=title` that answered `200` now answers `422`. Needs a version bump, with
ADR-0002's changes. The remedy is one method, and writing it is the point: the fields a
caller may order by become a decision someone made rather than whatever the document
happens to store.

**The `&self` receiver costs three things.** The list cannot be published in the OpenAPI
document (`into_params` is static), it allocates a `Vec` per request, and — the one that
matters — an implementation that branched on a field value would let the caller widen
their own allowlist. No implementation does today. A static
`fn sortable_fields() -> &'static [&'static str]` would remove all three, and
`Query::default_sort` on the same trait is already static.

A declared name the backend's `FieldMapper` cannot resolve still advertises a sort the
storage cannot serve. Nothing checks that.

## Alternatives considered

- **Derive it from `Q` like `_q`** — refuses ordering by a view column that is not a
  filter, which is the common case.
- **Empty means no restriction** — non-breaking, and the first draft of this record. It
  cannot express "this view is not sortable", and it leaves every existing view ordering
  on columns nobody chose to expose.
- **Reuse `_q`'s set and allow additions** — two rules where one is enough, and the
  additive reading is the one a reader gets wrong.
