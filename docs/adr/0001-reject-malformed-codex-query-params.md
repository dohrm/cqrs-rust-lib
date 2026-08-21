# ADR-0001 — A malformed codex query parameter is rejected, not dropped

- **Status**: Accepted
- **Date**: 2026-08-19

## Context

`CqrsHttpQuery<Q>` reads `_q`, `skip`, `limit`, `page`, `page_size` and `sort` off the
query string. Every one of them was parsed with `.ok()`: `RestSql::new(q).ok()` for `_q`,
`v.parse().ok()` for the integers. A value the extractor could not read was therefore
discarded and replaced by a default.

For `_q` that means the filter fails **open**: a caller who mistypes
`?_q=status=bad=active` asked for a subset and receives the whole collection, with `200`
and nothing saying a filter was ignored. On a view the caller was not meant to enumerate,
that is a disclosure. For `?limit=abc` the caller asked one question and was answered
another. rest-sql already produces a positioned error — line, column, caret — and `.ok()`
threw it away.

## Decision

We will reject a codex query parameter the extractor cannot read, during extraction,
with **422 Unprocessable Entity**.

- `_q` is parsed in `from_request_parts`; `CqrsHttpQuery` stores the validated `RestSql`,
  so `Query::filter()` keeps its signature and can no longer receive invalid input.
- The rejection message carries rest-sql's `ParseErrorAt` verbatim, caret included.
- `skip` and `page` must be `>= 0`; `limit` and `page_size` must be `>= 1`. Out of range
  is refused rather than clamped because it has no single meaning: a negative `skip` is
  clamped by two backends and errors on the third, and `limit=0` is "zero rows" on
  postgres/surrealdb but the MongoDB wire protocol's *no limit*.
- `page` without `page_size` is refused: `pagination()` reads the pair, so on its own it
  was silently answered with page 0.
- `page * page_size` must not overflow `i64` — unchecked, it wraps to a negative skip.
- `422`, not `400`: the request is well-formed HTTP; it is the *content* of a parameter
  the server cannot act on. Every parameter **the extractor parses** answers this way,
  typed fields of `Q` included — moving those from `400` to `422`. `sort` is not one of
  them; see Consequences.
- An *empty* value (`?_q=&limit=10`) is unset, not unreadable, and is accepted — that is
  how a form serializes an absent field.
- No feature flag. See Consequences.

## Consequences

Requests that returned `200` now return `422`, and a typed-field rejection moves from
`400` to `422`. That is a breaking change for any caller relying on a bad `_q` being
ignored, and it needs a version bump. Documented in
`docs/migration_guide/codex_query_rejection.md`.

There is no flag to restore the old behaviour: a flag would ship the fail-open filter as
a supported configuration, and only one of the two behaviours is defensible.

The MongoDB read backend compiled a parsed filter with `.unwrap_or_default()`, and an
empty BSON document matches everything — the same fail-open one layer down. It now
propagates the error, as postgres and surrealdb already did.

`sort` still answers `400`: it is validated at the storage layer, because a `Sorter` also
arrives from `Query::default_sort()` and never passes the extractor. A caller does see two
statuses — the split is extractor versus sink, not an accident.

## Alternatives considered

- **Keep `.ok()` and document the trap** — leaves the filter failing open; a documented
  disclosure is still a disclosure.
- **Answer `400`** — defensible for a parse error, but splits codex parameters across two
  statuses for one class of caller mistake.
- **Reject `_q` in `filter()` rather than in the extractor** — `Query::filter()` returns
  `Option<RestSql>` with nowhere to put an error, so it would have to widen the `Query`
  trait for every backend.
- **Feature-flag the old behaviour** — see Consequences.
