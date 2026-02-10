use cqrs_rust_lib::{define_domain_errors, CqrsError, CqrsErrorCode};
use http::StatusCode;

define_domain_errors! {
    domain: "account",
    prefix: 10,
    errors: {
        InsufficientFunds => (1, StatusCode::BAD_REQUEST, "INSUFFICIENT_FUNDS"),
        InvalidAmount => (2, StatusCode::BAD_REQUEST, "INVALID_AMOUNT"),
        AccountClosed => (3, StatusCode::GONE, "ACCOUNT_CLOSED"),
        NegativeDeposit => (4, StatusCode::BAD_REQUEST, "NEGATIVE_DEPOSIT"),
    }
}

impl From<ErrorCode> for CqrsError {
    fn from(e: ErrorCode) -> Self {
        e.error(e.to_string())
    }
}
