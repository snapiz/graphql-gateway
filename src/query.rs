use graphql_parser::query::{
    Definition, Document, Field, InlineFragment, Mutation, OperationDefinition, Query, Selection,
    SelectionSet, Type, TypeCondition, Value as AstValue, VariableDefinition,
};
use graphql_parser::Pos;
use serde_json::{Map, Value};
use std::collections::HashMap;

use super::context::Context;
use super::error::{Error, GraphQLError, QueryError, Result};
use super::executor::Executor;
use super::graphql::Payload;

pub async fn query_root_selections<'a>(
    ctx: &'a Context<'a>,
    selections: Vec<Selection<'a, String>>,
    type_name: String,
) -> Result<Value> {
    let executors = resolve_executors(ctx, selections.clone(), type_name.to_owned(), Value::Null)?;
    let mut futures = Vec::new();

    for (name, _) in executors {
        let (executor_selections, mut variable_definitions, fragments) =
            resolve_executor_selections(
                name.to_owned().to_string(),
                ctx,
                selections.clone(),
                type_name.to_owned(),
                Value::Null,
            )?;

        let executor = match ctx.executor(name) {
            Some(executor) => executor,
            _ => return Err(Error::UnknownExecutor(name.to_owned())),
        };

        let mut definitions = Vec::new();
        let mut errors = Vec::new();

        for (fragment_name, _) in fragments {
            let fragment = match ctx.fragments.get(fragment_name.as_str()) {
                Some(fragment) => fragment,
                _ => {
                    let error = GraphQLError {
                        pos: Pos { line: 0, column: 0 },
                        err: QueryError::UnknownFragment {
                            name: fragment_name,
                        },
                    };
                    errors.push(error);
                    continue;
                }
            };

            let mut fragment = fragment.clone();

            let type_name = match &fragment.type_condition {
                TypeCondition::On(name) => name.to_owned(),
            };

            let (executor_selections, fragment_variable_definitions, _) =
                resolve_executor_selections(
                    name.to_owned().to_string(),
                    ctx,
                    fragment.selection_set.items.clone(),
                    type_name,
                    Value::Null,
                )?;

            fragment.selection_set.items = executor_selections;
            definitions.push(Definition::Fragment(fragment));

            for (key, value) in fragment_variable_definitions {
                variable_definitions.insert(key, value);
            }
        }

        if errors.len() > 0 {
            return Err(Error::Query(errors));
        }

        let variable_definitions = ctx
            .variable_definitions
            .clone()
            .into_iter()
            .filter(|variable_definition| {
                variable_definitions
                    .get(variable_definition.name.as_str())
                    .is_some()
            })
            .collect::<Vec<VariableDefinition<'a, String>>>();

        let operation = match type_name.as_str() {
            "Query" => OperationDefinition::Query(Query {
                position: Pos { line: 0, column: 0 },
                name: ctx.payload.operation_name.clone(),
                variable_definitions,
                directives: vec![],
                selection_set: SelectionSet {
                    span: (Pos { line: 0, column: 0 }, Pos { line: 0, column: 0 }),
                    items: executor_selections,
                },
            }),
            "Mutation" => OperationDefinition::Mutation(Mutation {
                position: Pos { line: 0, column: 0 },
                name: ctx.payload.operation_name.clone(),
                variable_definitions,
                directives: vec![],
                selection_set: SelectionSet {
                    span: (Pos { line: 0, column: 0 }, Pos { line: 0, column: 0 }),
                    items: executor_selections,
                },
            }),
            _ => continue,
        };

        definitions.push(Definition::Operation(operation));

        let query = Document { definitions };
        let query = query.to_string();

        futures.push(execute(
            executor,
            query,
            ctx.payload.variables.clone(),
            ctx.payload.operation_name.clone(),
        ));
    }

    let res = futures::future::try_join_all(futures).await?;

    let mut map = Map::new();

    for object in res {
        for (key, value) in object {
            map.insert(key, value);
        }
    }

    Ok(map.into())
}

