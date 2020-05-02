use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};
use serde_json::Value;

use super::error::{Error, Result};

#[derive(Serialize, Deserialize, Debug)]
pub struct Payload {
    pub query: String,
    #[serde(rename = "operationName")]
    pub operation_name: Option<String>,
    pub variables: Option<Value>,
}

pub struct Response(pub Result<Value>);

impl Serialize for Response {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match &self.0 {
            Ok(data) => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_key("data")?;
                map.serialize_value(&data)?;
                map.end()
            }
            Err(err) => match err {
                Error::Executor(value) => {
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

pub struct GQLError<'a>(pub &'a Error);

impl<'a> Serialize for GQLError<'a> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            Error::Query(errors) => {
                let mut seq = serializer.serialize_seq(Some(errors.len()))?;
                for graphql_error in errors {
                    seq.serialize_element(&serde_json::json!({
                        "message": graphql_error.err.to_string(),
                        "locations": [{"line": graphql_error.pos.line, "column": graphql_error.pos.column}]
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
