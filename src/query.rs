use crate::context::Context;
use crate::data::Data;
use crate::gateway::Gateway;
use crate::schema::Type;
use futures::future::{BoxFuture, FutureExt};
use graphql_parser::query::{
    Definition, Document, Field, FragmentDefinition, InlineFragment, Mutation, OperationDefinition,
    ParseError as QueryParseError, Query, Selection, SelectionSet, Type as AstType, TypeCondition,
    Value as AstValue, VariableDefinition,
};
use graphql_parser::Pos;
use serde_json::{Map, Value};
use std::any::Any;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct ResolveInfo<'a> {
    selections: Vec<Selection<'a, String>>,
    fragments: HashMap<String, FragmentDefinition<'a, String>>,
    variable_definitions: HashMap<String, VariableDefinition<'a, String>>,
}

#[derive(Debug)]
pub struct QueryPosError(pub Pos, pub QueryError);

#[derive(Debug, Error)]
pub enum QueryError {
    #[error("Not supported.")]
    NotSupported,
    #[error("Schema is not configured for queries.")]
    NotConfiguredQueries,
    #[error("Schema is not configured for mutations.")]
    NotConfiguredMutations,
    #[error("Cannot query field \"{1}\" on type \"{0}\".")]
    FieldNotFound(String, String),
    #[error("Cannot get field data \"{1}\" on type \"{0}\".")]
    FieldDataNotFound(String, String),
    #[error("Cannot query field \"id\" on type \"{0}\".")]
    FieldIdNotFound(String),
    #[error("\"__typename\" must be an existing string")]
    TypeNameNotExists(String),
    #[error("Missing type condition on inline fragment.")]
    MissingTypeConditionInlineFragment,
    #[error("Unknown fragment \"{0}\".")]
    UnknownFragment(String),
    #[error("Unknown executor \"{0}\".")]
    UnknownExecutor(String),
    #[error("Invalid executor response")]
    InvalidExecutorResponse,
    #[error("Executor error: {0}")]
    Executor(Value),
    #[error("Parse error: {0}")]
    QueryParse(QueryParseError),
    #[error("Query errors.")]
    Errors(Vec<QueryPosError>),
    #[error("{0}")]
    Custom(String),
}

impl From<QueryParseError> for QueryError {
    fn from(e: QueryParseError) -> QueryError {
        QueryError::QueryParse(e)
    }
}

impl From<String> for QueryError {
    fn from(e: String) -> QueryError {
        QueryError::Custom(e)
    }
}

pub type QueryResult<T> = Result<T, QueryError>;

pub struct QueryBuilder {
    pub(crate) query_source: String,
    pub(crate) operation_name: Option<String>,
    pub(crate) variables: Option<Value>,
    pub(crate) ctx_data: Option<Data>,
}

impl QueryBuilder {
    pub fn new<T: Into<String>>(source: T) -> Self {
        QueryBuilder {
            query_source: source.into(),
            operation_name: None,
            variables: None,
            ctx_data: None,
        }
    }

    pub fn operation_name<T: Into<String>>(mut self, e: T) -> Self {
        self.operation_name = Some(e.into());
        self
    }

    pub fn variables(mut self, e: Value) -> Self {
        self.variables = Some(e);
        self
    }

    pub fn data<T: Any + Sync + Send>(mut self, e: T) -> Self {
        if let Some(ctx_data) = &mut self.ctx_data {
            ctx_data.insert(e);
        } else {
            let mut ctx_data = Data::default();
            ctx_data.insert(e);
            self.ctx_data = Some(ctx_data);
        }
        self
    }

