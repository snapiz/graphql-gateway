use std::fmt;

pub const INTROSPECTION_QUERY: &str = r#"
  query IntrospectionQuery {
    __schema {
      queryType {
        kind
        name
      }
      mutationType {
        kind
        name
      }
      subscriptionType {
        kind
        name
      }
      types {
        ...FullType
      }
      directives {
        name
        description
        locations
        args {
          ...InputValue
        }
      }
    }
  }
  fragment FullType on __Type {
    kind
    name
    description
    fields(includeDeprecated: true) {
      name
      description
      args {
        ...InputValue
      }
      type {
        ...TypeRef
      }
      isDeprecated
      deprecationReason
    }
    inputFields {
      ...InputValue
    }
    interfaces {
      ...TypeRef
    }
    enumValues(includeDeprecated: true) {
      name
      description
      isDeprecated
      deprecationReason
    }
    possibleTypes {
      ...TypeRef
    }
  }
  fragment InputValue on __InputValue {
    name
    description
    type {
      ...TypeRef
    }
    defaultValue
  }
  fragment TypeRef on __Type {
    kind
    name
    ofType {
      kind
      name
      ofType {
        kind
        name
        ofType {
          kind
          name
          ofType {
            kind
            name
            ofType {
              kind
              name
              ofType {
                kind
                name
                ofType {
                  kind
                  name
                }
              }
            }
          }
        }
      }
    }
  }  
"#;

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct Schema {
  pub description: Option<String>,
  pub types: Vec<Type>,
  #[serde(rename = "queryType")]
  pub query_type: Option<Type>,
  #[serde(rename = "mutationType")]
  pub mutation_type: Option<Type>,
  #[serde(rename = "subscriptionType")]
  pub subscription_type: Option<Type>,
  pub directives: Vec<Directive>,
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

impl Type {
  pub fn name(&self) -> &str {
    self.name.as_ref().expect("Type name does not exist.")
  }

  pub fn of_type(&self) -> &Type {
    self
      .of_type
      .as_ref()
      .expect("Type of_type does not exist.")
      .as_ref()
  }
}

impl fmt::Display for Type {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:#?}.{}", self.kind, self.name())
  }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
pub struct Field {
  pub name: String,
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
  pub fn field_type(&self) -> &Type {
    get_final_field_type(&self.field_type)
  }
}

impl fmt::Display for Field {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.field_type().name())
  }
}

fn get_final_field_type(field_type: &Type) -> &Type {
  match field_type.kind {
    TypeKind::List | TypeKind::NonNull => get_final_field_type(field_type.of_type()),
    _ => field_type,
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
