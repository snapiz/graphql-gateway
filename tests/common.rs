use async_graphql::http::GQLResponse;
use async_graphql::{ObjectType, Schema, SubscriptionType, Variables, ID, QueryBuilder};
use async_trait::async_trait;
use base64::DecodeError;
use graphql_gateway::{Executor, Payload};
use serde_json::Value;
use std::convert::From;
use std::num::ParseIntError;
use std::str::{from_utf8, Utf8Error};

pub enum Error {
    DecodeError(DecodeError),
    Utf8Error(Utf8Error),
    ParseIntError(ParseIntError),
    ParseGlobalId,
}

impl From<DecodeError> for Error {
    fn from(e: DecodeError) -> Error {
        Error::DecodeError(e)
    }
}

impl From<Utf8Error> for Error {
    fn from(e: Utf8Error) -> Error {
        Error::Utf8Error(e)
    }
}

impl From<ParseIntError> for Error {
    fn from(e: ParseIntError) -> Error {
        Error::ParseIntError(e)
    }
}

pub fn to_global_id(name: &str, id: usize) -> ID {
    let encoded = base64::encode(format!("{}:{}", name, id));

    return ID::from(encoded);
}

pub fn from_global_id(id: &ID) -> Result<(String, usize), Error> {
    let decoded = &base64::decode(id.as_str())?;
    let decoded = from_utf8(decoded)?;
    let data: Vec<_> = decoded.splitn(2, ":").collect();

    if data.len() != 2 {
        return Err(Error::ParseGlobalId);
    }

    let decoded_type = data[0].to_string();
    let decoded_id = data[1].parse::<usize>()?;

    return Ok((decoded_type, decoded_id));
}

pub struct TestExecutor<'a, Q, M, S>(&'a str, Schema<Q, M, S>)
where
    Q: ObjectType + Send + Sync + 'static,
    M: ObjectType + Send + Sync + 'static,
    S: SubscriptionType + Send + Sync + 'static;

impl<'a, Q, M, S> TestExecutor<'a, Q, M, S>
where
    Q: ObjectType + Send + Sync + 'static,
    M: ObjectType + Send + Sync + 'static,
    S: SubscriptionType + Send + Sync + 'static,
{
    pub fn new(name: &'a str, query: Q, mutation: M, subscription: S) -> Self {
        TestExecutor(name, Schema::new(query, mutation, subscription))
    }
}

impl<'a, Q, M, S> Clone for TestExecutor<'a, Q, M, S>
where
    Q: ObjectType + Send + Sync + 'static,
    M: ObjectType + Send + Sync + 'static,
    S: SubscriptionType + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        TestExecutor(self.0, self.1.clone())
    }
}

#[async_trait]
impl<'a, Q, M, S> Executor for TestExecutor<'a, Q, M, S>
where
    Q: ObjectType + Send + Sync + 'static,
    M: ObjectType + Send + Sync + 'static,
    S: SubscriptionType + Send + Sync + 'static,
{
    fn name(&self) -> String {
        self.0.to_owned()
    }

    async fn execute(&self, payload: &Payload) -> graphql_gateway::Result<Value> {
        let mut builder = QueryBuilder::new(&payload.query);

        if let Some(operation_name) = &payload.operation_name {
            builder = builder.operator_name(operation_name);
        }

        if let Some(variables) = &payload.variables {
            if let Ok(variables) = Variables::parse_from_json(variables.clone()) {
                builder = builder.variables(variables);
            }
        }

        Ok(serde_json::to_value(GQLResponse(builder.execute(&self.1).await)).unwrap())
    }
}

pub mod account {
    use async_graphql::{EmptyMutation, EmptySubscription, ID};

    use super::TestExecutor;

    #[derive(Clone)]
    pub struct User(usize, String, String);

    #[async_graphql::Object]
    impl User {
        #[field]
        async fn id(&self) -> ID {
            super::to_global_id("User", self.0)
        }

