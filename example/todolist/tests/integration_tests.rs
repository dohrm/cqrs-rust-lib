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
        let Some(client) = setup_pg().await else {
            eprintln!("PG_TEST_URI not set or connection failed — skipping Postgres test");
            return;
        };
        let client = Arc::new(client);
        let store =
            cqrs_rust_lib::prelude::postgres::EventStorePersist::<TodoList>::new(client.clone());
        testcases(store).await;

        // Same test, not a separate one: `setup_pg` drops and recreates the tables, so a
        // second Postgres test running concurrently would pull them out from under this.
        read_route_reads_what_the_event_store_wrote(client).await;
    }

    /// The read route #10 was about. An aggregate written through the event store's own
    /// API, then read back through the snapshot storage the API wires — the two disagreed
    /// on the row shape, so both reads answered 500 before the fix.
    async fn read_route_reads_what_the_event_store_wrote(client: Arc<tokio_postgres::Client>) {
        use cqrs_rust_lib::prelude::postgres as db;
        use cqrs_rust_lib::read::storage::Storage;
        use cqrs_rust_lib::CqrsContext;
        use todolist::todolist::query::TodoListQuery;

        let es = db::EventStorePersist::<TodoList>::from_client(client.clone());
        let mut session = es.start_session().await.expect("session");
        es.save_snapshot(
            &TodoList {
                id: "t1".to_string(),
                name: "groceries".to_string(),
                todos: vec![],
            },
            1,
            &mut session,
        )
        .await
        .expect("save_snapshot");
        es.close_session(session).await.expect("commit");

        let storage = db::FromSnapshotStorage::<TodoList, TodoListQuery>::new(
            client,
            es.snapshot_table_name(),
        );
        let ctx = CqrsContext::default();

        let found = storage
            .find_by_id(None, "t1", ctx.clone())
            .await
            .expect("find_by_id must not error");
        assert_eq!(found.map(|t| t.name), Some("groceries".to_string()));

        // `testcases` above left its own snapshot behind, so assert on this row rather
        // than on the count.
        let page = storage
            .filter(None, TodoListQuery { name: None }, ctx.clone())
            .await
            .expect("filter must not error");
        assert!(
            page.items.iter().any(|t| t.id == "t1"),
            "the unfiltered page must contain the row just written, got {:?}",
            page.items.iter().map(|t| &t.id).collect::<Vec<_>>()
        );

        // And a declared filter reaches `data->>'name'`, where the aggregate's field is.
        let page = storage
            .filter(
                None,
                TodoListQuery {
                    name: Some("groceries".to_string()),
                },
                ctx,
            )
            .await
            .expect("filter by name");
        assert_eq!(
            page.items.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            vec!["t1"],
            "the filter must match exactly the row whose name it names"
        );
    }

    #[tokio::test]
    async fn test_inmemory_event_store() {
        let store = cqrs_rust_lib::prelude::inmemory::EventStorePersist::<TodoList>::new();
        testcases(store).await;
    }
}
