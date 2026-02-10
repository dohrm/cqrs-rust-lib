use cqrs_rust_lib::{define_domain_errors, CqrsError, CqrsErrorCode};
use http::StatusCode;

define_domain_errors! {
    domain: "todolist",
    prefix: 20,
    errors: {
        TodoNotFound => (1, StatusCode::NOT_FOUND, "TODO_NOT_FOUND"),
        TodoAlreadyResolved => (2, StatusCode::CONFLICT, "TODO_ALREADY_RESOLVED"),
        EmptyTitle => (3, StatusCode::BAD_REQUEST, "EMPTY_TITLE"),
        ListNameRequired => (4, StatusCode::BAD_REQUEST, "LIST_NAME_REQUIRED"),
    }
}

impl From<ErrorCode> for CqrsError {
    fn from(e: ErrorCode) -> Self {
        e.error(e.to_string())
    }
}
