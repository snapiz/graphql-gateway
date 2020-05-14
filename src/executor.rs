use async_trait::async_trait;
use serde_json::{Map, Value};

use super::context::Data;
use super::error::{Error, Result};
use super::introspection::{Schema, INTROSPECTION_QUERY};

#[async_trait]
pub trait Executor: Send + Sync + CloneExecutor {
  fn name(&self) -> String;

  async fn query(
    &self,
    ctx: Option<&Data>,
    query: &str,
    operation_name: Option<&str>,
    variables: Option<Value>,
  ) -> Result<Value>;

  async fn execute(
    &self,
    ctx: Option<&Data>,
    query: &str,
    operation_name: Option<&str>,
    variables: Option<Value>,
  ) -> Result<Map<String, Value>> {
    let res = self.query(ctx, query, operation_name, variables).await?;

    if res.get("errors").is_some() {
      return Err(Error::Executor(res));
    }

    let data = res.get("data").ok_or(Error::InvalidExecutorResponse)?;

    match data {
      Value::Object(values) => Ok(values.clone()),
      _ => Err(Error::InvalidExecutorResponse),
    }
  }

  async fn introspection<'a>(&self) -> Result<(String, Schema)> {
    let data = self
      .execute(None, INTROSPECTION_QUERY, Some("IntrospectionQuery"), None)
      .await?;

    data
      .get("__schema")
      .ok_or(Error::InvalidExecutorResponse)
      .and_then(|data| Ok((self.name(), serde_json::from_value(data.clone())?)))
  }
}

pub trait CloneExecutor {
  fn into_boxed(&self) -> Box<dyn Executor>;
}

impl<T> CloneExecutor for T
where
  T: Executor + Clone + 'static,
{
  fn into_boxed(&self) -> Box<dyn Executor> {
    Box::new(self.clone())
  }
}

impl Clone for Box<dyn Executor> {
  fn clone(&self) -> Self {
    self.into_boxed()
  }
}
