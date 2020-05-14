use futures::future::{BoxFuture, FutureExt};
use graphql_parser::query::{
    Definition, Document, Field, FragmentDefinition, InlineFragment, Mutation, OperationDefinition,
    Query, Selection, SelectionSet, Type as AstType, TypeCondition, Value as AstValue,
    VariableDefinition,
};
use graphql_parser::Pos;
use serde_json::{Map, Value};
use std::any::Any;
use std::collections::HashMap;
use std::convert::Into;

use super::context::{Context, Data};
use super::error::{Error, GraphQLError, QueryError, Result};
use super::gateway::Gateway;

#[derive(Debug, Clone)]
struct ResolveInfo<'a> {
    selections: Vec<Selection<'a, String>>,
    fragments: HashMap<String, FragmentDefinition<'a, String>>,
    variable_definitions: HashMap<String, VariableDefinition<'a, String>>,
}

pub struct QueryBuilder {
    query_source: String,
    operation_name: Option<String>,
    variables: Option<Value>,
    ctx_data: Option<Data>,
}

impl<'a> QueryBuilder {
    pub fn new<T: Into<String>>(query: T) -> Self {
        QueryBuilder {
            query_source: query.into(),
            operation_name: None,
            variables: None,
            ctx_data: None,
        }
    }

    pub fn operation_name<T: Into<String>>(mut self, operation_name: T) -> Self {
        self.operation_name = Some(operation_name.into());
        self
    }

    pub fn variables(mut self, variables: Value) -> Self {
        self.variables = Some(variables);
        self
    }

    pub fn data<D: Any + Send + Sync>(mut self, data: D) -> Self {
        if let Some(ctx_data) = &mut self.ctx_data {
            ctx_data.insert(data);
        } else {
            let mut ctx_data = Data::default();
            ctx_data.insert(data);
            self.ctx_data = Some(ctx_data);
        }
        self
    }

    pub async fn execute(&'a self, gateway: &'a Gateway<'a>) -> Result<Value> {
        let document = graphql_parser::parse_query::<String>(&self.query_source)?;
        let fragments = document
            .definitions
            .iter()
            .filter_map(|definition| match definition {
                Definition::Fragment(fragment) => Some((fragment.name.clone(), fragment.clone())),
                _ => None,
            })
            .collect::<HashMap<String, FragmentDefinition<'_, String>>>();

        let (is_query, selections, variable_definitions) = document
            .definitions
            .iter()
            .find_map(|definition| match definition {
                Definition::Operation(operation) => match operation {
                    OperationDefinition::SelectionSet(selection_set) => {
                        Some((true, selection_set.items.clone(), vec![]))
                    }
                    OperationDefinition::Query(query) => Some((
                        true,
                        query.selection_set.items.clone(),
                        query.variable_definitions.clone(),
                    )),
                    OperationDefinition::Mutation(mutation) => Some((
                        false,
                        mutation.selection_set.items.clone(),
                        mutation.variable_definitions.clone(),
                    )),
                    _ => None,
                },
                _ => None,
            })
            .ok_or(Error::Query(vec![GraphQLError {
                pos: Pos::default(),
                err: QueryError::NotSupported,
            }]))?;

        let variable_definitions = variable_definitions
            .iter()
            .map(|variable_definition| {
                (
                    variable_definition.name.clone(),
                    variable_definition.clone(),
                )
            })
            .collect();

        let schema = gateway
            .schema
            .as_ref()
            .expect("The document does not exist.");

        let ctx = Context {
            schema,
            fragments,
            variable_definitions,
            ctx_data: self.ctx_data.as_ref(),
            executors: &gateway.executors,
            operation_name: self.operation_name.as_ref().map(|v| v.as_str()),
            variables: self.variables.as_ref(),
        };

        let object_type = if is_query {
            ctx.object("Query")
                .ok_or(Error::Query(vec![GraphQLError {
                    pos: Pos::default(),
                    err: QueryError::NotConfiguredQueries,
                }]))?
                .name()
        } else {
            ctx.object("Mutation")
                .ok_or(Error::Query(vec![GraphQLError {
                    pos: Pos::default(),
                    err: QueryError::NotConfiguredMutations,
                }]))?
                .name()
        };

        Ok(resolve(ctx.clone(), object_type.to_owned(), selections, None).await?)
    }
}

