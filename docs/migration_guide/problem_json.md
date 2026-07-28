# Migration guide — RFC 9457 `application/problem+json`

The REST layer can serve errors as [RFC 9457](https://www.rfc-editor.org/rfc/rfc9457)
problem documents. The behaviour is **opt-in**: without the `problem-json`
feature nothing changes.

## Enabling it

```toml
cqrs-rust-lib = { version = "0.7", features = ["rest", "problem-json"] }
```

## What changes

| | Default | `problem-json` |
|---|---|---|
| `Content-Type` | `application/json` | `application/problem+json` |
| Message field | `message` | `detail` |
| Status in body | absent | `status` |
| Problem type | — | `type` |
| Occurrence URI | — | `instance` |
| OpenAPI error schema | `CqrsErrorData` shape | `ProblemDetails` shape (same `CqrsError` component name) |

Before:

```json
{
  "domain": "account",
  "code": "ACCOUNT_INSUFFICIENT_FUNDS",
  "internalCode": 10001,
  "message": "Cannot withdraw 500, balance is 200",
  "requestId": "req-123"
}
```

After:

```json
{
  "type": "urn:cqrs-error:account:ACCOUNT_INSUFFICIENT_FUNDS",
  "title": "ACCOUNT_INSUFFICIENT_FUNDS",
  "status": 400,
  "detail": "Cannot withdraw 500, balance is 200",
  "instance": "urn:cqrs-request:req-123",
  "domain": "account",
  "code": "ACCOUNT_INSUFFICIENT_FUNDS",
  "internalCode": 10001,
  "requestId": "req-123"
}
```

`domain`, `code`, `internalCode`, `details` and `requestId` are kept as RFC 9457
extension members, so support tooling keyed on `internalCode` keeps working.
Clients reading `message` must switch to `detail`.

## Client migration

```diff
- const message = body.message;
+ const message = body.detail ?? body.message;
```

Accept both media types while rolling out:

```diff
- if (res.headers.get("content-type")?.includes("application/json")) { … }
+ const ct = res.headers.get("content-type") ?? "";
+ if (ct.includes("application/json") || ct.includes("application/problem+json")) { … }
```

## Configuring the `type` member

Resolution order:

1. Per-error override — `CqrsError::with_type_uri("https://…")`
2. Process-wide base URI — `problem::set_problem_type_base_uri("https://api.example.com/errors")`
   yields `{base}/{code}`
3. Default — `urn:cqrs-error:{domain}:{code}`

```rust
use cqrs_rust_lib::problem::set_problem_type_base_uri;

// once, at startup
set_problem_type_base_uri("https://api.example.com/errors").unwrap();
```

The base URI lives in a `OnceLock`: set it before serving requests; a second
call returns `Err` and leaves the first value in place.

## `instance` and `requestId`

The REST routers stamp errors with `CqrsContext::request_id()` unless the error
already carries one, so `instance` (`urn:cqrs-request:{id}`) and `requestId` are
populated whenever the context has a request id — e.g. behind a middleware
calling `CqrsContext::with_next_request_id()`. Empty request ids are omitted.

## Rendering problem documents yourself

`CqrsError::to_problem()` and `problem::ProblemDetails` are available with or
without the feature:

```rust
use cqrs_rust_lib::problem::PROBLEM_JSON;

let problem = err.to_problem();
(status, [(header::CONTENT_TYPE, PROBLEM_JSON)], Json(problem))
```

## Related changes in the same release

- Malformed command bodies now answer **422 Unprocessable Content** instead of
  500 (`INFRASTRUCTURE_SERIALIZATION_ERROR`).
- `CqrsHttpQuery` rejections and read-router 404s go through `CqrsError`, so
  every error on a generated route shares one body shape.
- `CqrsError::from_status` no longer collapses unmapped statuses to 500 — see
  `docs/migration_guide/domain_errors.md`.
- Generated OpenAPI operations declare their error responses with the matching
  media type and a `CqrsError` schema reference.
