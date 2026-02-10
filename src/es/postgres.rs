use crate::errors::CqrsError;
use crate::es::storage::{EventStoreStorage, EventStream};
use crate::snapshot::Snapshot;
use crate::{Aggregate, EventEnvelope};
use futures::stream;
use serde_json::Value as JsonValue;
use std::fmt::Debug;
use std::sync::Arc;
use tokio_postgres::Client;

fn map_pg_error<E: std::error::Error + Send + Sync + 'static>(e: E) -> CqrsError {
    CqrsError::database_error(e)
}

/// Access to a `tokio_postgres::Client`.
pub trait PgConn: Send + Sync {
    fn client(&self) -> &Client;
}

/// Factory / pool of connections.
#[async_trait::async_trait]
pub trait PgPool: Send + Sync + Debug + Clone + 'static {
    type Connection: PgConn + Send + Sync + 'static;
    async fn acquire(&self) -> Result<Self::Connection, CqrsError>;
}

/// Wraps a single `Arc<Client>`. NOT safe for concurrent transactions.
#[derive(Debug, Clone)]
pub struct SharedClient(pub Arc<Client>);

impl PgConn for SharedClient {
    fn client(&self) -> &Client {
        &self.0
    }
}

#[async_trait::async_trait]
impl PgPool for SharedClient {
    type Connection = SharedClient;
    async fn acquire(&self) -> Result<Self::Connection, CqrsError> {
        Ok(self.clone())
    }
}

/// Wraps a connection obtained from a `PgPool`, tracking transaction state.
pub struct PgSession<C: PgConn + 'static> {
    connection: C,
    committed: bool,
}

impl<C: PgConn + 'static> PgSession<C> {
    fn client(&self) -> &Client {
        self.connection.client()
    }
}

impl<C: PgConn + 'static> Drop for PgSession<C> {
    fn drop(&mut self) {
        if !self.committed {
            tracing::warn!("PgSession dropped without commit/rollback");
        }
    }
}

// PgSession is Send+Sync when C is
unsafe impl<C: PgConn + Send + 'static> Send for PgSession<C> {}
unsafe impl<C: PgConn + Sync + 'static> Sync for PgSession<C> {}

#[derive(Clone, Debug)]
pub struct PostgresPersist<A, P = SharedClient>
where
    A: Aggregate,
    P: PgPool,
{
    _phantom: std::marker::PhantomData<A>,
    pool: P,
    snapshot_table_name: String,
    journal_table_name: String,
}

impl<A> PostgresPersist<A, SharedClient>
where
    A: Aggregate,
{
    /// Backward-compatible constructor wrapping a single `Arc<Client>`.
    #[must_use]
    pub fn new(client: Arc<Client>) -> Self {
        Self::from_client(client)
    }

    /// Backward-compatible constructor wrapping a single `Arc<Client>`.
    #[must_use]
    pub fn from_client(client: Arc<Client>) -> Self {
        Self::with_pool(SharedClient(client))
    }
}

impl<A, P> PostgresPersist<A, P>
where
    A: Aggregate,
    P: PgPool,
{
    #[must_use]
    pub fn with_pool(pool: P) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            pool,
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

    /// Returns the DDL statements to create the journal and snapshot tables,
    /// including a `UNIQUE(aggregate_id, version)` constraint on the journal.
    pub fn schema() -> String {
        let snapshot_table = format!("{}_snapshots", A::TYPE);
        let journal_table = format!("{}_journal", A::TYPE);
        format!(
            r#"CREATE TABLE IF NOT EXISTS {snapshot_table} (
    aggregate_id TEXT PRIMARY KEY,
    data JSONB NOT NULL,
    version BIGINT NOT NULL
);
CREATE TABLE IF NOT EXISTS {journal_table} (
    event_id TEXT PRIMARY KEY,
    aggregate_id TEXT NOT NULL,
    version BIGINT NOT NULL,
    payload JSONB NOT NULL,
    metadata JSONB NOT NULL,
    at TIMESTAMPTZ NOT NULL,
    UNIQUE(aggregate_id, version)
);
CREATE INDEX IF NOT EXISTS idx_{journal_table}_agg_ver ON {journal_table}(aggregate_id, version);"#
        )
    }
}

