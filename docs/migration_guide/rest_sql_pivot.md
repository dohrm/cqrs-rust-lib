# Migration Guide: QueryBuilder → Query trait (0.6 → 0.7)

## Overview

Version 0.7 replaces the three backend-specific `QueryBuilder<Q>` traits with a single
`Query` trait backed by `rest-sql` as an AST pivot. This is a breaking change that affects:

- All types implementing `QueryBuilder<Q>` (one per backend)
- All storage instantiations that included a `QueryBuilder` type parameter

`rest-sql` is now a mandatory (non-optional) dependency. `rest-sql-drivers` remains optional
and is pulled in automatically by the `postgres` and `surrealdb` features.

---

## Before / After at a glance

### Before (0.6)

Each backend had its own `QueryBuilder<Q>` trait with incompatible signatures:

```rust
// postgres — returns (String, Vec<Box<dyn ToSql>>)
impl QueryBuilder<TodoListQuery> for QueryBuilderTodoList {
    fn build_where(&self, query: &TodoListQuery) -> Option<(String, Vec<Box<dyn ToSql + Sync + Send>>)> {
        let name = query.name.as_ref()?;
        Some(("name = $1".to_string(), vec![Box::new(name.clone())]))
    }
    fn skip_limit(&self, query: &TodoListQuery) -> SkipLimit {
        SkipLimit { skip: query.skip, limit: query.limit }
    }
}

// Storage: three type params
let storage = PostgresStorage::<TodoList, TodoListQuery, QueryBuilderTodoList>::new(
    client, "todolist", "todolist_view"
);
```

```rust
// mongodb — returns Option<Document>
impl QueryBuilder<AccountQuery> for QueryBuilderAccount {
    fn build_filter(&self, query: &AccountQuery) -> Option<Document> {
        let owner = query.owner.as_ref()?;
        Some(doc! { "data.owner": owner })
    }
    fn skip_limit(&self, query: &AccountQuery) -> SkipLimit { ... }
}
```

```rust
// surrealdb — returns Option<String> (SurrealQL fragment)
impl QueryBuilder<GameQuery> for GameQueryBuilder {
    fn build_where(&self, query: &GameQuery) -> Option<String> { ... }
    fn bind_params(&self, query: &GameQuery) -> Vec<(String, surrealdb::Value)> { ... }
    fn skip_limit(&self, query: &GameQuery) -> SkipLimit { ... }
}
```

### After (0.7)

One trait, all backends:

```rust
use cqrs_rust_lib::read::{Query, Pagination, Sorter, SortDirection};
use cqrs_rust_lib::rsql::{Ast, Constraint, Operator, RestSql, Value};

impl Query for TodoListQuery {
    fn filter(&self) -> Option<RestSql> {
        let name = self.name.as_ref()?;
        RestSql::from_ast(Ast::Constraint(Constraint {
            field: "name".into(),
            operator: Operator::Eq,
            value: Value::String(name.clone()),
        })).ok()
    }
    fn pagination(&self) -> Option<Pagination> {
        Some(Pagination { skip: self.skip, limit: self.limit })
    }
    fn sort(&self) -> Option<Vec<Sorter>> { None }
}

// Storage: two type params (mapper defaults to IdentityMapper)
let storage = PostgresStorage::<TodoList, TodoListQuery>::new(client, "todolist", "todolist_view");
```

---

## Step-by-step migration

### 1. Update Cargo.toml

In your application crate, `rest-sql` is now transitively available through `cqrs-rust-lib`.
You do not need to add it as a direct dependency, but you *can* if you want version control:

```toml
[dependencies]
cqrs-rust-lib = { version = "0.7", features = ["postgres"] }  # or mongodb / surrealdb
```

### 2. Delete QueryBuilder implementations

Remove every `impl QueryBuilder<YourQuery> for YourQueryBuilder` struct and its module.

### 3. Implement `Query` on your query struct

```rust
use cqrs_rust_lib::read::{Query, Pagination, Sorter, SortDirection};
// rest-sql types re-exported as cqrs_rust_lib::rsql
use cqrs_rust_lib::rsql::{Ast, Constraint, Operator, RestSql, Value};

impl Query for YourQuery {
    fn filter(&self) -> Option<RestSql> {
        // return None for "no filter"
        // otherwise build an Ast and wrap it
    }

    fn pagination(&self) -> Option<Pagination> {
        Some(Pagination { skip: self.skip, limit: self.limit })
    }

    fn sort(&self) -> Option<Vec<Sorter>> {
        None // or Some(vec![Sorter { field: "name".into(), direction: SortDirection::Asc }])
    }
}
```

#### Building filters

Single condition:
```rust
RestSql::from_ast(Ast::Constraint(Constraint {
    field: "status".into(),
    operator: Operator::Eq,
    value: Value::String("active".into()),
})).ok()
```

