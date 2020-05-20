use crate::query::{QueryBuilder, QueryError, QueryResult};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct GraphQLPayload {
    pub query: String,
    #[serde(rename = "operationName")]
    pub operation_name: Option<String>,
    pub variables: Option<Value>,
}

impl GraphQLPayload {
    pub fn into_query_builder(&self) -> QueryBuilder {
        QueryBuilder {
            query_source: self.query.clone(),
            operation_name: self.operation_name.clone(),
            variables: self.variables.clone(),
            ctx_data: None,
        }
    }
}

pub struct GraphQLResponse(pub QueryResult<Value>);

impl Serialize for GraphQLResponse {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match &self.0 {
            Ok(data) => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_key("data")?;
                map.serialize_value(&data)?;
                map.end()
            }
            Err(err) => match err {
                QueryError::Executor(value) => {
                    let mut map = serializer.serialize_map(None)?;
                    if let Value::Object(object) = value {
                        for (k, v) in object {
                            map.serialize_key(k)?;
                            map.serialize_value(&v)?;
                        }
                    }
                    map.end()
                }
                _ => {
                    let mut map = serializer.serialize_map(None)?;
                    map.serialize_key("errors")?;
                    map.serialize_value(&GQLError(err))?;
                    map.end()
                }
            },
        }
    }
}

pub struct GQLError<'a>(pub &'a QueryError);

impl<'a> Serialize for GQLError<'a> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            QueryError::Errors(errors) => {
                let mut seq = serializer.serialize_seq(Some(errors.len()))?;
                for graphql_error in errors {
                    seq.serialize_element(&serde_json::json!({
                        "message": graphql_error.1.to_string(),
                        "locations": [{"line": graphql_error.0.line, "column": graphql_error.0.column}]
                    }))?;
                }
                seq.end()
            }
            _ => {
                let mut seq = serializer.serialize_seq(Some(1))?;
                seq.serialize_element(&serde_json::json! ({
                    "message": self.0.to_string(),
                    "locations": [{"line": 0, "column": 0}]
                }))?;
                seq.end()
            }
        }
    }
}
