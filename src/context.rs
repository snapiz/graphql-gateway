use fnv::FnvHashMap;
use graphql_parser::query::{FragmentDefinition, VariableDefinition};
use serde_json::Value;
use std::any::{Any, TypeId};
use std::collections::HashMap;

use super::executor::Executor;
use super::introspection::{Field, Type};
use super::schema::Schema;

#[derive(Default)]
pub struct Data(FnvHashMap<TypeId, Box<dyn Any + Sync + Send>>);

impl Data {
    #[allow(missing_docs)]
    pub fn insert<D: Any + Send + Sync>(&mut self, data: D) {
        self.0.insert(TypeId::of::<D>(), Box::new(data));
    }

    pub fn get<D: Any + Send + Sync>(&self) -> &D {
        self.get_opt::<D>()
            .expect("The specified data type does not exist.")
    }

    pub fn get_opt<D: Any + Send + Sync>(&self) -> Option<&D> {
        self.0
            .get(&TypeId::of::<D>())
            .and_then(|d| d.downcast_ref::<D>())
    }
}

#[derive(Clone)]
pub struct Context<'a> {
    pub schema: &'a Schema<'a>,
    pub executors: &'a HashMap<String, Box<dyn Executor>>,
    pub ctx_data: Option<&'a Data>,
    pub operation_name: Option<&'a str>,
    pub variables: Option<&'a Value>,
    pub fragments: HashMap<String, FragmentDefinition<'a, String>>,
    pub variable_definitions: HashMap<String, VariableDefinition<'a, String>>,
}

impl<'a> Context<'a> {
    pub fn object<T: Into<String>>(&self, name: T) -> Option<&'a Type> {
        self.schema.object(name)
    }

    pub fn interface<T: Into<String>>(&self, name: T) -> Option<&'a Type> {
        self.schema.interface(name)
    }

    pub fn field<T: Into<String>>(&self, object: T, name: T) -> Option<(String, &'a Field)> {
        self.schema.field(object, name)
    }
}
