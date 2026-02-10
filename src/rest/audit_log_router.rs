use crate::event::Event;
use crate::read::Paged;
use crate::{Aggregate, CqrsContext, DynEventStore, EventEnvelope};
use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::openapi::path::ParameterIn;
use utoipa::openapi::{HttpMethod, Ref, RefOr};
use utoipa::{IntoParams, PartialSchema, ToSchema};
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouter};

use crate::rest::helpers;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogEntry {
    pub id: String,
    pub aggregate_id: String,
    pub version: usize,
    pub event_type: String,
    pub metadata: HashMap<String, String>,
    #[schema(value_type = String)]
    pub at: DateTime<Utc>,
}

impl<A: Aggregate> From<EventEnvelope<A>> for AuditLogEntry {
    fn from(envelope: EventEnvelope<A>) -> Self {
        AuditLogEntry {
            id: envelope.event_id,
            aggregate_id: envelope.aggregate_id,
            version: envelope.version,
            event_type: envelope.payload.event_type(),
            metadata: envelope.metadata,
            at: envelope.at,
        }
    }
}

#[derive(Clone, Debug, Deserialize, IntoParams)]
pub struct AuditLogQuery {
    #[serde(default)]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

fn default_page_size() -> usize {
    10
}

#[derive(Clone)]
pub struct CQRSAuditLogRouter<A>
where
    A: Aggregate + 'static,
{
    _phantom: std::marker::PhantomData<A>,
    store: DynEventStore<A>,
}

impl<A> CQRSAuditLogRouter<A>
where
    A: Aggregate + 'static,
{
    #[must_use]
    fn new(store: DynEventStore<A>) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            store,
        }
    }

    fn path_aggregate_id_field() -> String {
        format!("{}_id", A::TYPE)
    }

    fn audit_log_route(
        router: OpenApiRouter<CQRSAuditLogRouter<A>>,
        tag: &str,
    ) -> OpenApiRouter<CQRSAuditLogRouter<A>> {
        let path = format!("/{{{}}}/audit", Self::path_aggregate_id_field());
        let response_schema_name = format!("Paged_{}_AuditLog", A::TYPE);
        let schemas = vec![(
            response_schema_name.to_string(),
            Paged::<AuditLogEntry>::schema(),
        )];

        let paths = helpers::generate_route(
            tag,
            HttpMethod::Get,
            &path,
            RefOr::Ref(Ref::from_schema_name(response_schema_name)),
            vec![(Self::path_aggregate_id_field(), String::schema())],
            AuditLogQuery::into_params(|| Some(ParameterIn::Query)),
            None,
        );

        let handler = get(
            move |State(router): State<CQRSAuditLogRouter<A>>,
                  Path(aggregate_id): Path<String>,
                  Query(query): Query<AuditLogQuery>,
                  Extension(_context): Extension<CqrsContext>| async move {
                Self::get_audit_log(router, aggregate_id, query).await
            },
        );

        router.routes(UtoipaMethodRouter::<CQRSAuditLogRouter<A>>::from((
            schemas, paths, handler,
        )))
    }

    pub fn routes(store: DynEventStore<A>, tag: &'static str) -> OpenApiRouter {
        let state = Self::new(store);
        let mut result = OpenApiRouter::<CQRSAuditLogRouter<A>>::new();
        result = Self::audit_log_route(result, tag);
        result.with_state(state)
    }

    async fn get_audit_log(
        router: CQRSAuditLogRouter<A>,
        aggregate_id: String,
        query: AuditLogQuery,
    ) -> impl IntoResponse {
        match router
            .store
            .load_events_paged(&aggregate_id, query.page, query.page_size)
            .await
        {
            Ok((events, total)) => {
                let items: Vec<AuditLogEntry> =
                    events.into_iter().map(AuditLogEntry::from).collect();
                let response = Paged {
                    items,
                    total,
                    page: query.page as i64,
                    page_size: query.page_size as i64,
                };
                (StatusCode::OK, Json(response)).into_response()
            }
            Err(err) => err.into_response(),
        }
    }
}