    pub async fn execute(&self, gateway: &Gateway<'_>) -> QueryResult<Value> {
        let document = graphql_parser::parse_query::<String>(&self.query_source)?;

        let fragments = document
            .definitions
            .iter()
            .filter_map(|definition| match definition {
                Definition::Fragment(fragment) => Some((fragment.name.clone(), fragment.clone())),
                _ => None,
            })
            .collect::<HashMap<String, FragmentDefinition<'_, String>>>();

        let (object_type_name, selections, variable_definitions) = document
            .definitions
            .iter()
            .find_map(|definition| match definition {
                Definition::Operation(operation) => match operation {
                    OperationDefinition::SelectionSet(selection_set) => {
                        Some(("Query", selection_set.items.clone(), vec![]))
                    }
                    OperationDefinition::Query(query) => Some((
                        "Query",
                        query.selection_set.items.clone(),
                        query.variable_definitions.clone(),
                    )),
                    OperationDefinition::Mutation(mutation) => Some((
                        "Mutation",
                        mutation.selection_set.items.clone(),
                        mutation.variable_definitions.clone(),
                    )),
                    _ => None,
                },
                _ => None,
            })
            .ok_or(QueryError::NotSupported)?;

        let variable_definitions = variable_definitions
            .iter()
            .map(|variable_definition| {
                (
                    variable_definition.name.clone(),
                    variable_definition.clone(),
                )
            })
            .collect();

        let context = Context {
            gateway,
            data: self.ctx_data.as_ref(),
            operation_name: self.operation_name.as_ref().map(|e| e.as_str()),
            variables: self.variables.as_ref(),
            fragments,
            variable_definitions,
        };

        let object_type = match context.object(object_type_name) {
            Some(object_type) => object_type,
            _ => {
                let err = match object_type_name {
                    "Query" => QueryError::NotConfiguredQueries,
                    "Mutation" => QueryError::NotConfiguredMutations,
                    _ => QueryError::NotSupported,
                };

                return Err(err);
            }
        };

        let data = get_root_data(&context, object_type, &selections).await?;

        Ok(resolve(&context, object_type, data, &selections).await?)
    }
}

fn resolve<'a, 'b>(
    context: &'a Context<'a, 'b>,
    object_type: &'a Type,
    data: Value,
    selections: &'a [Selection<'a, String>],
) -> BoxFuture<'a, QueryResult<Value>> {
    async move {
        if data.is_null() || selections.is_empty() {
            return Ok(data.clone());
        }

        if let Value::Array(values) = &data {
            if values.is_empty() {
                return Ok(data.clone());
            }
        }

        let data = get_node_data(context, object_type, &data, selections).await?;

        if let Value::Array(values) = &data {
            let futures = values
                .iter()
                .map(|value| resolve(context, object_type, value.clone(), selections))
                .collect::<Vec<BoxFuture<'a, QueryResult<Value>>>>();

            let values = futures::future::try_join_all(futures).await?;
            return Ok(Value::Array(values));
        }

        let mut errors = Vec::new();
        let mut map = Map::new();

        for selection in selections {
            match selection {
                Selection::Field(field) => {
                    let field_name = field.alias.as_ref().unwrap_or(&field.name);
                    let (field_type, field_data) = if field.name == "__schema" {
                        (context.object("__Schema"), Some(context.schema_data()))
                    } else {
                        let field_type = context
                            .field_object_type(object_type, field.name.as_str())
                            .map(|(_, field_type)| field_type);
                        (field_type, data.get(&field_name))
                    };

                    let field_data = match field_data {
                        Some(field_data) => field_data,
                        _ => {
                            errors.push(QueryPosError(
                                field.position,
                                QueryError::FieldDataNotFound(
                                    object_type.name().to_owned(),
                                    field_name.to_string(),
                                ),
                            ));
                            continue;
                        }
                    };

                    let field_type = match field_type {
                        Some(field_type) => field_type,
                        _ => {
                            map.insert(field_name.clone(), field_data.clone());
                            continue;
                        }
                    };

                    let data = resolve(
                        context,
                        field_type,
                        field_data.clone(),
                        &field.selection_set.items,
                    )
                    .await?;

                    map.insert(field_name.clone(), data.clone());
                }
                Selection::FragmentSpread(fragment_spread) => {
                    let fragment = match context.fragments.get(&fragment_spread.fragment_name) {
                        Some(fragment) => fragment,
                        _ => {
                            errors.push(QueryPosError(
                                fragment_spread.position,
                                QueryError::UnknownFragment(fragment_spread.fragment_name.clone()),
                            ));
                            continue;
                        }
                    };

                    let object_type = match &fragment.type_condition {
                        TypeCondition::On(v) => match context.object(v) {
                            Some(object_type) => object_type,
                            _ => {
                                errors.push(QueryPosError(
                                    fragment_spread.position,
                                    QueryError::TypeNameNotExists(v.to_string()),
                                ));
                                continue;
                            }
                        },
                    };

                    let data = resolve(
                        context,
                        object_type,
                        data.clone(),
                        &fragment.selection_set.items,
                    )
                    .await?;

                    if let Value::Object(object) = data {
                        map.extend(object);
                    }
                }
                Selection::InlineFragment(inline_fragment) => {
                    let type_condition = match inline_fragment.type_condition.as_ref() {
                        Some(type_condition) => type_condition,
                        _ => {
                            errors.push(QueryPosError(
                                inline_fragment.position,
                                QueryError::MissingTypeConditionInlineFragment,
                            ));
                            continue;
                        }
                    };

                    let object_type = match type_condition {
                        TypeCondition::On(v) => match context.object(v) {
                            Some(object_type) => object_type,
                            _ => {
                                errors.push(QueryPosError(
                                    inline_fragment.position,
                                    QueryError::TypeNameNotExists(v.to_string()),
                                ));
                                continue;
                            }
                        },
                    };

                    let data = resolve(
                        context,
                        object_type,
                        data.clone(),
                        &inline_fragment.selection_set.items,
                    )
                    .await?;

                    if let Value::Object(object) = data {
                        map.extend(object);
                    }
                }
            };
        }

        if errors.is_empty() {
            Ok(map.into())
        } else {
            Err(QueryError::Errors(errors))
        }
    }
    .boxed()
}

