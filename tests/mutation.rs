mod common;

use common::{account, inventory, product, review};
use futures_await_test::async_test;
use graphql_gateway::{Data, Payload};
use serde_json::json;

#[async_test]
async fn mutation_executor() {
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
                    mutation {
                        addProduct(id: "UHJvZHVjdDow") {
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
            "addProduct": { "id": "UHJvZHVjdDow", "name": "Product 1" }
        })
    );
}
