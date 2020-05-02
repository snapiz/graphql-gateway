use futures::future::{BoxFuture, FutureExt};
use graphql_parser::query::{Selection, TypeCondition};
use serde_json::{Map, Value};

use super::context::Context;
use super::error::{Error, GraphQLError, QueryError, Result};
use super::query;

pub fn resolve_selections<'a>(
    ctx: &'a Context<'a>,
    selections: Vec<Selection<'a, String>>,
    type_name: String,
    data: Value,
) -> BoxFuture<'a, Result<Value>> {
    async move {
        if selections.len() == 0 || data == Value::Null {
            return Ok(data.clone());
        }

        let data = query::query_node_selections(
            ctx,
            selections.clone(),
            type_name.to_owned(),
            data.clone(),
        )
        .await?;

        if let Value::Array(values) = &data {
            if values.len() == 0 {
                return Ok(Value::Array(vec![]));
            }

            let futures = values
                .iter()
                .map(|value| {
                    resolve_selections(ctx, selections.clone(), type_name.to_owned(), value.clone())
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
                    let s_field = match ctx.field(&type_name, field.name.as_str()) {
                        Some(f) => f,
                        _ => {
                            let error = GraphQLError {
                                pos: field.position,
                                err: QueryError::FieldNotFound {
                                    name: field.name,
                                    object: type_name.to_owned(),
                                },
                            };
                            errors.push(error);
                            continue;
                        }
                    };

                    let field_type_name = match s_field.field_type() {
                        Some(field_type) => field_type
                            .name
                            .as_ref()
                            .expect("Field type to have name")
                            .into(),
                        _ => continue,
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
                                field.selection_set.items,
                                field_type_name,
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
                        _ => {
                            resolve_selections(ctx, fragment_items, type_name.to_owned(), data)
                                .await?
                        }
                    };

                    if let Value::Object(data) = selection_data {
                        for (name, value) in data {
                            selections_data.insert(name, value);
                        }
                    }
                }
                Selection::InlineFragment(fragment) => {
                    let type_name = match fragment.type_condition {
                        Some(type_condition) => match type_condition {
                            TypeCondition::On(name) => name,
                        },
                        _ => type_name.to_owned(),
                    };

                    let data = data.clone();

                    let selection_data = match data {
                        Value::Null => continue,
                        _ => {
                            resolve_selections(ctx, fragment.selection_set.items, type_name, data)
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
