mod common;

use common::{account, product, review};
use futures_await_test::async_test;
use graphql_gateway::{Data, Error, Payload, Response};
use serde_json::json;

#[async_test]
async fn error_not_supported() {
    let account = account::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let gateway = graphql_gateway::from_executors(vec![&account, &product, &review])
        .await
        .unwrap();

    let res = graphql_gateway::execute(
        &gateway,
        &Data::default(),
        &Payload {
            query: r#"
                subscription {
                    commentAdded(repoFullName: "yes"){
                        id
                        content
                    }
                }
            "#
            .to_owned(),
            operation_name: None,
            variables: None,
        },
    )
    .await;

    let response = serde_json::to_value(Response(res)).unwrap();

    assert_eq!(
        response,
        json!({
            "errors": [{ "message": "Not supported.", "locations": [{ "line": 0, "column": 0 }] }]
        })
    );
}

#[async_test]
async fn error_field_not_found() {
    let account = account::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let gateway = graphql_gateway::from_executors(vec![&account, &product, &review])
        .await
        .unwrap();

    let res = graphql_gateway::execute(
        &gateway,
        &Data::default(),
        &Payload {
            query: r#"
                query {
                    products {
                        id
                        name
                        in_stock
                    }
                }
            "#
            .to_owned(),
            operation_name: None,
            variables: None,
        },
    )
    .await;

    let response = serde_json::to_value(Response(res)).unwrap();

    assert_eq!(
        response,
        json!({
            "errors": [{ "message": "Cannot query field \"in_stock\" on type \"Product\".", "locations": [{ "line": 6, "column": 25 }] }]
        })
    );
}

#[async_test]
async fn error_unknown_fragment() {
    let account = account::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let gateway = graphql_gateway::from_executors(vec![&account, &product, &review])
        .await
        .unwrap();

    let res = graphql_gateway::execute(
        &gateway,
        &Data::default(),
        &Payload {
            query: r#"
                query {
                    products {
                        id
                        ...ProductDetail
                    }
                }
            "#
            .to_owned(),
            operation_name: None,
            variables: None,
        },
    )
    .await;

    let response = serde_json::to_value(Response(res)).unwrap();

    assert_eq!(
        response,
        json!({
            "errors": [{ "message": "Unknown fragment \"ProductDetail\".", "locations": [{ "line": 5, "column": 28 }] }]
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
