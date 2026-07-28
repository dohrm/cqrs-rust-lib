mod audit_log_router;
pub mod codex;
mod codex_router;
mod helpers;
mod read_router;

use axum::response::{IntoResponse, Response};
pub use audit_log_router::*;
pub use codex::CqrsHttpQuery;
pub use codex_router::CQRSCodexReadRouter;
pub use read_router::*;
mod write_router;
use crate::CqrsError;
pub use write_router::*;

/// With the `problem-json` feature: RFC 9457 document served as
/// `application/problem+json`. Without it: the legacy `CqrsError` body served as
/// `application/json`.
impl IntoResponse for CqrsError {
    fn into_response(self) -> Response {
        let status = self.http_status();

        #[cfg(feature = "problem-json")]
        {
            use crate::problem::PROBLEM_JSON;
            use http::header::CONTENT_TYPE;
            use http::HeaderValue;

            let body = match serde_json::to_vec(&self.to_problem()) {
                Ok(body) => body,
                // Serializing a ProblemDetails cannot fail for the fields we
                // build, but never panic on a response path.
                Err(e) => {
                    tracing::error!(error = %e, "failed to serialize problem document");
                    return (status, axum::Json(self)).into_response();
                }
            };
            let mut response = (status, body).into_response();
            response
                .headers_mut()
                .insert(CONTENT_TYPE, HeaderValue::from_static(PROBLEM_JSON));
            response
        }

        #[cfg(not(feature = "problem-json"))]
        {
            (status, axum::Json(self)).into_response()
        }
    }
}

/// Media type of the error body produced by [`CqrsError::into_response`].
pub(crate) const ERROR_MEDIA_TYPE: &str = if cfg!(feature = "problem-json") {
    crate::problem::PROBLEM_JSON
} else {
    "application/json"
};

#[cfg(test)]
mod tests {
    use super::*;
    use http::header::CONTENT_TYPE;

    #[tokio::test]
    async fn error_response_uses_the_expected_media_type_and_status() {
        let response = CqrsError::from_status(http::StatusCode::TOO_MANY_REQUESTS, "slow down")
            .with_request_id("req-1")
            .into_response();

        assert_eq!(response.status(), 429);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            ERROR_MEDIA_TYPE,
        );

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(json["code"], "GENERIC_TOO_MANY_REQUESTS");
        assert_eq!(json["internalCode"], 1429);

        if cfg!(feature = "problem-json") {
            assert_eq!(
                json["type"],
                "urn:cqrs-error:generic:GENERIC_TOO_MANY_REQUESTS"
            );
            assert_eq!(json["status"], 429);
            assert_eq!(json["detail"], "slow down");
            assert_eq!(json["instance"], "urn:cqrs-request:req-1");
        } else {
            assert_eq!(json["message"], "slow down");
            // The legacy body never carried the status.
            assert!(json.get("status").is_none());
        }
    }
}