fn resolve<'a, T: Into<String> + Send + 'a>(
    ctx: Context<'a>,
    object_type: T,
    selections: Vec<Selection<'a, String>>,
    data: Option<Value>,
) -> BoxFuture<'a, Result<Value>> {
    async move {
        let object_type = object_type.into();

        if let Some(data) = &data {
            if selections.is_empty() || data.is_null() {
                return Ok(data.clone());
            }

            if let Value::Array(values) = data {
                if values.is_empty() {
                    return Ok(Value::Array(vec![]));
                }
                let futures = values
                    .iter()
                    .map(|value| {
                        resolve(
                            ctx.clone(),
                            object_type.clone(),
                            selections.clone(),
                            Some(value.clone()),
                        )
                    })
                    .collect::<Vec<BoxFuture<'a, Result<Value>>>>();
                let values = futures::future::try_join_all(futures).await?;
                return Ok(Value::Array(values));
            }
        }

        let query_data = query(
            ctx.clone(),
            object_type.clone(),
            data.clone(),
            selections.clone(),
        )
        .await?;

        let mut map = Map::new();

        for selection in selections {
            match selection {
                Selection::FragmentSpread(fragment_spread) => {
                    let fragment = match ctx.fragments.get(&fragment_spread.fragment_name) {
                        Some(fragment) => fragment,
                        _ => continue,
                    };

                    let object_type = match &fragment.type_condition {
                        TypeCondition::On(v) => v,
                    };

                    let data = resolve(
                        ctx.clone(),
                        object_type.clone(),
                        fragment.selection_set.items.clone(),
                        data.clone(),
                    )
                    .await?;

                    if let Value::Object(object) = data {
                        map.extend(object);
                    }
                }
                Selection::InlineFragment(inline_fragment) => {
                    let type_condition =
                        inline_fragment
                            .type_condition
                            .as_ref()
                            .ok_or(Error::Query(vec![GraphQLError {
                                pos: inline_fragment.position,
                                err: QueryError::MissingTypeConditionInlineFragment,
                            }]))?;

                    let object_type = match type_condition {
                        TypeCondition::On(v) => v,
                    };

                    let data = resolve(
                        ctx.clone(),
                        object_type.clone(),
                        inline_fragment.selection_set.items.clone(),
                        data.clone(),
                    )
                    .await?;

                    if let Value::Object(object) = data {
                        map.extend(object);
                    }
                }
                Selection::Field(field) => {
                    let object_type = object_type.clone();
                    let field_name = field.alias.as_ref().unwrap_or(&field.name);
                    let field_data = if field.name == "__schema" {
                        serde_json::to_value(&ctx.schema.schema)?
                    } else {
                        let data = match data.as_ref().and_then(|data| data.get(field_name)) {
                            Some(field_data) => field_data,
                            _ => query_data
                                .get(field_name)
                                .ok_or(Error::InvalidExecutorResponse)?,
                        };

                        data.clone()
                    };

                    let field_type_name = match ctx.field(object_type.as_str(), field.name.as_str())
                    {
                        Some((_, field)) => field.field_type().name(),
                        _ => {
                            if field.name == "__schema" {
                                "__Schema"
                            } else {
                                map.insert(field_name.clone(), field_data.clone());
                                continue;
                            }
                        }
                    };

                    let data = resolve(
                        ctx.clone(),
                        field_type_name,
                        field.selection_set.items.clone(),
                        Some(field_data.clone()),
                    )
                    .await?;

                    map.insert(field_name.clone(), data.clone());
                }
            };
        }
        Ok(map.into())
    }
    .boxed()
}

fn query<'a, T: Into<String> + Send + 'a>(
    ctx: Context<'a>,
    object_type: T,
    object_data: Option<Value>,
    selections: Vec<Selection<'a, String>>,
) -> BoxFuture<'a, Result<Value>> {
    async move {
        let object_type = object_type.into();
        let first_data = object_data.as_ref().and_then(|v| match v {
            Value::Array(values) => values.first(),
            _ => Some(v),
        });
        let executors = resolve_executors(
            ctx.clone(),
            object_type.clone(),
            first_data.cloned(),
            selections.clone(),
        );

        let mut data = Map::new();

        for executor in executors {
            let result = resolve_executor(
                ctx.clone(),
                object_type.clone(),
                selections.clone(),
                executor.clone(),
            )?;

            let query_data = match object_type.as_str() {
                "Query" | "Mutation" => {
                    query_root(
                        ctx.clone(),
                        object_type.clone(),
                        result.clone(),
                        executor.clone(),
                    )
                    .await?
                }
                _ => {
                    let object_data = object_data
                        .as_ref()
                        .ok_or(Error::MissingFieldId(object_type.clone()))?;

                    query_node(
                        ctx.clone(),
                        object_type.clone(),
                        object_data.clone(),
                        result.clone(),
                        executor.clone(),
                    )
                    .await?
                }
            };

            data.extend(query_data);
        }

        return Ok(data.into());
    }
    .boxed()
}

