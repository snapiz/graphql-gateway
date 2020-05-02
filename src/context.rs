use graphql_parser::query::{FragmentDefinition, VariableDefinition};
use std::collections::HashMap;

use super::executor::Executor;
use super::graphql::Payload;
use super::schema::{Field, Schema, Type};

#[derive(Clone)]
pub struct Context<'a> {
    pub schema: &'a Schema,
    pub fragments: HashMap<&'a str, FragmentDefinition<'a, String>>,
    pub payload: &'a Payload,
    pub variable_definitions: Vec<VariableDefinition<'a, String>>,
    pub executors: HashMap<String, &'a (dyn Executor + 'a)>,
    pub types: HashMap<String, Type>,
    pub fields: HashMap<String, Field>,
    pub objects: HashMap<String, Type>,
}

impl<'a> Context<'a> {
    pub fn executor(&self, name: &str) -> Option<&dyn Executor> {
        self.executors.get(name).map(|_| self.executors[name])
    }

    pub fn field(&self, type_name: &str, field_name: &str) -> Option<&Field> {
        let key = format!("{}.{}", type_name, field_name);

        self.fields.get(&key)
    }

    pub fn object(&self, type_name: &str, executor_name: &str) -> Option<&Type> {
        let key = format!("{}.{}", executor_name, type_name);

        self.objects.get(&key)
    }

    pub fn object_type(&self, key: &str) -> Option<&Type> {
        self.types.get(key)
    }
}
