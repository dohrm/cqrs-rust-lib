# Skill: cqrs-migrate-query

Migrate a consumer project from the cqrs-rust-lib 0.6 `QueryBuilder` pattern to the 0.7 `Query`
trait pattern. Operates on the **current working directory** (the consumer project, not the lib
itself).

## What this skill does

1. Locates every `impl QueryBuilder<…>` and `skip`/`limit` field in query structs
2. Determines which backend is in use (postgres / mongodb / surrealdb)
3. Rewrites each query struct to `impl Query` — using auto-derivation where possible
4. Removes `skip`/`limit` fields from query structs (pagination now lives in `CqrsHttpQuery`)
5. Removes the old `QueryBuilder` structs and their modules
6. Updates storage instantiations to drop the `QueryBuilder` type parameter
7. Updates prelude/import lines
8. Runs `cargo build`, `cargo clippy`, `cargo fmt`

## Trigger

User types: `/cqrs-migrate-query`

---

## Key concept: Option B (auto-derived filter)

In 0.7 the `Query` trait has `Serialize` as a supertrait. The default `filter()` implementation
converts every non-`None` field of the struct into an equality constraint automatically.

**This means most query structs need zero `filter()` boilerplate:**

```rust
// Before (0.6)
impl QueryBuilder<GameQuery> for GameQueryBuilder {
    fn to_query(&self, q: &GameQuery, _ctx: &CqrsContext) -> Document {
        let mut doc = Document::new();
        if let Some(cat) = &q.category { doc.insert("category", cat); }
        doc
    }
    fn to_skip_limit(&self, q: &GameQuery, _: &CqrsContext) -> SkipLimit {
        SkipLimit::new(q.skip.map(|s| s as u64), q.limit)
    }
    fn to_sort(&self, _: &GameQuery, _: &CqrsContext) -> Option<Document> { None }
}

// After (0.7)
impl Query for GameQuery {}   // filter auto-derived from Serialize fields
```

Override only when you need non-equality operators or a static sort order.

---

## Instructions

### Step 0 — Orient

Read `Cargo.toml` to confirm:
- `cqrs-rust-lib` is a dependency
- The version is `< 0.7` (or user is explicitly asking to migrate)
- Which features are active: `postgres`, `mongodb`, `surrealdb`

If the version is already `>= 0.7` and no `QueryBuilder` references exist, report "already migrated" and stop.

### Step 1 — Inventory

```bash
grep -rn "QueryBuilder\|SkipLimit" src/ tests/ --include="*.rs"
grep -rn "impl QueryBuilder" src/ --include="*.rs"
grep -rn "skip.*Option<i64>\|limit.*Option<i64>" src/ --include="*.rs"
```

Build a list of:
- Files with `QueryBuilder` implementations
- Files with `QueryBuilder` in storage type parameters
- Query structs containing `skip`/`limit` pagination fields

### Step 2 — Understand each QueryBuilder

For each `impl QueryBuilder<Q> for SomeStruct`:

1. Read the file and the query struct `Q`
2. Classify each field:
   - Equality filter (`==`) → **auto-derived**, no override needed
   - Non-equality filter (`>=`, `like`, etc.) → **manual `filter()` override**
   - `skip`/`limit` → **remove** from struct (pagination comes from HTTP or `pagination()`)
   - Sort → **`default_sort()`** if static, **`sort(&self)`** if dynamic

### Step 3 — Determine what to write

#### Case A — All fields are equality filters (most common)

```rust
// Query struct: add Serialize derive, remove skip/limit
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct GameQuery {
    pub category: Option<String>,
    pub available: Option<bool>,
    // skip and limit removed
}

// Impl: empty — filter auto-derived
impl Query for GameQuery {}
```

#### Case B — Has non-equality operators

```rust
use cqrs_rust_lib::read::Query;
use cqrs_rust_lib::rsql::{Ast, Constraint, Operator, RestSql, Value};

impl Query for ProductQuery {
    fn filter(&self) -> Option<RestSql> {
        // combine auto-derived equality fields with custom constraints
        let mut constraints = vec![];
        if let Some(min) = self.min_price {
            constraints.push(Ast::Constraint(Constraint {
                field: "price".into(),
                operator: Operator::Gte,
                value: Value::Float(min),
            }));
        }
        Ast::try_and(constraints).and_then(|ast| RestSql::from_ast(ast).ok())
    }
}
```