pub async fn query_node_selections<'a>(
    ctx: &'a Context<'a>,
    selections: Vec<Selection<'a, String>>,
    type_name: String,
    data: Value,
) -> Result<Value> {
    let first_data = match &data {
        Value::Array(values) => match values.get(0) {
            Some(value) => value.clone(),
            _ => return Ok(data.clone()),
        },
        _ => data.clone(),
    };

    if first_data.get("id").is_none() {
        return Ok(data.clone());
    }

    let ids = match &data {
        Value::Array(values) => {
            let values = values
                .iter()
                .filter(|v| v["id"] != Value::Null)
                .map(|v| v["id"].clone())
                .collect::<Vec<Value>>();

            if values.len() == 0 {
                return Ok(data.clone());
            }

            values
        }
        _ => match data["id"].clone() {
            Value::String(s) => vec![Value::String(s)],
            _ => return Ok(data.clone()),
        },
    };

    let is_array = ids.len() > 1;

    let executors = resolve_executors(
        ctx,
        selections.clone(),
        type_name.to_owned(),
        first_data.clone(),
    )?;

    let mut futures = Vec::new();

    for (name, _) in executors {
        let (executor_selections, mut variable_definitions, fragments) =
            resolve_executor_selections(
                name.to_owned().to_string(),
                ctx,
                selections.clone(),
                type_name.to_owned(),
                first_data.clone(),
            )?;

        let executor = match ctx.executor(name) {
            Some(executor) => executor,
            _ => return Err(Error::UnknownExecutor(name.to_owned())),
        };

        let mut definitions = Vec::new();
        let mut errors = Vec::new();

        for (fragment_name, _) in fragments {
            let fragment = match ctx.fragments.get(fragment_name.as_str()) {
                Some(fragment) => fragment,
                _ => {
                    let error = GraphQLError {
                        pos: Pos { line: 0, column: 0 },
                        err: QueryError::UnknownFragment {
                            name: fragment_name,
                        },
                    };
                    errors.push(error);
                    continue;
                }
            };

            let mut fragment = fragment.clone();

            let type_name = match &fragment.type_condition {
                TypeCondition::On(name) => name.to_owned(),
            };

            let (executor_selections, fragment_variable_definitions, _) =
                resolve_executor_selections(
                    name.to_owned().to_string(),
                    ctx,
                    fragment.selection_set.items.clone(),
                    type_name,
                    first_data.clone(),
                )?;

            fragment.selection_set.items = executor_selections;
            definitions.push(Definition::Fragment(fragment));

            for (key, value) in fragment_variable_definitions {
                variable_definitions.insert(key, value);
            }
        }

        if errors.len() > 0 {
            return Err(Error::Query(errors));
        }

        let (var_name, var_type, field_name) = if is_array {
            (
                "ids".to_owned(),
                Type::NonNullType(Box::new(Type::ListType(Box::new(Type::NamedType(
                    "ID".to_owned(),
                ))))),
                "nodes".to_owned(),
            )
        } else {
            (
                "id".to_owned(),
                Type::NonNullType(Box::new(Type::NamedType("ID".to_owned()))),
                "node".to_owned(),
            )
        };

        let node_items = match type_name.as_str() {
            "Node" => executor_selections,
            _ => vec![Selection::InlineFragment(InlineFragment {
                position: Pos { line: 0, column: 0 },
                type_condition: Some(TypeCondition::On(type_name.to_owned())),
                directives: vec![],
                selection_set: SelectionSet {
                    span: (Pos { line: 0, column: 0 }, Pos { line: 0, column: 0 }),
                    items: executor_selections,
                },
            })],
        };

        let mut variable_definitions = ctx
            .variable_definitions
            .clone()
            .into_iter()
            .filter(|variable_definition| {
                variable_definitions
                    .get(variable_definition.name.as_str())
                    .is_some()
            })
            .collect::<Vec<VariableDefinition<'a, String>>>();

        variable_definitions.push(VariableDefinition {
            position: Pos { line: 0, column: 0 },
            name: var_name.to_owned(),
            var_type: var_type,
            default_value: None,
        });

        let operation = OperationDefinition::Query(Query {
            position: Pos { line: 0, column: 0 },
            name: Some("NodeQuery".to_owned()),
            variable_definitions,
            directives: vec![],
            selection_set: SelectionSet {
                span: (Pos { line: 0, column: 0 }, Pos { line: 0, column: 0 }),
                items: vec![Selection::Field(Field {
                    alias: None,
                    arguments: vec![(var_name.to_owned(), AstValue::Variable(var_name.to_owned()))],
                    directives: vec![],
                    name: field_name,
                    position: Pos { line: 0, column: 0 },
                    selection_set: SelectionSet {
                        span: (Pos { line: 0, column: 0 }, Pos { line: 0, column: 0 }),
                        items: node_items,
                    },
                })],
            },
        });

        definitions.push(Definition::Operation(operation));

        let query = Document { definitions };
        let query = query.to_string();

        let mut variables = match &ctx.payload.variables {
            Some(payload_variables) => match payload_variables {
                Value::Object(object) => object.clone(),
                _ => Map::new(),
            },
            _ => Map::new(),
        };

        if is_array {
            variables.insert("ids".to_owned(), Value::Array(ids.clone()));
        } else {
            variables.insert("id".to_owned(), ids[0].clone());
        };

        futures.push(execute(
            executor,
            query,
            Some(variables.into()),
            Some("NodeQuery".to_owned()),
        ));
    }

    let res = futures::future::try_join_all(futures).await?;

    match data.clone() {
        Value::Object(mut object) => {
            for data in res {
                let node = match data.get("node") {
                    Some(node) => node,
                    _ => continue,
                };

                let node = match node {
                    Value::Object(node) => node,
                    _ => continue,
                };
                for (key, value) in node {
                    object.insert(key.to_string(), value.clone());
                }
            }

            Ok(object.into())
        }
        Value::Array(values) => {
            let mut objects = Vec::new();

            for data in values.into_iter() {
                let value = match data {
                    Value::Object(mut object) => {
                        for data in &res {
                            let nodes = match data.get("nodes") {
                                Some(node) => node,
                                _ => continue,
                            };

                            let nodes = match nodes {
                                Value::Array(nodes) => nodes,
                                _ => continue,
                            };

                            let node_id = match object.get("id") {
                                Some(node) => node,
                                _ => continue,
                            };

                            let node = nodes.iter().find(|v| match v {
                                Value::Object(object) => match object.get("id") {
                                    Some(id) => id == node_id,
                                    _ => false,
                                },
                                _ => false,
                            });

                            let node = match node {
                                Some(node) => node,
                                _ => {
                                    object = Map::new();

                                    break;
                                }
                            };

                            let node = match node {
                                Value::Object(node) => node,
                                _ => continue,
                            };

                            for (key, value) in node {
                                object.insert(key.to_string(), value.clone());
                            }
                        }

                        if object.len() == 0 {
                            Value::Null
                        } else {
                            object.into()
                        }
                    }
                    _ => data,
                };

                objects.push(value);
            }

            Ok(Value::Array(objects))
        }
        _ => Ok(data.clone()),
    }
}