async fn get_root_data<'a, 'b>(
    context: &'a Context<'a, 'b>,
    object_type: &'a Type,
    selections: &'a [Selection<'a, String>],
) -> QueryResult<Value> {
    let mut map = Map::new();
    let executors = resolve_executors(context, object_type, None, selections)?;

    for executor in executors {
        let result = resolve_executor(context, object_type, selections.to_vec(), executor.clone())?;
        let data = get_executor_root_data(context, object_type, result, executor).await?;

        merge_object(&mut map, data);
    }

    Ok(map.into())
}

async fn get_executor_root_data<'a, 'b, T: Into<String>>(
    context: &'a Context<'a, 'b>,
    object_type: &'a Type,
    resolve_info: ResolveInfo<'a>,
    executor: T,
) -> QueryResult<Map<String, Value>> {
    let variable_definitions = resolve_info
        .variable_definitions
        .values()
        .cloned()
        .collect::<_>();
    let executor = executor.into();
    let operation = match object_type.name() {
        "Query" => OperationDefinition::Query(Query {
            position: Pos::default(),
            name: context.operation_name.map(|v| v.to_owned()),
            variable_definitions,
            directives: vec![],
            selection_set: SelectionSet {
                span: (Pos::default(), Pos::default()),
                items: resolve_info.selections,
            },
        }),
        "Mutation" => OperationDefinition::Mutation(Mutation {
            position: Pos::default(),
            name: context.operation_name.map(|v| v.to_owned()),
            variable_definitions,
            directives: vec![],
            selection_set: SelectionSet {
                span: (Pos::default(), Pos::default()),
                items: resolve_info.selections,
            },
        }),
        _ => unreachable!(),
    };

    let mut definitions = resolve_info
        .fragments
        .into_iter()
        .map(|(_, fragment)| Definition::Fragment(fragment))
        .collect::<Vec<Definition<'a, String>>>();

    definitions.push(Definition::Operation(operation));

    let document = Document { definitions };
    let query_source = document.to_string();

    let executor = context
        .executor(&executor)
        .ok_or(QueryError::UnknownExecutor(executor))?;

    let res = executor
        .execute(
            context.data,
            query_source,
            context.operation_name.map(|e| e.to_owned()),
            context.variables.cloned(),
        )
        .await?;

    check_executor_response(res)
}

