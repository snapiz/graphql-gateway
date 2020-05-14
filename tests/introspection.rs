mod common;

use futures_await_test::async_test;
use graphql_gateway::{IntrospectionSchema, QueryBuilder, TypeKind};
use serde_json::{json, Value};

#[async_test]
async fn introspection() {
    let query = QueryBuilder::new(graphql_gateway::INTROSPECTION_QUERY.to_owned())
        .operation_name("IntrospectionQuery");
    let gateway = common::gateway().await;
    let res = query.execute(&gateway).await.unwrap();

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

    let schema: IntrospectionSchema = serde_json::from_value(res["__schema"].clone()).unwrap();

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
            && t.fields.as_ref().unwrap().len() == 2
            && t.fields
                .as_ref()
                .unwrap()
                .iter()
                .filter(|field| match field.name.as_ref() {
                    "addProduct" => true,
                    "signIn" => true,
                    _ => false,
                })
                .count()
                == 2),
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
            && t.fields.as_ref().unwrap().len() == 6
            && t.fields
                .as_ref()
                .unwrap()
                .iter()
                .filter(|field| match field.name.as_ref() {
                    "id" | "email" | "username" | "reviews" | "sayHello" | "role" => true,
                    _ => false,
                })
                .count()
                == 6),
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
        schema.types.iter().any(|t| t.kind == TypeKind::Enum
            && t.name == Some("UserRole".to_owned())
            && t.enum_values.as_ref().unwrap().len() == 3
            && t.enum_values
                .as_ref()
                .unwrap()
                .iter()
                .filter(|enum_value| match enum_value.name.as_ref() {
                    "ADMIN" | "STAFF" | "USER" => true,
                    _ => false,
                })
                .count()
                == 3),
        true
    );
}