async fn execute(
    executor: &dyn Executor,
    query: String,
    variables: Option<Value>,
    operation_name: Option<String>,
) -> Result<Map<String, Value>> {
    let payload = &Payload {
        query,
        variables,
        operation_name,
    };

    let res = executor.execute(payload).await?;

    if res.get("errors").is_some() {
        return Err(Error::Executor(res));
    }

    let data = match res.get("data") {
        Some(data) => data,
        _ => return Err(Error::InvalidExecutorResponse),
    };

    match data {
        Value::Object(values) => Ok(values.clone()),
        _ => Err(Error::InvalidExecutorResponse),
    }
}

fn resolve_executors<'a>(
    ctx: &'a Context<'a>,
    selections: Vec<Selection<'a, String>>,
    type_name: String,
    data: Value,
) -> Result<HashMap<&'a str, bool>> {
    let mut executors = HashMap::new();
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

                let executor_name = match &s_field.executor_name {
                    Some(name) => name.as_str(),
                    _ => continue,
                };

                if type_name.as_str() == "Query"
                    && (&field.name == "node" || &field.name == "nodes")
                {
                    let executor_names = resolve_executors(
                        ctx,
                        field.selection_set.items.clone(),
                        "Node".to_owned(),
                        Value::Null,
                    )?;

                    for (executor_name, _) in executor_names {
                        executors.insert(executor_name, true);
                    }

                    continue;
                }

                let field_name = field.alias.as_ref().unwrap_or(&field.name);

                if data.get(field_name).is_none() {
                    executors.insert(executor_name, true);
                }
            }
            Selection::FragmentSpread(fragment) => {
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

                let executor_names =
                    resolve_executors(ctx, fragment_items, type_name.to_owned(), data.clone())?;

                for (executor_name, _) in executor_names {
                    executors.insert(executor_name, true);
                }
            }
            Selection::InlineFragment(fragment) => {
                let type_name = match fragment.type_condition {
                    Some(type_condition) => match type_condition {
                        TypeCondition::On(name) => name,
                    },
                    _ => type_name.to_owned(),
                };

                let executor_names = resolve_executors(
                    ctx,
                    fragment.selection_set.items,
                    type_name.to_owned(),
                    data.clone(),
                )?;

                for (executor_name, _) in executor_names {
                    executors.insert(executor_name, true);
                }
            }
        };
    }

    match errors.len() {
        0 => Ok(executors),
        _ => Err(Error::Query(errors)),
    }
}

