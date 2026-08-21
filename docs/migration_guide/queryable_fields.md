# Migration guide — `_q` names the query's own fields, and `sort` an optional list

Three changes. The first two are **breaking for every view**; the third is narrower.

## 1. `_q` may only name fields of your query type — breaking

An endpoint has two ways to filter, and they are not two features: the typed params of
`Q` (`?category=famille`) and the RSQL string `_q`. They are **one set of filterable
fields in two syntaxes** — RSQL exists because a flat `?field=value` cannot express `>=`,
`=in=`, `or` or a range.

`_q` used to escape that. `RestSql::new` validates operators and syntax, not field names,
so `_q` reached anything a caller could name in the stored document while the typed params
reached only `Q`'s fields. A field not reachable as a query param has no reason to be
reachable from `_q`.

The set is now derived from the query struct's `Deserialize` impl. There is no list to
declare, and no way to offer a filter with no field behind it.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct GameQuery {
    pub category: Option<String>,
    pub available: Option<bool>,
    pub title: Option<String>,   // filterable, therefore a field
}
```

| Request | Before | After |
|---|---|---|
| `?_q=category==famille` | `200` | `200` — `category` is a field |
| `?_q=title==Catan` (no `title` field) | `200`, filter applied | **`422`** naming `title` |
| `?_q=borrower==alice` (view-only field) | `200`, filter applied | **`422`** |
| `?_q=…` on a query type with no fields | `200`, filter applied | **`422`** — the endpoint offers no filter |

**What to do**: add the field to the query struct. That is the whole remedy, and it is
also the gain — the field becomes a typed parameter in the OpenAPI document at the same
time.

**What it buys, precisely**: the fields are still returned in the response body, so this
is not a confidentiality boundary. It stops a listing endpoint from doubling as a *lookup
by* a field the endpoint does not offer.

### One shape that breaks the equivalence

Field names are read off `Deserializer::deserialize_struct`. serde does not emit that for
a `#[serde(flatten)]` field, nor for a unit, newtype or tuple struct — so a query type of
one of those shapes derives **no** field while `IntoParams` still publishes its params.
The two syntaxes then disagree, which is exactly what this change exists to prevent. It is
logged at `warn`; the fix is to keep the fields directly on the query struct rather than
flattening another type into it.

## 2. `Query::sortable_fields()` — breaking: a view sorts on nothing until it says so

`sort` reached `ORDER BY` after a check that the name is an identifier, which says nothing
about whether the view offers it — so any column of the stored document was orderable by
anyone who guessed its name.

```rust
impl Query for GameQuery {
    fn sortable_fields(&self) -> Vec<&str> {
        vec!["id", "title", "category"]
    }
}
```

| Request | Before | After |
|---|---|---|
| `?sort=title`, list declares `title` | `200` | `200` |
| `?sort=internal_rank`, list declares `title` | `200` | **`422`** naming the field |
| `?sort=anything`, **no list declared** | `200` | **`422`** — the view offers no sort |
| `?sort=` (empty value) | `200` | `200` — an absent param, not an empty sort |

- **Empty is the default and it means the view offers no sort.** A field is sortable
  because the view says so, never by default — the same reading as an empty filterable
  set above. **Every view served by `CQRSCodexReadRouter` loses `sort` until it declares
  a list.**
- The list is the **whole** sortable set, not an addition to the query's own fields:
  declare `vec!["title"]` and `?sort=category` is refused even if `category` is a field.
- It is a list rather than a derivation because ordering by a column of the *view* —
  `title`, `created_at` — is reasonable where filtering on it is not, so there is nothing
  to derive it from.
- It constrains the **caller**. `Query::default_sort()` is written in Rust and is not
  filtered by it: a view can order its own results while offering the caller no say.
- It is **not** published in the OpenAPI document: the method takes `&self`, and
  `into_params` is static.

Sort field *shape* — that a name is an identifier at all — is still validated at the
storage layer and still answers **400**. The two checks ask different questions.

## 3. `Q: Query` on the extractor — breaking for one case

`impl FromRequestParts for CqrsHttpQuery<Q>` also requires `Q: Query` now, which is what
gives it `Q::sortable_fields()`.

| You | Affected |
|---|---|
| Use `CQRSCodexReadRouter` | **No** — it already required `Q: Query`. |
| Hand-write a handler and call `.filter()` / `.sort()` / `.pagination()` | **No** — those need the same bound already. |
| Hand-write a handler only to read `typed()` | **Yes** — add `impl Query for MyQuery {}`; every method has a default. |

## Nested paths

The derived set holds top-level field names only, so `_q=amount.value>=10` now answers
**422**. Nested filtering is out of scope: add a flat field to the query struct and let
the storage's `FieldMapper` point it at the nested path.

## `#[serde(alias)]` is unsupported on a query type

`#[serde(rename)]` is fine: it moves the name on both sides. `alias` does not — it adds a
name serde *accepts* without changing the one it *produces*. The derived set is
alias-expanded, so `_q=label==x` is admitted and reaches the storage as `label`, while
`?label=x` deserialises into `name` and filters on `name`. One asks for a column that does
not exist: `500` on Postgres, an empty page with a correct-looking `total: 0` on MongoDB
and SurrealDB.

Nothing detects this. **Do not put `#[serde(alias)]` on a query struct** — rename the
field, or add a second field.

## A trap the check cannot catch

The names are checked against your struct, then handed to the backend's `FieldMapper`. A
view stored as the document itself — MongoDB, or SurrealDB under `DataPrefixMapper` —
resolves them as written; a view inside a JSONB column under the default `IdentityMapper`
does not, and `name` compiles to `WHERE name = $1` against a table with no such column.
Give the storage a mapper that resolves the names your struct declares.