async fn get_node_data<'a, 'b>(
    context: &Context<'a, 'b>,
    object_type: &'a Type,
    data: &Value,
    selections: &'a [Selection<'a, String>],
) -> QueryResult<Value> {
    if !object_type.is_node() {
        return Ok(data.clone());
    }

    let mut map = Map::new();

    let first_data = match data {
        Value::Array(values) => values.first(),
        _ => Some(data),
    };

    let executors = resolve_executors(context, object_type, first_data, selections)?;

    if executors.is_empty() {
        return Ok(data.clone());
    }

    for executor in executors {
        let result = resolve_executor(context, object_type, selections.to_vec(), executor.clone())?;
        let node_data =
            get_executor_node_data(context, object_type, data, result, executor).await?;

        merge_object(&mut map, node_data);
    }

    let res = if data.is_array() {
        map.get("nodes")
    } else {
        map.get("nodes").and_then(|nodes| nodes.get(0))
    };

    let node_data = res.ok_or(QueryError::InvalidExecutorResponse)?;
    let mut data = data.clone();

    merge_value(&mut data, node_data);

    Ok(data)
}

async fn get_executor_node_data<'a, 'b, T: Into<String>>(
    context: &Context<'a, 'b>,
    object_type: &Type,
    data: &Value,
    resolve_info: ResolveInfo<'a>,
    executor: T,
) -> QueryResult<Map<String, Value>> {
    let var_name_node_ids = "__gql_gateway_ids";
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
        .unwrap_or_else(|| "id".to_owned());

    let ids = match data {
        Value::Array(values) => {
            let mut ids = Vec::new();

            for value in values {
                ids.push(
                    value
                        .get(&field_id)
                        .ok_or_else(|| QueryError::FieldIdNotFound(object_type.name().to_owned()))?
                        .clone(),
                );
            }

            ids
        }
        _ => vec![data
            .get(&field_id)
            .ok_or_else(|| QueryError::FieldIdNotFound(object_type.name().to_owned()))?
            .clone()],
    };

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
        type_condition: Some(TypeCondition::On(object_type.name().to_owned())),
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

    if let Some(ctx_variables) = context
        .variables
        .and_then(|variables| variables.as_object())
    {
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

    let executor = context
        .executor(&executor)
        .ok_or(QueryError::UnknownExecutor(executor))?;

    let res = executor
        .execute(
            context.data,
            query_source,
            Some("NodeQuery".to_owned()),
            Some(variables.into()),
        )
        .await?;

    check_executor_response(res)
}

fn check_executor_response(res: Value) -> QueryResult<Map<String, Value>> {
    if res.get("errors").is_some() {
        Err(QueryError::Executor(res))
    } else {
        Ok(res
            .get("data")
            .ok_or(QueryError::InvalidExecutorResponse)?
            .as_object()
            .cloned()
            .ok_or(QueryError::InvalidExecutorResponse)?)
    }
}

fn resolve_executors<'a, 'b>(
    context: &Context<'a, 'b>,
    object_type: &Type,
    data: Option<&Value>,
    selections: &[Selection<'a, String>],
) -> QueryResult<Vec<String>> {
    let mut executors = vec![];
    let mut cache = HashMap::new();
    let mut errors = Vec::new();

    for selection in selections {
        match selection {
            Selection::Field(field) => {
                if field.name.starts_with("__") {
                    continue;
                }

                let (field_executor, field_type) =
                    match context.field_object_type(object_type, &field.name) {
                        Some(field_type) => field_type,
                        _ => {
                            errors.push(QueryPosError(
                                field.position,
                                QueryError::FieldNotFound(
                                    object_type.name().to_owned(),
                                    field.name.clone(),
                                ),
                            ));
                            continue;
                        }
                    };

                if field_type.is_interface() {
                    let field_executors =
                        resolve_executors(context, field_type, data, &field.selection_set.items)?;

                    for field_executor in field_executors {
                        if !cache.contains_key(&field_executor) {
                            cache.insert(field_executor.clone(), true);
                            executors.push(field_executor);
                        }
                    }

                    continue;
                }

                let field_name = field.alias.as_ref().unwrap_or(&field.name);
                let field_data = data.as_ref().and_then(|data| data.get(field_name));

                if !cache.contains_key(&field_executor) && field_data.is_none() {
                    cache.insert(field_executor.clone(), true);
                    executors.push(field_executor);
                }
            }
            Selection::FragmentSpread(fragment_spread) => {
                let fragment = match context.fragments.get(&fragment_spread.fragment_name) {
                    Some(fragment) => fragment,
                    _ => {
                        errors.push(QueryPosError(
                            fragment_spread.position,
                            QueryError::UnknownFragment(fragment_spread.fragment_name.clone()),
                        ));
                        continue;
                    }
                };

                let object_type = match &fragment.type_condition {
                    TypeCondition::On(v) => match context.object(v) {
                        Some(object_type) => object_type,
                        _ => {
                            errors.push(QueryPosError(
                                fragment_spread.position,
                                QueryError::TypeNameNotExists(v.to_string()),
                            ));
                            continue;
                        }
                    },
                };

                let fragment_executors =
                    resolve_executors(context, object_type, data, &fragment.selection_set.items)?
                        .into_iter()
                        .filter(|executor| !cache.contains_key(executor.as_str()))
                        .collect::<Vec<String>>();

                for fragment_executor in fragment_executors {
                    if !cache.contains_key(&fragment_executor) {
                        cache.insert(fragment_executor.clone(), true);
                        executors.push(fragment_executor);
                    }
                }
            }
            Selection::InlineFragment(inline_fragment) => {
                let type_condition = match inline_fragment.type_condition.as_ref() {
                    Some(type_condition) => type_condition,
                    _ => {
                        errors.push(QueryPosError(
                            inline_fragment.position,
                            QueryError::MissingTypeConditionInlineFragment,
                        ));
                        continue;
                    }
                };

                let object_type = match type_condition {
                    TypeCondition::On(v) => match context.object(v) {
                        Some(object_type) => object_type,
                        _ => {
                            errors.push(QueryPosError(
                                inline_fragment.position,
                                QueryError::TypeNameNotExists(v.to_string()),
                            ));
                            continue;
                        }
                    },
                };

                let fragment_executors = resolve_executors(
                    context,
                    object_type,
                    data,
                    &inline_fragment.selection_set.items,
                )?
                .into_iter()
                .filter(|executor| !cache.contains_key(executor.as_str()))
                .collect::<Vec<String>>();

                for fragment_executor in fragment_executors {
                    if !cache.contains_key(&fragment_executor) {
                        cache.insert(fragment_executor.clone(), true);
                        executors.push(fragment_executor);
                    }
                }
            }
        };
    }

    if errors.is_empty() {
        Ok(executors)
    } else {
        Err(QueryError::Errors(errors))
    }
}

fn resolve_executor<'a, 'b>(
    context: &Context<'a, 'b>,
    object_type: &Type,
    selections: Vec<Selection<'a, String>>,
    executor: String,
) -> QueryResult<ResolveInfo<'a>> {
    let mut items = vec![];
    let mut fragments = HashMap::new();
    let mut variable_definitions = HashMap::new();
    let mut errors = Vec::new();

    if !selections.is_empty() && object_type.is_node() {
        let selection_field_id = selections
            .iter()
            .find_map(|selection| match selection {
                Selection::Field(field) => {
                    if field.name == "id" {
                        Some(field.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .unwrap_or(Field {
                position: Pos::default(),
                alias: None,
                name: "id".to_owned(),
                arguments: vec![],
                directives: vec![],
                selection_set: SelectionSet {
                    span: (Pos::default(), Pos::default()),
                    items: vec![],
                },
            });

        items.push(Selection::Field(selection_field_id));
    }

    for selection in selections {
        match selection {
            Selection::Field(field) => {
                if field.name == "id" {
                    continue;
                }

                let (mut field_executor, field_type) =
                    match context.field_object_type(object_type, field.name.as_str()) {
                        Some(field_type) => field_type,
                        _ => {
                            errors.push(QueryPosError(
                                field.position,
                                QueryError::FieldNotFound(
                                    object_type.name().to_owned(),
                                    field.name.clone(),
                                ),
                            ));
                            continue;
                        }
                    };

                if field_type.is_interface() {
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
                            let variable = context.variable_definitions.get(variable)?;
                            Some((name.clone(), variable.clone()))
                        }
                        _ => None,
                    })
                    .collect::<HashMap<String, VariableDefinition<'a, String>>>();

                let mut field = field.clone();
                if !field.selection_set.items.is_empty() {
                    let result = resolve_executor(
                        context,
                        field_type,
                        field.selection_set.items,
                        field_executor,
                    )?;

                    if result.selections.is_empty() && result.fragments.is_empty() {
                        continue;
                    }

                    field.selection_set.items = result.selections;
                    fragments.extend(result.fragments);
                    variable_definitions.extend(result.variable_definitions);
                }
                variable_definitions.extend(field_variable_definitions);
                items.push(Selection::Field(field));
            }
            Selection::FragmentSpread(fragment_spread) => {
                let fragment = match context.fragments.get(&fragment_spread.fragment_name) {
                    Some(fragment) => fragment,
                    _ => {
                        errors.push(QueryPosError(
                            fragment_spread.position,
                            QueryError::UnknownFragment(fragment_spread.fragment_name.clone()),
                        ));
                        continue;
                    }
                };

                let object_type = match &fragment.type_condition {
                    TypeCondition::On(v) => match context.object(v) {
                        Some(object_type) => object_type,
                        _ => {
                            errors.push(QueryPosError(
                                fragment_spread.position,
                                QueryError::TypeNameNotExists(v.to_string()),
                            ));
                            continue;
                        }
                    },
                };

                let resolve_info = resolve_executor(
                    context,
                    object_type,
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
                let type_condition = match inline_fragment.type_condition.as_ref() {
                    Some(type_condition) => type_condition,
                    _ => {
                        errors.push(QueryPosError(
                            inline_fragment.position,
                            QueryError::MissingTypeConditionInlineFragment,
                        ));
                        continue;
                    }
                };

                let object_type = match type_condition {
                    TypeCondition::On(v) => match context.object(v) {
                        Some(object_type) => object_type,
                        _ => {
                            errors.push(QueryPosError(
                                inline_fragment.position,
                                QueryError::TypeNameNotExists(v.to_string()),
                            ));
                            continue;
                        }
                    },
                };

                let resolve_info = resolve_executor(
                    context,
                    object_type,
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
        };
    }

    if errors.is_empty() {
        Ok(ResolveInfo {
            selections: items,
            fragments,
            variable_definitions,
        })
    } else {
        Err(QueryError::Errors(errors))
    }
}

fn merge_object(a: &mut Map<String, Value>, b: Map<String, Value>) {
    for (key, value) in b {
        match a.get_mut(&key) {
            Some(v) => {
                merge_value(v, &value);
            }
            _ => {
                a.insert(key, value);
            }
        };
    }
}

fn merge_value(a: &mut Value, b: &Value) {
    match (a, b) {
        (Value::Object(a_object), Value::Object(b_object)) => a_object.extend(b_object.clone()),
        (Value::Array(a_values), Value::Array(b_values)) => {
            for (i, a_value) in a_values.iter_mut().enumerate() {
                let b_value = match b_values.get(i) {
                    Some(b_value) => b_value,
                    _ => continue,
                };

                match (a_value, b_value) {
                    (Value::Object(a_object), Value::Object(b_object)) => {
                        a_object.extend(b_object.clone())
                    }
                    (a_value, _) => *a_value = Value::Null,
                };
            }
        }
        (a, b) => *a = b.clone(),
    };
}
