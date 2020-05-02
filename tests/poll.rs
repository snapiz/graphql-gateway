mod common;

use common::{account, inventory, inventory_updated, product, review};
use futures_await_test::async_test;
use graphql_gateway::{Payload, Response};
use serde_json::json;

#[async_test]
async fn poll() {
    let account = account::EXECUTOR.clone();
    let inventory = inventory::EXECUTOR.clone();
    let inventory_updated = inventory_updated::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let mut gateway =
        graphql_gateway::from_executors(vec![&account, &inventory, &product, &review])
            .await
            .unwrap();

    let res = graphql_gateway::execute(
        &gateway,
        &Payload {
            query: r#"
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
            operation_name: None,
            variables: None,
        },
    )
    .await;

    let response = serde_json::to_value(Response(res)).unwrap();

    assert_eq!(
        response,
        json!({
            "errors": [{ "message": "Cannot query field \"delivered\" on type \"Product\".", "locations": [{ "line": 6, "column": 29 }] }]
        })
    );

    gateway
        .executors
        .insert("inventory".to_owned(), &inventory_updated);

    gateway.poll("inventory").await.unwrap();

    assert_eq!(
        graphql_gateway::execute(
            &gateway,
            &Payload {
                query: r#"
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
                operation_name: None,
                variables: None,
            }
        )
        .await
        .unwrap(),
        json!({
            "node": {
                "name": "Product 1",
                "delivered": true
            }
        })
    );

    let res = graphql_gateway::execute(
        &gateway,
        &Payload {
            query: r#"
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
            operation_name: None,
            variables: None,
        },
    )
    .await;

    let response = serde_json::to_value(Response(res)).unwrap();

    assert_eq!(
        response,
        json!({
            "errors": [{ "message": "Cannot query field \"inStock\" on type \"Product\".", "locations": [{ "line": 6, "column": 29 }] }]
        })
    );

    gateway.executors.insert("inventory".to_owned(), &inventory);

    gateway.poll("inventory").await.unwrap();

    assert_eq!(
        graphql_gateway::execute(
            &gateway,
            &Payload {
                query: r#"
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
                operation_name: None,
                variables: None,
            }
        )
        .await
        .unwrap(),
        json!({
            "node": {
                "name": "Product 1",
                "inStock": true
            }
        })
    );

    let res = graphql_gateway::execute(
        &gateway,
        &Payload {
            query: r#"
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
            operation_name: None,
            variables: None,
        },
    )
    .await;

    let response = serde_json::to_value(Response(res)).unwrap();

    assert_eq!(
        response,
        json!({
            "errors": [{ "message": "Cannot query field \"delivered\" on type \"Product\".", "locations": [{ "line": 6, "column": 29 }] }]
        })
    );
}
