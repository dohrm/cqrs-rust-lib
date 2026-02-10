use crate::errors::CqrsError;
use crate::es::storage::{EventStoreStorage, EventStream};
use crate::snapshot::Snapshot;
use crate::{Aggregate, EventEnvelope};
use futures::{StreamExt, TryStreamExt};
use mongodb::bson::doc;
use mongodb::{ClientSession, Database};

fn map_mongo_error(e: mongodb::error::Error) -> CqrsError {
    CqrsError::database_error(e)
}

#[derive(Clone, Debug)]
pub struct MongoDBPersist<A>
where
    A: Aggregate,
{
    _phantom: std::marker::PhantomData<A>,
    database: Database,
    snapshot_collection_name: String,
    journal_collection_name: String,
}

impl<A> MongoDBPersist<A>
where
    A: Aggregate,
{
    #[must_use]
    pub fn new(database: Database) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            database,
            snapshot_collection_name: format!("{}_snapshots", A::TYPE),
            journal_collection_name: format!("{}_journal", A::TYPE),
        }
    }

    pub fn snapshot_collection_name(&self) -> &str {
        self.snapshot_collection_name.as_str()
    }
    pub fn journal_collection_name(&self) -> &str {
        self.journal_collection_name.as_str()
    }

    fn snapshot_collection(
        &self,
        session: Option<&ClientSession>,
    ) -> mongodb::Collection<Snapshot<A>> {
        if let Some(session) = session {
            session
                .client()
                .database(self.database.name())
                .collection(self.snapshot_collection_name.as_str())
        } else {
            self.database
                .collection(self.snapshot_collection_name.as_str())
        }
    }
    pub fn journal_collection(
        &self,
        session: Option<&ClientSession>,
    ) -> mongodb::Collection<EventEnvelope<A>> {
        if let Some(session) = session {
            session
                .client()
                .database(self.database.name())
                .collection(self.journal_collection_name.as_str())
        } else {
            self.database
                .collection(self.journal_collection_name.as_str())
        }
    }
}

#[async_trait::async_trait]
impl<A> EventStoreStorage<A> for MongoDBPersist<A>
where
    A: Aggregate + 'static,
{
    type Session = ClientSession;
    async fn start_session(&self) -> Result<Self::Session, CqrsError> {
        let mut session = self
            .database
            .client()
            .start_session()
            .await
            .map_err(map_mongo_error)?;
        session.start_transaction().await.map_err(map_mongo_error)?;
        Ok(session)
    }

    async fn close_session(&self, mut session: Self::Session) -> Result<(), CqrsError> {
        session.commit_transaction().await.map_err(map_mongo_error)
    }

    async fn fetch_snapshot(
        &self,
        aggregate_id: &str,
    ) -> Result<Option<Snapshot<A>>, CqrsError> {
        self.snapshot_collection(None)
            .find_one(doc! { "_id": aggregate_id})
            .await
            .map_err(map_mongo_error)
    }

    async fn fetch_events_from_version(
        &self,
        aggregate_id: &str,
        version: usize,
    ) -> Result<EventStream<A>, CqrsError> {
        let cursor = self
            .journal_collection(None)
            .find(doc! {"aggregateId": aggregate_id, "version": {"$gt": version as i64}})
            .await
            .map_err(map_mongo_error)?;

        Ok(Box::pin(cursor.map(|result| {
            result.map_err(|e| CqrsError::database_error(e))
        })))
    }

    async fn fetch_all_events(&self, aggregate_id: &str) -> Result<EventStream<A>, CqrsError> {
        let cursor = self
            .journal_collection(None)
            .find(doc! {"aggregateId": aggregate_id})
            .await
            .map_err(map_mongo_error)?;

        Ok(Box::pin(cursor.map(|result| {
            result.map_err(|e| CqrsError::database_error(e))
        })))
    }

    async fn fetch_events_paged(
        &self,
        aggregate_id: &str,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<EventEnvelope<A>>, i64), CqrsError> {
        // Get total count
        let total = self
            .journal_collection(None)
            .count_documents(doc! {"aggregateId": aggregate_id})
            .await
            .map_err(map_mongo_error)?;

        // Get paginated events
        let offset = ((page.max(1) - 1) * page_size) as u64;
        let mut cursor = self
            .journal_collection(None)
            .find(doc! {"aggregateId": aggregate_id})
            .skip(offset)
            .limit(page_size as i64)
            .await
            .map_err(map_mongo_error)?;

        let mut events = Vec::new();
        while let Some(next) = cursor.try_next().await.map_err(map_mongo_error)? {
            events.push(next);
        }

        Ok((events, total as i64))
    }

    async fn fetch_latest_event(
        &self,
        aggregate: &A,
        session: &Self::Session,
    ) -> Result<Option<EventEnvelope<A>>, CqrsError> {
        self.journal_collection(Some(session))
            .find_one(doc! {"aggregateId": aggregate.aggregate_id()})
            .sort(doc! {"version": -1})
            .await
            .map_err(map_mongo_error)
    }

    async fn save_events(
        &self,
        events: Vec<EventEnvelope<A>>,
        session: &mut Self::Session,
    ) -> Result<(), CqrsError> {
        let _r = self
            .journal_collection(Some(session))
            .insert_many(&events)
            .await
            .map_err(map_mongo_error)?;
        Ok(())
    }

    async fn save_snapshot(
        &self,
        aggregate: &A,
        version: usize,
        session: &mut Self::Session,
    ) -> Result<(), CqrsError> {
        self.snapshot_collection(Some(session))
            .find_one_and_replace(
                doc! {"_id": aggregate.aggregate_id()},
                Snapshot::<A> {
                    aggregate_id: aggregate.aggregate_id(),
                    state: aggregate.clone(),
                    version,
                },
            )
            .upsert(true)
            .await
            .map_err(map_mongo_error)?;
        Ok(())
    }

    async fn abort_session(&self, mut session: Self::Session) -> Result<(), CqrsError> {
        session
            .abort_transaction()
            .await
            .map_err(map_mongo_error)
    }
}
