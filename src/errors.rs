//! Unified Error Handling for CQRS
//!
//! This module provides a structured error system where:
//! - Each domain defines its own error codes via the `CqrsErrorCode` trait
//! - All errors are serialized to a unified `CqrsError` format for API responses
//! - Technical/infrastructure errors are mapped to a dedicated prefix
//!
//! # Domain Prefixes
//!
//!
//! Internal codes are formatted as: `prefix * 1000 + error_index`
//! Example: Tenant NotFound = 4001

use http::StatusCode;

use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use thiserror::Error;

#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

/// Trait that all domain error codes must implement.
///
/// Each domain defines its own enum implementing this trait.
/// The trait provides the contract for error code metadata.
///
/// # Example
///
/// ```rust,ignore
/// use cqrs_rust_lib::define_domain_errors;
///
/// define_domain_errors! {
///     domain: "plan",
///     prefix: 4,
///     errors: {
///         NotFound => (1, StatusCode::NOT_FOUND, "NOT_FOUND"),
///         SlugExists => (2, StatusCode::CONFLICT, "SLUG_EXISTS"),
///     }
/// }
/// ```
pub trait CqrsErrorCode: Debug + Display + Clone + Send + Sync + 'static {
    /// The domain this error belongs to (e.g., "tenant", "license")
    fn domain() -> &'static str;

    /// Domain prefix for internal codes (0-9)
    /// Each domain gets a unique prefix.
    fn domain_prefix() -> u16;

    /// Unique error index within the domain (0-999)
    fn error_index(&self) -> u16;

    /// HTTP status code for this error
    fn http_status(&self) -> StatusCode;

    /// Full internal code: domain_prefix * 1000 + error_index
    /// Example: Tenant (4) + NotFound (1) = 4001
    fn internal_code(&self) -> u16 {
        Self::domain_prefix() * 1000 + self.error_index()
    }

    /// String representation of the error code for JSON serialization
    /// Format: DOMAIN_ERROR_NAME (e.g., "PLAN_NOT_FOUND")
    fn code_string(&self) -> String {
        format!("{}_{}", Self::domain().to_uppercase(), self)
    }

    /// Create a CqrsError from this code with a message
    fn error(&self, message: impl Into<String>) -> CqrsError
    where
        Self: Sized,
    {
        CqrsError::from_code(self, message)
    }
}

// ============================================
// CqrsError - Unified Error Struct
// ============================================

/// Internal data for `CqrsError`. Access fields via `Deref` on `CqrsError`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CqrsErrorData {
    /// Domain this error originated from (e.g., "plan", "user")
    pub domain: String,

    /// Error code as string (e.g., "PLAN_NOT_FOUND")
    pub code: String,

    /// Unique internal code for support/debugging (e.g., 4001)
    pub internal_code: u16,

    /// HTTP status code (not serialized, used for response)
    #[serde(skip)]
    pub status: u16,

    /// Human-readable error message
    pub message: String,

    /// Additional context (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,

    /// Request ID for tracing (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Unified error for API responses.
///
/// This is a thin wrapper around `Box<CqrsErrorData>` to keep `Result<_, CqrsError>`
/// small on the stack. Access fields via `Deref` (e.g. `err.domain`, `err.code`).
///
/// # JSON Format
///
/// ```json
/// {
///   "domain": "plan",
///   "code": "PLAN_NOT_FOUND",
///   "internalCode": 4001,
///   "message": "Tenant with ID 'abc' not found",
///   "details": { "id": "abc" },
///   "requestId": "req-123"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CqrsError(Box<CqrsErrorData>);

impl std::ops::Deref for CqrsError {
    type Target = CqrsErrorData;
    fn deref(&self) -> &CqrsErrorData {
        &self.0
    }
}

impl std::ops::DerefMut for CqrsError {
    fn deref_mut(&mut self) -> &mut CqrsErrorData {
        &mut self.0
    }
}

#[cfg(feature = "utoipa")]
impl utoipa::PartialSchema for CqrsError {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        CqrsErrorData::schema()
    }
}

#[cfg(feature = "utoipa")]
impl utoipa::ToSchema for CqrsError {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("CqrsError")
    }
}

