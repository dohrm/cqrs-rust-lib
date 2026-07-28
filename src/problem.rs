//! RFC 9457 "Problem Details for HTTP APIs".
//!
//! [`ProblemDetails`] is the wire format for [`CqrsError`] when the
//! `problem-json` feature is enabled: responses are served as
//! `application/problem+json` with the standard members (`type`, `title`,
//! `status`, `detail`, `instance`) plus the CQRS-specific extension members
//! (`domain`, `code`, `internalCode`, `details`, `requestId`).
//!
//! ```json
//! {
//!   "type": "urn:cqrs-error:account:ACCOUNT_INSUFFICIENT_FUNDS",
//!   "title": "ACCOUNT_INSUFFICIENT_FUNDS",
//!   "status": 400,
//!   "detail": "Cannot withdraw 500, balance is 200",
//!   "instance": "urn:cqrs-request:req-123",
//!   "domain": "account",
//!   "code": "ACCOUNT_INSUFFICIENT_FUNDS",
//!   "internalCode": 10001,
//!   "requestId": "req-123"
//! }
//! ```
//!
//! The conversion is always available, even without the feature, so an
//! application can render problem documents on its own routes.

use crate::errors::CqrsError;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

/// Media type of a problem document (RFC 9457 §3).
pub const PROBLEM_JSON: &str = "application/problem+json";

static TYPE_BASE_URI: OnceLock<String> = OnceLock::new();

/// Sets the base URI used to build the `type` member of problem documents:
/// the resulting URI is `{base}/{code}` (e.g.
/// `https://api.example.com/errors/ACCOUNT_INSUFFICIENT_FUNDS`).
///
/// Call once at startup, before serving requests. Returns `Err` with the
/// already-configured base if it was set before. Without a base, and unless the
/// error carries its own URI via [`CqrsError::with_type_uri`], `type` falls back
/// to `urn:cqrs-error:{domain}:{code}`.
pub fn set_problem_type_base_uri(base: impl Into<String>) -> Result<(), &'static str> {
    TYPE_BASE_URI
        .set(base.into().trim_end_matches('/').to_string())
        .map_err(|_| "problem type base URI is already set")
}

/// Returns the configured base URI, if any.
#[must_use]
pub fn problem_type_base_uri() -> Option<&'static str> {
    TYPE_BASE_URI.get().map(String::as_str)
}

/// An RFC 9457 problem document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetails {
    /// URI identifying the problem type.
    #[serde(rename = "type")]
    pub type_uri: String,

    /// Short, human-readable summary of the problem type. Stable per type.
    pub title: String,

    /// HTTP status code of the response.
    pub status: u16,

    /// Human-readable explanation specific to this occurrence.
    pub detail: String,

    /// URI identifying this specific occurrence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,

    // ── extension members ────────────────────────────────────────────────────
    /// Domain the error originated from (e.g. `"account"`).
    pub domain: String,

    /// Error code as string (e.g. `"ACCOUNT_INSUFFICIENT_FUNDS"`).
    pub code: String,

    /// Internal code for support/debugging (e.g. `10001`).
    pub internal_code: u16,

    /// Additional context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,

    /// Request ID for tracing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl From<&CqrsError> for ProblemDetails {
    fn from(err: &CqrsError) -> Self {
        // An empty request id carries no information — treat it as absent.
        let request_id = err.request_id.clone().filter(|id| !id.is_empty());

        Self {
            type_uri: problem_type_uri(err),
            title: err.code.clone(),
            status: err.http_status().as_u16(),
            detail: err.message.clone(),
            instance: request_id
                .as_ref()
                .map(|id| format!("urn:cqrs-request:{id}")),
            domain: err.domain.clone(),
            code: err.code.clone(),
            internal_code: err.internal_code,
            details: err.details.clone(),
            request_id,
        }
    }
}

impl From<CqrsError> for ProblemDetails {
    fn from(err: CqrsError) -> Self {
        Self::from(&err)
    }
}

fn problem_type_uri(err: &CqrsError) -> String {
    if let Some(uri) = err.type_uri.as_deref() {
        return uri.to_string();
    }
    match problem_type_base_uri() {
        Some(base) => format!("{base}/{}", err.code),
        None => format!("urn:cqrs-error:{}:{}", err.domain, err.code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::GenericErrorCode;
    use crate::CqrsErrorCode;

    #[test]
    fn maps_every_rfc_member() {
        let err = GenericErrorCode::NotFound
            .error("User 'abc' not found")
            .with_details(serde_json::json!({ "id": "abc" }))
            .with_request_id("req-123");
        let problem = ProblemDetails::from(&err);

        assert_eq!(problem.type_uri, "urn:cqrs-error:generic:GENERIC_NOT_FOUND");
        assert_eq!(problem.title, "GENERIC_NOT_FOUND");
        assert_eq!(problem.status, 404);
        assert_eq!(problem.detail, "User 'abc' not found");
        assert_eq!(
            problem.instance.as_deref(),
            Some("urn:cqrs-request:req-123")
        );
        assert_eq!(problem.domain, "generic");
        assert_eq!(problem.internal_code, 1002);
        assert_eq!(problem.details.unwrap()["id"], "abc");
        assert_eq!(problem.request_id.as_deref(), Some("req-123"));
    }

    #[test]
    fn omits_instance_without_request_id() {
        let problem = ProblemDetails::from(&CqrsError::conflict("boom"));
        assert!(problem.instance.is_none());
        assert!(problem.request_id.is_none());
    }

    #[test]
    fn empty_request_id_is_not_reported() {
        let err = CqrsError::conflict("boom").with_request_id("");
        let problem = ProblemDetails::from(&err);
        assert!(problem.instance.is_none());
        assert!(problem.request_id.is_none());
    }

    #[test]
    fn per_error_type_uri_wins() {
        let err = CqrsError::validation("nope").with_type_uri("https://errors.example.com/nope");
        assert_eq!(
            ProblemDetails::from(&err).type_uri,
            "https://errors.example.com/nope"
        );
    }

    #[test]
    fn serializes_with_rfc_member_names() {
        let err = CqrsError::from_status(http::StatusCode::TOO_MANY_REQUESTS, "slow down");
        let json = serde_json::to_value(ProblemDetails::from(&err)).unwrap();

        assert_eq!(
            json["type"],
            "urn:cqrs-error:generic:GENERIC_TOO_MANY_REQUESTS"
        );
        assert_eq!(json["status"], 429);
        assert_eq!(json["detail"], "slow down");
        assert_eq!(json["internalCode"], 1429);
        assert!(json.get("message").is_none());
        assert!(json.get("instance").is_none());
    }
}
