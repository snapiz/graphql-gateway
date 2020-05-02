use std::collections::HashMap;

use super::error::Result;

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct Schema {
  pub executor_name: Option<String>,
  pub description: Option<String>,
  pub types: Vec<Type>,
  #[serde(rename = "queryType")]
  pub query_type: Type,
  #[serde(rename = "mutationType")]
  pub mutation_type: Option<Type>,
  #[serde(rename = "subscriptionType")]
  pub subscription_type: Option<Type>,
  pub directives: Vec<Directive>,
}

impl Schema {
  pub fn to_fields(&self) -> HashMap<String, Field> {
    let mut fields = self
      .types
      .iter()
      .filter(|t| t.kind == TypeKind::Object && t.fields.is_some())
      .flat_map(move |object| {
        object.fields.as_ref().unwrap().iter().map(move |field| {
          let key = format!(
            "{}.{}",
            object.name.as_ref().unwrap_or(&"".to_owned()),
            field.name
          );
          (key, field.clone())
        })
      })
      .collect::<HashMap<String, Field>>();

    fields.insert(
      "Query.__schema".to_owned(),
      Field {
        name: "__schema".to_owned(),
        field_type: Type {
          name: Some("__Schema".to_owned()),
          kind: TypeKind::Object,
          ..Type::default()
        },
        ..Field::default()
      },
    );
    fields
  }

  pub fn to_objects(&self) -> HashMap<String, Type> {
    self
      .types
      .iter()
      .filter(|t| t.kind == TypeKind::Object && t.fields.is_some())
      .flat_map(move |object| {
        object.fields.as_ref().unwrap().iter().map(move |field| {
          let key = format!(
            "{}.{}",
            field.executor_name.as_ref().unwrap(),
            object.name.as_ref().unwrap()
          );
          (key, object.clone())
        })
      })
      .collect()
  }

