#[cfg(test)]
mod integration_tests {
    use cqrs_rust_lib::es::storage::EventStoreStorage;
    use cqrs_rust_lib::es::EventStoreImpl;
    use cqrs_rust_lib::CqrsContext;
    use bank::account::{Account, CreateCommands, UpdateCommands};
    use mongodb::{Client, Database};
    use std::env;

    async fn setup_test_db() -> Database {
        let mongodb_uri = env::var("MONGODB_TEST_URI")
            .unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
        let client = Client::with_uri_str(&mongodb_uri)
            .await
            .expect("Failed to connect to MongoDB");
        let database = client.database("test_db");
        let _r = database.drop().await;
        database
    }

    async fn testcases<P>(store: P)
    where
        P: EventStoreStorage<Account>,
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
        let db = setup_test_db().await;
        let store = cqrs_rust_lib::es::mongodb::MongoDBPersist::<Account>::new(db);
        testcases(store).await;
    }

    #[tokio::test]
    async fn test_inmemory_event_store() {
        let store = cqrs_rust_lib::es::inmemory::InMemoryPersist::<Account>::new();
        testcases(store).await;
    }
}
