# Migration guide — commands that produce no events

A command handler may return `Ok(vec![])`. What that means now depends on the command.

| Command | Before | After |
|---|---|---|
| Update, no events | `Ok` on Postgres, SurrealDB and InMemory; **500** on MongoDB (`insert_many` rejects an empty batch, #13) | `Ok` on every backend. Nothing is written: no transaction, no snapshot rewrite. |
| Update, no events, while another writer commits | **409** `concurrency_error` | `Ok` |
| Create, no events | `Ok` on Postgres, SurrealDB and InMemory, with an aggregate stored without any event; **500** on MongoDB | **500** `INTERNAL_ERROR`: `"<TYPE> create command produced no events"`. Nothing is stored. |

## Update: why the conflict is gone

The engine loads the aggregate, runs the handler, then commits at the version it loaded. A
409 on an empty update meant another writer had committed in between. But a command that
writes nothing can be ordered *before* that writer: it read a valid state and decided
there was nothing to do. Rejecting it asked the client to retry a command whose answer
was already right.

## Create: why it is an error

An aggregate exists through its creation event. A create that emits nothing used to leave
a snapshot with an empty journal: the id worked, but no event recorded where the
aggregate came from.

If your handler declines to create on purpose, return your own domain error from
`handle_create`. It reaches the client with its own code and status, instead of the
generic 500 the engine raises for an empty list:

```rust
async fn handle_create(&self, command: Self::CreateCommand, /* … */)
    -> Result<Vec<Self::Event>, Self::Error>
{
    match command {
        CreateCommands::Import { rows } if rows.is_empty() => {
            Err(ErrorCode::NothingToImport.error("No rows to import"))
        }
        // …
    }
}
```

Aggregates created without events before this change are still loaded: their snapshot is
read as usual.
