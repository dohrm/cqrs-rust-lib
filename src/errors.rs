/// Represents the various errors that can occur in an aggregate operation.
///
/// This enum is used to encapsulate different categories of errors that may arise
/// while performing aggregate-related functionalities in the application. Each variant
/// captures specific error types or contexts.
///
/// Variants:
/// - `UserError`: Represents an error caused by user input or behavior. It encapsulates
///   a `Problem` type, which should ideally contain information about the specific issue.
/// - `Conflict`: Indicates a conflict-related error. For example, this could represent
///   a violation of business rules or constraints.
/// - `DatabaseError`: Represents an error related to database operations. It encapsulates
///   a boxed trait object of type `std::error::Error`, allowing any database-related error
///   to be captured regardless of its concrete type.
/// - `SerializationError`: Represents an error occurring during (de)serialization operations,
///   such as converting data to/from formats like JSON or binary. It encapsulates a boxed
///   trait object of type `std::error::Error`.
/// - `UnexpectedError`: Represents any other unexpected error that does not fall under
///   the above categories. It also encapsulates a boxed trait object of type `std::error::Error`.
///
/// Example:
/// ```
/// use cqrs_rust_lib::AggregateError;
///
/// fn example() -> Result<(), AggregateError> {
///     Err(AggregateError::Conflict)
/// }
///
/// match example() {
///     Ok(_) => println!("Operation succeeded"),
///     Err(e) => eprintln!("Operation failed: {:?}", e),
/// }
/// ```
///
/// This error enum derives `Debug` for debugging purposes and uses `thiserror::Error`
/// to implement the `std::error::Error` trait seamlessly, enabling compatibility with
/// other error-handling mechanisms.
#[derive(Debug, thiserror::Error)]
pub enum AggregateError {
    #[error("{0}")]
    UserError(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Conflict")]
    Conflict,
    #[error("{0}")]
    DatabaseError(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("{0}")]
    SerializationError(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("{0}")]
    UnexpectedError(Box<dyn std::error::Error + Send + Sync + 'static>),
}
