mod audit_log_router;
mod helpers;
mod read_router;

use axum::response::{IntoResponse, Response};
use axum::Json;
pub use audit_log_router::*;
pub use read_router::*;
mod write_router;
use crate::CqrsError;
pub use write_router::*;

impl IntoResponse for CqrsError {
    fn into_response(self) -> Response {
        let status = self.http_status();
        (status, Json(self)).into_response()
    }
}