async fn query_root<'a, T: Into<String>>(
    ctx: Context<'a>,
    object_type: T,
    resolve_info: ResolveInfo<'a>,
    executor: T,
) -> Result<Map<String, Value>> {
    let variable_definitions = resolve_info
        .variable_definitions
        .values()
        .cloned()
        .collect::<_>();
    let executor = executor.into();
    let object_type = object_type.into();
    let operation = match object_type.as_str() {
        "Query" => OperationDefinition::Query(Query {
            position: Pos::default(),
            name: ctx.operation_name.map(|v| v.to_owned()),
            variable_definitions,
            directives: vec![],
            selection_set: SelectionSet {
                span: (Pos::default(), Pos::default()),
                items: resolve_info.selections,
            },
        }),
        "Mutation" => OperationDefinition::Mutation(Mutation {
            position: Pos::default(),
            name: ctx.operation_name.map(|v| v.to_owned()),
            variable_definitions,
            directives: vec![],
            selection_set: SelectionSet {
                span: (Pos::default(), Pos::default()),
                items: resolve_info.selections,
            },
        }),
        _ => {
            return Err(Error::Query(vec![GraphQLError {
                pos: Pos::default(),
                err: QueryError::NotSupported,
            }]))
        }
    };

    let mut definitions = resolve_info
        .fragments
        .into_iter()
        .map(|(_, fragment)| Definition::Fragment(fragment))
        .collect::<Vec<Definition<'a, String>>>();

    definitions.push(Definition::Operation(operation));

    let document = Document { definitions };
    let query_source = document.to_string();

    let executor = ctx
        .executors
        .get(&executor)
        .ok_or(Error::UnknownExecutor(executor))?;

    executor
        .execute(
            ctx.ctx_data,
            query_source.as_str(),
            ctx.operation_name,
            ctx.variables.cloned(),
        )
        .await
}

async fn query_node<'a, T: Into<String>>(
    ctx: Context<'a>,
    object_type: T,
    object_data: Value,
    resolve_info: ResolveInfo<'a>,
    executor: T,
) -> Result<Map<String, Value>> {
    let var_name_node_ids = "__gql_gateway_ids";
    let object_type = object_type.into();
    let executor = executor.into();

    let field_id = resolve_info
        .selections
        .iter()
        .find_map(|selection| match selection {
            Selection::Field(field) => {
                if field.name == "id" {
                    Some(field.alias.as_ref().unwrap_or(&field.name).to_owned())
                } else {
                    None
                }
            }
            _ => None,
        })
        .unwrap_or("id".to_owned());

    let is_array = object_data.is_array();
    let ids = vec![object_data
        .get(&field_id)
        .ok_or(Error::MissingFieldId(object_type.clone()))?
        .clone()];
    let mut variable_definitions = resolve_info
        .variable_definitions
        .values()
        .cloned()
        .collect::<Vec<VariableDefinition<'a, String>>>();

    variable_definitions.push(VariableDefinition {
        var_type: AstType::NonNullType(Box::new(AstType::ListType(Box::new(AstType::NamedType(
            "ID".to_owned(),
        ))))),
        position: Pos::default(),
        name: var_name_node_ids.to_owned(),
        default_value: None,
    });

    let node_items = vec![Selection::InlineFragment(InlineFragment {
        position: Pos::default(),
        type_condition: Some(TypeCondition::On(object_type.to_owned())),
        directives: vec![],
        selection_set: SelectionSet {
            span: (Pos::default(), Pos::default()),
            items: resolve_info.selections,
        },
    })];

    let operation = OperationDefinition::Query(Query {
        position: Pos::default(),
        name: Some("NodeQuery".to_owned()),
        variable_definitions,
        directives: vec![],
        selection_set: SelectionSet {
            span: (Pos::default(), Pos::default()),
            items: vec![Selection::Field(Field {
                alias: None,
                arguments: vec![(
                    "ids".to_owned(),
                    AstValue::Variable(var_name_node_ids.to_owned()),
                )],
                directives: vec![],
                name: "nodes".to_owned(),
                position: Pos::default(),
                selection_set: SelectionSet {
                    span: (Pos::default(), Pos::default()),
                    items: node_items,
                },
            })],
        },
    });

    let mut variables = Map::new();
    variables.insert(var_name_node_ids.to_owned(), Value::Array(ids));

    if let Some(ctx_variables) = ctx.variables.and_then(|variables| variables.as_object()) {
        variables.extend(ctx_variables.clone());
    }

    let mut definitions = resolve_info
        .fragments
        .into_iter()
        .map(|(_, fragment)| Definition::Fragment(fragment))
        .collect::<Vec<Definition<'a, String>>>();

    definitions.push(Definition::Operation(operation));

    let document = Document { definitions };
    let query_source = document.to_string();

    let executor = ctx
        .executors
        .get(&executor)
        .ok_or(Error::UnknownExecutor(executor))?;

    let query_data = executor
        .execute(
            ctx.ctx_data,
            query_source.as_str(),
            Some("NodeQuery"),
            Some(variables.into()),
        )
        .await?;

    let nodes = query_data
        .get("nodes")
        .and_then(|nodes| nodes.as_array())
        .ok_or(Error::InvalidExecutorResponse)?;

    if !is_array {
        let node = nodes
            .get(0)
            .and_then(|node| node.as_object())
            .ok_or(Error::InvalidExecutorResponse)?;

        return Ok(node.clone());
    }
    unimplemented!()
}