impl CqrsError {
    /// Create a CqrsError from any domain error code.
    pub fn from_code<C: CqrsErrorCode>(code: &C, message: impl Into<String>) -> Self {
        Self(Box::new(CqrsErrorData {
            domain: C::domain().to_string(),
            code: code.code_string(),
            internal_code: code.internal_code(),
            status: code.http_status().as_u16(),
            message: message.into(),
            details: None,
            request_id: None,
        }))
    }

    /// Add additional details to the error.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Add a request ID for tracing.
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Get the HTTP status code for this error.
    pub fn http_status(&self) -> StatusCode {
        StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }

    // ============================================
    // Convenience constructors for common errors
    // ============================================

    /// Create a generic not found error.
    pub fn not_found(message: impl Into<String>) -> Self {
        GenericErrorCode::NotFound.error(message)
    }

    /// Create a generic validation error.
    pub fn validation(message: impl Into<String>) -> Self {
        GenericErrorCode::ValidationFailed.error(message)
    }

    /// Create a generic internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        GenericErrorCode::InternalError.error(message)
    }

    /// Create a generic conflict error.
    pub fn conflict(message: impl Into<String>) -> Self {
        GenericErrorCode::Conflict.error(message)
    }

    /// Create a generic unauthorized error.
    pub fn unauthorized(message: impl Into<String>) -> Self {
        GenericErrorCode::Unauthorized.error(message)
    }

    /// Create a generic forbidden error.
    pub fn forbidden(message: impl Into<String>) -> Self {
        GenericErrorCode::Forbidden.error(message)
    }

    // ============================================
    // Migration helpers (mirror AggregateError variants)
    // ============================================

    /// Create a user/domain error.
    pub fn user_error(e: impl std::fmt::Display) -> Self {
        InfrastructureErrorCode::DomainError.error(e.to_string())
    }

    /// Create a database error.
    pub fn database_error(e: impl std::fmt::Display) -> Self {
        InfrastructureErrorCode::DatabaseError.error(e.to_string())
    }

    /// Create a serialization error.
    pub fn serialization_error(e: impl std::fmt::Display) -> Self {
        InfrastructureErrorCode::SerializationError.error(e.to_string())
    }

    /// Create a concurrency/version conflict error.
    pub fn concurrency_error() -> Self {
        InfrastructureErrorCode::ConcurrencyError.error("Version conflict")
    }

    /// Create an aggregate not found error.
    pub fn aggregate_not_found(id: &str) -> Self {
        InfrastructureErrorCode::AggregateNotFound.error(format!("Aggregate '{}' not found", id))
    }

    /// Create an aggregate already exists error.
    pub fn aggregate_already_exists(id: &str) -> Self {
        InfrastructureErrorCode::Conflict.error(format!("Aggregate '{}' already exists", id))
    }

    /// Create an error from an HTTP status code and message.
    pub fn from_status(status: StatusCode, message: impl Into<String>) -> Self {
        GenericErrorCode::from(status).error(message)
    }
}

impl Display for CqrsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {}: {}",
            self.internal_code, self.code, self.message
        )
    }
}

impl std::error::Error for CqrsError {}

impl From<std::io::Error> for CqrsError {
    fn from(e: std::io::Error) -> Self {
        CqrsError::user_error(e)
    }
}

// ============================================
// Infrastructure Error Codes (prefix = 0)
// ============================================

/// Error codes for infrastructure/technical errors.
///
/// These are used when converting from low-level errors.
/// Domain-specific errors should use their own error codes instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InfrastructureErrorCode {
    #[error("INTERNAL_ERROR")]
    InternalError,
    #[error("VALIDATION_FAILED")]
    ValidationFailed,
    #[error("NOT_FOUND")]
    NotFound,
    #[error("CONFLICT")]
    Conflict,
    #[error("UNAUTHORIZED")]
    Unauthorized,
    #[error("FORBIDDEN")]
    Forbidden,
    #[error("GONE")]
    Gone,
    #[error("DATABASE_ERROR")]
    DatabaseError,
    #[error("SERIALIZATION_ERROR")]
    SerializationError,
    #[error("AGGREGATE_NOT_FOUND")]
    AggregateNotFound,
    #[error("CONCURRENCY_ERROR")]
    ConcurrencyError,
    #[error("DOMAIN_ERROR")]
    DomainError,
    #[error("CQRS_ERROR")]
    CqrsInternalError,
    #[error("CONFIGURATION_ERROR")]
    ConfigurationError,
    #[error("UNKNOWN")]
    Unknown,
}

