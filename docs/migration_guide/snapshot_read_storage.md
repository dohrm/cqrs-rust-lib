# Migration guide — `FromSnapshotStorage` takes the snapshot table, not a view storage

`FromSnapshotStorage` could not read a single row on any backend. The event store and the
read side disagreed about the shape of a snapshot, and the disagreement was different on
each one. Fixing it changes the constructor.

## What was broken

| Backend | `find_by_id` / `filter` before | Why |
|---|---|---|
| Postgres | **500** | The view storage queried `WHERE id = $1` against a table keyed on `aggregate_id`, and deserialized `data` into a `Snapshot<A>` while the column holds the **bare aggregate**. |
| SurrealDB | **500** | Same deserialization mismatch — `missing field '_id'`. |
| MongoDB | `find_by_id` worked; a filter returned **an empty page and no error** | The document is `{_id, state: {…}, version}`, so a filter on `name` has to reach `state.name`. Under the identity mapper it reached `name`, which matched nothing. |

A **second, independent** defect on Postgres: `PostgresStorage::save` strips the id field
out of the `data` payload — the id lives in its own column — and nothing put it back on
read, so *every* view read failed with `missing field 'id'`. Fixed by reinstating it from
the `id` column, which also makes rows written before this readable.

## The new constructor

```rust
// before — a view storage wrapped around a table whose schema did not match
let repo = db::FromSnapshotStorage::<TodoList, TodoListQuery>::new(
    Arc::new(db::ReadStorage::new(client.clone(), TodoList::TYPE, es.snapshot_table_name())),
);

// after — the snapshot table itself
let repo = db::FromSnapshotStorage::<TodoList, TodoListQuery>::new(
    client.clone(),
    es.snapshot_table_name(),
);
```

Same shape on all three backends: `new(client_or_database, snapshot_table_or_collection)`,
plus `with_mapper(..., mapper)` and, on Postgres, `with_pool` / `with_pool_and_mapper`.

## The default mapper changed, and it matters

A filter has to name the place the aggregate actually sits inside the stored row. The
default is no longer `IdentityMapper`:

| Backend | Default mapper | A filter on `name` compiles to |
|---|---|---|
| Postgres | `JsonbDataMapper` | `data->>'name'` |
| SurrealDB | `DataPrefixMapper` (unchanged) | `data.name` |
| MongoDB | `SnapshotStateMapper` | `state.name` |

Pass `with_mapper` if your snapshot rows are laid out differently.

**On Postgres, only string filters work.** `->>` yields `text` while the driver binds a
filter value by its own type, and tokio-postgres refuses the pair: `counter==3` against a
snapshot answers **500**, not a wrong result. SurrealDB and MongoDB are unaffected — their
stored values keep their types.

This is inherent to filtering a JSONB blob whose field types are unknown, and it is why a
snapshot table is a shortcut rather than a read model. Project a real view when a filter
needs types, or pass `with_mapper` a mapper that casts the fields you filter on.

## What else changed

- `filter` and `find_by_id` reject a non-`None` parent id on **all three** backends: a
  snapshot table has no parent column. Previously the argument was accepted and quietly
  ignored.
- `save` is still unsupported — the event store owns the table.
