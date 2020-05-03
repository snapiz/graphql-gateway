mod common;

use common::{account, inventory, product, review};
use futures_await_test::async_test;
use graphql_gateway::{Data, Payload};
use serde_json::json;

#[async_test]
async fn query_executor() {
    let account = account::EXECUTOR.clone();
    let inventory = inventory::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let gateway = graphql_gateway::from_executors(vec![&account, &inventory, &product, &review])
        .await
        .unwrap();

    assert_eq!(
        graphql_gateway::execute(
            &gateway,
            &Data::default(),
            &Payload {
                query: r#"
                    {
                        products {
                            id
                            name
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
            "products": [
                { "id": "UHJvZHVjdDow", "name": "Product 1" },
                { "id": "UHJvZHVjdDox", "name": "Product 2" }
            ]
        })
    );
}

#[async_test]
async fn query_executors() {
    let account = account::EXECUTOR.clone();
    let inventory = inventory::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let gateway = graphql_gateway::from_executors(vec![&account, &inventory, &product, &review])
        .await
        .unwrap();

    assert_eq!(
        graphql_gateway::execute(
            &gateway,
            &Data::default(),
            &Payload {
                query: r#"
                    query {
                        viewer {
                            email
                            username
                            reviews {
                                body
                                author {
                                    id
                                    username
                                }
                                product {
                                    name
                                }
                            }
                        }
                        products {
                            id
                            name
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
            "viewer": {
                "email": "john@doe.com",
                "username": "john",
                "reviews": [
                    {
                        "body": "Good product",
                        "author": {
                            "id": "VXNlcjow",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 1"
                        }
                    },
                    {
                        "body": "Bad product",
                        "author": {
                            "id": "VXNlcjow",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 2"
                        }
                    }
                ]
            },
            "products": [
                { "id": "UHJvZHVjdDow", "name": "Product 1" },
                { "id": "UHJvZHVjdDox", "name": "Product 2" }
            ]
        })
    );
}

#[async_test]
async fn query_batch() {
    let account = account::EXECUTOR.clone();
    let inventory = inventory::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let gateway = graphql_gateway::from_executors(vec![&account, &inventory, &product, &review])
        .await
        .unwrap();

    assert_eq!(
        graphql_gateway::execute(
            &gateway,
            &Data::default(),
            &Payload {
                query: r#"
                    query {
                        users {
                            email
                            username
                            reviews {
                                body
                                author {
                                    id
                                    username
                                }
                                product {
                                    name
                                }
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
            "users": [
                {
                    "email": "john@doe.com",
                    "username": "john",
                    "reviews": [
                        {
                            "body": "Good product",
                            "author": {
                                "id": "VXNlcjow",
                                "username": "john"
                            },
                            "product": {
                                "name": "Product 1"
                            }
                        },
                        {
                            "body": "Bad product",
                            "author": {
                                "id": "VXNlcjow",
                                "username": "john"
                            },
                            "product": {
                                "name": "Product 2"
                            }
                        }
                    ]
                },
                {
                    "email": "albert@dupont.com",
                    "username": "albert",
                    "reviews": [
                        {
                            "body": "Fake description",
                            "author": {
                                "id": "VXNlcjox",
                                "username": "albert"
                            },
                            "product": {
                                "name": "Product 1"
                            }
                        }
                    ]
                }
            ]
        })
    );
}

#[async_test]
async fn query_node() {
    let account = account::EXECUTOR.clone();
    let inventory = inventory::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let gateway = graphql_gateway::from_executors(vec![&account, &inventory, &product, &review])
        .await
        .unwrap();

    assert_eq!(
        graphql_gateway::execute(
            &gateway,
            &Data::default(),
            &Payload {
                query: r#"
                    query {
                        node(id: "VXNlcjow") {
                            ... on User {
                                email
                                username
                                reviews {
                                    body
                                    author {
                                        id
                                        username
                                    }
                                    product {
                                        name
                                    }
                                }
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
                "email": "john@doe.com",
                "username": "john",
                "reviews": [
                    {
                        "body": "Good product",
                        "author": {
                            "id": "VXNlcjow",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 1"
                        }
                    },
                    {
                        "body": "Bad product",
                        "author": {
                            "id": "VXNlcjow",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 2"
                        }
                    }
                ]
            }
        })
    );
}

#[async_test]
async fn query_nodes_batch() {
    let account = account::EXECUTOR.clone();
    let inventory = inventory::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let gateway = graphql_gateway::from_executors(vec![&account, &inventory, &product, &review])
        .await
        .unwrap();

    assert_eq!(
        graphql_gateway::execute(
            &gateway,
            &Data::default(),
            &Payload {
                query: r#"
                    query {
                        nodes(ids: ["VXNlcjow", "VXNlcjoxMA=="]) {
                            ... on User {
                                email
                                username
                                reviews {
                                    body
                                    author {
                                        id
                                        username
                                    }
                                    product {
                                        name
                                    }
                                }
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
            "nodes": [{
                "email": "john@doe.com",
                "username": "john",
                "reviews": [
                    {
                        "body": "Good product",
                        "author": {
                            "id": "VXNlcjow",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 1"
                        }
                    },
                    {
                        "body": "Bad product",
                        "author": {
                            "id": "VXNlcjow",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 2"
                        }
                    }
                ]
            }, null]
        })
    );
}

#[async_test]
async fn query_nodes() {
    let account = account::EXECUTOR.clone();
    let inventory = inventory::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let gateway = graphql_gateway::from_executors(vec![&account, &inventory, &product, &review])
        .await
        .unwrap();

    assert_eq!(
        graphql_gateway::execute(
            &gateway,
            &Data::default(),
            &Payload {
                query: r#"
                    query {
                        nodes(ids: ["UmV2aWV3OjA=", "UmV2aWV3OjEw"]) {
                            ... on Review {
                                body
                                author {
                                    id
                                    username
                                }
                                product {
                                    name
                                }
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
            "nodes": [{
                "body": "Good product",
                "author": {
                    "id": "VXNlcjow",
                    "username": "john"
                },
                "product": {
                    "name": "Product 1"
                }
            }, null]
        })
    );
}

#[async_test]
async fn query_alias() {
    let account = account::EXECUTOR.clone();
    let inventory = inventory::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let gateway = graphql_gateway::from_executors(vec![&account, &inventory, &product, &review])
        .await
        .unwrap();

    assert_eq!(
        graphql_gateway::execute(
            &gateway,
            &Data::default(),
            &Payload {
                query: r#"
                    query {
                        user: node(id: "VXNlcjow") {
                            ... on User {
                                userEmail: email
                                username
                                reviews {
                                    body
                                    author {
                                        id
                                        username
                                    }
                                    product {
                                        name
                                    }
                                }
                            }
                        }
                        review: node(id: "UmV2aWV3OjA=") {
                            ... on Review {
                                body
                                author {
                                    id
                                    username
                                }
                                product {
                                    name
                                }
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
            "user": {
                "userEmail": "john@doe.com",
                "username": "john",
                "reviews": [
                    {
                        "body": "Good product",
                        "author": {
                            "id": "VXNlcjow",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 1"
                        }
                    },
                    {
                        "body": "Bad product",
                        "author": {
                            "id": "VXNlcjow",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 2"
                        }
                    }
                ]
            },
            "review": {
                "body": "Good product",
                "author": {
                    "id": "VXNlcjow",
                    "username": "john"
                },
                "product": {
                    "name": "Product 1"
                }
            }
        })
    );
}

#[async_test]
async fn query_variables() {
    let account = account::EXECUTOR.clone();
    let inventory = inventory::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let gateway = graphql_gateway::from_executors(vec![&account, &inventory, &product, &review])
        .await
        .unwrap();

    assert_eq!(
        graphql_gateway::execute(
            &gateway,
            &Data::default(),
            &Payload {
                query: r#"
                    query NodeQuery ($userId: ID!, $reviewId: ID!, $name: String!) {
                        user: node(id: $userId) {
                            ... on User {
                                userEmail: email
                                username
                                reviews {
                                    body
                                    author {
                                        id
                                        sayHello(name: $name)
                                        username
                                    }
                                    product {
                                        name
                                    }
                                }
                            }
                        }
                        review: node(id: $reviewId) {
                            ... on Review {
                                body
                                author {
                                    id
                                    username
                                }
                                product {
                                    name
                                }
                            }
                        }
                    }
                "#
                .to_owned(),
                operation_name: Some("NodeQuery".to_owned()),
                variables: Some(json!({
                    "userId": "VXNlcjow",
                    "reviewId": "UmV2aWV3OjA=",
                    "name": "John"
                })),
            }
        )
        .await
        .unwrap(),
        json!({
            "user": {
                "userEmail": "john@doe.com",
                "username": "john",
                "reviews": [
                    {
                        "body": "Good product",
                        "author": {
                            "id": "VXNlcjow",
                            "sayHello": "Hello, John",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 1"
                        }
                    },
                    {
                        "body": "Bad product",
                        "author": {
                            "id": "VXNlcjow",
                            "sayHello": "Hello, John",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 2"
                        }
                    }
                ]
            },
            "review": {
                "body": "Good product",
                "author": {
                    "id": "VXNlcjow",
                    "username": "john"
                },
                "product": {
                    "name": "Product 1"
                }
            }
        })
    );
}

#[async_test]
async fn query_fragment_spread() {
    let account = account::EXECUTOR.clone();
    let inventory = inventory::EXECUTOR.clone();
    let product = product::EXECUTOR.clone();
    let review = review::EXECUTOR.clone();

    let gateway = graphql_gateway::from_executors(vec![&account, &inventory, &product, &review])
        .await
        .unwrap();

    assert_eq!(
        graphql_gateway::execute(
            &gateway,
            &Data::default(),
            &Payload {
                query: r#"
                    query FragmentSpreadQuery ($userId: ID!) {
                        user: node(id: $userId) {
                            ... on User {
                                ...UserInfo
                            }
                        }
                        viewer {
                            ...UserInfo
                        }
                    }
                    fragment UserInfo on User {
                        ...Author
                        reviews {
                            body
                            author {
                                ...Author
                            }
                            product {
                                name
                            }
                        }
                    }
                    fragment Author on User {
                        id
                        ...AuthorField
                    }
                    fragment AuthorField on User {
                        email
                        username
                    }
                "#
                .to_owned(),
                operation_name: Some("FragmentSpreadQuery".to_owned()),
                variables: Some(json!({
                    "userId": "VXNlcjow"
                })),
            }
        )
        .await
        .unwrap(),
        json!({
            "user": {
                "id": "VXNlcjow",
                "email": "john@doe.com",
                "username": "john",
                "reviews": [
                    {
                        "body": "Good product",
                        "author": {
                            "id": "VXNlcjow",
                            "email": "john@doe.com",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 1"
                        }
                    },
                    {
                        "body": "Bad product",
                        "author": {
                            "id": "VXNlcjow",
                            "email": "john@doe.com",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 2"
                        }
                    }
                ]
            },
            "viewer": {
                "id": "VXNlcjow",
                "email": "john@doe.com",
                "username": "john",
                "reviews": [
                    {
                        "body": "Good product",
                        "author": {
                            "id": "VXNlcjow",
                            "email": "john@doe.com",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 1"
                        }
                    },
                    {
                        "body": "Bad product",
                        "author": {
                            "id": "VXNlcjow",
                            "email": "john@doe.com",
                            "username": "john"
                        },
                        "product": {
                            "name": "Product 2"
                        }
                    }
                ]
            }
        })
    );
}
