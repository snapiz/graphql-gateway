use crate::data::Data;
use crate::executor::Executor;
use crate::gateway::Gateway;
use crate::schema::{Field, Type, TypeKind};
use graphql_parser::query::{FragmentDefinition, VariableDefinition};
use serde_json::Value;
use std::collections::HashMap;

pub struct Context<'a, 'b> {
    pub gateway: &'a Gateway<'b>,
    pub operation_name: Option<&'a str>,
    pub variables: Option<&'a Value>,
    pub data: Option<&'a Data>,
    pub fragments: HashMap<String, FragmentDefinition<'a, String>>,
    pub variable_definitions: HashMap<String, VariableDefinition<'a, String>>,
}

impl<'b> Context<'_, 'b> {
    pub fn schema_data(&self) -> &Value {
        &self.gateway.schema.1
    }

    pub fn executor(&self, name: &str) -> Option<&Box<dyn Executor>> {
        self.gateway.executors.get(name)
    }

    pub fn object_by_kind<T: Into<String>>(&self, kind: &TypeKind, name: T) -> Option<&Type> {
        self.gateway
            .schema
            .2
            .get(&format!("{}.{}", kind, name.into()))
            .and_then(|&i| self.gateway.schema.0.types.get(i))
    }

    pub fn object<T: Into<String>>(&self, name: T) -> Option<&Type> {
        self.object_by_kind(&TypeKind::Object, name)
    }

    pub fn field<T: Into<String>>(&self, object: &Type, name: T) -> Option<(String, &Field)> {
        let fields = self
            .object_by_kind(&object.kind, object.name())
            .and_then(|object| object.fields.as_ref())?;

        self.gateway
            .schema
            .3
            .get(&format!("{}.{}", object, name.into()))
            .and_then(|(name, i)| fields.get(*i).map(|field| (name.clone(), field)))
    }

    pub fn field_object_type<T: Into<String>>(
        &self,
        object: &Type,
        name: T,
    ) -> Option<(String, &Type)> {
        let (executor, field) = self.field(object, name)?;
        let field_type = field.field_type();

        self.object_by_kind(&field_type.kind, field_type.name())
            .map(|object_type| (executor, object_type))
    }
}