  pub fn to_types(&self) -> HashMap<String, Type> {
    self
      .types
      .iter()
      .filter(|t| t.kind == TypeKind::Object && t.fields.is_some())
      .map(|object| (object.name.as_ref().unwrap().to_string(), object.clone()))
      .collect()
  }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct Type {
  pub kind: TypeKind,
  pub name: Option<String>,
  pub description: Option<String>,
  pub fields: Option<Vec<Field>>,
  pub interfaces: Option<Vec<Type>>,
  #[serde(rename = "possibleTypes")]
  pub possible_types: Option<Vec<Type>>,
  #[serde(rename = "enumValues")]
  pub enum_values: Option<Vec<EnumValue>>,
  #[serde(rename = "inputFields")]
  pub input_fields: Option<Vec<InputValue>>,
  #[serde(rename = "ofType")]
  pub of_type: Option<Box<Type>>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct Field {
  pub name: String,
  pub executor_name: Option<String>,
  pub description: Option<String>,
  pub args: Vec<InputValue>,
  #[serde(rename = "type")]
  pub field_type: Type,
  #[serde(rename = "isDeprecated")]
  pub is_deprecated: bool,
  #[serde(rename = "deprecationReason")]
  pub deprecation_reason: Option<String>,
}

impl Field {
  pub fn field_type(&self) -> Option<&Type> {
    field_type(&self.field_type)
  }
}


fn field_type<'a>(of_type: &Type) -> Option<&Type> {
  match &of_type.name {
    Some(_) => Some(of_type),
    _ => match &of_type.of_type {
      Some(of_type) => field_type(of_type.as_ref()),
      _ => None,
    },
  }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct InputValue {
  pub name: String,
  pub description: Option<String>,
  #[serde(rename = "type")]
  pub input_type: Type,
  #[serde(rename = "defaultValue")]
  pub default_value: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct EnumValue {
  pub name: String,
  pub description: Option<String>,
  #[serde(rename = "isDeprecated")]
  pub is_deprecated: bool,
  #[serde(rename = "deprecationReason")]
  pub deprecation_reason: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum TypeKind {
  #[serde(rename = "SCALAR")]
  Scalar,
  #[serde(rename = "OBJECT")]
  Object,
  #[serde(rename = "INTERFACE")]
  Interface,
  #[serde(rename = "UNION")]
  Union,
  #[serde(rename = "ENUM")]
  Enum,
  #[serde(rename = "INPUT_OBJECT")]
  InputObject,
  #[serde(rename = "LIST")]
  List,
  #[serde(rename = "NON_NULL")]
  NonNull,
}

impl Default for TypeKind {
  fn default() -> Self {
    TypeKind::Scalar
  }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct Directive {
  pub name: String,
  pub description: Option<String>,
  pub locations: Vec<DirectiveLocation>,
  pub args: Vec<InputValue>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum DirectiveLocation {
  #[serde(rename = "QUERY")]
  Query,
  #[serde(rename = "MUTATION")]
  Mutation,
  #[serde(rename = "SUBSCRIPTION")]
  Subscription,
  #[serde(rename = "FIELD")]
  Field,
  #[serde(rename = "FRAGMENT_DEFINITION")]
  FragmentDefinition,
  #[serde(rename = "FRAGMENT_SPREAD")]
  FragmentSpread,
  #[serde(rename = "INLINE_FRAGMENT")]
  InlineFragment,
  #[serde(rename = "SCHEMA")]
  Schema,
  #[serde(rename = "SCALAR")]
  Scalar,
  #[serde(rename = "OBJECT")]
  Object,
  #[serde(rename = "FIELD_DEFINITION")]
  FieldDefinition,
  #[serde(rename = "ARGUMENT_DEFINITION")]
  ArgumentDefinition,
  #[serde(rename = "INTERFACE")]
  Interface,
  #[serde(rename = "UNION")]
  Union,
  #[serde(rename = "ENUM")]
  Enum,
  #[serde(rename = "ENUM_VALUE")]
  EnumValue,
  #[serde(rename = "INPUT_OBJECT")]
  InputObject,
  #[serde(rename = "INPUT_FIELD_DEFINITION")]
  InputFieldDefinition,
}

impl Default for DirectiveLocation {
  fn default() -> Self {
    DirectiveLocation::Query
  }
}

pub async fn combine<'a>(schemas: &Vec<Schema>) -> Result<Schema> {
  let mut types = HashMap::<String, Type>::new();
  let mut directives = HashMap::new();

  for schema in schemas.iter() {
    for schema_type in schema.types.iter() {
      let key = schema_type
        .name
        .as_ref()
        .map(|name| format!("{:?}{}", schema_type.kind, name))
        .unwrap_or("".to_owned());

      match schema_type.kind {
        TypeKind::Interface => {
          if let Some(interface) = types.get_mut(&key) {
            let mut possible_types = HashMap::new();

            if let Some(schema_possible_types) = &schema_type.possible_types {
              for possible_type in schema_possible_types.into_iter() {
                if let Some(name) = &possible_type.name {
                  possible_types.insert(name, possible_type.clone());
                }
              }
            }

            if let Some(schema_possible_types) = &interface.possible_types {
              for possible_type in schema_possible_types.into_iter() {
                if let Some(name) = &possible_type.name {
                  possible_types.insert(name, possible_type.clone());
                }
              }
            }

            interface.possible_types =
              Some(possible_types.values().cloned().collect::<Vec<Type>>());
          } else {
            types.insert(key, schema_type.clone());
          }
        }
        TypeKind::Object => {
          let mut schema_type = schema_type.clone();

          schema_type.fields = match schema_type.fields {
            Some(mut fields) => {
              for field in fields.iter_mut() {
                field.executor_name = schema.executor_name.as_ref().map(|n| n.to_string());
              }

              Some(fields)
            }
            _ => None,
          };

          if let Some(object) = types.get_mut(&key) {
            let mut fields = HashMap::new();

            if let Some(schema_fields) = &schema_type.fields {
              for field in schema_fields.iter() {
                fields.insert(field.name.to_owned(), field.clone());
              }
            }

            if let Some(schema_fields) = &object.fields {
              for field in schema_fields.iter() {
                fields.insert(field.name.to_owned(), field.clone());
              }
            }

            object.fields = Some(fields.values().cloned().collect::<Vec<Field>>());
          } else {
            types.insert(key, schema_type);
          }
        }
        _ => {
          types.insert(key, schema_type.clone());
        }
      };
    }

    for directive in schema.directives.iter() {
      directives.insert(directive.name.to_owned(), directive.clone());
    }
  }

  let mutation_type = types.get("Mutation").map(|_| Type {
    kind: TypeKind::Object,
    name: Some("Mutation".to_owned()),
    ..Type::default()
  });

  Ok(Schema {
    query_type: Type {
      kind: TypeKind::Object,
      name: Some("Query".to_owned()),
      ..Type::default()
    },
    mutation_type,
    types: types.values().cloned().collect::<Vec<Type>>(),
    directives: directives.values().cloned().collect::<Vec<Directive>>(),
    ..Schema::default()
  })
}
