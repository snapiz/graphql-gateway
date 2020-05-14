use futures::future;
use std::collections::HashMap;
use std::fmt;

use super::error::{Error, Result};
use super::executor::Executor;
use super::introspection::Schema as IntrospectionSchema;
use super::schema::Schema;

#[derive(Clone)]
pub struct Gateway<'a> {
    pub executors: HashMap<String, Box<dyn Executor>>,
    pub schemas: HashMap<String, IntrospectionSchema>,
    pub schema: Option<Schema<'a>>,
}

impl<'a> Gateway<'a> {
    pub fn new() -> Self {
        Gateway {
            executors: HashMap::new(),
            schemas: HashMap::new(),
            schema: None,
        }
    }

    pub fn executor<T: Executor + 'static>(mut self, e: T) -> Self {
        self.executors.insert(e.name(), Box::new(e));
        self
    }

    pub async fn build(self) -> Result<Gateway<'a>> {
        let futures = self.executors.iter().map(|(_, e)| e.introspection());
        let schemas = future::join_all(futures)
            .await
            .iter()
            .filter_map(|e| Some(e.as_ref().ok().cloned()?))
            .collect::<HashMap<String, IntrospectionSchema>>();

        let schema: Schema = schemas.clone().into();

        if !schema.duplicate_object_fields.is_empty() {
            return Err(Error::DuplicateObjectFields(schema.duplicate_object_fields));
        }

        Ok(Gateway {
            schema: Some(schema),
            schemas,
            ..self
        })
    }

    pub async fn pull<T: Into<String>>(&mut self, name: T) -> Result<()> {
        let name = name.into();
        let executor = self
            .executors
            .get(&name)
            .ok_or(Error::UnknownExecutor(name))?;

        let (name, schema) = executor.introspection().await?;
        let mut schemas = self.schemas.clone();
        schemas.insert(name, schema);
        let schema: Schema = schemas.clone().into();

        if !schema.duplicate_object_fields.is_empty() {
            return Err(Error::DuplicateObjectFields(schema.duplicate_object_fields));
        }

        self.schemas = schemas;
        self.schema = Some(schema);

        Ok(())
    }

    pub fn validate<T: Into<String>>(
        &self,
        name: T,
        introspection_schema: IntrospectionSchema,
    ) -> Result<()> {
        let mut schemas = self.schemas.clone();
        schemas.insert(name.into(), introspection_schema);
        let schema: Schema = schemas.clone().into();

        if !schema.duplicate_object_fields.is_empty() {
            Err(Error::DuplicateObjectFields(schema.duplicate_object_fields))
        } else {
            Ok(())
        }
    }
}

impl<'a> fmt::Display for Gateway<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.schema.as_ref().expect("The schema does not exist.")
        )
    }
}