        #[field]
        async fn email(&self) -> &str {
            &self.1
        }

        #[field]
        async fn username(&self) -> &str {
            &self.2
        }

        #[field]
        async fn say_hello(&self, name: String) -> String {
            format!("Hello, {}", name)
        }
    }

    lazy_static::lazy_static! {
        pub static ref USERS: Vec<User> = vec![
            User(0, "john@doe.com".to_owned(), "john".to_owned()),
            User(1, "albert@dupont.com".to_owned(), "albert".to_owned())
            ];
    }

    #[async_graphql::Interface(field(name = "id", type = "ID"))]
    pub struct Node(User);

    pub struct Query;

    #[async_graphql::Object]
    impl Query {
        #[field]
        async fn node(&self, id: ID) -> Option<Node> {
            let (node_type, id) = match super::from_global_id(&id) {
                Ok((node_type, id)) => (node_type, id),
                _ => return None,
            };

            match node_type.as_str() {
                "User" => USERS.clone().get(id).map(|u| Node::User(u.clone())),
                _ => None,
            }
        }

        #[field]
        async fn nodes(&self, ids: Vec<ID>) -> Vec<Option<Node>> {
            let node_type = match super::from_global_id(&ids[0]) {
                Ok((node_type, _)) => node_type,
                _ => return vec![],
            };

            ids.iter()
                .map(|node_id| {
                    let id = match super::from_global_id(&node_id) {
                        Ok((_, id)) => id,
                        _ => return None,
                    };

                    match node_type.as_str() {
                        "User" => USERS.get(id).map(|u| Node::User(u.clone())),
                        _ => None,
                    }
                })
                .collect()
        }

        #[field]
        async fn viewer(&self) -> Option<User> {
            USERS.clone().get(0).map(|u| u.clone())
        }

        #[field]
        async fn users(&self) -> Vec<&User> {
            USERS.iter().collect()
        }
    }

    lazy_static::lazy_static! {
        pub static ref EXECUTOR: TestExecutor<'static, Query, EmptyMutation, EmptySubscription> = TestExecutor::new("account", Query {}, EmptyMutation, EmptySubscription);
    }
}

pub mod inventory {
    use async_graphql::{EmptyMutation, EmptySubscription, ID};

    use super::TestExecutor;

    #[derive(Clone)]
    pub struct Product(usize, bool);

    #[async_graphql::Object]
    impl Product {
        #[field]
        async fn id(&self) -> ID {
            super::to_global_id("Product", self.0)
        }

        #[field]
        async fn in_stock(&self) -> bool {
            self.1
        }
    }

    lazy_static::lazy_static! {
        pub static ref PRODUCTS: Vec<Product> = vec![
            Product(0, true),
            Product(1, false)
            ];
    }

    #[async_graphql::Interface(field(name = "id", type = "ID"))]
    pub struct Node(Product);

    pub struct Query;

    #[async_graphql::Object]
    impl Query {
        #[field]
        async fn node(&self, id: ID) -> Option<Node> {
            let (node_type, id) = match super::from_global_id(&id) {
                Ok((node_type, id)) => (node_type, id),
                _ => return None,
            };

            match node_type.as_str() {
                "Product" => PRODUCTS.clone().get(id).map(|u| Node::Product(u.clone())),
                _ => None,
            }
        }

        #[field]
        async fn nodes(&self, ids: Vec<ID>) -> Vec<Option<Node>> {
            let node_type = match super::from_global_id(&ids[0]) {
                Ok((node_type, _)) => node_type,
                _ => return vec![],
            };

            ids.iter()
                .map(|node_id| {
                    let id = match super::from_global_id(&node_id) {
                        Ok((_, id)) => id,
                        _ => return None,
                    };

                    match node_type.as_str() {
                        "Product" => PRODUCTS.get(id).map(|u| Node::Product(u.clone())),
                        _ => None,
                    }
                })
                .collect()
        }
    }