fn resolve_executors<'a, T: Into<String>>(
    ctx: Context,
    object_type: T,
    data: Option<Value>,
    selections: Vec<Selection<'a, String>>,
) -> Vec<String> {
    let mut executors = vec![];
    let mut cache = HashMap::new();

    let object_type = object_type.into();
    for selection in selections {
        match selection {
            Selection::FragmentSpread(fragment_spread) => {
                let fragment = match ctx.fragments.get(&fragment_spread.fragment_name) {
                    Some(fragment) => fragment,
                    _ => continue,
                };

                let object_type = match &fragment.type_condition {
                    TypeCondition::On(v) => v,
                };

                let fragment_executors = resolve_executors(
                    ctx.clone(),
                    object_type.clone(),
                    data.clone(),
                    fragment.selection_set.items.clone(),
                )
                .into_iter()
                .filter(|executor| cache.get(executor.as_str()).is_none())
                .collect::<Vec<String>>();

                for fragment_executor in fragment_executors {
                    if cache.get(fragment_executor.as_str()).is_none() {
                        cache.insert(fragment_executor.clone(), true);
                        executors.push(fragment_executor);
                    }
                }
            }
            Selection::InlineFragment(inline_fragment) => {
                let type_condition = match inline_fragment.type_condition.as_ref() {
                    Some(type_condition) => type_condition,
                    _ => continue,
                };

                let object_type = match type_condition {
                    TypeCondition::On(v) => v,
                };

                let fragment_executors = resolve_executors(
                    ctx.clone(),
                    object_type.clone(),
                    data.clone(),
                    inline_fragment.selection_set.items.clone(),
                )
                .into_iter()
                .filter(|executor| cache.get(executor.as_str()).is_none())
                .collect::<Vec<String>>();

                for fragment_executor in fragment_executors {
                    if cache.get(fragment_executor.as_str()).is_none() {
                        cache.insert(fragment_executor.clone(), true);
                        executors.push(fragment_executor);
                    }
                }
            }
            Selection::Field(field) => {
                let object_type = object_type.clone();
                let (field_executor, schema_field) =
                    match ctx.field(object_type.as_str(), field.name.as_str()) {
                        Some(field_object_type) => field_object_type,
                        _ => continue,
                    };
                let field_type_name = schema_field.field_type().name();
                if ctx.interface(field_type_name).is_some() {
                    let field_executors = resolve_executors(
                        ctx.clone(),
                        field_type_name,
                        data.clone(),
                        field.selection_set.items.clone(),
                    );

                    for field_executor in field_executors {
                        if cache.get(field_executor.as_str()).is_none() {
                            cache.insert(field_executor.clone(), true);
                            executors.push(field_executor);
                        }
                    }

                    continue;
                }

                let field_name = field.alias.as_ref().unwrap_or(&field.name);
                let field_data = data.as_ref().and_then(|data| data.get(field_name));

                if cache.get(field_executor.as_str()).is_none() && field_data.is_none() {
                    cache.insert(field_executor.clone(), true);
                    executors.push(field_executor);
                }
            }
        };
    }

    executors
}

