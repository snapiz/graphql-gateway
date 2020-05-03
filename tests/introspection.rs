mod common;

use common::{account, inventory, product, review};
use futures_await_test::async_test;
use graphql_gateway::{Payload, Schema, TypeKind};
use serde_json::{json, Value};

#[async_test]
async fn introspection() {
    let account = account::EXECUTOR.clone();
    let inventory = inventory::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let gateway = graphql_gateway::from_executors(vec![&account, &inventory, &product, &review])
        .await
        .unwrap();

    let res = graphql_gateway::execute(
        &gateway,
        &Payload {
            query: graphql_gateway::INTROSPECTION_QUERY.to_owned(),
            operation_name: Some("IntrospectionQuery".to_owned()),
            variables: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        res["__schema"]["queryType"],
        json!({
            "kind": "OBJECT",
            "name": "Query"
        })
    );

    assert_eq!(
        res["__schema"]["mutationType"],
        json!({
            "kind": "OBJECT",
            "name": "Mutation"
        })
    );

    assert_eq!(res["__schema"]["subscriptionType"], Value::Null);

    let schema: Schema = serde_json::from_value(res["__schema"].clone()).unwrap();

    assert_eq!(
        schema.types.iter().any(|t| t.kind == TypeKind::Object
            && t.name == Some("Query".to_owned())
            && t.fields.as_ref().unwrap().len() == 5
            && t.fields
                .as_ref()
                .unwrap()
                .iter()
                .filter(|field| match field.name.as_ref() {
                    "node" | "nodes" | "products" | "viewer" | "users" => true,
                    _ => false,
                })
                .count()
                == 5),
        true
    );

    assert_eq!(
        schema.types.iter().any(|t| t.kind == TypeKind::Object
            && t.name == Some("Mutation".to_owned())
            && t.fields.as_ref().unwrap().len() == 1
            && t.fields
                .as_ref()
                .unwrap()
                .iter()
                .filter(|field| match field.name.as_ref() {
                    "addProduct" => true,
                    _ => false,
                })
                .count()
                == 1),
        true
    );

    assert_eq!(
        schema.types.iter().any(|t| t.kind == TypeKind::Interface
            && t.name == Some("Node".to_owned())
            && t.fields.as_ref().unwrap().len() == 1
            && t.fields
                .as_ref()
                .unwrap()
                .iter()
                .any(|field| field.name == "id")
            && t.possible_types
                .as_ref()
                .unwrap()
                .iter()
                .filter(|possible_type| {
                    match possible_type.name.as_ref().unwrap().as_ref() {
                        "User" | "Product" | "Review" => possible_type.kind == TypeKind::Object,
                        _ => false,
                    }
                })
                .count()
                == 3),
        true
    );

    assert_eq!(
        schema.types.iter().any(|t| t.kind == TypeKind::Object
            && t.name == Some("User".to_owned())
            && t.fields.as_ref().unwrap().len() == 5
            && t.fields
                .as_ref()
                .unwrap()
                .iter()
                .filter(|field| match field.name.as_ref() {
                    "id" | "email" | "username" | "reviews" | "sayHello" => true,
                    _ => false,
                })
                .count()
                == 5),
        true
    );

    assert_eq!(
        schema.types.iter().any(|t| t.kind == TypeKind::Object
            && t.name == Some("Product".to_owned())
            && t.fields.as_ref().unwrap().len() == 4
            && t.fields
                .as_ref()
                .unwrap()
                .iter()
                .filter(|field| match field.name.as_ref() {
                    "id" | "name" | "reviews" | "inStock" => true,
                    _ => false,
                })
                .count()
                == 4),
        true
    );

    assert_eq!(
        schema.types.iter().any(|t| t.kind == TypeKind::Object
            && t.name == Some("Review".to_owned())
            && t.fields.as_ref().unwrap().len() == 4
            && t.fields
                .as_ref()
                .unwrap()
                .iter()
                .filter(|field| match field.name.as_ref() {
                    "id" | "body" | "author" | "product" => true,
                    _ => false,
                })
                .count()
                == 4),
        true
    );

    assert_eq!(
        schema
            .directives
            .iter()
            .filter(|directive| directive.name == "include" || directive.name == "skip")
            .count()
            == 2,
        true
    );
}
