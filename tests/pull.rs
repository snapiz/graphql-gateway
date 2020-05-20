mod common;

use async_graphql::{EmptyMutation, EmptySubscription};
use common::{account, inventory, inventory_updated, TestExecutor};
use futures_await_test::async_test;
use graphql_gateway::{Executor, GatewayError, QueryBuilder, GraphQLResponse};
use serde_json::json;

#[async_test]
async fn poll() {
    let query = QueryBuilder::new(
        r#"
            query {
                node(id: "UHJvZHVjdDow") {
                    ... on Product {
                        name
                        delivered
                    }
                }
            }
        "#
        .to_owned(),
    );

    let gateway = common::gateway().await;
    let response = serde_json::to_value(GraphQLResponse(query.execute(&gateway).await)).unwrap();

    assert_eq!(
        response,
        json!({
            "errors": [{ "message": "Cannot query field \"delivered\" on type \"Product\".", "locations": [{ "line": 6, "column": 25 }] }]
        })
    );

    let inventory = TestExecutor::new(
        "inventory",
        inventory_updated::Query {},
        EmptyMutation,
        EmptySubscription,
    );

    let mut gateway = gateway.clone().executor(inventory);

    gateway.pull("inventory").await.unwrap();

    let response = serde_json::to_value(GraphQLResponse(query.execute(&gateway).await)).unwrap();

    assert_eq!(
        response,
        json!({
            "data": {
                "node": {
                    "name": "Product 1",
                    "delivered": true
                }
            }
        })
    );

    let query = QueryBuilder::new(
        r#"
            query {
                node(id: "UHJvZHVjdDow") {
                    ... on Product {
                        name
                        inStock
                    }
                }
            }
        "#
        .to_owned(),
    );

    let response = serde_json::to_value(GraphQLResponse(query.execute(&gateway).await)).unwrap();

    assert_eq!(
        response,
        json!({
            "errors": [{ "message": "Cannot query field \"inStock\" on type \"Product\".", "locations": [{ "line": 6, "column": 25 }] }]
        })
    );

    let inventory = TestExecutor::new(
        "inventory",
        inventory::Query {},
        EmptyMutation,
        EmptySubscription,
    );

    let mut gateway = gateway.clone().executor(inventory);

    gateway.pull("inventory").await.unwrap();

    let response = serde_json::to_value(GraphQLResponse(query.execute(&gateway).await)).unwrap();

    assert_eq!(
        response,
        json!({
            "data": {
                "node": {
                    "name": "Product 1",
                    "inStock": true
                }
            }
        })
    );

    let query = QueryBuilder::new(
        r#"
            query {
                node(id: "UHJvZHVjdDow") {
                    ... on Product {
                        name
                        delivered
                    }
                }
            }
        "#
        .to_owned(),
    );

    let response = serde_json::to_value(GraphQLResponse(query.execute(&gateway).await)).unwrap();

    assert_eq!(
        response,
        json!({
            "errors": [{ "message": "Cannot query field \"delivered\" on type \"Product\".", "locations": [{ "line": 6, "column": 25 }] }]
        })
    );
}

#[async_test]
async fn validate() {
    let gateway = common::gateway().await;

    let inventory_updated = TestExecutor::new(
        "inventory_updated",
        inventory_updated::Query {},
        EmptyMutation,
        EmptySubscription,
    );

    let (name, schema) = inventory_updated.introspect().await.unwrap();
    assert_eq!(gateway.validate(name, schema).is_ok(), true);
}

#[async_test]
async fn validate_failed() {
    let gateway = common::gateway().await;

    let account = TestExecutor::new(
        "account_plus",
        account::Query {},
        EmptyMutation,
        EmptySubscription,
    );

    let (name, schema) = account.introspect().await.unwrap();

    match gateway.validate(name, schema).unwrap_err() {
        GatewayError::DuplicateObjectFields(fields) => {
            assert_eq!(
                fields.iter().any(|(_, _, key)| key == "Object.Query.users"),
                true
            );
            assert_eq!(
                fields
                    .iter()
                    .any(|(_, _, key)| key == "Object.Query.viewer"),
                true
            );
            assert_eq!(
                fields.iter().any(|(_, _, key)| key == "Object.User.email"),
                true
            );
            assert_eq!(
                fields
                    .iter()
                    .any(|(_, _, key)| key == "Object.User.username"),
                true
            );
            assert_eq!(
                fields.iter().any(|(_, _, key)| key == "Object.User.role"),
                true
            );
            assert_eq!(
                fields
                    .iter()
                    .any(|(_, _, key)| key == "Object.User.sayHello"),
                true
            );
        }
        _ => panic!("thread 'validate' panicked at 'Excepted an duplicate error"),
    };
}
