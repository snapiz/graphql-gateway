#[macro_use]
extern crate thiserror;

#[macro_use]
extern crate serde;

mod context;
mod data;
mod executor;
mod gateway;
mod http;
mod query;
mod schema;

pub use crate::data::Data;
pub use crate::executor::{Executor, INTROSPECTION_QUERY};
pub use crate::gateway::{Gateway, GatewayError};
pub use crate::http::{GraphQLPayload, GraphQLResponse};
pub use crate::query::{QueryBuilder, QueryError};
pub use crate::schema::{Schema, TypeKind};
