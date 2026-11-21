//! The blocking `ureq` client: `BASE=http://127.0.0.1:3000 ureq_client`. Its futures never wait, so
//! `veld::block_on` drives them without an async runtime.

use orders_client_ureq::orders::catalog::{OrderItem, Payment, Status};
use orders_client_ureq::orders::orders::CreateOrder;
use orders_client_ureq::orders::orders_client::{CreateOrderRequest, GetOrderRequest, OrdersClient};
use orders_client_ureq::prelude::{Internal, NotFound, Unauthorized};
use orders_client_ureq::veld::client::ClientOptions;
use orders_client_ureq::veld::{DeclaredError, block_on};

use harness::check::{equal, that};

fn main() {
    let base = std::env::var("BASE").unwrap_or_else(|_| panic!("set BASE to the server's base URL"));
    let client = OrdersClient::ureq(ClientOptions::new().base_url(&base).bearer(|| "good".to_string())).unwrap_or_else(|e| panic!("{e}"));
    block_on(client.health()).unwrap_or_else(|e| panic!("health: {e}"));
    let created = block_on(client.create_order(CreateOrderRequest {
        body: CreateOrder {
            customer_id: "11111111-1111-4111-8111-111111111111".into(),
            items: vec![OrderItem { sku: "ABC-1".into(), quantity: 2 }],
            note: None,
            payment: Some(Payment::Card(orders_client_ureq::orders::catalog::Card { last4: "4242".into() })),
        },
        idempotency_key: Some("key-1".into()),
    }))
    .unwrap_or_else(|e| panic!("create: {e}"));
    equal(Status::Pending, created.status, "default status");
    let fetched = block_on(client.get_order(GetOrderRequest { id: created.id.clone() })).unwrap_or_else(|e| panic!("get: {e}"));
    equal(created, fetched, "fetched equals created");
    let missing = block_on(client.get_order(GetOrderRequest { id: "00000000-0000-4000-8000-0000000000aa".into() })).err();
    that(missing.as_ref().is_some_and(NotFound::matches), "declared error");
    let boom = block_on(client.boom()).err();
    that(boom.as_ref().is_some_and(Internal::matches), "undeclared failure is Internal");
    let anon = OrdersClient::ureq(ClientOptions::new().base_url(&base)).unwrap_or_else(|e| panic!("{e}"));
    let denied = block_on(anon.get_order(GetOrderRequest { id: created_id() })).err();
    that(denied.as_ref().is_some_and(Unauthorized::matches), "401 without a token");
    println!("ureq ok");
}

fn created_id() -> String {
    "00000000-0000-4000-8000-0000000000bb".into()
}