For `Value` mapping:
| Rust type            | `Value` variant             |
|----------------------|-----------------------------|
| `String` / `&str`   | `Value::String(s)`          |
| `i32`/`i64`/`u32`   | `Value::Int(n as i64)`      |
| `f32`/`f64`         | `Value::Float(f as f64)`    |
| `bool`              | `Value::Bool(b)`            |
| `Vec<_>`            | `Value::List(vec![...])`    |

For `Ast` composition:
| Pattern              | Ast                                   |
|----------------------|---------------------------------------|
| single condition     | `Ast::Constraint(Constraint { ... })` |
| AND                  | `Ast::try_and(vec![...])`             |
| OR                   | `Ast::Or(vec![...])`                  |

#### Case C — Has a static sort

```rust
use cqrs_rust_lib::read::{Query, Sorter, SortDirection};

impl Query for GameQuery {
    fn default_sort() -> Option<Vec<Sorter>> {
        Some(vec![Sorter { field: "title".into(), direction: SortDirection::Asc }])
    }
}
```

Use `default_sort()` (associated fn, no `&self`) for unconditional sort. Use `sort(&self)` only
when the sort direction depends on query field values.

### Step 4 — Write the migration

**Query struct file** (e.g., `src/game/query.rs`):

```rust
// Import from cqrs_rust_lib::read directly — backend-agnostic
use cqrs_rust_lib::read::Query;
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;

#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct GameQuery {
    pub category: Option<String>,
    pub available: Option<bool>,
    // skip and limit removed — pagination is handled by CqrsHttpQuery or the HTTP layer
}

impl Query for GameQuery {}
```

**Remove** the old `struct QueryBuilderXxx;` and its `impl QueryBuilder`.

**If the QueryBuilder was a dedicated module** (e.g., `mod query_builder;` in `lib.rs`):
- Delete the file
- Remove `mod query_builder;` from `lib.rs`

**Storage instantiation** (e.g., `src/api.rs`):

```rust
// Before: storage had a third type parameter for the QueryBuilder
SomeStorage::<V, Q, MyQueryBuilder>::new(...)
SomeStorage::<V, Q, MyQueryBuilder>::with_mapper(...)

// After: no QueryBuilder type param
SomeStorage::<V, Q>::new(...)
SomeStorage::<V, Q>::with_mapper(...)
```

**Backend import** — use the prelude pattern:

```rust
// One import controls the entire backend
use cqrs_rust_lib::prelude::postgres as db;   // or mongodb / surrealdb
// use cqrs_rust_lib::prelude::mongodb as db;
// use cqrs_rust_lib::prelude::surrealdb as db;

// Then use db::EventStorePersist, db::ReadStorage, db::FromSnapshotStorage
```

**Remove** from imports:
```rust
// Remove these
use cqrs_rust_lib::prelude::postgres::{QueryBuilder, SkipLimit};
use cqrs_rust_lib::prelude::mongodb::{QueryBuilder, SkipLimit};
use cqrs_rust_lib::prelude::surrealdb::{QueryBuilder, SkipLimit};
```

### Step 5 — Version bump

```toml
cqrs-rust-lib = { version = "0.7", features = ["postgres"] }
```

### Step 6 — Verify

```bash
cargo build
cargo clippy -- -D warnings
cargo fmt --all -- --check
```

Common compilation errors after migration:
- `Query` not in scope → add `use cqrs_rust_lib::read::Query;`
- `Serialize` bound not satisfied → add `#[derive(Serialize)]` to the query struct
- `RestSql::from_ast` returns `Result` → chain `.ok()` to get `Option`
- Multi-condition `Ast::And` not accepted → use `Ast::try_and(vec![...])` instead
- `Value::Int` type mismatch → cast explicitly: `Value::Int(n as i64)`
- `default_sort` not found → it requires `Self: Sized`; don't call it through `dyn Query`

### Step 7 — Report

Summarize:
- Files modified / deleted
- Query structs that used auto-derivation (Case A) vs. manual override (Case B/C)
- Whether `default_sort()` was introduced
- `cargo build` and `cargo clippy` status

---

## SurrealDB field mapping note

SurrealDB storage uses `DataPrefixMapper` by default, which maps `field` → `data.field` in the
stored document. In `Query::filter()`, use the **logical field name** (e.g., `"title"`), not the
stored path (`"data.title"`). The mapper adds the prefix transparently.

If the old `QueryBuilder` hardcoded `data.field`, strip the `data.` prefix when building
`Constraint.field`.

## MongoDB field mapping note

Same rule: use the logical field name in `Constraint.field`. If the storage uses `IdentityMapper`
(default), the field name is passed through as-is.
