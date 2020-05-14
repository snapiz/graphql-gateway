mod common;

use futures_await_test::async_test;
use graphql_gateway::QueryBuilder;
use serde_json::json;

#[async_test]
async fn mutation() {
    let query = QueryBuilder::new(
        r#"
        mutation AddProductMutation($id: ID!, $input: SignInInput!) {
            addProduct(id: $id) {
                id
                name
            }
            signIn(input: $input) {
                id
                email
                username
            }
        }
        "#
        .to_owned(),
    )
    .operation_name("AddProductMutation")
    .variables(json!({
        "id": "UHJvZHVjdDow",
        "input": {
            "email": "john@doe.co",
            "password": "yep"
        }
    }));

    let gateway = common::gateway().await;

    assert_eq!(
        query.execute(&gateway).await.unwrap(),
        json!({
            "addProduct": { "id": "UHJvZHVjdDow", "name": "Product 1" },
            "signIn": { "id": "VXNlcjow", "email": "john@doe.com", "username": null },
        })
    );
}
