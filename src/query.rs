use futures::future::{BoxFuture, FutureExt};
use graphql_parser::query::{
    Definition, Document, Field, InlineFragment, Mutation, OperationDefinition, Query, Selection,
    SelectionSet, Type as AstType, TypeCondition, Value as AstValue, VariableDefinition,
};
use graphql_parser::Pos;
use serde_json::{Map, Value};
use std::collections::HashMap;

use super::context::Context;
use super::error::{Error, GraphQLError, QueryError, Result};
use super::schema::{Type, TypeKind};

pub async fn query<'a>(
    ctx: &'a Context<'a>,
    object_type: &'a Type,
    selections: Vec<Selection<'a, String>>,
) -> Result<Value> {
    let executors = resolve_executors(ctx, object_type, selections.clone(), Value::Null)?;
    let mut futures = Vec::new();

    let object_type_name = match object_type.name.as_ref() {
        Some(name) => name,
        _ => return Err(Error::Custom("object_type name must be define".to_owned())),
    };

    for (name, _) in executors {
        let (executor_selections, mut variable_definitions, fragments) = resolve_executor(
            name.to_owned().to_string(),
            ctx,
            object_type,
            selections.clone(),
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

            let object_type = match &fragment.type_condition {
                TypeCondition::On(name) => match ctx.object_type(name) {
                    Some(object_type) => object_type,
                    _ => {
                        let error = GraphQLError {
                            pos: fragment.position,
                            err: QueryError::MissingTypeConditionInlineFragment {
                                name: name.to_owned(),
                            },
                        };
                        errors.push(error);
                        continue;
                    }
                },
            };

            let (executor_selections, fragment_variable_definitions, _) = resolve_executor(
                name.to_owned().to_string(),
                ctx,
                object_type,
                fragment.selection_set.items.clone(),
                Value::Null,
            )?;

            fragment.selection_set.items = executor_selections;
            definitions.push(Definition::Fragment(fragment));

            for (key, value) in fragment_variable_definitions {
                variable_definitions.insert(key, value);
            }
        }

        if !errors.is_empty() {
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

        let operation = match object_type_name.as_str() {
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

        futures.push(executor.get_data(
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

pub async fn query_node<'a>(
    ctx: &'a Context<'a>,
    object_type: &'a Type,
    selections: Vec<Selection<'a, String>>,
    data: Value,
) -> Result<Value> {
    let object_type_name = match object_type.name.as_ref() {
        Some(name) => name.as_str(),
        _ => return Err(Error::Custom("object_type name must be define".to_owned())),
    };

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

            if values.is_empty() {
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

    let executors = resolve_executors(ctx, object_type, selections.clone(), first_data.clone())?;

    let mut futures = Vec::new();

    for (name, _) in executors {
        let (executor_selections, mut variable_definitions, fragments) = resolve_executor(
            name.to_owned().to_string(),
            ctx,
            object_type,
            selections.clone(),
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

            let object_type = match &fragment.type_condition {
                TypeCondition::On(name) => match ctx.object_type(&name) {
                    Some(object_type) => object_type,
                    _ => {
                        let error = GraphQLError {
                            pos: fragment.position,
                            err: QueryError::MissingTypeConditionInlineFragment {
                                name: name.to_owned(),
                            },
                        };
                        errors.push(error);
                        continue;
                    }
                },
            };

            let (executor_selections, fragment_variable_definitions, _) = resolve_executor(
                name.to_owned().to_string(),
                ctx,
                object_type,
                fragment.selection_set.items.clone(),
                first_data.clone(),
            )?;

            fragment.selection_set.items = executor_selections;
            definitions.push(Definition::Fragment(fragment));

            for (key, value) in fragment_variable_definitions {
                variable_definitions.insert(key, value);
            }
        }

        if !errors.is_empty() {
            return Err(Error::Query(errors));
        }

        let (var_name, var_type, field_name) = if is_array {
            (
                "ids".to_owned(),
                AstType::NonNullType(Box::new(AstType::ListType(Box::new(AstType::NamedType(
                    "ID".to_owned(),
                ))))),
                "nodes".to_owned(),
            )
        } else {
            (
                "id".to_owned(),
                AstType::NonNullType(Box::new(AstType::NamedType("ID".to_owned()))),
                "node".to_owned(),
            )
        };

        let node_items = match object_type_name {
            "Node" => executor_selections,
            _ => vec![Selection::InlineFragment(InlineFragment {
                position: Pos { line: 0, column: 0 },
                type_condition: Some(TypeCondition::On(object_type_name.to_owned())),
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
            var_type,
            position: Pos { line: 0, column: 0 },
            name: var_name.to_owned(),
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

        futures.push(executor.get_data(
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

                        if object.is_empty() {
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

fn resolve_executors<'a>(
    ctx: &'a Context<'a>,
    object_type: &'a Type,
    selections: Vec<Selection<'a, String>>,
    data: Value,
) -> Result<HashMap<&'a str, bool>> {
    let mut executors = HashMap::new();
    let mut errors = Vec::new();

    let object_type_name = match object_type.name.as_ref() {
        Some(name) => name.as_str(),
        _ => return Err(Error::Custom("object_type name must be define".to_owned())),
    };

    for selection in selections {
        match selection {
            Selection::Field(field) => {
                if field.name.as_str() == "__schema" {
                    continue;
                }

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

                let executor_name = match &s_field.executor_name {
                    Some(name) => name.as_str(),
                    _ => {
                        return Err(Error::Custom(
                            "field executor_name must be define".to_owned(),
                        ))
                    }
                };

                let field_type = match s_field.field_type() {
                    Some(field_type) => field_type,
                    _ => return Err(Error::Custom("field type must be define".to_owned())),
                };

                if field_type.kind == TypeKind::Interface {
                    let executor_names = resolve_executors(
                        ctx,
                        field_type,
                        field.selection_set.items.clone(),
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
                    resolve_executors(ctx, object_type, fragment_items, data.clone())?;

                for (executor_name, _) in executor_names {
                    executors.insert(executor_name, true);
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
                                    err: QueryError::MissingTypeConditionInlineFragment { name },
                                };
                                errors.push(error);
                                continue;
                            }
                        },
                    },
                    _ => object_type,
                };

                let executor_names = resolve_executors(
                    ctx,
                    object_type,
                    fragment.selection_set.items,
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

fn resolve_executor<'a>(
    executor_name: String,
    ctx: &'a Context<'a>,
    object_type: &'a Type,
    selections: Vec<Selection<'a, String>>,
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

    let object_type_name = match object_type.name.as_ref() {
        Some(name) => name.as_str(),
        _ => return Err(Error::Custom("object_type name must be define".to_owned())),
    };

    if ctx.field(object_type_name, "id").is_some() {
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

                let field_executor_name = match &s_field.executor_name {
                    Some(name) => name.as_str(),
                    _ => {
                        return Err(Error::Custom(
                            "field executor_name must be define".to_owned(),
                        ))
                    }
                };

                if cache.get(field_name).is_some() {
                    continue;
                }

                if field_type.kind == TypeKind::Interface {
                    let mut field = field.clone();

                    let (field_items, field_variable_definitions, field_fragments) =
                        resolve_executor(
                            executor_name.to_owned(),
                            ctx,
                            field_type,
                            field.selection_set.items,
                            Value::Null,
                        )?;

                    field.selection_set.items = field_items;

                    if !field.selection_set.items.is_empty() {
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
                        resolve_executor(
                            executor_name.to_owned(),
                            ctx,
                            field_type,
                            field.selection_set.items,
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

                let (_, _, fragment_fragments) = resolve_executor(
                    executor_name.to_owned(),
                    ctx,
                    object_type,
                    fragment.selection_set.items.clone(),
                    data.clone(),
                )?;

                for (key, value) in fragment_fragments {
                    fragments.insert(key, value);
                }
            }
            Selection::InlineFragment(mut fragment) => {
                let (object_type_name, object_type) = match &fragment.type_condition {
                    Some(type_condition) => match type_condition {
                        TypeCondition::On(name) => match ctx.object_type(&name) {
                            Some(object_type) => (name.as_str(), object_type),
                            _ => {
                                let error = GraphQLError {
                                    pos: fragment.position,
                                    err: QueryError::MissingTypeConditionInlineFragment {
                                        name: name.to_owned(),
                                    },
                                };
                                errors.push(error);
                                continue;
                            }
                        },
                    },
                    _ => (object_type_name, object_type),
                };

                if ctx.object(&object_type_name, &executor_name).is_none() {
                    continue;
                }

                let (fragment_items, fragment_varaible_definitions, fragment_fragments) =
                    resolve_executor(
                        executor_name.to_owned(),
                        ctx,
                        object_type,
                        fragment.selection_set.items.clone(),
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

pub fn resolve<'a>(
    ctx: &'a Context<'a>,
    object_type: &'a Type,
    selections: Vec<Selection<'a, String>>,
    data: Value,
) -> BoxFuture<'a, Result<Value>> {
    async move {
        if selections.is_empty() || data == Value::Null {
            return Ok(data.clone());
        }

        let object_type_name = match object_type.name.as_ref() {
            Some(name) => name.as_str(),
            _ => return Err(Error::Custom("object_type name must be define".to_owned())),
        };

        let data = query_node(ctx, object_type, selections.clone(), data.clone()).await?;

        if let Value::Array(values) = &data {
            if values.is_empty() {
                return Ok(Value::Array(vec![]));
            }

            let futures = values
                .iter()
                .map(|value| resolve(ctx, object_type, selections.clone(), value.clone()))
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
                        "__schema" => serde_json::to_value(&ctx.gateway.schema)?,
                        _ => data
                            .get(field_name)
                            .cloned()
                            .unwrap_or(Value::Null),
                    };

                    let selection_data = match field_data {
                        Value::Null => continue,
                        _ => {
                            resolve(ctx, field_type, field.selection_set.items, field_data).await?
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
                        _ => resolve(ctx, object_type, fragment_items, data).await?,
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
                        _ => resolve(ctx, object_type, fragment.selection_set.items, data).await?,
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
