#[cfg(test)]
mod integration_tests {
    use cqrs_rust_lib::es::storage::EventStoreStorage;
    use cqrs_rust_lib::es::EventStoreImpl;
    use cqrs_rust_lib::CqrsContext;
    use std::fmt::Debug;
    use std::sync::Arc;
    use todolist::todolist::{CreateCommands, TodoList, UpdateCommands};

    async fn setup_pg() -> Option<tokio_postgres::Client> {
        use tokio_postgres::NoTls;
        // Only run Postgres-backed tests if PG_TEST_URI is provided.
        let dsn = match std::env::var("PG_TEST_URI") {
            Ok(v) => v,
            Err(_) => return None,
        };
        let conn = tokio_postgres::connect(&dsn, NoTls).await;
        let (client, connection) = match conn {
            Ok(parts) => parts,
            Err(_) => return None,
        };
        // Spawn the connection driver
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Postgres connection error: {}", e);
            }
        });

        // Clean and (re)create tables needed by PostgresPersist for TodoList aggregate
        let _ = client
            .batch_execute(
                r#"
                DROP TABLE IF EXISTS todolist_journal;
                DROP TABLE IF EXISTS todolist_snapshots;
                CREATE TABLE IF NOT EXISTS todolist_snapshots (
                    aggregate_id TEXT PRIMARY KEY,
                    data JSONB NOT NULL,
                    version BIGINT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS todolist_journal (
                    event_id TEXT PRIMARY KEY,
                    aggregate_id TEXT NOT NULL,
                    version BIGINT NOT NULL,
                    payload JSONB NOT NULL,
                    metadata JSONB NOT NULL,
                    at TIMESTAMPTZ NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_todolist_journal_agg_ver ON todolist_journal(aggregate_id, version);
                "#,
            )
            .await;

        Some(client)
    }

    async fn testcases<P>(store: P)
    where
        P: EventStoreStorage<TodoList> + Send + Sync + Clone + Debug + 'static,
    {
        let event_store = EventStoreImpl::new(store);
        let engine =
            cqrs_rust_lib::CqrsCommandEngine::new(event_store, vec![], (), Box::new(|_e| {}));
        let context = CqrsContext::default();

        // Create
        let create_res = engine
            .execute_create(
                CreateCommands::Create {
                    name: "My list".into(),
                },
                &context,
            )
            .await;
        assert!(
            create_res.is_ok(),
            "Create command failed: {:?}",
            create_res
        );
        let list_id = create_res.unwrap();

        // Add a todo
        let add_res = engine
            .execute_update(
                &list_id,
                UpdateCommands::AddTodo {
                    title: "Do something".into(),
                },
                &context,
            )
            .await;
        assert!(add_res.is_ok(), "AddTodo failed: {:?}", add_res);

        // Optionally exercise other commands with a placeholder id; domain tolerates missing id
        let _ = engine
            .execute_update(
                &list_id,
                UpdateCommands::AssignTodo {
                    todo_id: "t1".into(),
                    assignee: "bob".into(),
                },
                &context,
            )
            .await;
        let _ = engine
            .execute_update(
                &list_id,
                UpdateCommands::ResolveTodo {
                    todo_id: "t1".into(),
                },
                &context,
            )
            .await;
        let _ = engine
            .execute_update(
                &list_id,
                UpdateCommands::RemoveTodo {
                    todo_id: "t1".into(),
                },
                &context,
            )
            .await;
    }

    #[tokio::test]
    async fn test_postgres_event_store() {
        if let Some(client) = setup_pg().await {
            let store =
                cqrs_rust_lib::es::postgres::PostgresPersist::<TodoList>::new(Arc::new(client));
            testcases(store).await;
        } else {
            panic!("PG_TEST_URI not set or connection failed; skipping Postgres test");
        }
    }

    #[tokio::test]
    async fn test_inmemory_event_store() {
        let store = cqrs_rust_lib::es::inmemory::InMemoryPersist::<TodoList>::new();
        testcases(store).await;
    }
}