fn resolve_executor_selections<'a>(
    executor_name: String,
    ctx: &'a Context<'a>,
    selections: Vec<Selection<'a, String>>,
    type_name: String,
    data: Value,
) -> Result<(
    Vec<Selection<'a, String>>,
    HashMap<String, bool>,
    HashMap<String, bool>,
)> {
    let mut cache = HashMap::new();
    let mut variable_definitions = HashMap::new();
    let mut fragments = HashMap::new();
    let mut items = Vec::new();
    let mut errors = Vec::new();

    if let Some(_) = ctx.field(&type_name, "id") {
        cache.insert("id".to_owned(), true);
        items.push(Selection::Field(Field {
            alias: None,
            arguments: vec![],
            directives: vec![],
            name: "id".to_owned(),
            position: Pos { line: 0, column: 0 },
            selection_set: SelectionSet {
                span: (Pos { line: 0, column: 0 }, Pos { line: 0, column: 0 }),
                items: vec![],
            },
        }));
    }

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

                let field_executor_name = match &s_field.executor_name {
                    Some(name) => name.as_str(),
                    _ => continue,
                };

                if cache.get(field_name).is_some() {
                    continue;
                }

                if type_name.as_str() == "Query"
                    && (&field.name == "node" || &field.name == "nodes")
                {
                    let mut field = field.clone();

                    let (field_items, field_variable_definitions, field_fragments) =
                        resolve_executor_selections(
                            executor_name.to_owned(),
                            ctx,
                            field.selection_set.items,
                            "Node".to_owned(),
                            Value::Null,
                        )?;

                    field.selection_set.items = field_items;

                    if field.selection_set.items.len() > 0 {
                        for (_, value) in field.arguments.clone() {
                            if let AstValue::Variable(name) = value {
                                variable_definitions.insert(name, true);
                            }
                        }

                        cache.insert(field_name.to_string(), true);
                        items.push(Selection::Field(field));

                        for (key, value) in field_variable_definitions {
                            variable_definitions.insert(key, value);
                        }

                        for (key, value) in field_fragments {
                            fragments.insert(key, value);
                        }
                    }

                    continue;
                }

                if data.get(field_name).is_none() && executor_name == field_executor_name {
                    for (_, value) in field.arguments.clone() {
                        if let AstValue::Variable(name) = value {
                            variable_definitions.insert(name, true);
                        }
                    }

                    let mut field = field.clone();

                    let (field_items, field_variable_definitions, field_fragments) =
                        resolve_executor_selections(
                            executor_name.to_owned(),
                            ctx,
                            field.selection_set.items,
                            field_type_name,
                            data.clone(),
                        )?;

                    field.selection_set.items = field_items;

                    cache.insert(field_name.to_string(), true);
                    items.push(Selection::Field(field));

                    for (key, value) in field_variable_definitions {
                        variable_definitions.insert(key, value);
                    }

                    for (key, value) in field_fragments {
                        fragments.insert(key, value);
                    }
                }
            }
            Selection::FragmentSpread(fragment_spread) => {
                let fragment = match ctx.fragments.get(fragment_spread.fragment_name.as_str()) {
                    Some(fragment) => fragment,
                    _ => {
                        let error = GraphQLError {
                            pos: fragment_spread.position,
                            err: QueryError::UnknownFragment {
                                name: fragment_spread.fragment_name,
                            },
                        };
                        errors.push(error);
                        continue;
                    }
                };

                fragments.insert(fragment_spread.fragment_name.to_owned(), true);
                items.push(Selection::FragmentSpread(fragment_spread));

                let (_, _, fragment_fragments) = resolve_executor_selections(
                    executor_name.to_owned(),
                    ctx,
                    fragment.selection_set.items.clone(),
                    type_name.to_owned(),
                    data.clone(),
                )?;

                for (key, value) in fragment_fragments {
                    fragments.insert(key, value);
                }
            }
            Selection::InlineFragment(mut fragment) => {
                let type_name = match &fragment.type_condition {
                    Some(type_condition) => match type_condition {
                        TypeCondition::On(name) => name.into(),
                    },
                    _ => type_name.to_owned(),
                };

                if ctx.object(&type_name, &executor_name).is_none() {
                    continue;
                }

                let (fragment_items, fragment_varaible_definitions, fragment_fragments) =
                    resolve_executor_selections(
                        executor_name.to_owned(),
                        ctx,
                        fragment.selection_set.items.clone(),
                        type_name.to_owned(),
                        data.clone(),
                    )?;

                fragment.selection_set.items = fragment_items;
                items.push(Selection::InlineFragment(fragment));

                for (key, value) in fragment_varaible_definitions {
                    variable_definitions.insert(key, value);
                }

                for (key, value) in fragment_fragments {
                    fragments.insert(key, value);
                }
            }
        };
    }

    match errors.len() {
        0 => Ok((items, variable_definitions, fragments)),
        _ => Err(Error::Query(errors)),
    }
}
