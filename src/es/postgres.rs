use crate::errors::CqrsError;
use crate::es::storage::{EventStoreStorage, EventStream};
use crate::snapshot::Snapshot;
use crate::{Aggregate, EventEnvelope};
use futures::stream;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio_postgres::Client;

fn map_pg_error<E: std::error::Error + Send + Sync + 'static>(e: E) -> CqrsError {
    CqrsError::database_error(e)
}

#[derive(Clone, Debug)]
pub struct PostgresPersist<A>
where
    A: Aggregate,
{
    _phantom: std::marker::PhantomData<A>,
    client: Arc<Client>,
    snapshot_table_name: String,
    journal_table_name: String,
}

impl<A> PostgresPersist<A>
where
    A: Aggregate,
{
    #[must_use]
    pub fn new(client: Arc<Client>) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            client,
            snapshot_table_name: format!("{}_snapshots", A::TYPE),
            journal_table_name: format!("{}_journal", A::TYPE),
        }
    }

    pub fn snapshot_table_name(&self) -> &str {
        self.snapshot_table_name.as_str()
    }
    pub fn journal_table_name(&self) -> &str {
        self.journal_table_name.as_str()
    }
}

#[async_trait::async_trait]
impl<A> EventStoreStorage<A> for PostgresPersist<A>
where
    A: Aggregate + 'static,
{
    // Minimal session: we control transaction with BEGIN/COMMIT on the same client
    type Session = ();

    async fn start_session(&self) -> Result<Self::Session, CqrsError> {
        self.client
            .batch_execute("BEGIN")
            .await
            .map_err(map_pg_error)?;
        Ok(())
    }

    async fn close_session(&self, _session: Self::Session) -> Result<(), CqrsError> {
        self.client
            .batch_execute("COMMIT")
            .await
            .map_err(map_pg_error)
    }

    async fn fetch_snapshot(
        &self,
        aggregate_id: &str,
    ) -> Result<Option<Snapshot<A>>, CqrsError> {
        let sql = format!(
            "SELECT data, version FROM {} WHERE aggregate_id = $1",
            self.snapshot_table_name
        );
        let row_opt = self
            .client
            .query_opt(&sql, &[&aggregate_id])
            .await
            .map_err(map_pg_error)?;
        if let Some(row) = row_opt {
            let data: JsonValue = row.try_get("data").map_err(map_pg_error)?;
            let version: i64 = row.try_get("version").map_err(map_pg_error)?;
            let state: A = serde_json::from_value(data)
                .map_err(|e| CqrsError::serialization_error(e))?;
            Ok(Some(Snapshot::<A> {
                aggregate_id: aggregate_id.to_string(),
                state,
                version: version as usize,
            }))
        } else {
            Ok(None)
        }
    }

    async fn fetch_events_from_version(
        &self,
        aggregate_id: &str,
        version: usize,
    ) -> Result<EventStream<A>, CqrsError> {
        let sql = format!(
            "SELECT event_id, aggregate_id, version, payload, metadata, at FROM {} WHERE aggregate_id = $1 AND version > $2 ORDER BY version ASC",
            self.journal_table_name
        );
        let rows = self
            .client
            .query(&sql, &[&aggregate_id, &(version as i64)])
            .await
            .map_err(map_pg_error)?;

        let events: Result<Vec<EventEnvelope<A>>, CqrsError> = rows
            .into_iter()
            .map(|row| {
                let payload: JsonValue = row.try_get("payload").map_err(map_pg_error)?;
                let metadata: JsonValue = row.try_get("metadata").map_err(map_pg_error)?;
                Ok(EventEnvelope::<A> {
                    event_id: row.try_get::<_, String>("event_id").map_err(map_pg_error)?,
                    aggregate_id: row
                        .try_get::<_, String>("aggregate_id")
                        .map_err(map_pg_error)?,
                    version: row.try_get::<_, i64>("version").map_err(map_pg_error)? as usize,
                    payload: serde_json::from_value(payload)
                        .map_err(|e| CqrsError::serialization_error(e))?,
                    metadata: serde_json::from_value(metadata)
                        .map_err(|e| CqrsError::serialization_error(e))?,
                    at: row.try_get("at").map_err(map_pg_error)?,
                })
            })
            .collect();

        let events = events?;
        Ok(Box::pin(stream::iter(events.into_iter().map(Ok))))
    }

    async fn fetch_all_events(&self, aggregate_id: &str) -> Result<EventStream<A>, CqrsError> {
        let sql = format!(
            "SELECT event_id, aggregate_id, version, payload, metadata, at FROM {} WHERE aggregate_id = $1 ORDER BY version ASC",
            self.journal_table_name
        );
        let rows = self
            .client
            .query(&sql, &[&aggregate_id])
            .await
            .map_err(map_pg_error)?;

        let events: Result<Vec<EventEnvelope<A>>, CqrsError> = rows
            .into_iter()
            .map(|row| {
                let payload: JsonValue = row.try_get("payload").map_err(map_pg_error)?;
                let metadata: JsonValue = row.try_get("metadata").map_err(map_pg_error)?;
                Ok(EventEnvelope::<A> {
                    event_id: row.try_get::<_, String>("event_id").map_err(map_pg_error)?,
                    aggregate_id: row
                        .try_get::<_, String>("aggregate_id")
                        .map_err(map_pg_error)?,
                    version: row.try_get::<_, i64>("version").map_err(map_pg_error)? as usize,
                    payload: serde_json::from_value(payload)
                        .map_err(|e| CqrsError::serialization_error(e))?,
                    metadata: serde_json::from_value(metadata)
                        .map_err(|e| CqrsError::serialization_error(e))?,
                    at: row.try_get("at").map_err(map_pg_error)?,
                })
            })
            .collect();

        let events = events?;
        Ok(Box::pin(stream::iter(events.into_iter().map(Ok))))
    }

    async fn fetch_events_paged(
        &self,
        aggregate_id: &str,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<EventEnvelope<A>>, i64), CqrsError> {
        // Get total count
        let count_sql = format!(
            "SELECT COUNT(*) FROM {} WHERE aggregate_id = $1",
            self.journal_table_name
        );
        let count_row = self
            .client
            .query_one(&count_sql, &[&aggregate_id])
            .await
            .map_err(map_pg_error)?;
        let total: i64 = count_row.try_get(0).map_err(map_pg_error)?;

        // Get paginated events
        let offset = ((page.max(1) - 1) * page_size) as i64;
        let sql = format!(
            "SELECT event_id, aggregate_id, version, payload, metadata, at FROM {} WHERE aggregate_id = $1 ORDER BY version ASC LIMIT $2 OFFSET $3",
            self.journal_table_name
        );
        let rows = self
            .client
            .query(&sql, &[&aggregate_id, &(page_size as i64), &offset])
            .await
            .map_err(map_pg_error)?;

        let events: Result<Vec<EventEnvelope<A>>, CqrsError> = rows
            .into_iter()
            .map(|row| {
                let payload: JsonValue = row.try_get("payload").map_err(map_pg_error)?;
                let metadata: JsonValue = row.try_get("metadata").map_err(map_pg_error)?;
                Ok(EventEnvelope::<A> {
                    event_id: row.try_get::<_, String>("event_id").map_err(map_pg_error)?,
                    aggregate_id: row
                        .try_get::<_, String>("aggregate_id")
                        .map_err(map_pg_error)?,
                    version: row.try_get::<_, i64>("version").map_err(map_pg_error)? as usize,
                    payload: serde_json::from_value(payload)
                        .map_err(|e| CqrsError::serialization_error(e))?,
                    metadata: serde_json::from_value(metadata)
                        .map_err(|e| CqrsError::serialization_error(e))?,
                    at: row.try_get("at").map_err(map_pg_error)?,
                })
            })
            .collect();

        let events = events?;
        Ok((events, total))
    }

    async fn fetch_latest_event(
        &self,
        aggregate: &A,
        _session: &Self::Session,
    ) -> Result<Option<EventEnvelope<A>>, CqrsError> {
        let sql = format!(
            "SELECT event_id, aggregate_id, version, payload, metadata, at FROM {} WHERE aggregate_id = $1 ORDER BY version DESC LIMIT 1",
            self.journal_table_name
        );
        let row_opt = self
            .client
            .query_opt(&sql, &[&aggregate.aggregate_id()])
            .await
            .map_err(map_pg_error)?;
        if let Some(row) = row_opt {
            let payload: JsonValue = row.try_get("payload").map_err(map_pg_error)?;
            let metadata: JsonValue = row.try_get("metadata").map_err(map_pg_error)?;
            Ok(Some(EventEnvelope::<A> {
                event_id: row.try_get::<_, String>("event_id").map_err(map_pg_error)?,
                aggregate_id: row
                    .try_get::<_, String>("aggregate_id")
                    .map_err(map_pg_error)?,
                version: row.try_get::<_, i64>("version").map_err(map_pg_error)? as usize,
                payload: serde_json::from_value(payload)
                    .map_err(|e| CqrsError::serialization_error(e))?,
                metadata: serde_json::from_value(metadata)
                    .map_err(|e| CqrsError::serialization_error(e))?,
                at: row.try_get("at").map_err(map_pg_error)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn save_events(
        &self,
        events: Vec<EventEnvelope<A>>,
        session: Self::Session,
    ) -> Result<Self::Session, CqrsError> {
        if events.is_empty() {
            return Ok(session);
        }
        let sql = format!(
            "INSERT INTO {} (event_id, aggregate_id, version, payload, metadata, at) VALUES ($1,$2,$3,$4,$5,$6)",
            self.journal_table_name
        );
        // Use single INSERT per event to keep it simple and stay within the explicit transaction
        for e in events.iter() {
            let payload = serde_json::to_value(&e.payload)
                .map_err(|err| CqrsError::serialization_error(err))?;
            let metadata = serde_json::to_value(&e.metadata)
                .map_err(|err| CqrsError::serialization_error(err))?;
            self.client
                .execute(
                    &sql,
                    &[
                        &e.event_id,
                        &e.aggregate_id,
                        &(e.version as i64),
                        &payload,
                        &metadata,
                        &e.at,
                    ],
                )
                .await
                .map_err(map_pg_error)?;
        }
        Ok(session)
    }

    async fn save_snapshot(
        &self,
        aggregate: &A,
        version: usize,
        session: Self::Session,
    ) -> Result<Self::Session, CqrsError> {
        let data = serde_json::to_value(aggregate)
            .map_err(|e| CqrsError::serialization_error(e))?;
        let sql = format!(
            "INSERT INTO {} (aggregate_id, data, version) VALUES ($1, $2, $3) \
             ON CONFLICT (aggregate_id) DO UPDATE SET data = EXCLUDED.data, version = EXCLUDED.version",
            self.snapshot_table_name
        );
        self.client
            .execute(&sql, &[&aggregate.aggregate_id(), &data, &(version as i64)])
            .await
            .map_err(map_pg_error)?;
        Ok(session)
    }
}