    lazy_static::lazy_static! {
        pub static ref EXECUTOR: TestExecutor<'static, Query, EmptyMutation, EmptySubscription> = TestExecutor::new("inventory", Query {}, EmptyMutation, EmptySubscription);
    }
}


pub mod inventory_updated {
    use async_graphql::{EmptyMutation, EmptySubscription, ID};

    use super::TestExecutor;

    #[derive(Clone)]
    pub struct Product(usize, bool);

    #[async_graphql::Object]
    impl Product {
        #[field]
        async fn id(&self) -> ID {
            super::to_global_id("Product", self.0)
        }

        #[field]
        async fn delivered(&self) -> bool {
            self.1
        }
    }

    lazy_static::lazy_static! {
        pub static ref PRODUCTS: Vec<Product> = vec![
            Product(0, true),
            Product(1, false)
            ];
    }

    #[async_graphql::Interface(field(name = "id", type = "ID"))]
    pub struct Node(Product);

    pub struct Query;

    #[async_graphql::Object]
    impl Query {
        #[field]
        async fn node(&self, id: ID) -> Option<Node> {
            let (node_type, id) = match super::from_global_id(&id) {
                Ok((node_type, id)) => (node_type, id),
                _ => return None,
            };

            match node_type.as_str() {
                "Product" => PRODUCTS.clone().get(id).map(|u| Node::Product(u.clone())),
                _ => None,
            }
        }

        #[field]
        async fn nodes(&self, ids: Vec<ID>) -> Vec<Option<Node>> {
            let node_type = match super::from_global_id(&ids[0]) {
                Ok((node_type, _)) => node_type,
                _ => return vec![],
            };

            ids.iter()
                .map(|node_id| {
                    let id = match super::from_global_id(&node_id) {
                        Ok((_, id)) => id,
                        _ => return None,
                    };

                    match node_type.as_str() {
                        "Product" => PRODUCTS.get(id).map(|u| Node::Product(u.clone())),
                        _ => None,
                    }
                })
                .collect()
        }
    }

    lazy_static::lazy_static! {
        pub static ref EXECUTOR: TestExecutor<'static, Query, EmptyMutation, EmptySubscription> = TestExecutor::new("inventory", Query {}, EmptyMutation, EmptySubscription);
    }
}

pub mod product {
    use async_graphql::{EmptyMutation, EmptySubscription, ID};

    use super::TestExecutor;

    #[derive(Clone, Debug)]
    pub struct Product(usize, String);

    #[async_graphql::Object]
    impl Product {
        #[field]
        async fn id(&self) -> ID {
            super::to_global_id("Product", self.0)
        }

        #[field]
        async fn name(&self) -> &str {
            &self.1
        }
    }

    lazy_static::lazy_static! {
        pub static ref PRODUCTS: Vec<Product> = vec![
            Product(0, "Product 1".to_owned()),
            Product(1, "Product 2".to_owned())
            ];
    }

    #[async_graphql::Interface(field(name = "id", type = "ID"))]
    pub struct Node(Product);

    pub struct Query;

    #[async_graphql::Object]
    impl Query {
        #[field]
        async fn node(&self, id: ID) -> Option<Node> {
            let (node_type, id) = match super::from_global_id(&id) {
                Ok((node_type, id)) => (node_type, id),
                _ => return None,
            };

            match node_type.as_str() {
                "Product" => PRODUCTS.clone().get(id).map(|u| Node::Product(u.clone())),
                _ => None,
            }
        }

        #[field]
        async fn nodes(&self, ids: Vec<ID>) -> Vec<Option<Node>> {
            let node_type = match super::from_global_id(&ids[0]) {
                Ok((node_type, _)) => node_type,
                _ => return vec![],
            };

            ids.iter()
                .map(|node_id| {
                    let id = match super::from_global_id(&node_id) {
                        Ok((_, id)) => id,
                        _ => return None,
                    };

                    match node_type.as_str() {
                        "Product" => PRODUCTS.get(id).map(|u| Node::Product(u.clone())),
                        _ => None,
                    }
                })
                .collect()
        }