impl CqrsErrorCode for InfrastructureErrorCode {
    fn domain() -> &'static str {
        "infrastructure"
    }
    fn domain_prefix() -> u16 {
        0
    }

    fn error_index(&self) -> u16 {
        match self {
            Self::InternalError => 0,
            Self::ValidationFailed => 1,
            Self::NotFound => 2,
            Self::Conflict => 3,
            Self::Unauthorized => 4,
            Self::Forbidden => 5,
            Self::Gone => 6,
            Self::DatabaseError => 10,
            Self::SerializationError => 11,
            Self::AggregateNotFound => 12,
            Self::ConcurrencyError => 13,
            Self::DomainError => 14,
            Self::CqrsInternalError => 15,
            Self::ConfigurationError => 16,
            Self::Unknown => 99,
        }
    }

    fn http_status(&self) -> StatusCode {
        match self {
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ValidationFailed => StatusCode::BAD_REQUEST,
            Self::NotFound | Self::AggregateNotFound => StatusCode::NOT_FOUND,
            Self::Conflict | Self::ConcurrencyError => StatusCode::CONFLICT,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::Gone => StatusCode::GONE,
            Self::DatabaseError
            | Self::SerializationError
            | Self::CqrsInternalError
            | Self::ConfigurationError
            | Self::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
            Self::DomainError => StatusCode::BAD_REQUEST,
        }
    }
}

// ============================================
// Generic Error Codes (prefix = 1)
// ============================================

/// Generic error codes for common scenarios.
///
/// Use these when a domain-specific error code is not available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum GenericErrorCode {
    #[error("INTERNAL_ERROR")]
    InternalError,
    #[error("VALIDATION_FAILED")]
    ValidationFailed,
    #[error("NOT_FOUND")]
    NotFound,
    #[error("CONFLICT")]
    Conflict,
    #[error("UNAUTHORIZED")]
    Unauthorized,
    #[error("FORBIDDEN")]
    Forbidden,
    #[error("GONE")]
    Gone,
}

impl CqrsErrorCode for GenericErrorCode {
    fn domain() -> &'static str {
        "generic"
    }
    fn domain_prefix() -> u16 {
        1
    }

    fn error_index(&self) -> u16 {
        match self {
            Self::InternalError => 0,
            Self::ValidationFailed => 1,
            Self::NotFound => 2,
            Self::Conflict => 3,
            Self::Unauthorized => 4,
            Self::Forbidden => 5,
            Self::Gone => 6,
        }
    }

    fn http_status(&self) -> StatusCode {
        match self {
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ValidationFailed => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::Gone => StatusCode::GONE,
        }
    }
}

impl From<StatusCode> for GenericErrorCode {
    fn from(status: StatusCode) -> Self {
        match status.as_u16() {
            400 => GenericErrorCode::ValidationFailed,
            401 => GenericErrorCode::Unauthorized,
            403 => GenericErrorCode::Forbidden,
            404 => GenericErrorCode::NotFound,
            409 => GenericErrorCode::Conflict,
            410 => GenericErrorCode::Gone,
            _ => GenericErrorCode::InternalError,
        }
    }
}

// ============================================
// Backward Compatibility
// ============================================

#[deprecated(since = "0.2.0", note = "Use CqrsError instead")]
pub type AggregateError = CqrsError;

// ============================================
// Domain Error Code Macro
// ============================================