#[async_trait::async_trait]
impl<A, P> EventStoreStorage<A> for PostgresPersist<A, P>
where
    A: Aggregate + 'static,
    P: PgPool,
{
    type Session = PgSession<P::Connection>;

    async fn start_session(&self) -> Result<Self::Session, CqrsError> {
        let connection = self.pool.acquire().await?;
        connection
            .client()
            .batch_execute("BEGIN")
            .await
            .map_err(map_pg_error)?;
        Ok(PgSession {
            connection,
            committed: false,
        })
    }

    async fn close_session(&self, mut session: Self::Session) -> Result<(), CqrsError> {
        session
            .client()
            .batch_execute("COMMIT")
            .await
            .map_err(map_pg_error)?;
        session.committed = true;
        Ok(())
    }

    async fn abort_session(&self, mut session: Self::Session) -> Result<(), CqrsError> {
        session
            .client()
            .batch_execute("ROLLBACK")
            .await
            .map_err(map_pg_error)?;
        session.committed = true;
        Ok(())
    }

    async fn fetch_snapshot(&self, aggregate_id: &str) -> Result<Option<Snapshot<A>>, CqrsError> {
        let conn = self.pool.acquire().await?;
        let sql = format!(
            "SELECT data, version FROM {} WHERE aggregate_id = $1",
            self.snapshot_table_name
        );
        let row_opt = conn
            .client()
            .query_opt(&sql, &[&aggregate_id])
            .await
            .map_err(map_pg_error)?;
        if let Some(row) = row_opt {
            let data: JsonValue = row.try_get("data").map_err(map_pg_error)?;
            let version: i64 = row.try_get("version").map_err(map_pg_error)?;
            let state: A =
                serde_json::from_value(data).map_err(CqrsError::serialization_error)?;
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
        let conn = self.pool.acquire().await?;
        let sql = format!(
            "SELECT event_id, aggregate_id, version, payload, metadata, at FROM {} WHERE aggregate_id = $1 AND version > $2 ORDER BY version ASC",
            self.journal_table_name
        );
        let rows = conn
            .client()
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
                        .map_err(CqrsError::serialization_error)?,
                    metadata: serde_json::from_value(metadata)
                        .map_err(CqrsError::serialization_error)?,
                    at: row.try_get("at").map_err(map_pg_error)?,
                })
            })
            .collect();

        let events = events?;
        Ok(Box::pin(stream::iter(events.into_iter().map(Ok))))
    }

    async fn fetch_all_events(&self, aggregate_id: &str) -> Result<EventStream<A>, CqrsError> {
        let conn = self.pool.acquire().await?;
        let sql = format!(
            "SELECT event_id, aggregate_id, version, payload, metadata, at FROM {} WHERE aggregate_id = $1 ORDER BY version ASC",
            self.journal_table_name
        );
        let rows = conn
            .client()
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
                        .map_err(CqrsError::serialization_error)?,
                    metadata: serde_json::from_value(metadata)
                        .map_err(CqrsError::serialization_error)?,
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
        let conn = self.pool.acquire().await?;
        // Get total count
        let count_sql = format!(
            "SELECT COUNT(*) FROM {} WHERE aggregate_id = $1",
            self.journal_table_name
        );
        let count_row = conn
            .client()
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
        let rows = conn
            .client()
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
                        .map_err(CqrsError::serialization_error)?,
                    metadata: serde_json::from_value(metadata)
                        .map_err(CqrsError::serialization_error)?,
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
        session: &Self::Session,
    ) -> Result<Option<EventEnvelope<A>>, CqrsError> {
        let sql = format!(
            "SELECT event_id, aggregate_id, version, payload, metadata, at FROM {} WHERE aggregate_id = $1 ORDER BY version DESC LIMIT 1 FOR UPDATE",
            self.journal_table_name
        );
        let row_opt = session
            .client()
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
                    .map_err(CqrsError::serialization_error)?,
                metadata: serde_json::from_value(metadata)
                    .map_err(CqrsError::serialization_error)?,
                at: row.try_get("at").map_err(map_pg_error)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn save_events(
        &self,
        events: Vec<EventEnvelope<A>>,
        session: &mut Self::Session,
    ) -> Result<(), CqrsError> {
        if events.is_empty() {
            return Ok(());
        }
        let sql = format!(
            "INSERT INTO {} (event_id, aggregate_id, version, payload, metadata, at) VALUES ($1,$2,$3,$4,$5,$6)",
            self.journal_table_name
        );
        for e in events.iter() {
            let payload = serde_json::to_value(&e.payload)
                .map_err(CqrsError::serialization_error)?;
            let metadata = serde_json::to_value(&e.metadata)
                .map_err(CqrsError::serialization_error)?;
            session
                .client()
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
        Ok(())
    }

    async fn save_snapshot(
        &self,
        aggregate: &A,
        version: usize,
        session: &mut Self::Session,
    ) -> Result<(), CqrsError> {
        let data =
            serde_json::to_value(aggregate).map_err(CqrsError::serialization_error)?;
        let sql = format!(
            "INSERT INTO {} (aggregate_id, data, version) VALUES ($1, $2, $3) \
             ON CONFLICT (aggregate_id) DO UPDATE SET data = EXCLUDED.data, version = EXCLUDED.version",
            self.snapshot_table_name
        );
        session
            .client()
            .execute(&sql, &[&aggregate.aggregate_id(), &data, &(version as i64)])
            .await
            .map_err(map_pg_error)?;
        Ok(())
    }
}
