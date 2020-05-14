use graphql_parser::query::ParseError;
use graphql_parser::Pos;
use serde_json::error::Error as JsonError;
use std::convert::From;
use std::fmt::Debug;

use super::schema::DuplicateObjectField;

#[derive(Debug, Error)]
pub enum QueryError {
    #[error("Not supported.")]
    NotSupported,

    #[error("Cannot query field \"{name}\" on type \"{object}\".")]
    FieldNotFound { name: String, object: String },

    #[error("Unknown fragment \"{name}\".")]
    UnknownFragment { name: String },

    #[error("Missing type condition on inline fragment.")]
    MissingTypeConditionInlineFragment,

    #[error("Schema is not configured for queries.")]
    NotConfiguredQueries,

    #[error("Schema is not configured for mutations.")]
    NotConfiguredMutations,
}

#[derive(Debug)]
pub struct GraphQLError {
    pub pos: Pos,
    pub err: QueryError,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Custom(String),

    #[error("Json error: {0}")]
    Json(JsonError),

    #[error("Parse error: {0}")]
    GraphQLParser(ParseError),

    #[error("Executor error: {0}")]
    Executor(serde_json::Value),

    #[error("Unknown executor \"{0}\".")]
    UnknownExecutor(String),

    #[error("Invalid executor response")]
    InvalidExecutorResponse,

    #[error("Missing field id on {0}")]
    MissingFieldId(String),

    #[error("Query error.")]
    Query(Vec<GraphQLError>),

    #[error("Duplicate object fields error.")]
    DuplicateObjectFields(Vec<DuplicateObjectField>),
}

impl From<JsonError> for Error {
    fn from(e: JsonError) -> Error {
        Error::Json(e)
    }
}

impl From<ParseError> for Error {
    fn from(e: ParseError) -> Error {
        Error::GraphQLParser(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
