#[cfg(test)]
mod integration_tests {
    use chrono::Utc;
    use cqrs_rust_lib::dispatchers::ViewDispatcher;
    use cqrs_rust_lib::es::inmemory::InMemoryPersist;
    use cqrs_rust_lib::es::surrealdb::SurrealDBPersist;
    use cqrs_rust_lib::es::storage::EventStoreStorage;
    use cqrs_rust_lib::es::EventStoreImpl;
    use cqrs_rust_lib::read::surrealdb::SurrealDBStorage;
    use cqrs_rust_lib::read::storage::DynStorage;
    use cqrs_rust_lib::{Aggregate, CqrsCommandEngine, CqrsContext, Dispatcher};
    use ludotheque::game::{CreateCommands, Game, GameQuery, GameView, UpdateCommands};
    use ludotheque::query_builder::GameQueryBuilder;
    use std::fmt::Debug;
    use std::sync::Arc;
    use surrealdb::engine::any::{connect, Any};
    use surrealdb::Surreal;

    async fn setup_surreal() -> Surreal<Any> {
        let db = connect("mem://").await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db.query(SurrealDBPersist::<Game>::schema())
            .await
            .unwrap()
            .check()
            .unwrap();
        db.query("DEFINE TABLE IF NOT EXISTS game_view SCHEMALESS;")
            .await
            .unwrap()
            .check()
            .unwrap();
        db
    }

    // ─── Tests commandes (event store uniquement) ─────────────────────────────

    async fn testcases_event_store<P>(store: P)
    where
        P: EventStoreStorage<Game> + Send + Sync + Clone + Debug + 'static,
    {
        let event_store = EventStoreImpl::new(store);
        let engine = CqrsCommandEngine::new(event_store, vec![], (), Box::new(|_e| {}));
        let context = CqrsContext::default();

        // Enregistrer un jeu
        let game_id = engine
            .execute_create(
                CreateCommands::Register {
                    title: "Catan".into(),
                    description: "Colonisateurs de Catane".into(),
                    category: "stratégie".into(),
                    min_players: 3,
                    max_players: 4,
                },
                &context,
            )
            .await
            .expect("Register failed");

        // Titre vide → erreur 400
        let err = engine
            .execute_create(
                CreateCommands::Register {
                    title: "   ".into(),
                    description: "".into(),
                    category: "".into(),
                    min_players: 2,
                    max_players: 4,
                },
                &context,
            )
            .await
            .expect_err("Empty title should fail");
        assert_eq!(err.http_status(), http::StatusCode::BAD_REQUEST);

        // Nombre de joueurs invalide (min > max) → erreur 400
        let err = engine
            .execute_create(
                CreateCommands::Register {
                    title: "Jeu invalide".into(),
                    description: "".into(),
                    category: "".into(),
                    min_players: 5,
                    max_players: 2,
                },
                &context,
            )
            .await
            .expect_err("Invalid player count should fail");
        assert_eq!(err.http_status(), http::StatusCode::BAD_REQUEST);

        // Emprunter le jeu
        engine
            .execute_update(
                &game_id,
                UpdateCommands::Borrow {
                    borrower: "Alice".into(),
                    until: Utc::now() + chrono::Duration::days(7),
                },
                &context,
            )
            .await
            .expect("Borrow failed");

        // Re-emprunter → erreur 409
        let err = engine
            .execute_update(
                &game_id,
                UpdateCommands::Borrow {
                    borrower: "Bob".into(),
                    until: Utc::now() + chrono::Duration::days(3),
                },
                &context,
            )
            .await
            .expect_err("Double borrow should fail");
        assert_eq!(err.http_status(), http::StatusCode::CONFLICT);

        // Retourner le jeu
        engine
            .execute_update(&game_id, UpdateCommands::Return, &context)
            .await
            .expect("Return failed");

        // Re-retourner → erreur 409
        let err = engine
            .execute_update(&game_id, UpdateCommands::Return, &context)
            .await
            .expect_err("Double return should fail");
        assert_eq!(err.http_status(), http::StatusCode::CONFLICT);

        // Mettre à jour le titre
        engine
            .execute_update(
                &game_id,
                UpdateCommands::Update {
                    title: Some("Les Colons de Catane".into()),
                    description: None,
                },
                &context,
            )
            .await
            .expect("Update failed");

        // Titre vide dans Update → erreur 400
        let err = engine
            .execute_update(
                &game_id,
                UpdateCommands::Update {
                    title: Some("".into()),
                    description: None,
                },
                &context,
            )
            .await
            .expect_err("Empty title in Update should fail");
        assert_eq!(err.http_status(), http::StatusCode::BAD_REQUEST);
    }

    // ─── Test pipeline complet (vue + requêtes) ───────────────────────────────