/// Macro to define domain-specific error codes with minimal boilerplate.
///
/// # Example
///
/// ```rust,ignore
/// use cqrs_rust_lib::define_domain_errors;
/// use http::StatusCode;
///
/// define_domain_errors! {
///     domain: "tenant",
///     prefix: 4,
///     errors: {
///         NotFound => (1, StatusCode::NOT_FOUND, "NOT_FOUND"),
///         Suspended => (2, StatusCode::BAD_REQUEST, "SUSPENDED"),
///         Deleted => (3, StatusCode::GONE, "DELETED"),
///         SlugExists => (4, StatusCode::CONFLICT, "SLUG_EXISTS"),
///     }
/// }
///
/// // Usage:
/// let err = ErrorCode::NotFound.error("Tenant 'abc' not found");
/// // -> CqrsError { domain: "tenant", code: "TENANT_NOT_FOUND", internal_code: 4001, ... }
/// ```
#[macro_export]
macro_rules! define_domain_errors {
    (
        domain: $domain:literal,
        prefix: $prefix:expr,
        errors: {
            $( $variant:ident => ($index:expr, $status:expr, $display:literal) ),* $(,)?
        }
    ) => {
        /// Domain-specific error codes.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, ::thiserror::Error)]
        pub enum ErrorCode {
            $(
                #[error($display)]
                $variant,
            )*
        }

        impl $crate::CqrsErrorCode for ErrorCode {
            fn domain() -> &'static str { $domain }
            fn domain_prefix() -> u16 { $prefix }

            fn error_index(&self) -> u16 {
                match self {
                    $( Self::$variant => $index, )*
                }
            }

            fn http_status(&self) -> ::http::StatusCode {
                match self {
                    $( Self::$variant => $status, )*
                }
            }
        }
    };
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_error_code() {
        let err = GenericErrorCode::NotFound.error("Resource not found");
        assert_eq!(err.domain, "generic");
        assert_eq!(err.code, "GENERIC_NOT_FOUND");
        assert_eq!(err.internal_code, 1002);
        assert_eq!(err.status, 404);
    }

    #[test]
    fn test_infrastructure_error_code() {
        let err = InfrastructureErrorCode::DatabaseError.error("Connection failed");
        assert_eq!(err.domain, "infrastructure");
        assert_eq!(err.code, "INFRASTRUCTURE_DATABASE_ERROR");
        assert_eq!(err.internal_code, 10); // 0 * 1000 + 10
        assert_eq!(err.status, 500);
    }

    #[test]
    fn test_convenience_constructors() {
        let err = CqrsError::not_found("User not found");
        assert_eq!(err.code, "GENERIC_NOT_FOUND");

        let err = CqrsError::validation("Invalid email");
        assert_eq!(err.code, "GENERIC_VALIDATION_FAILED");
    }

    #[test]
    fn test_migration_constructors() {
        let err = CqrsError::user_error("bad input");
        assert_eq!(err.code, "INFRASTRUCTURE_DOMAIN_ERROR");
        assert_eq!(err.status, 400);

        let err = CqrsError::database_error("connection lost");
        assert_eq!(err.code, "INFRASTRUCTURE_DATABASE_ERROR");
        assert_eq!(err.status, 500);

        let err = CqrsError::serialization_error("invalid json");
        assert_eq!(err.code, "INFRASTRUCTURE_SERIALIZATION_ERROR");
        assert_eq!(err.status, 500);

        let err = CqrsError::concurrency_error();
        assert_eq!(err.code, "INFRASTRUCTURE_CONCURRENCY_ERROR");
        assert_eq!(err.status, 409);

        let err = CqrsError::aggregate_not_found("abc");
        assert_eq!(err.code, "INFRASTRUCTURE_AGGREGATE_NOT_FOUND");
        assert_eq!(err.status, 404);
        assert!(err.message.contains("abc"));

        let err = CqrsError::aggregate_already_exists("xyz");
        assert_eq!(err.code, "INFRASTRUCTURE_CONFLICT");
        assert_eq!(err.status, 409);
        assert!(err.message.contains("xyz"));
    }

    #[test]
    fn test_with_details() {
        let err = GenericErrorCode::NotFound
            .error("User not found")
            .with_details(serde_json::json!({"user_id": "123"}));

        assert!(err.details.is_some());
        assert_eq!(err.details.as_ref().unwrap()["user_id"], "123");
    }

    #[test]
    fn test_serialization() {
        let err = GenericErrorCode::Conflict.error("Already exists");
        let json = serde_json::to_string(&err).unwrap();

        assert!(json.contains("\"domain\":\"generic\""));
        assert!(json.contains("\"code\":\"GENERIC_CONFLICT\""));
        assert!(json.contains("\"internalCode\":1003"));
        assert!(json.contains("\"message\":\"Already exists\""));
        // status should not be serialized
        assert!(!json.contains("\"status\""));
    }
}
