use crate::errors::CqrsError;
use crate::es::storage::{EventStoreStorage, EventStream};
use crate::snapshot::Snapshot;
use crate::{Aggregate, EventEnvelope};
use futures::stream;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fmt::Debug;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use surrealdb_types::{Datetime, RecordId, SurrealValue};

fn map_surreal_error(e: surrealdb::Error) -> CqrsError {
    CqrsError::database_error(e)
}

fn is_concurrency_error(e: &surrealdb::Error) -> bool {
    // surrealdb-core 3.x maps IndexExists (unique-index violation) to TypesError::internal,
    // not AlreadyExists — no typed variant is available. We match the formatted message from
    // the #[error] template: "Database index `{index}` already contains {value}, with record …"
    // Verify on surrealdb-core upgrades: src/err/to_types.rs, variant IndexExists.
    e.message().contains("already contains")
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct JournalInsert {
    event_id: String,
    aggregate_id: String,
    version: i64,
    payload: JsonValue,
    metadata: JsonValue,
    at: Datetime,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct JournalRow {
    #[allow(dead_code)]
    id: Option<RecordId>,
    event_id: String,
    aggregate_id: String,
    version: i64,
    payload: JsonValue,
    metadata: JsonValue,
    at: Datetime,
}

#[derive(Debug, Serialize, Deserialize, SurrealValue)]
struct SnapshotRow {
    aggregate_id: String,
    data: JsonValue,
    version: i64,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct CountRow {
    cnt: i64,
}

fn row_to_envelope<A: Aggregate>(row: JournalRow) -> Result<EventEnvelope<A>, CqrsError> {
    let payload: A::Event =
        serde_json::from_value(row.payload).map_err(CqrsError::serialization_error)?;
    let metadata: HashMap<String, String> =
        serde_json::from_value(row.metadata).map_err(CqrsError::serialization_error)?;
    Ok(EventEnvelope {
        event_id: row.event_id,
        aggregate_id: row.aggregate_id,
        version: row.version as usize,
        payload,
        metadata,
        at: row.at.into(),
    })
}

/// SurrealDB-backed event store for a given aggregate type.
///
/// Uses optimistic concurrency: a `UNIQUE(aggregate_id, version)` index on the
/// journal table prevents duplicate versions; any collision surfaces as
/// [`CqrsError::concurrency_error`].
///
/// Call [`SurrealDBPersist::schema`] to obtain the DDL that must be applied
/// once before first use.
#[derive(Clone, Debug)]
pub struct SurrealDBPersist<A>
where
    A: Aggregate,
{
    _phantom: std::marker::PhantomData<A>,
    db: Surreal<Any>,
    snapshot_table: String,
    journal_table: String,
}

impl<A> SurrealDBPersist<A>
where
    A: Aggregate,
{
    #[must_use]
    pub fn new(db: Surreal<Any>) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            db,
            snapshot_table: format!("{}_snapshots", A::TYPE),
            journal_table: format!("{}_journal", A::TYPE),
        }
    }

    pub fn snapshot_table(&self) -> &str {
        &self.snapshot_table
    }

    pub fn journal_table(&self) -> &str {
        &self.journal_table
    }

    /// Returns the SurrealQL DDL statements needed to set up tables and indexes.
    ///
    /// Run once during application startup or migrations:
    /// ```ignore
    /// db.query(SurrealDBPersist::<MyAggregate>::schema()).await?.check()?;
    /// ```
    pub fn schema() -> String {
        let snapshot_table = format!("{}_snapshots", A::TYPE);
        let journal_table = format!("{}_journal", A::TYPE);
        format!(
            r#"DEFINE TABLE IF NOT EXISTS {snapshot_table} SCHEMALESS;

DEFINE TABLE IF NOT EXISTS {journal_table} SCHEMALESS;
DEFINE INDEX IF NOT EXISTS idx_{journal_table}_agg_ver ON {journal_table} FIELDS aggregate_id, version UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_{journal_table}_agg ON {journal_table} FIELDS aggregate_id;"#
        )
    }
}

// ─── Implementation ───────────────────────────────────────────────────────────
cqrs_async_trait! {
impl<A> EventStoreStorage<A> for SurrealDBPersist<A>
where
    A: Aggregate + 'static,
{
    type Session = ();

    async fn start_session(&self) -> Result<Self::Session, CqrsError> {
        Ok(())
    }

    async fn close_session(&self, _session: Self::Session) -> Result<(), CqrsError> {
        Ok(())
    }

    async fn fetch_snapshot(&self, aggregate_id: &str) -> Result<Option<Snapshot<A>>, CqrsError> {
        let id = aggregate_id.to_string();
        let sql = format!(
            "SELECT aggregate_id, data, version FROM {} WHERE aggregate_id = $id LIMIT 1",
            self.snapshot_table
        );
        let mut result = self
            .db
            .query(sql)
            .bind(("id", id.clone()))
            .await
            .map_err(map_surreal_error)?;
        let rows: Vec<SnapshotRow> = result.take(0).map_err(map_surreal_error)?;
        match rows.into_iter().next() {
            Some(row) => {
                let state: A = serde_json::from_value(row.data)
                    .map_err(CqrsError::serialization_error)?;
                Ok(Some(Snapshot {
                    aggregate_id: id,
                    state,
                    version: row.version as usize,
                }))
            }
            None => Ok(None),
        }
    }

    async fn fetch_events_from_version(
        &self,
        aggregate_id: &str,
        version: usize,
    ) -> Result<EventStream<A>, CqrsError> {
        let id = aggregate_id.to_string();
        let sql = format!(
            "SELECT * FROM {} WHERE aggregate_id = $id AND version > $ver ORDER BY version ASC",
            self.journal_table
        );
        let mut result = self
            .db
            .query(sql)
            .bind(("id", id))
            .bind(("ver", version as i64))
            .await
            .map_err(map_surreal_error)?;
        let rows: Vec<JournalRow> = result.take(0).map_err(map_surreal_error)?;
        let envelopes: Result<Vec<_>, _> = rows.into_iter().map(row_to_envelope).collect();
        Ok(Box::pin(stream::iter(envelopes?.into_iter().map(Ok))))
    }

    async fn fetch_all_events(&self, aggregate_id: &str) -> Result<EventStream<A>, CqrsError> {
        let id = aggregate_id.to_string();
        let sql = format!(
            "SELECT * FROM {} WHERE aggregate_id = $id ORDER BY version ASC",
            self.journal_table
        );
        let mut result = self
            .db
            .query(sql)
            .bind(("id", id))
            .await
            .map_err(map_surreal_error)?;
        let rows: Vec<JournalRow> = result.take(0).map_err(map_surreal_error)?;
        let envelopes: Result<Vec<_>, _> = rows.into_iter().map(row_to_envelope).collect();
        Ok(Box::pin(stream::iter(envelopes?.into_iter().map(Ok))))
    }

    async fn fetch_events_paged(
        &self,
        aggregate_id: &str,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<EventEnvelope<A>>, i64), CqrsError> {
        let id = aggregate_id.to_string();
        let count_sql = format!(
            "SELECT count() AS cnt FROM {} WHERE aggregate_id = $id GROUP ALL",
            self.journal_table
        );
        let mut r = self
            .db
            .query(count_sql)
            .bind(("id", id.clone()))
            .await
            .map_err(map_surreal_error)?;
        let counts: Vec<CountRow> = r.take(0).map_err(map_surreal_error)?;
        let total = counts.first().map(|c| c.cnt).unwrap_or(0);

        let offset = (page.max(1) - 1) * page_size;
        let sql = format!(
            "SELECT * FROM {} WHERE aggregate_id = $id ORDER BY version ASC LIMIT $limit START $offset",
            self.journal_table
        );
        let mut result = self
            .db
            .query(sql)
            .bind(("id", id))
            .bind(("limit", page_size as i64))
            .bind(("offset", offset as i64))
            .await
            .map_err(map_surreal_error)?;
        let rows: Vec<JournalRow> = result.take(0).map_err(map_surreal_error)?;
        let envelopes: Result<Vec<_>, _> = rows.into_iter().map(row_to_envelope).collect();
        Ok((envelopes?, total))
    }

    async fn fetch_latest_event(
        &self,
        aggregate: &A,
        _session: &Self::Session,
    ) -> Result<Option<EventEnvelope<A>>, CqrsError> {
        let id = aggregate.aggregate_id();
        let sql = format!(
            "SELECT * FROM {} WHERE aggregate_id = $id ORDER BY version DESC LIMIT 1",
            self.journal_table
        );
        let mut result = self
            .db
            .query(sql)
            .bind(("id", id))
            .await
            .map_err(map_surreal_error)?;
        let rows: Vec<JournalRow> = result.take(0).map_err(map_surreal_error)?;
        match rows.into_iter().next() {
            Some(row) => Ok(Some(row_to_envelope(row)?)),
            None => Ok(None),
        }
    }

    async fn save_events(
        &self,
        events: Vec<EventEnvelope<A>>,
        _session: &mut Self::Session,
    ) -> Result<(), CqrsError> {
        if events.is_empty() {
            return Ok(());
        }
        let inserts: Vec<JournalInsert> = events
            .iter()
            .map(|e| {
                let payload =
                    serde_json::to_value(&e.payload).map_err(CqrsError::serialization_error)?;
                let metadata =
                    serde_json::to_value(&e.metadata).map_err(CqrsError::serialization_error)?;
                Ok(JournalInsert {
                    event_id: e.event_id.clone(),
                    aggregate_id: e.aggregate_id.clone(),
                    version: e.version as i64,
                    payload,
                    metadata,
                    at: e.at.into(),
                })
            })
            .collect::<Result<_, CqrsError>>()?;

        let sql = format!("INSERT INTO {} $events", self.journal_table);
        self.db
            .query(sql)
            .bind(("events", inserts))
            .await
            .map_err(map_surreal_error)?
            .check()
            .map_err(|e| {
                if is_concurrency_error(&e) {
                    CqrsError::concurrency_error()
                } else {
                    CqrsError::database_error(e)
                }
            })?;
        Ok(())
    }

    async fn save_snapshot(
        &self,
        aggregate: &A,
        version: usize,
        _session: &mut Self::Session,
    ) -> Result<(), CqrsError> {
        let data = serde_json::to_value(aggregate).map_err(CqrsError::serialization_error)?;
        let id = aggregate.aggregate_id();
        let table = self.snapshot_table.clone();
        // Use UPSERT with a deterministic record ID derived from aggregate_id.
        // type::record($table, $id) constructs a RecordId that serves as the primary key,
        // giving us atomic create-or-replace semantics without a separate index.
        self.db
            .query(
                "UPSERT type::record($table, $id) SET aggregate_id = $id, data = $data, version = $ver",
            )
            .bind(("table", table))
            .bind(("id", id))
            .bind(("data", data))
            .bind(("ver", version as i64))
            .await
            .map_err(map_surreal_error)?
            .check()
            .map_err(map_surreal_error)?;
        Ok(())
    }
}
}

// ─── Tests ───────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::es::storage::EventStoreStorage;
    use crate::testing::{TestAggregate, TestEvent};
    use crate::EventEnvelope;
    use chrono::Utc;
    use futures::StreamExt;
    use std::collections::HashMap;
    use surrealdb::engine::any::connect;

    async fn setup() -> SurrealDBPersist<TestAggregate> {
        let db = connect("mem://").await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db.query(SurrealDBPersist::<TestAggregate>::schema())
            .await
            .unwrap()
            .check()
            .unwrap();
        SurrealDBPersist::new(db)
    }

    fn envelope(
        aggregate_id: &str,
        version: usize,
        event: TestEvent,
    ) -> EventEnvelope<TestAggregate> {
        EventEnvelope {
            event_id: format!("{}-v{}", aggregate_id, version),
            aggregate_id: aggregate_id.to_string(),
            version,
            payload: event,
            metadata: HashMap::new(),
            at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn save_and_fetch_all_events() {
        let p = setup().await;
        p.save_events(
            vec![
                envelope("a1", 1, TestEvent::Created { name: "foo".into() }),
                envelope("a1", 2, TestEvent::Incremented),
            ],
            &mut (),
        )
        .await
        .unwrap();

        let stream = p.fetch_all_events("a1").await.unwrap();
        let rows: Vec<_> = stream.map(|r| r.unwrap()).collect().await;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].version, 1);
        assert_eq!(rows[1].version, 2);
        assert!(matches!(&rows[0].payload, TestEvent::Created { name } if name == "foo"));
    }

    #[tokio::test]
    async fn fetch_events_from_version_skips_earlier() {
        let p = setup().await;
        p.save_events(
            vec![
                envelope("a1", 1, TestEvent::Created { name: "x".into() }),
                envelope("a1", 2, TestEvent::Incremented),
                envelope("a1", 3, TestEvent::Decremented),
            ],
            &mut (),
        )
        .await
        .unwrap();

        let stream = p.fetch_events_from_version("a1", 1).await.unwrap();
        let rows: Vec<_> = stream.map(|r| r.unwrap()).collect().await;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].version, 2);
        assert_eq!(rows[1].version, 3);
    }

    #[tokio::test]
    async fn fetch_events_paged_returns_correct_page() {
        let p = setup().await;
        p.save_events(
            (1..=5)
                .map(|v| envelope("a1", v, TestEvent::Incremented))
                .collect(),
            &mut (),
        )
        .await
        .unwrap();

        let (page1, total) = p.fetch_events_paged("a1", 1, 2).await.unwrap();
        assert_eq!(total, 5);
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].version, 1);
        assert_eq!(page1[1].version, 2);

        let (page2, _) = p.fetch_events_paged("a1", 2, 2).await.unwrap();
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].version, 3);
    }

    #[tokio::test]
    async fn fetch_latest_event_returns_highest_version() {
        let p = setup().await;
        p.save_events(
            vec![
                envelope("a1", 1, TestEvent::Created { name: "x".into() }),
                envelope("a1", 2, TestEvent::Incremented),
                envelope("a1", 3, TestEvent::Decremented),
            ],
            &mut (),
        )
        .await
        .unwrap();

        let agg = TestAggregate::default().with_aggregate_id("a1".to_string());
        let latest = p.fetch_latest_event(&agg, &()).await.unwrap();
        assert_eq!(latest.unwrap().version, 3);
    }

    #[tokio::test]
    async fn fetch_latest_event_none_when_empty() {
        let p = setup().await;
        let agg = TestAggregate::default().with_aggregate_id("missing".to_string());
        let latest = p.fetch_latest_event(&agg, &()).await.unwrap();
        assert!(latest.is_none());
    }

    #[tokio::test]
    async fn save_and_fetch_snapshot() {
        let p = setup().await;
        assert!(p.fetch_snapshot("a1").await.unwrap().is_none());

        let mut agg = TestAggregate::default().with_aggregate_id("a1".to_string());
        agg.apply(TestEvent::Created { name: "bar".into() })
            .unwrap();
        p.save_snapshot(&agg, 3, &mut ()).await.unwrap();

        let snap = p.fetch_snapshot("a1").await.unwrap().unwrap();
        assert_eq!(snap.aggregate_id, "a1");
        assert_eq!(snap.version, 3);
        assert_eq!(snap.state.aggregate_id(), "a1");
    }

    #[tokio::test]
    async fn snapshot_upsert_replaces_previous() {
        let p = setup().await;
        let agg = TestAggregate::default().with_aggregate_id("a1".to_string());
        p.save_snapshot(&agg, 1, &mut ()).await.unwrap();
        p.save_snapshot(&agg, 5, &mut ()).await.unwrap();

        let snap = p.fetch_snapshot("a1").await.unwrap().unwrap();
        assert_eq!(snap.version, 5);
    }

    #[tokio::test]
    async fn duplicate_version_is_concurrency_error() {
        let p = setup().await;
        p.save_events(vec![envelope("a1", 1, TestEvent::Incremented)], &mut ())
            .await
            .unwrap();

        let result = p
            .save_events(vec![envelope("a1", 1, TestEvent::Decremented)], &mut ())
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        // ConcurrencyError has HTTP status 409
        assert_eq!(err.status, 409, "expected concurrency error, got: {err}");
    }

    #[tokio::test]
    async fn events_from_different_aggregates_are_isolated() {
        let p = setup().await;
        p.save_events(
            vec![
                envelope("a1", 1, TestEvent::Incremented),
                envelope("a2", 1, TestEvent::Decremented),
            ],
            &mut (),
        )
        .await
        .unwrap();

        let stream = p.fetch_all_events("a1").await.unwrap();
        let rows: Vec<_> = stream.map(|r| r.unwrap()).collect().await;
        assert_eq!(rows.len(), 1);
        assert!(matches!(rows[0].payload, TestEvent::Incremented));
    }
}