    async fn testcases_full_pipeline(db: Surreal<Any>) {
        let view_storage: DynStorage<GameView, GameQuery> = Arc::new(SurrealDBStorage::new(
            db.clone(),
            Game::TYPE,
            GameQueryBuilder,
            "game_view",
        ));

        let view_dispatcher =
            ViewDispatcher::<Game, GameView, GameQuery>::new(view_storage.clone());
        let es_persist = SurrealDBPersist::<Game>::new(db.clone());
        let event_store = EventStoreImpl::new(es_persist);
        let effects: Vec<Box<dyn Dispatcher<Game> + Send + Sync>> = vec![Box::new(view_dispatcher)];
        let engine = Arc::new(CqrsCommandEngine::new(
            event_store,
            effects,
            (),
            Box::new(|_e| {}),
        ));
        let context = CqrsContext::default();

        // Enregistrer deux jeux dans des catégories différentes
        let id_catan = engine
            .execute_create(
                CreateCommands::Register {
                    title: "Catan".into(),
                    description: "Colonisateurs de Catane".into(),
                    category: "stratégie".into(),
                    min_players: 3,
                    max_players: 4,
                },
                &context,
            )
            .await
            .expect("Register Catan failed");

        let id_uno = engine
            .execute_create(
                CreateCommands::Register {
                    title: "Uno".into(),
                    description: "Jeu de cartes classique".into(),
                    category: "cartes".into(),
                    min_players: 2,
                    max_players: 10,
                },
                &context,
            )
            .await
            .expect("Register Uno failed");

        // Les deux vues doivent exister et être disponibles
        let view_catan = view_storage
            .find_by_id(None, &id_catan, context.clone())
            .await
            .unwrap()
            .expect("Catan view missing");
        assert_eq!(view_catan.title, "Catan");
        assert!(view_catan.available);
        assert!(view_catan.borrower.is_none());

        let view_uno = view_storage
            .find_by_id(None, &id_uno, context.clone())
            .await
            .unwrap()
            .expect("Uno view missing");
        assert_eq!(view_uno.title, "Uno");
        assert!(view_uno.available);

        // Emprunter Catan
        engine
            .execute_update(
                &id_catan,
                UpdateCommands::Borrow {
                    borrower: "Alice".into(),
                    until: Utc::now() + chrono::Duration::days(7),
                },
                &context,
            )
            .await
            .expect("Borrow Catan failed");

        // La vue de Catan reflète l'emprunt
        let view_catan = view_storage
            .find_by_id(None, &id_catan, context.clone())
            .await
            .unwrap()
            .expect("Catan view missing after borrow");
        assert!(!view_catan.available);
        assert_eq!(view_catan.borrower, Some("Alice".into()));
        assert!(view_catan.borrow_until.is_some());

        // Filtre : disponibles seulement → Uno uniquement
        let available = view_storage
            .filter(
                None,
                GameQuery {
                    category: None,
                    available: Some(true),
                    skip: None,
                    limit: None,
                },
                context.clone(),
            )
            .await
            .unwrap();
        assert_eq!(available.total, 1);
        assert_eq!(available.items[0].title, "Uno");

        // Filtre : non disponibles → Catan uniquement
        let borrowed = view_storage
            .filter(
                None,
                GameQuery {
                    category: None,
                    available: Some(false),
                    skip: None,
                    limit: None,
                },
                context.clone(),
            )
            .await
            .unwrap();
        assert_eq!(borrowed.total, 1);
        assert_eq!(borrowed.items[0].title, "Catan");

        // Filtre par catégorie
        let strategy = view_storage
            .filter(
                None,
                GameQuery {
                    category: Some("stratégie".into()),
                    available: None,
                    skip: None,
                    limit: None,
                },
                context.clone(),
            )
            .await
            .unwrap();
        assert_eq!(strategy.total, 1);
        assert_eq!(strategy.items[0].title, "Catan");

        // Retourner Catan
        engine
            .execute_update(&id_catan, UpdateCommands::Return, &context)
            .await
            .expect("Return Catan failed");

        let view_catan = view_storage
            .find_by_id(None, &id_catan, context.clone())
            .await
            .unwrap()
            .expect("Catan view missing after return");
        assert!(view_catan.available);
        assert!(view_catan.borrower.is_none());
        assert!(view_catan.borrow_until.is_none());

        // Mettre à jour le titre
        engine
            .execute_update(
                &id_catan,
                UpdateCommands::Update {
                    title: Some("Les Colons de Catane".into()),
                    description: Some("Édition collector".into()),
                },
                &context,
            )
            .await
            .expect("Update title failed");

        let view_catan = view_storage
            .find_by_id(None, &id_catan, context.clone())
            .await
            .unwrap()
            .expect("Catan view missing after update");
        assert_eq!(view_catan.title, "Les Colons de Catane");
        assert_eq!(view_catan.description, "Édition collector");

        // Tous les jeux sans filtre → 2
        let all = view_storage
            .filter(
                None,
                GameQuery {
                    category: None,
                    available: None,
                    skip: None,
                    limit: None,
                },
                context.clone(),
            )
            .await
            .unwrap();
        assert_eq!(all.total, 2);
    }

    // ─── Cas de tests ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_surreal_event_store() {
        let db = setup_surreal().await;
        let store = SurrealDBPersist::<Game>::new(db);
        testcases_event_store(store).await;
    }

    #[tokio::test]
    async fn test_inmemory_event_store() {
        let store = InMemoryPersist::<Game>::new();
        testcases_event_store(store).await;
    }

    #[tokio::test]
    async fn test_full_pipeline_surreal() {
        let db = setup_surreal().await;
        testcases_full_pipeline(db).await;
    }
}
