use graphql_parser::schema::{
  Definition, Document, EnumType, EnumValue, Field, InputObjectType, InputValue, InterfaceType,
  ObjectType, ScalarType, SchemaDefinition, Type, TypeDefinition, UnionType,
};
use graphql_parser::Pos;
use std::collections::HashMap;
use std::convert::{From, Into};
use std::fmt;

pub use serde_json::{Number, Value};

use super::introspection;
use super::introspection::{Schema as IntrospectionSchema, TypeKind};

#[derive(Debug, Clone, Default)]
pub struct DuplicateObjectField {
  pub current_executor: String,
  pub next_executor: String,
  pub object_type: String,
  pub field_name: String,
}

#[derive(Debug, Clone, Default)]
pub struct Schema<'a> {
  pub schema: IntrospectionSchema,
  pub document: Document<'a, String>,
  pub types_by_name: HashMap<String, usize>,
  pub type_fields_by_name: HashMap<String, (String, usize)>,
  pub duplicate_object_fields: Vec<DuplicateObjectField>,
}

impl<'a> Schema<'a> {
  pub fn object_by_kind<T: Into<String>>(
    &self,
    kind: TypeKind,
    name: T,
  ) -> Option<&introspection::Type> {
    self
      .types_by_name
      .get(&format!("{:?}.{}", kind, name.into()))
      .and_then(|&i| self.schema.types.get(i))
  }

  pub fn object<T: Into<String>>(&self, name: T) -> Option<&introspection::Type> {
    self.object_by_kind(TypeKind::Object, name)
  }

  pub fn interface<T: Into<String>>(&self, name: T) -> Option<&introspection::Type> {
    self.object_by_kind(TypeKind::Interface, name)
  }

  pub fn field<T: Into<String>>(
    &self,
    object: T,
    name: T,
  ) -> Option<(String, &introspection::Field)> {
    let object_name = object.into();
    let fields = self
      .object(object_name.clone())
      .and_then(|object| object.fields.as_ref())?;

    self
      .type_fields_by_name
      .get(&format!(
        "{:?}.{}.{}",
        TypeKind::Object,
        object_name,
        name.into()
      ))
      .and_then(|(name, i)| fields.get(*i).map(|field| (name.clone(), field)))
  }
}

impl<'a> fmt::Display for Schema<'a> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.document)
  }
}

impl<'a> From<HashMap<String, IntrospectionSchema>> for Schema<'a> {
  fn from(schemas: HashMap<String, IntrospectionSchema>) -> Schema<'a> {
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
          let mut current_possible_types = current_type.possible_types.clone().unwrap_or(vec![]);

          for possible_type in possible_types {
            let possible_type_key = format!("{}.{:#?}", key.clone(), possible_type.name());

            if possible_types_by_name.get(&possible_type_key).is_some() {
              continue;
            }

            possible_types_by_name.insert(possible_type_key.clone(), (executor_name.clone(), true));
            current_possible_types.push(possible_type.clone());
          }

          current_type.possible_types = Some(current_possible_types);
        }

        if let Some(fields) = &schema_type.fields {
          let mut current_fields = current_type.fields.clone().unwrap_or(vec![]);

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

                duplicate_object_fields.push(DuplicateObjectField {
                  current_executor: current_executor_name.clone(),
                  next_executor: executor_name.clone(),
                  object_type: key.clone(),
                  field_name: field.name.clone(),
                });
              }
              _ => {
                type_fields_by_name.insert(
                  field_key.clone(),
                  (executor_name.clone(), current_fields.len()),
                );
                current_fields.push(field.clone());
              }
            }
          }

          current_type.fields = Some(current_fields);
        }
      }
    }

    let query_type = types_by_name
      .get("Object.Query")
      .map(|_| introspection::Type {
        kind: TypeKind::Object,
        name: Some("Query".to_owned()),
        ..introspection::Type::default()
      });

    let mutation_type = types_by_name
      .get("Object.Mutation")
      .map(|_| introspection::Type {
        kind: TypeKind::Object,
        name: Some("Mutation".to_owned()),
        ..introspection::Type::default()
      });

    let schema = IntrospectionSchema {
      query_type,
      mutation_type,
      types,
      ..IntrospectionSchema::default()
    };

    let document: Document<'a, String> = schema.clone().into();

    Schema {
      schema,
      document,
      types_by_name,
      type_fields_by_name,
      duplicate_object_fields,
    }
  }
}