        #[field]
        async fn products(&self) -> Vec<&Product> {
            PRODUCTS.iter().collect()
        }
    }

    lazy_static::lazy_static! {
        pub static ref EXECUTOR: TestExecutor<'static, Query, EmptyMutation, EmptySubscription> = TestExecutor::new("product", Query {}, EmptyMutation, EmptySubscription);
    }
}

pub mod review {
    use async_graphql::{EmptyMutation, EmptySubscription, ID};

    use super::TestExecutor;

    #[derive(Clone)]
    pub struct User(usize);

    #[async_graphql::Object]
    impl User {
        #[field]
        async fn id(&self) -> ID {
            super::to_global_id("User", self.0)
        }

        #[field]
        async fn reviews(&self) -> Vec<&Review> {
            REVIEWS.iter().filter(|r| r.1 == self.0).collect()
        }
    }

    #[derive(Clone)]
    pub struct Review(usize, usize, usize, String);

    #[async_graphql::Object]
    impl Review {
        #[field]
        async fn id(&self) -> ID {
            super::to_global_id("Review", self.0)
        }

        #[field]
        async fn body(&self) -> &str {
            &self.3
        }

        #[field]
        async fn author(&self) -> Option<User> {
            Some(User(self.1))
        }

        #[field]
        async fn product(&self) -> Option<Product> {
            Some(Product(self.2))
        }
    }

    #[derive(Clone)]
    pub struct Product(usize);

    #[async_graphql::Object]
    impl Product {
        #[field]
        async fn id(&self) -> ID {
            super::to_global_id("Product", self.0)
        }

        #[field]
        async fn reviews(&self) -> Vec<&Review> {
            REVIEWS.iter().filter(|r| r.2 == self.0).collect()
        }
    }

    lazy_static::lazy_static! {
        pub static ref REVIEWS: Vec<Review> = vec![
            Review(0, 0, 0, "Good product".to_owned()),
            Review(1, 0, 1, "Bad product".to_owned()),
            Review(2, 1, 0, "Fake description".to_owned())
            ];
    }

    #[async_graphql::Interface(field(name = "id", type = "ID"))]
    pub struct Node(Review, Product, User);

    pub struct Query;

    #[async_graphql::Object]
    impl Query {
        #[field]
        async fn node(&self, id: ID) -> Option<Node> {
            let (node_type, id) = match super::from_global_id(&id) {
                Ok((node_type, id)) => (node_type, id),
                _ => return None,
            };

            match node_type.as_str() {
                "Review" => REVIEWS.clone().get(id).map(|u| Node::Review(u.clone())),
                "User" => Some(Node::User(User(id))),
                "Product" => Some(Node::Product(Product(id))),
                _ => None,
            }
        }

        #[field]
        async fn nodes(&self, ids: Vec<ID>) -> Vec<Option<Node>> {
            let node_type = match super::from_global_id(&ids[0]) {
                Ok((node_type, _)) => node_type,
                _ => return vec![],
            };

            ids.iter()
                .map(|node_id| {
                    let id = match super::from_global_id(&node_id) {
                        Ok((_, id)) => id,
                        _ => return None,
                    };

                    match node_type.as_str() {
                        "Review" => REVIEWS.get(id).map(|u| Node::Review(u.clone())),
                        "User" => Some(Node::User(User(id))),
                        "Product" => Some(Node::Product(Product(id))),
                        _ => None,
                    }
                })
                .collect()
        }
    }

    lazy_static::lazy_static! {
        pub static ref EXECUTOR: TestExecutor<'static, Query, EmptyMutation, EmptySubscription> = TestExecutor::new("review", Query {}, EmptyMutation, EmptySubscription);
    }
}
