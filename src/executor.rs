use crate::data::Data;
use crate::schema::Schema;
use async_trait::async_trait;
use serde_json::Value;

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
pub trait Executor: Send + Sync + CloneExecutor {
    fn name(&self) -> &str;

    async fn execute(
        &self,
        data: Option<&Data>,
        query: String,
        operation_name: Option<String>,
        variables: Option<Value>,
    ) -> Result<Value, String>;

    async fn introspect(&self) -> Result<(String, Schema), String> {
        self.execute(
            None,
            INTROSPECTION_QUERY.to_owned(),
            Some("IntrospectionQuery".to_owned()),
            None,
        )
        .await?
        .get("data")
        .and_then(|data| data.get("__schema"))
        .ok_or("data.__schema does not exist.".to_owned())
        .and_then(|schema| serde_json::from_value(schema.clone()).map_err(|e| e.to_string()))
        .map(|schema| (self.name().to_string(), schema))
    }
}

pub trait CloneExecutor {
    fn clone_executor(&self) -> Box<dyn Executor>;
}

impl<T> CloneExecutor for T
where
    T: Executor + Clone + 'static,
{
    fn clone_executor(&self) -> Box<dyn Executor> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Executor> {
    fn clone(&self) -> Self {
        self.clone_executor()
    }
}
