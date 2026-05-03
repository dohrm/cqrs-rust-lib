use cqrs_rust_lib::{define_domain_errors, CqrsError, CqrsErrorCode};
use http::StatusCode;

define_domain_errors! {
    domain: "ludotheque",
    prefix: 30,
    errors: {
        AlreadyBorrowed => (1, StatusCode::CONFLICT, "ALREADY_BORROWED"),
        NotBorrowed => (2, StatusCode::CONFLICT, "NOT_BORROWED"),
        EmptyTitle => (3, StatusCode::BAD_REQUEST, "EMPTY_TITLE"),
        InvalidPlayerCount => (4, StatusCode::BAD_REQUEST, "INVALID_PLAYER_COUNT"),
    }
}

impl From<ErrorCode> for CqrsError {
    fn from(e: ErrorCode) -> Self {
        e.error(e.to_string())
    }
}
