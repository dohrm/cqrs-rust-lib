#[cfg(test)]
mod integration_tests {
    use bank::account::{Account, CreateCommands, UpdateCommands};
    use cqrs_rust_lib::es::storage::EventStoreStorage;
    use cqrs_rust_lib::es::EventStoreImpl;
    use cqrs_rust_lib::CqrsContext;
    use mongodb::{Client, Database};
    use std::env;
    use std::fmt::Debug;

    /// `None` when `MONGODB_TEST_URI` is unset, so the suite skips instead of failing.
    ///
    /// It used to default to `mongodb://localhost:27017`, which meant `cargo test
    /// --workspace` hung for 60s and then failed on any machine without a server — the
    /// same shape `setup_pg` in the todolist example already avoided. `just db-up`
    /// provides the URI; `just test-db` passes it in.
    async fn setup_test_db() -> Option<Database> {
        let mongodb_uri = env::var("MONGODB_TEST_URI").ok()?;
        let client = Client::with_uri_str(&mongodb_uri)
            .await
            .expect("MONGODB_TEST_URI is set but does not parse");
        let database = client.database("test_db");
        let _r = database.drop().await;
        Some(database)
    }

    async fn testcases<P>(store: P)
    where
        P: EventStoreStorage<Account> + Send + Sync + Clone + Debug + 'static,
    {
        let event_store = EventStoreImpl::new(store);
        let engine =
            cqrs_rust_lib::CqrsCommandEngine::new(event_store, vec![], (), Box::new(|_e| {}));
        let context = CqrsContext::default();

        let value = engine
            .execute_create(
                CreateCommands::Create {
                    owner: "bob".into(),
                },
                &context,
            )
            .await;
        assert!(value.is_ok());
        let uuid = value.unwrap();

        let value = engine
            .execute_update(
                &uuid,
                UpdateCommands::Deposit {
                    amount: 50f64.into(),
                },
                &context,
            )
            .await;
        println!("{:?}", value);
        assert!(value.is_ok());

        let value = engine
            .execute_update(
                &uuid,
                UpdateCommands::Deposit {
                    amount: 50f64.into(),
                },
                &context,
            )
            .await;
        assert!(value.is_ok());
        let value = engine
            .execute_update(
                &uuid,
                UpdateCommands::Deposit {
                    amount: 50f64.into(),
                },
                &context,
            )
            .await;
        assert!(value.is_ok());
        let value = engine
            .execute_update(
                &uuid,
                UpdateCommands::Deposit {
                    amount: 50f64.into(),
                },
                &context,
            )
            .await;
        assert!(value.is_ok());
    }

    #[tokio::test]
    async fn test_mongodb_event_store() {
        let Some(db) = setup_test_db().await else {
            eprintln!("skipped: MONGODB_TEST_URI unset — run `just db-up` then `just test-db`");
            return;
        };
        let store = cqrs_rust_lib::prelude::mongodb::EventStorePersist::<Account>::new(db);
        testcases(store).await;
    }

    #[tokio::test]
    async fn test_inmemory_event_store() {
        let store = cqrs_rust_lib::prelude::inmemory::EventStorePersist::<Account>::new();
        testcases(store).await;
    }
}
