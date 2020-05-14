mod common;

use futures_await_test::async_test;
use graphql_gateway::{Error, QueryBuilder, Response};
use serde_json::json;

#[async_test]
async fn error_not_supported() {
    let query = QueryBuilder::new(
        r#"
            subscription {
                commentAdded(repoFullName: "yes"){
                    id
                    content
                }
            }
        "#
        .to_owned(),
    );

    let gateway = common::gateway().await;
    let response = serde_json::to_value(Response(query.execute(&gateway).await)).unwrap();

    assert_eq!(
        response,
        json!({
            "errors": [{ "message": "Not supported.", "locations": [{ "line": 0, "column": 0 }] }]
        })
    );
}

#[async_test]
async fn error_field_not_found() {
    let query = QueryBuilder::new(
        r#"
            query {
                products {
                    id
                    name
                    in_stock
                }
            }
        "#
        .to_owned(),
    );

    let gateway = common::gateway().await;
    let response = serde_json::to_value(Response(query.execute(&gateway).await)).unwrap();

    assert_eq!(
        response,
        json!({
            "errors": [{ "message": "Cannot query field \"in_stock\" on type \"Product\".", "locations": [{ "line": 6, "column": 21 }] }]
        })
    );
}

#[async_test]
async fn error_unknown_fragment() {
    let query = QueryBuilder::new(
        r#"
            query {
                products {
                    id
                    ...ProductDetail
                }
            }
        "#
        .to_owned(),
    );

    let gateway = common::gateway().await;
    let response = serde_json::to_value(Response(query.execute(&gateway).await)).unwrap();

    assert_eq!(
        response,
        json!({
            "errors": [{ "message": "Unknown fragment \"ProductDetail\".", "locations": [{ "line": 5, "column": 24 }] }]
        })
    );
}

#[async_test]
async fn error_executor() {
    let response =
        serde_json::to_value(Response(Err(Error::Executor(json!({
            "data": null,
            "errors": [{ "message": "Unknown fragment \"ProductDetail\".", "locations": [{ "line": 5, "column": 28 }] }]
        }))))).unwrap();

    assert_eq!(
        response,
        json!({
            "data": null,
            "errors": [{ "message": "Unknown fragment \"ProductDetail\".", "locations": [{ "line": 5, "column": 28 }] }]
        })
    );
}

#[async_test]
async fn error_json() {
    let response = serde_json::to_value(Response(Err(Error::Json(serde::ser::Error::custom(
        "field missing",
    )))))
    .unwrap();

    assert_eq!(
        response,
        json!({
            "errors": [{ "message": "Json error: field missing", "locations": [{ "line": 0, "column": 0 }] }]
        })
    );
}
