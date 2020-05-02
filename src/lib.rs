#[macro_use]
extern crate thiserror;

#[macro_use]
extern crate serde_derive;
extern crate serde;

mod context;
mod error;
mod executor;
mod gateway;
mod graphql;
mod query;
mod schema;

use graphql_parser::query::{Definition, OperationDefinition};
use graphql_parser::Pos;
use serde_json::Value;

use context::Context;

pub use error::{Error, GraphQLError, QueryError, Result};
pub use executor::{Executor, INTROSPECTION_QUERY};
pub use gateway::{from_executors, Gateway};
pub use graphql::{Payload, Response};
pub use schema::{
    Directive, DirectiveLocation, EnumValue, Field, InputValue, Schema, Type, TypeKind,
};

pub async fn execute<'a>(gateway: &Gateway<'a>, payload: &Payload) -> Result<Value> {
    let query = graphql_parser::parse_query::<String>(payload.query.as_str())?;

    for definition in &query.definitions {
        match definition {
            Definition::Operation(operation) => match operation {
                OperationDefinition::Query(ast_query) => {
                    let ctx = &Context::new(
                        gateway,
                        payload,
                        &query,
                        ast_query.variable_definitions.clone(),
                    );
                    let object_type = match ctx.object_type("Query") {
                        Some(object_type) => object_type,
                        _ => return Err(Error::Custom("Schema must have Query type".to_owned())),
                    };
                    let data =
                        query::query(ctx, object_type, ast_query.selection_set.items.clone())
                            .await?;

                    return query::resolve(
                        ctx,
                        object_type,
                        ast_query.selection_set.items.clone(),
                        data,
                    )
                    .await;
                }
                OperationDefinition::Mutation(mutation) => {
                    let ctx = &Context::new(
                        gateway,
                        payload,
                        &query,
                        mutation.variable_definitions.clone(),
                    );
                    let object_type = match ctx.object_type("Mutation") {
                        Some(object_type) => object_type,
                        _ => {
                            let err = GraphQLError {
                                pos: Pos { line: 0, column: 0 },
                                err: QueryError::NotConfiguredMutations,
                            };
                            return Err(Error::Query(vec![err]));
                        }
                    };
                    let mutation = mutation.clone();
                    let root_data =
                        query::query(ctx, object_type, mutation.selection_set.items.clone())
                            .await?;

                    return query::resolve(
                        ctx,
                        object_type,
                        mutation.selection_set.items,
                        root_data,
                    )
                    .await;
                }
                _ => {}
            },
            _ => {}
        };
    }

    let err = GraphQLError {
        pos: Pos { line: 0, column: 0 },
        err: QueryError::NotSupported,
    };

    Err(Error::Query(vec![err]))
}