fn resolve_executor<'a, T: Into<String> + Send + 'a>(
    ctx: Context<'a>,
    object_type: T,
    selections: Vec<Selection<'a, String>>,
    executor: String,
) -> Result<ResolveInfo<'a>> {
    let mut items = vec![];
    let mut fragments = HashMap::new();
    let mut variable_definitions = HashMap::new();
    let mut errors = Vec::new();
    let object_type = object_type.into();

    let has_item_id = selections.iter().any(|selection| match selection {
        Selection::Field(field) => field.name == "id",
        _ => false,
    });

    if ctx.field(object_type.as_str(), "id").is_some() && !has_item_id {
        items.push(Selection::Field(Field {
            position: Pos::default(),
            alias: None,
            name: "id".to_owned(),
            arguments: vec![],
            directives: vec![],
            selection_set: SelectionSet {
                span: (Pos::default(), Pos::default()),
                items: vec![],
            },
        }));
    }

    for selection in selections {
        match selection {
            Selection::FragmentSpread(fragment_spread) => {
                let fragment = match ctx.fragments.get(&fragment_spread.fragment_name) {
                    Some(fragment) => fragment,
                    _ => {
                        errors.push(GraphQLError {
                            pos: fragment_spread.position,
                            err: QueryError::UnknownFragment {
                                name: fragment_spread.fragment_name,
                            },
                        });

                        continue;
                    }
                };

                let object_type = match &fragment.type_condition {
                    TypeCondition::On(v) => v,
                };

                let resolve_info = resolve_executor(
                    ctx.clone(),
                    object_type.clone(),
                    fragment.selection_set.items.clone(),
                    executor.clone(),
                )?;

                if resolve_info.selections.len() <= 1 {
                    continue;
                }

                items.push(Selection::FragmentSpread(fragment_spread));

                if fragments.contains_key(&fragment.name) {
                    continue;
                }

                let mut fragment = fragment.clone();
                fragment.selection_set.items = resolve_info.selections;
                fragments.insert(fragment.name.clone(), fragment);
                fragments.extend(resolve_info.fragments);
                variable_definitions.extend(resolve_info.variable_definitions);
            }
            Selection::InlineFragment(inline_fragment) => {
                let type_condition =
                    inline_fragment
                        .type_condition
                        .as_ref()
                        .ok_or(Error::Query(vec![GraphQLError {
                            pos: inline_fragment.position,
                            err: QueryError::MissingTypeConditionInlineFragment,
                        }]))?;

                let object_type = match type_condition {
                    TypeCondition::On(v) => v,
                };

                let resolve_info = resolve_executor(
                    ctx.clone(),
                    object_type.clone(),
                    inline_fragment.selection_set.items.clone(),
                    executor.clone(),
                )?;

                if resolve_info.selections.len() <= 1 {
                    continue;
                }

                let mut inline_fragment = inline_fragment.clone();
                inline_fragment.selection_set.items = resolve_info.selections;
                fragments.extend(resolve_info.fragments);
                variable_definitions.extend(resolve_info.variable_definitions);

                items.push(Selection::InlineFragment(inline_fragment));
            }
            Selection::Field(field) => {
                if field.name.as_str() == "id" {
                    items.push(Selection::Field(field));
                    continue;
                }

                let object_type = object_type.clone();
                let (mut field_executor, schema_field) =
                    match ctx.field(object_type.as_str(), field.name.as_str()) {
                        Some(schema_field) => schema_field,
                        _ => {
                            errors.push(GraphQLError {
                                pos: field.position,
                                err: QueryError::FieldNotFound {
                                    object: object_type.into(),
                                    name: field.name,
                                },
                            });
                            continue;
                        }
                    };

                let field_type_name = schema_field.field_type().name();

                if ctx.interface(field_type_name).is_some() {
                    field_executor = executor.clone();
                }

                if executor != field_executor {
                    continue;
                }

                let field_variable_definitions = field
                    .arguments
                    .iter()
                    .filter_map(|(name, argument)| match argument {
                        AstValue::Variable(variable) => {
                            let variable = ctx.variable_definitions.get(variable)?;
                            Some((name.clone(), variable.clone()))
                        }
                        _ => None,
                    })
                    .collect::<HashMap<String, VariableDefinition<'a, String>>>();

                variable_definitions.extend(field_variable_definitions);
                let mut field = field.clone();
                if !field.selection_set.items.is_empty() {
                    let result = resolve_executor(
                        ctx.clone(),
                        field_type_name,
                        field.selection_set.items.clone(),
                        field_executor,
                    )?;
                    field.selection_set.items = result.selections;
                    fragments.extend(result.fragments);
                    variable_definitions.extend(result.variable_definitions);
                }
                items.push(Selection::Field(field));
            }
        };
    }

    if !errors.is_empty() {
        return Err(Error::Query(errors));
    }

    Ok(ResolveInfo {
        selections: items,
        fragments,
        variable_definitions,
    })
}
