use async_trait::async_trait;
use serde_json::Value;

use super::error::{Error, Result};
use super::graphql::Payload;
use super::schema::Schema;

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

#[async_trait]
pub trait Executor: Send + Sync {
  fn name(&self) -> String;

  async fn execute(&self, payload: &Payload) -> Result<Value>;

  async fn get_schema(&self) -> Result<Schema> {
    let res = self
      .execute(&Payload {
        query: INTROSPECTION_QUERY.to_owned(),
        operation_name: Some("IntrospectionQuery".to_owned()),
        variables: None,
      })
      .await?;

    if res.get("errors").is_some() {
      return Err(Error::Executor(res));
    }

    let data = match res.get("data") {
      Some(data) => data,
      _ => return Err(Error::InvalidExecutorResponse),
    };

    let data = match data {
      Value::Object(values) => values,
      _ => return Err(Error::InvalidExecutorResponse),
    };

    let data = match data.get("__schema") {
      Some(schema) => schema.clone(),
      _ => return Err(Error::InvalidExecutorResponse),
    };

    let mut schema: Schema = serde_json::from_value(data)?;

    schema.executor_name = Some(self.name());

    Ok(schema)
  }
}
