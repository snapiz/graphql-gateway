use std::collections::HashMap;

use super::error::{Error, Result};
use super::executor::Executor;
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
