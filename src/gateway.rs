use crate::executor::Executor;
use crate::schema::{Schema, Type, TypeKind};
use futures::future;
use graphql_parser::schema::{Definition, Document, SchemaDefinition};
use graphql_parser::Pos;
use serde_json::{Error as JsonError, Value};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("Json error: {0}")]
    Json(JsonError),
    #[error("{0}")]
    Custom(String),
    #[error("Unknown executor \"{0}\"")]
    UnknownExecutor(String),
    #[error("Duplicate object fields: {0:#?}")]
    DuplicateObjectFields(Vec<(String, String, String)>),
}

impl From<String> for GatewayError {
    fn from(e: String) -> GatewayError {
        GatewayError::Custom(e)
    }
}

impl From<JsonError> for GatewayError {
    fn from(e: JsonError) -> GatewayError {
        GatewayError::Json(e)
    }
}

pub type GatewayResult<T> = Result<T, GatewayError>;

#[derive(Clone, Default)]
pub struct Gateway<'a> {
    pub executors: HashMap<String, Box<dyn Executor>>,
    pub(crate) introspections: HashMap<String, Schema>,
    pub(crate) schema: GatewaySchema,
    pub(crate) document: Document<'a, String>,
}

impl<'a> Gateway<'a> {
    pub fn executor<E: Executor + 'static>(mut self, e: E) -> Self {
        self.executors.insert(e.name().to_owned(), Box::new(e));
        self
    }

    pub async fn build(mut self) -> GatewayResult<Gateway<'a>> {
        let futures = self.executors.iter().map(|(_, e)| e.introspect());

        self.introspections = future::join_all(futures)
            .await
            .iter()
            .filter_map(|e| Some(e.as_ref().ok().cloned()?))
            .collect::<HashMap<String, Schema>>();

        self.schema = create_schema(&self.introspections)?;
        self.document = create_document(&self.schema.0);

        Ok(self)
    }

    pub async fn pull<T: Into<String>>(&mut self, name: T) -> GatewayResult<()> {
        let name = name.into();
        let executor = self
            .executors
            .get(&name)
            .ok_or(GatewayError::UnknownExecutor(name))?;

        let (name, schema) = executor.introspect().await?;

        let mut introspections = self.introspections.clone();
        introspections.insert(name, schema);
        self.schema = create_schema(&introspections)?;
        self.document = create_document(&self.schema.0);
        self.introspections = introspections;

        Ok(())
    }

    pub fn validate<T: Into<String>>(&self, name: T, schema: Schema) -> GatewayResult<()> {
        let mut introspections = self.introspections.clone();
        introspections.insert(name.into(), schema);
        create_schema(&introspections)?;

        Ok(())
    }
}

impl fmt::Display for Gateway<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.document)
    }
}

#[derive(Default, Clone)]
pub struct GatewaySchema(
    pub(crate) Schema,
    pub(crate) Value,
    pub(crate) HashMap<String, usize>,
    pub(crate) HashMap<String, (String, usize)>,
);

fn create_schema(schemas: &HashMap<String, Schema>) -> GatewayResult<GatewaySchema> {
    let mut types = vec![];
    let mut types_by_name = HashMap::new();
    let mut type_fields_by_name: HashMap<String, (String, usize)> = HashMap::new();
    let mut duplicate_object_fields = Vec::new();
    let mut possible_types_by_name = HashMap::new();

    for (executor_name, schema) in schemas {
        for schema_type in schema.types.iter() {
            let key = schema_type.to_string();
            let current_type = types_by_name.get(&key).and_then(|&i| types.get_mut(i));

            let current_type = match current_type {
                Some(current_type) => current_type,
                _ => {
                    types_by_name.insert(key.clone(), types.len());

                    let mut schema_type = schema_type.clone();
                    schema_type.fields = None;
                    schema_type.possible_types = None;

                    types.push(schema_type);
                    types
                        .last_mut()
                        .expect("Unexpected behavior when getting last definition")
                }
            };

            if let Some(possible_types) = &schema_type.possible_types {
                let mut current_possible_types = current_type
                    .possible_types
                    .clone()
                    .unwrap_or_else(|| vec![]);

                for possible_type in possible_types {
                    let possible_type_key = format!("{}.{:#?}", key.clone(), possible_type.name());

                    if possible_types_by_name.get(&possible_type_key).is_some() {
                        continue;
                    }

                    possible_types_by_name
                        .insert(possible_type_key.clone(), (executor_name.clone(), true));
                    current_possible_types.push(possible_type.clone());
                }

                current_type.possible_types = Some(current_possible_types);
            }

            if let Some(fields) = &schema_type.fields {
                let mut current_fields = current_type.fields.clone().unwrap_or_else(|| vec![]);

                for field in fields {
                    let field_key = format!("{}.{}", key, &field.name);

                    match type_fields_by_name.get(&field_key) {
                        Some((current_executor_name, _)) => {
                            let field_type = field.field_type();

                            if field_type.name() == "ID"
                                || current_type.kind != TypeKind::Object
                                || field_type.kind == TypeKind::Interface
                                || schema_type.name().starts_with("__")
                            {
                                continue;
                            }

                            duplicate_object_fields.push((
                                current_executor_name.clone(),
                                executor_name.clone(),
                                field_key,
                            ));
                        }
                        _ => {
                            type_fields_by_name
                                .insert(field_key, (executor_name.clone(), current_fields.len()));
                            current_fields.push(field.clone());
                        }
                    }
                }

                current_type.fields = Some(current_fields);
            }
        }
    }

    if !duplicate_object_fields.is_empty() {
        return Err(GatewayError::DuplicateObjectFields(duplicate_object_fields));
    }

    let query_type = types_by_name.get("Object.Query").map(|_| Type {
        kind: TypeKind::Object,
        name: Some("Query".to_owned()),
        ..Type::default()
    });

    let mutation_type = types_by_name.get("Object.Mutation").map(|_| Type {
        kind: TypeKind::Object,
        name: Some("Mutation".to_owned()),
        ..Type::default()
    });

    let schema = Schema {
        query_type,
        mutation_type,
        types,
        ..Schema::default()
    };

    let schema_value = serde_json::to_value(schema.clone())?;

    Ok(GatewaySchema(
        schema,
        schema_value,
        types_by_name,
        type_fields_by_name,
    ))
}

fn create_document<'a>(schema: &Schema) -> Document<'a, String> {
    let query = if schema.types.iter().any(|t| t.name() == "Query") {
        Some("Query".to_owned())
    } else {
        None
    };

    let mutation = if schema.types.iter().any(|t| t.name() == "Mutation") {
        Some("Mutation".to_owned())
    } else {
        None
    };

    let mut definitions = schema
        .types
        .iter()
        .filter_map(|t| {
            if t.name().starts_with("__") || t.kind == TypeKind::Scalar {
                None
            } else {
                Some(t.clone().into())
            }
        })
        .collect::<Vec<Definition<'a, String>>>();

    definitions.push(Definition::SchemaDefinition(SchemaDefinition {
        position: Pos::default(),
        directives: vec![],
        query,
        mutation,
        subscription: None,
    }));

    Document { definitions }
}