impl<'a> Into<Document<'a, String>> for IntrospectionSchema {
  fn into(self) -> Document<'a, String> {
    let query = if self.types.iter().any(|t| t.name() == "Query") {
      Some("Query".to_owned())
    } else {
      None
    };

    let mutation = if self.types.iter().any(|t| t.name() == "Mutation") {
      Some("Mutation".to_owned())
    } else {
      None
    };

    let mut definitions = self
      .types
      .into_iter()
      .filter_map(|t| {
        if t.name().starts_with("__") {
          None
        } else {
          Some(t.into())
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
}

impl<'a> Into<Definition<'a, String>> for introspection::Type {
  fn into(self) -> Definition<'a, String> {
    let name = self.name.expect("Type name does not exist.");
    let type_definition = match self.kind {
      TypeKind::Scalar => TypeDefinition::Scalar(ScalarType {
        position: Pos::default(),
        description: self.description,
        name,
        directives: vec![],
      }),
      TypeKind::Object => TypeDefinition::Object(ObjectType {
        position: Pos::default(),
        description: self.description,
        name,
        implements_interfaces: self
          .interfaces
          .expect("Type interfaces does not exist.")
          .into_iter()
          .map(|interface| interface.name().to_owned())
          .collect(),
        directives: vec![],
        fields: self
          .fields
          .expect("Type fields does not exist.")
          .into_iter()
          .map(|field| field.into())
          .collect(),
      }),
      TypeKind::Interface => TypeDefinition::Interface(InterfaceType {
        position: Pos::default(),
        description: self.description,
        name,
        directives: vec![],
        fields: self
          .fields
          .expect("Type fields does not exist.")
          .into_iter()
          .map(|field| field.into())
          .collect(),
      }),
      TypeKind::InputObject => TypeDefinition::InputObject(InputObjectType {
        position: Pos::default(),
        description: self.description,
        name,
        directives: vec![],
        fields: self
          .input_fields
          .expect("Type input_fields does not exist.")
          .into_iter()
          .map(|input_value| input_value.into())
          .collect(),
      }),
      TypeKind::Enum => TypeDefinition::Enum(EnumType {
        position: Pos::default(),
        description: self.description,
        name,
        directives: vec![],
        values: self
          .enum_values
          .expect("Type enum_values does not exist.")
          .into_iter()
          .map(|enum_value| enum_value.into())
          .collect(),
      }),
      TypeKind::Union => TypeDefinition::Union(UnionType {
        position: Pos::default(),
        description: self.description,
        name,
        directives: vec![],
        types: self
          .possible_types
          .expect("Type possible_types does not exist.")
          .into_iter()
          .map(|possible_type| possible_type.name().to_owned())
          .collect(),
      }),
      _ => unreachable!(),
    };

    Definition::TypeDefinition(type_definition)
  }
}

impl<'a> Into<Field<'a, String>> for introspection::Field {
  fn into(self) -> Field<'a, String> {
    Field {
      position: Pos::default(),
      description: self.description,
      name: self.name,
      directives: vec![],
      field_type: self.field_type.into(),
      arguments: self.args.into_iter().map(|arg| arg.into()).collect(),
    }
  }
}

impl<'a> Into<InputValue<'a, String>> for introspection::InputValue {
  fn into(self) -> InputValue<'a, String> {
    InputValue {
      position: Pos::default(),
      description: self.description,
      name: self.name,
      directives: vec![],
      value_type: self.input_type.into(),
      default_value: None,
    }
  }
}

impl<'a> Into<Type<'a, String>> for introspection::Type {
  fn into(self) -> Type<'a, String> {
    match self.kind {
      TypeKind::NonNull => Type::NonNullType(Box::new(self.of_type().clone().into())),
      TypeKind::List => Type::ListType(Box::new(self.of_type().clone().into())),
      _ => Type::NamedType(self.name().to_owned()),
    }
  }
}

impl<'a> Into<EnumValue<'a, String>> for introspection::EnumValue {
  fn into(self) -> EnumValue<'a, String> {
    EnumValue {
      position: Pos::default(),
      description: self.description,
      name: self.name,
      directives: vec![],
    }
  }
}
