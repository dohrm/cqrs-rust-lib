//! `set_problem_type_base_uri` writes to a process-wide `OnceLock`, so it needs
//! its own test binary to stay isolated from the other tests.

use cqrs_rust_lib::problem::{
    problem_type_base_uri, set_problem_type_base_uri, ProblemDetails, PROBLEM_JSON,
};
use cqrs_rust_lib::CqrsError;

#[test]
fn base_uri_drives_the_type_member() {
    assert_eq!(PROBLEM_JSON, "application/problem+json");
    assert!(problem_type_base_uri().is_none());

    // The trailing slash is normalised away.
    set_problem_type_base_uri("https://api.example.com/errors/").unwrap();
    assert_eq!(
        problem_type_base_uri(),
        Some("https://api.example.com/errors")
    );

    let problem = ProblemDetails::from(&CqrsError::not_found("nope"));
    assert_eq!(
        problem.type_uri,
        "https://api.example.com/errors/GENERIC_NOT_FOUND"
    );

    // A per-error URI still wins over the base.
    let overridden = CqrsError::not_found("nope").with_type_uri("urn:custom:thing");
    assert_eq!(overridden.to_problem().type_uri, "urn:custom:thing");

    // Setting it twice is reported instead of silently ignored.
    assert!(set_problem_type_base_uri("https://other.example.com").is_err());
    assert_eq!(
        problem_type_base_uri(),
        Some("https://api.example.com/errors")
    );
}
