#[macro_use]
extern crate thiserror;

#[macro_use]
extern crate serde;

mod context;
mod error;
mod executor;
mod gateway;
mod graphql;
mod introspection;
mod query;
mod schema;

pub use crate::context::Data;
pub use crate::error::{Error, GraphQLError, QueryError, Result};
pub use crate::executor::Executor;
pub use crate::gateway::Gateway;
pub use crate::graphql::{Payload, Response};
pub use crate::introspection::{
    Directive, DirectiveLocation, EnumValue, Field, InputValue, Schema as IntrospectionSchema,
    Type, TypeKind, INTROSPECTION_QUERY,
};
pub use crate::query::QueryBuilder;
pub use crate::schema::{DuplicateObjectField, Schema};
