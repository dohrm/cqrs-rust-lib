# Migration guide — a malformed codex query param is rejected

`CqrsHttpQuery<Q>` used to drop a query parameter it could not read and carry on with a
default. It now rejects the request with **422 Unprocessable Entity**.

This is a **behaviour change on a success path**: requests that returned `200` now
return `422`. Nothing about it is opt-in, because the old behaviour had no safe version —
see [Why not a feature flag](#why-not-a-feature-flag).

## What changed

| Request | Before | After |
|---|---|---|
| `?_q=status=bad=active` | `200` — filter dropped, **whole collection returned** | `422`, with rest-sql's positioned error |
| `?limit=abc` | `200` — silently `limit=20` | `422` naming `limit` |
| `?skip=abc`, `?page=abc`, `?page_size=abc`, `?pageSize=abc` | `200` — silently defaulted | `422` naming the parameter |
| `?limit=-1` (and any negative `skip`/`page`/`page_size`) | `200` on mongodb/surrealdb (clamped with `.max(0)`), `500` on postgres (the server rejects a negative `OFFSET`/`LIMIT`) | `422` |
| `?page=4611686018427387904&page_size=4` | panic under `overflow-checks`, a negative skip without them | `422` |
| `?limit=0`, `?page_size=0` | zero rows on postgres/surrealdb, **the whole collection** on mongodb (`0` is the driver's "no limit") | `422` |
| `?page=3` with no `page_size`, `skip` or `limit` | `200` — silently page 0 | `422` |
| `?_q=`, `?skip=`, any codex param with an empty value | `200` | `200` — unchanged, an empty value means *unset* |
| A typed field of `Q` that fails to deserialize | `400` | `422` |

Two rows deserve a second look. A typed field of `Q` moves from `400` to `422`, so
assertions on that status need updating. And an *empty* value is unchanged: `?_q=` is how
a form or a serialized object writes "unset", so it means the parameter is absent, not
unreadable.

### The error code moved with the status

The body carries a machine-readable code, and it changed too:

| | Before | After |
|---|---|---|
| `code` | `GENERIC_VALIDATION_FAILED` | `GENERIC_UNPROCESSABLE_ENTITY` |
| `internalCode` | `1001` | `1422` |

If your client switches on `code` or `internalCode` rather than on the HTTP status, this
is the line that breaks it.

## Why this is the fix and not a regression

`_q` is a filter. Dropping one that fails to parse means the filter fails **open**: a
caller who mistypes `status=bad=active` asked for a subset and received everything,
with a `200` and no indication anything was ignored. On a view holding data the caller
was not meant to enumerate, that is a disclosure, not an inconvenience.

The integer parameters had the same shape (`v.parse().ok()`): `?limit=abc` was answered
with the default of 20, so the caller asked one question and was answered another.

## What the response looks like now

`_q` errors carry rest-sql's `ParseErrorAt`, which the previous code discarded:

```
parse error at 1:8 — expected an operator (==, !=, =in=, ...)
  status=bad=active
         ^
```

With the `problem-json` feature the same text lands in `detail`; without it, in
`message`. Either way the status is `422` and the body is the crate's normal error
document — see [`problem_json.md`](problem_json.md).

## What you have to do

- **Callers sending a valid query**: nothing. A query that parsed before parses now.
- **Callers relying on a bad `_q` being ignored**: fix the query. There is no flag to
  restore the old behaviour.
- **Tests asserting `400` on a malformed query param**: change them to `422` — except
  for `sort`, which is validated at the storage layer and still answers `400`.
- **Clients treating `2xx` as "the filter was applied"**: that assumption is now true —
  but only because a second fail-open was closed with it. See below.

## A second fail-open, closed at the same time

`_q` parsing was not the only place a dropped filter meant "return everything". The
MongoDB read backend compiled the parsed filter with `.unwrap_or_default()`, and an empty
BSON `Document` matches **every** document. So a filter that parsed but failed to compile
— `=like=` against a non-string, for instance — produced the whole collection with a
`200`, exactly like a malformed `_q` did. Postgres and SurrealDB already propagated that
error; MongoDB was the outlier and now does too.

**What changes**: on MongoDB, a filter that cannot be compiled answers `500` instead of
`200`-with-everything. If you were relying on that (you were not: it returned unfiltered
data), the fix is to send a filter the backend supports.

## Why not a feature flag

A flag would mean shipping the fail-open filter as a supported configuration. The
`problem-json` feature is opt-in because both error formats are legitimate; here only
one of the two behaviours is, so there is nothing to choose between.

## Not covered by this change

The `sort` parameter still answers **400**, not 422. It is validated in the storage layer
rather than in the extractor, because a `Sorter` also reaches that layer from
`Query::default_sort()` written in Rust and never passes through the HTTP boundary. So
"every codex parameter answers 422" is not true and is not claimed: every parameter *the
extractor parses* does. See the `sort` section of the README.
