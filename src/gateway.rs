use graphql_parser::query::{Definition, OperationDefinition};
use graphql_parser::Pos;
use serde_json::Value;
use std::collections::HashMap;

use super::context::Context;
use super::error::{Error, GraphQLError, QueryError, Result};
use super::executor::Executor;
use super::graphql::Payload;
use super::query;
use super::schema;
use super::schema::{Field, Schema, Type};

pub struct Gateway<'a> {
    pub executors: HashMap<String, &'a (dyn Executor + 'a)>,
    pub schemas: HashMap<String, Schema>,
    pub schema: Schema,
    pub types: HashMap<String, Type>,
    pub fields: HashMap<String, Field>,
    pub objects: HashMap<String, Type>,
}

impl<'a> Gateway<'a> {
    pub async fn poll(&mut self, executor: &str) -> Result<()> {
        let executor = match self.executors.get(executor) {
            Some(executor) => executor,
            _ => return Err(Error::UnknownExecutor(executor.to_owned())),
        };

        let schema = executor.get_schema().await?;
        self.schemas.insert(executor.name(), schema);
        let schemas = self.schemas.values().cloned().collect::<Vec<Schema>>();
        self.schema = schema::combine(&schemas).await?;
        self.types = self.schema.to_types();
        self.fields = self.schema.to_fields();
        self.objects = self.schema.to_objects();

        Ok(())
    }

    pub async fn execute(&self, payload: &Payload) -> Result<Value> {
        let query = graphql_parser::parse_query::<String>(payload.query.as_str())?;

        for definition in &query.definitions {
            match definition {
                Definition::Operation(operation) => match operation {
                    OperationDefinition::Query(ast_query) => {
                        let ctx = &Context::new(&self, payload, &query, ast_query.variable_definitions.clone());
                        let object_type = match ctx.object_type("Query") {
                            Some(object_type) => object_type,
                            _ => {
                                return Err(Error::Custom("Schema must have Query type".to_owned()))
                            }
                        };
                        let data = query::query(
                            ctx,
                            object_type,
                            ast_query.selection_set.items.clone(),
                        )
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
                        let ctx = &Context::new(&self, payload, &query, mutation.variable_definitions.clone());
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
                        let root_data = query::query(
                            ctx,
                            object_type,
                            mutation.selection_set.items.clone(),
                        )
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
        return Err(Error::Query(vec![err]));
    }
}

pub async fn from_executors<'a>(executors: Vec<&'a (dyn Executor + 'a)>) -> Result<Gateway<'a>> {
    let futures = executors.iter().map(|e| e.get_schema());
    let schemas = futures::future::try_join_all(futures).await?;
    let schema = schema::combine(&schemas).await?;
    let types = schema.to_types();
    let fields = schema.to_fields();
    let objects = schema.to_objects();

    let executors = executors
        .into_iter()
        .map(|e| (e.name(), e))
        .collect::<HashMap<String, &'a (dyn Executor + 'a)>>();

    let schemas = schemas
        .into_iter()
        .map(|s| {
            (
                s.executor_name
                    .as_ref()
                    .expect("Schema must have executor at this point")
                    .to_owned(),
                s,
            )
        })
        .collect::<HashMap<String, Schema>>();

    Ok(Gateway {
        executors,
        schemas,
        schema,
        types,
        fields,
        objects,
    })
}
