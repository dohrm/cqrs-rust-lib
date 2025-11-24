mod audit_log_router;
mod helpers;
mod read_router;

use axum::response::{IntoResponse, Response};
use axum::Json;
use http::StatusCode;
pub use audit_log_router::*;
pub use read_router::*;
use serde_json::json;
mod write_router;
use crate::AggregateError;
pub use write_router::*;

impl IntoResponse for AggregateError {
    fn into_response(self) -> Response {
        match self {
            AggregateError::UserError(e) => (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": e.to_string()})),
            )
                .into_response(),
            AggregateError::Conflict => {
                (StatusCode::CONFLICT, Json(json!({"error": "conflict"}))).into_response()
            }
            AggregateError::DatabaseError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "database connection error"})),
            )
                .into_response(),
            AggregateError::SerializationError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "serialization error"})),
            )
                .into_response(),
            AggregateError::UnexpectedError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "unexpected error"})),
            )
                .into_response(),
        }
    }
}
