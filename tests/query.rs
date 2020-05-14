mod common;

use futures_await_test::async_test;
use graphql_gateway::QueryBuilder;
use serde_json::json;

#[async_test]
async fn query() {
    let query = QueryBuilder::new(
        r#"
            query {
                viewer {
                    email
                    username
                    reviews {
                        body
                        author {
                            authorId: id
                            username
                        }
                        product {
                            name
                        }
                    }
                }
                users {
                    userId: id
                    email
                    username
                }
                products {
                    id
                    productName: name
                }
            }
        "#
        .to_owned(),
    );

    let gateway = common::gateway().await;

    assert_eq!(
        query.execute(&gateway).await.unwrap(),
        json!({
            "viewer": {
                "email": "john@doe.com",
                "username": null,
                "reviews": [
                    {
                        "body": "Good product",
                        "author": {
                            "authorId": "VXNlcjow",
                            "username": null
                        },
                        "product": {
                            "name": "Product 1"
                        }
                    },
                    {
                        "body": "Bad product",
                        "author": {
                            "authorId": "VXNlcjow",
                            "username": null
                        },
                        "product": {
                            "name": "Product 2"
                        }
                    }
                ]
            },
            "users": [
                {
                    "userId": "VXNlcjow",
                    "email": "john@doe.com",
                    "username": null,

                },
                {
                    "userId": "VXNlcjox",
                    "email": null,
                    "username": "albert",
                }
            ],
            "products": [
                {
                    "id": "UHJvZHVjdDow",
                    "productName": "Product 1"
                },
                {
                    "id": "UHJvZHVjdDox",
                    "productName": "Product 2"
                }
            ]
        })
    );
}

#[async_test]
async fn query_with_fragment() {
    let query = QueryBuilder::new(
        r#"
            query {
                users {
                    ...User
                }
                products {
                    id
                    ...ProductInfo
                }
            }
            fragment ProductInfo on Product {
                productName: name
            }
            fragment User on User {
                userId: id
                email
                username
                reviews {
                    body
                    author {
                        id
                        email
                    }
                    product {
                        ...ProductInfo
                    }
                }
            }
        "#
        .to_owned(),
    );

    let gateway = common::gateway().await;

    assert_eq!(
        query.execute(&gateway).await.unwrap(),
        json!({
            "users": [
                {
                    "userId": "VXNlcjow",
                    "email": "john@doe.com",
                    "username": null,
                    "reviews": [
                        {
                            "body": "Good product",
                            "author": {
                                "id": "VXNlcjow",
                                "email": "john@doe.com"
                            },
                            "product": {
                                "productName": "Product 1"
                            }
                        },
                        {
                            "body": "Bad product",
                            "author": {
                                "id": "VXNlcjow",
                                "email": "john@doe.com"
                            },
                            "product": {
                                "productName": "Product 2"
                            }
                        }
                    ]
                },
                {
                    "userId": "VXNlcjox",
                    "email": null,
                    "username": "albert",
                    "reviews": [
                        {
                            "body": "Fake description",
                            "author": {
                                "id": "VXNlcjox",
                                "email": null
                            },
                            "product": {
                                "productName": "Product 1"
                            }
                        }
                    ]
                }
            ],
            "products": [
                {
                    "id": "UHJvZHVjdDow",
                    "productName": "Product 1"
                },
                {
                    "id": "UHJvZHVjdDox",
                    "productName": "Product 2"
                }
            ]
        })
    );
}

#[async_test]
async fn query_node() {
    let query = QueryBuilder::new(
        r#"
            query NodeQuery($id: ID!, $ids: [ID!]!, $name: String!) {
                node(id: $id) {
                    id
                    ...on Review {
                        body
                        author {
                            sayHello(name: $name)
                        }
                        product {
                            id
                            name
                        }
                    }
                }
                nodes(ids: $ids) {
                    ...on Review {
                        body
                    }
                }
            }
        "#
        .to_owned(),
    )
    .operation_name("NodeQuery")
    .variables(json!({
        "id": "UmV2aWV3OjA=",
        "ids": ["UmV2aWV3OjA=", "UmV2aWV3OjEwMA==", "UmV2aWV3OjE="],
        "name": "john"
    }));

    let gateway = common::gateway().await;

    assert_eq!(
        query.execute(&gateway).await.unwrap(),
        json!({
            "node": {
                "id": "UmV2aWV3OjA=",
                "body": "Good product",
                "author": {
                    "sayHello": "Hello, john"
                },
                "product": {
                    "id": "UHJvZHVjdDow",
                    "name": "Product 1"
                }
            },
            "nodes": [
                {
                    "body": "Good product",
                },
                null,
                {
                    "body": "Bad product",
                }
            ]
        })
    );
}
