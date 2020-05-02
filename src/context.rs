use graphql_parser::query::{Definition, Document, FragmentDefinition, VariableDefinition};
use std::collections::HashMap;

use super::executor::Executor;
use super::gateway::Gateway;
use super::graphql::Payload;
use super::schema::{Field, Type};

#[derive(Clone)]
pub struct Context<'a> {
    pub gateway: &'a Gateway<'a>,
    pub fragments: HashMap<&'a str, FragmentDefinition<'a, String>>,
    pub payload: &'a Payload,
    pub variable_definitions: Vec<VariableDefinition<'a, String>>,
}

impl<'a> Context<'a> {
    pub fn new(
        gateway: &'a Gateway,
        payload: &'a Payload,
        query: &'a Document<'a, String>,
        variable_definitions: Vec<VariableDefinition<'a, String>>,
    ) -> Self {
        let mut fragments = HashMap::new();

        for definition in &query.definitions {
            if let Definition::Fragment(fragment) = definition {
                fragments.insert(fragment.name.as_str(), fragment.clone());
            }
        }

        Context {
            gateway,
            fragments,
            payload,
            variable_definitions,
        }
    }

    pub fn executor(&self, name: &str) -> Option<&dyn Executor> {
        self.gateway
            .executors
            .get(name)
            .map(|_| self.gateway.executors[name])
    }

    pub fn field(&self, type_name: &str, field_name: &str) -> Option<&Field> {
        let key = format!("{}.{}", type_name, field_name);

        self.gateway.fields.get(&key)
    }

    pub fn object(&self, type_name: &str, executor_name: &str) -> Option<&Type> {
        let key = format!("{}.{}", executor_name, type_name);

        self.gateway.objects.get(&key)
    }

    pub fn object_type(&self, key: &str) -> Option<&Type> {
        self.gateway.types.get(key)
    }
}