Multiple conditions (AND):
```rust
let mut conditions = vec![];
if let Some(cat) = &self.category {
    conditions.push(Ast::Constraint(Constraint {
        field: "category".into(),
        operator: Operator::Eq,
        value: Value::String(cat.clone()),
    }));
}
if let Some(available) = self.available {
    conditions.push(Ast::Constraint(Constraint {
        field: "available".into(),
        operator: Operator::Eq,
        value: Value::Bool(available),
    }));
}
match conditions.len() {
    0 => None,
    1 => RestSql::from_ast(conditions.remove(0)).ok(),
    _ => RestSql::from_ast(Ast::And(conditions)).ok(),
}
```

Available operators: `Eq`, `Neq`, `Lt`, `Lte`, `Gt`, `Gte`, `Like`, `In`, `Out`.

### 4. Update storage instantiation

Remove the third type parameter from storage types:

| Before | After |
|---|---|
| `PostgresStorage::<V, Q, MyBuilder>::new(...)` | `PostgresStorage::<V, Q>::new(...)` |
| `MongoDbStorage::<V, Q, MyBuilder>::new(...)` | `MongoDbStorage::<V, Q>::new(...)` |
| `SurrealDBStorage::<V, Q, MyBuilder>::new(...)` | `SurrealDBStorage::<V, Q>::new(...)` |

Or use the prelude aliases:
```rust
use cqrs_rust_lib::prelude::postgres as db;
// db::ReadStorage<V, Q> — no mapper param needed
let storage = db::ReadStorage::<TodoList, TodoListQuery>::new(client, "todolist", "todolist_view");
```

### 5. Update prelude imports

Remove any imports of `QueryBuilder`, `SkipLimit` — they no longer exist.

```rust
// Before
use cqrs_rust_lib::prelude::postgres::{QueryBuilder, SkipLimit, ...};

// After
use cqrs_rust_lib::prelude::postgres::{Pagination, Query, Sorter, SortDirection, ...};
```

---

## Backend-specific notes

### PostgreSQL

The default mapper is `IdentityMapper` (field name → field name, no transformation). Filters are
compiled to `WHERE` clauses with `$1`/`$2`… positional parameters.

If your view stores the document in a `data` JSONB column and you filter on JSONB paths, pass a
custom `FieldMapper`:

```rust
use cqrs_rust_lib::rsql::FieldMapper;
use std::borrow::Cow;

#[derive(Debug, Clone)]
struct DataMapper;
impl FieldMapper for DataMapper {
    fn map<'a>(&self, field: &'a str) -> Cow<'a, str> {
        Cow::Owned(format!("data->>'{}' ", field))
    }
}

let storage = PostgresStorage::<V, Q, DataMapper>::with_mapper(client, type_name, table, DataMapper);
```

### MongoDB

The default mapper is `IdentityMapper`. Filters are compiled to BSON `Document` queries.
If your view document nests fields under a `data` sub-document, use:

```rust
struct DataMapper;
impl FieldMapper for DataMapper {
    fn map<'a>(&self, field: &'a str) -> Cow<'a, str> {
        Cow::Owned(format!("data.{}", field))
    }
}
```

### SurrealDB

The default mapper is `DataPrefixMapper`, which maps `field` → `data.field`. This matches the
table schema produced by `SurrealDBPersist` (event store) where the view row stores the entity
in a `data` object field. If your table stores fields at the root level, override with `IdentityMapper`:

```rust
use cqrs_rust_lib::read::surrealdb::SurrealDBStorage;
use cqrs_rust_lib::rsql::IdentityMapper;

let storage = SurrealDBStorage::<V, Q, IdentityMapper>::with_mapper(db, type_name, table, IdentityMapper);
```

---

## Snapshot storages

`PostgresFromSnapshotStorage`, `MongoDBFromSnapshotStorage`, and `SurrealDBFromSnapshotStorage`
follow the same pattern — remove the `QueryBuilder` type parameter:

```rust
// Before
PostgresFromSnapshotStorage::<A, Q, MyBuilder>::new(Arc::new(inner))

// After
PostgresFromSnapshotStorage::<A, Q>::new(Arc::new(inner))
```

---

## What changed internally

- `rest-sql` is compiled into the AST representation by the storage layer using the
  `rest-sql-drivers` backend (Postgres: `PgCompiler`, SurrealDB: `SurrealCompiler`).
- MongoDB uses a built-in compiler (bson 2.x) because `rest-sql-drivers/mongodb` requires bson 3.x,
  which conflicts with the `mongodb 3.2` client crate.
- `Pagination` and `Sorter` types are defined in `crate::read::query` and re-exported from all
  backend preludes and `crate::read`.
