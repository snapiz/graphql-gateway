use futures::future::{BoxFuture, FutureExt};
use graphql_parser::query::{Selection, TypeCondition};
use serde_json::{Map, Value};

use super::context::Context;
use super::error::{Error, GraphQLError, QueryError, Result};
use super::query;
use super::schema::Type;

pub fn resolve_selections<'a>(
    ctx: &'a Context<'a>,
    object_type: &'a Type,
    selections: Vec<Selection<'a, String>>,
    data: Value,
) -> BoxFuture<'a, Result<Value>> {
    async move {
        if selections.len() == 0 || data == Value::Null {
            return Ok(data.clone());
        }

        let object_type_name = match object_type.name.as_ref() {
            Some(name) => name.as_str(),
            _ => return Err(Error::Custom("object_type name must be define".to_owned())),
        };

        let data = query::query_node_selections(ctx, object_type, selections.clone(), data.clone())
            .await?;

        if let Value::Array(values) = &data {
            if values.len() == 0 {
                return Ok(Value::Array(vec![]));
            }

            let futures = values
                .iter()
                .map(|value| {
                    resolve_selections(ctx, object_type, selections.clone(), value.clone())
                })
                .collect::<Vec<BoxFuture<'a, Result<Value>>>>();

            let values = futures::future::try_join_all(futures).await?;

            return Ok(Value::Array(values));
        }

        let mut selections_data = Map::new();
        let mut errors = Vec::new();

        for selection in selections {
            match selection {
                Selection::Field(field) => {
                    let s_field = match ctx.field(object_type_name, field.name.as_str()) {
                        Some(f) => f,
                        _ => {
                            let error = GraphQLError {
                                pos: field.position,
                                err: QueryError::FieldNotFound {
                                    name: field.name,
                                    object: object_type_name.to_owned(),
                                },
                            };
                            errors.push(error);
                            continue;
                        }
                    };

                    let field_type = match s_field.field_type() {
                        Some(field_type) => field_type,
                        _ => return Err(Error::Custom("field type must be define".to_owned())),
                    };

                    let field_name = field.alias.as_ref().unwrap_or(&field.name);

                    let field_data = match field.name.as_str() {
                        "__schema" => serde_json::to_value(ctx.schema)?,
                        _ => data
                            .get(field_name)
                            .map(|v| v.clone())
                            .unwrap_or(Value::Null),
                    };

                    let selection_data = match field_data {
                        Value::Null => continue,
                        _ => {
                            resolve_selections(
                                ctx,
                                field_type,
                                field.selection_set.items,
                                field_data,
                            )
                            .await?
                        }
                    };

                    selections_data.insert(field_name.to_owned(), selection_data);
                }
                Selection::FragmentSpread(fragment) => {
                    let data = data.clone();
                    let fragment_items = match ctx.fragments.get(fragment.fragment_name.as_str()) {
                        Some(fragment) => fragment.selection_set.items.clone(),
                        _ => {
                            let error = GraphQLError {
                                pos: fragment.position,
                                err: QueryError::UnknownFragment {
                                    name: fragment.fragment_name,
                                },
                            };
                            errors.push(error);
                            continue;
                        }
                    };

                    let selection_data = match data {
                        Value::Null => continue,
                        _ => resolve_selections(ctx, object_type, fragment_items, data).await?,
                    };

                    if let Value::Object(data) = selection_data {
                        for (name, value) in data {
                            selections_data.insert(name, value);
                        }
                    }
                }
                Selection::InlineFragment(fragment) => {
                    let object_type = match fragment.type_condition {
                        Some(type_condition) => match type_condition {
                            TypeCondition::On(name) => match ctx.object_type(&name) {
                                Some(object_type) => object_type,
                                _ => {
                                    let error = GraphQLError {
                                        pos: fragment.position,
                                        err: QueryError::MissingTypeConditionInlineFragment {
                                            name,
                                        },
                                    };
                                    errors.push(error);
                                    continue;
                                }
                            },
                        },
                        _ => object_type,
                    };

                    let data = data.clone();

                    let selection_data = match data {
                        Value::Null => continue,
                        _ => {
                            resolve_selections(ctx, object_type, fragment.selection_set.items, data)
                                .await?
                        }
                    };

                    if let Value::Object(data) = selection_data {
                        for (name, value) in data {
                            selections_data.insert(name, value);
                        }
                    }
                }
            }
        }

        match errors.len() {
            0 => Ok(selections_data.into()),
            _ => Err(Error::Query(errors)),
        }
    }
    .boxed()
}
