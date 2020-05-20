use graphql_parser::{schema, Pos};
use std::fmt;

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
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

  pub fn is_interface(&self) -> bool {
    self.kind == TypeKind::Interface
  }

  pub fn is_node(&self) -> bool {
    match self.interfaces.as_ref() {
      Some(interfaces) => interfaces
        .iter()
        .any(|interface| interface.name() == "Node"),
      _ => false,
    }
  }
}

impl fmt::Display for Type {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}.{}", self.kind, self.name())
  }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InputValue {
  pub name: String,
  pub description: Option<String>,
  #[serde(rename = "type")]
  pub input_type: Type,
  #[serde(rename = "defaultValue")]
  pub default_value: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EnumValue {
  pub name: String,
  pub description: Option<String>,
  #[serde(rename = "isDeprecated")]
  pub is_deprecated: bool,
  #[serde(rename = "deprecationReason")]
  pub deprecation_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
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

impl fmt::Display for TypeKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      TypeKind::Scalar => write!(f, "Scalar"),
      TypeKind::Object => write!(f, "Object"),
      TypeKind::Interface => write!(f, "Interface"),
      TypeKind::Union => write!(f, "Union"),
      TypeKind::Enum => write!(f, "Enum"),
      TypeKind::InputObject => write!(f, "InputObject"),
      TypeKind::List => write!(f, "List"),
      TypeKind::NonNull => write!(f, "NonNull"),
    }
  }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Directive {
  pub name: String,
  pub description: Option<String>,
  pub locations: Vec<DirectiveLocation>,
  pub args: Vec<InputValue>,
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
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

impl<'a> Into<schema::Definition<'a, String>> for Type {
  fn into(self) -> schema::Definition<'a, String> {
    let name = self.name.expect("Type name does not exist.");
    let type_definition = match self.kind {
      TypeKind::Scalar => schema::TypeDefinition::Scalar(schema::ScalarType {
        position: Pos::default(),
        description: self.description,
        name,
        directives: vec![],
      }),
      TypeKind::Object => schema::TypeDefinition::Object(schema::ObjectType {
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
      TypeKind::Interface => schema::TypeDefinition::Interface(schema::InterfaceType {
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
      TypeKind::InputObject => schema::TypeDefinition::InputObject(schema::InputObjectType {
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
      TypeKind::Enum => schema::TypeDefinition::Enum(schema::EnumType {
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
      TypeKind::Union => schema::TypeDefinition::Union(schema::UnionType {
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

    schema::Definition::TypeDefinition(type_definition)
  }
}

impl<'a> Into<schema::Field<'a, String>> for Field {
  fn into(self) -> schema::Field<'a, String> {
    schema::Field {
      position: Pos::default(),
      description: self.description,
      name: self.name,
      directives: vec![],
      field_type: self.field_type.into(),
      arguments: self.args.into_iter().map(|arg| arg.into()).collect(),
    }
  }
}

impl<'a> Into<schema::InputValue<'a, String>> for InputValue {
  fn into(self) -> schema::InputValue<'a, String> {
    schema::InputValue {
      position: Pos::default(),
      description: self.description,
      name: self.name,
      directives: vec![],
      value_type: self.input_type.into(),
      default_value: None,
    }
  }
}

impl<'a> Into<schema::Type<'a, String>> for Type {
  fn into(self) -> schema::Type<'a, String> {
    match self.kind {
      TypeKind::NonNull => schema::Type::NonNullType(Box::new(self.of_type().clone().into())),
      TypeKind::List => schema::Type::ListType(Box::new(self.of_type().clone().into())),
      _ => schema::Type::NamedType(self.name().to_owned()),
    }
  }
}

impl<'a> Into<schema::EnumValue<'a, String>> for EnumValue {
  fn into(self) -> schema::EnumValue<'a, String> {
    schema::EnumValue {
      position: Pos::default(),
      description: self.description,
      name: self.name,
      directives: vec![],
    }
  }
}
