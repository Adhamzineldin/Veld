//! Cross-package equality: the same contract generated into two crates (`orders-client`,
//! `orders-client-b`), a changed shape (`-c`) and a different declaring package (`-d`), plus two
//! models with identical fields (`shop-a`, `shop-b`). Mirrors `tests/toolchain/java/Equality.java`.
//!
//! Every crate has its own `veld::VeldValue`, so values are compared through their identity as
//! plain data (`veld::Identity`, `veld::same_parts`).

use std::collections::BTreeMap;

use harness::check::{equal, that};
use orders_client::veld as va;
use orders_client::veld::{DeclaredError, VeldDecl};
use serde_json::json;

fn sigs(set: &str) -> BTreeMap<String, String> {
    let path = std::env::var("VELD_SIGS").unwrap_or_else(|_| "sigs.json".into());
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_default()).unwrap_or_default();
    v[set].as_object().map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect()).unwrap_or_default()
}

/// The identity (as crate A's plain data) of the declared error `$value` of crate `$krate`, raised
/// by the operation `$op` (`None`: not raised from an endpoint yet).
macro_rules! error_id {
    ($krate:ident, $value:expr, $op:expr) => {{
        use $krate::veld::DeclaredError as _;
        let mut e = $value.into_error();
        if let $krate::veld::Error::Problem(p) = &mut e {
            p.op = $op.map(|s: &str| s.to_string());
        }
        let i = $krate::veld::Identity::of(&e);
        va::Identity { sig: i.sig, op: i.op, canonical: i.canonical }
    }};
}

/// The identity of any value of crate `$krate`, as crate A's plain data.
macro_rules! value_id {
    ($krate:ident, $v:expr) => {{
        let i = $krate::veld::Identity::of(&$v);
        va::Identity { sig: i.sig, op: i.op, canonical: i.canonical }
    }};
}

fn same(a: &va::Identity, b: &va::Identity) -> bool {
    va::same_parts(a, b)
}

type Oos = orders_client::orders::orders::OutOfStock;

fn main() {
    let a = sigs("a");
    let (create, pay) = (a["Orders.createOrder"].clone(), a["Orders.payOrder"].clone());
    let (create, pay) = (create.as_str(), pay.as_str());
    let oos = |sku: &str, op: Option<&str>| error_id!(orders_client, Oos { sku: sku.into() }, op);
    let oos_b = |sku: &str, op: Option<&str>| {
        error_id!(orders_client_b, orders_client_b::orders::orders::OutOfStock { sku: sku.into() }, op)
    };

    // Equal: same signature, operation and payload, across two copies.
    let (x, y) = (oos("X", Some(create)), oos_b("X", Some(create)));
    that(same(&x, &y) && same(&y, &x), "equal errors across copies");
    that(!same(&x, &oos_b("Y", Some(create))), "different data");
    that(!same(&x, &oos_b("X", Some(pay))), "different op");
    that(!same(&x, &oos_b("X", None)), "op set vs unset");
    that(!same(&oos_b("X", None), &x), "op unset vs set");

    // Same prelude error name raised by two different services.
    let nf = |resource: &str, op: &str| {
        error_id!(orders_client, orders_client::prelude::NotFound { resource: Some(resource.into()) }, Some(op))
    };
    let nf_b = |op: &str| {
        error_id!(orders_client_b, orders_client_b::prelude::NotFound { resource: Some("x".into()) }, Some(op))
    };
    that(!same(&nf("x", &a["Orders.getOrder"]), &nf_b(&a["Iam.getUser"])), "same error name, different service");
    that(same(&nf("x", &a["Orders.getOrder"]), &nf_b(&a["Orders.getOrder"])), "same service");

    // Same error name declared in a different package (and so a different signature).
    let d = sigs("d");
    let dd = error_id!(orders_client_d, orders_client_d::warehouse::orders::OutOfStock { sku: "X".into() }, Some(d["Orders.createOrder"].as_str()));
    that(!same(&x, &dd), "different declaring package");

    // A changed contract shape has a different signature.
    let c = sigs("c");
    let cc = error_id!(
        orders_client_c,
        orders_client_c::orders::orders::OutOfStock { sku: "X".into(), warehouse: "w".into() },
        Some(c["Orders.createOrder"].as_str())
    );
    that(Oos::VELD_SIG != orders_client_c::orders::orders::OutOfStock::VELD_SIG, "changed shape, changed sig");
    that(!same(&x, &cc), "changed contract shape");

    // matches: the declaration raised by its own service, from any operation.
    let raised = |sku: &str, op: &str| {
        let mut e = Oos { sku: sku.into() }.into_error();
        if let va::Error::Problem(p) = &mut e {
            p.op = Some(op.to_string());
        }
        e
    };
    that(Oos::matches(&raised("X", create)), "matches");
    that(Oos::matches(&raised("Z", pay)), "matches from another operation of the service");
    that(!Oos::matches(&va::Error::other("x")), "plain errors never match");
    that(Oos::matches_service(&raised("Z", pay), "orders/Orders"), "service filter");
    that(!Oos::matches_service(&raised("Z", pay), "orders/Payments"), "service filter excludes");
    that(!va::same(&va::Error::other("x"), &va::Error::other("x")), "an error without a signature is never the same");
    that(va::same(&raised("X", create), &raised("X", create)), "an error is the same as an equal one");

    // Models: canonical bytes agree across copies and differ when the value differs.
    let m = orders_client::orders::catalog::Money { amount: "1.50".into(), currency: "USD".into() };
    let m_b = |amount: &str| {
        value_id!(orders_client_b, orders_client_b::orders::catalog::Money { amount: amount.into(), currency: "USD".into() })
    };
    that(same(&va::Identity::of(&m), &m_b("1.50")), "models equal across copies");
    that(!same(&va::Identity::of(&m), &m_b("1.5")), "decimal text matters");
    equal(orders_client::orders::catalog::Money::VELD_SIG, orders_client_b::orders::catalog::Money::VELD_SIG, "model signature");

    // Error data: schema defaults are applied before comparing (spec/ir.md 5.3).
    let absent = va::Identity::of(&va::Error::Problem(va::Problem {
        sig: orders_client::orders::orders::Throttled::VELD_SIG.into(),
        title: "Throttled".into(),
        status: 429,
        op: Some(create.into()),
        data: json!({}),
        schema: Some(orders_client::orders::orders::Throttled::schema()),
    }));
    let throttled = |n: i32| error_id!(orders_client_b, orders_client_b::orders::orders::Throttled { retry_after: n }, Some(create));
    that(same(&absent, &throttled(30)), "absent defaulted field equals the default value");
    that(!same(&absent, &throttled(31)), "a different value is different");

    // Companion signatures equal the IR's.
    equal(a["decl:orders:catalog.Money"].as_str(), orders_client::orders::catalog::Money::VELD_SIG, "Money sig");
    equal(a["decl:orders:orders.OutOfStock"].as_str(), Oos::VELD_SIG, "OutOfStock sig");

    models();
    println!("equality ok");
}

/// Model identity: `Item` and `Thing` of the shop contract have the same fields, so the same
/// canonical bytes, but they are two declarations.
fn models() {
    use shop_a::veld as sa;
    use shop_a::veld::{DeclaredError, VeldDecl, VeldValue};
    let item = shop_a::shop::shop::Item { id: "x".into() };
    let thing = shop_a::shop::shop::Thing { id: "x".into() };
    let item_b = shop_b::shop::shop::Item { id: "x".into() };
    let thing_b = shop_b::shop::shop::Thing { id: "x".into() };
    let conv = |i: shop_b::veld::Identity| sa::Identity { sig: i.sig, op: i.op, canonical: i.canonical };
    equal(item.canonical(), thing.canonical(), "identical canonical bytes");
    that(shop_a::shop::shop::Item::VELD_SIG != shop_a::shop::shop::Thing::VELD_SIG, "two declarations, two signatures");
    // Never the same, whatever the data and whichever crate they come from.
    that(!sa::same(&item, &thing) && !sa::same(&thing, &item), "Item vs Thing");
    that(!sa::same_parts(&sa::Identity::of(&item), &conv(shop_b::veld::Identity::of(&thing_b))), "Item vs Thing across crates");
    // The same declaration with equal data is the same, across separately generated crates.
    that(sa::same_parts(&sa::Identity::of(&item), &conv(shop_b::veld::Identity::of(&item_b))), "Item across crates");
    that(sa::same_parts(&sa::Identity::of(&thing), &conv(shop_b::veld::Identity::of(&thing_b))), "Thing across crates");
    let other = shop_b::shop::shop::Item { id: "y".into() };
    that(!sa::same_parts(&sa::Identity::of(&item), &conv(shop_b::veld::Identity::of(&other))), "different data");
    // A decoded value against a constructed one.
    let decoded = shop_a::shop::shop::Item::from_wire(&json!({ "id": "x" })).unwrap_or_else(|e| panic!("{e}"));
    that(sa::same_parts(&sa::Identity::of(&decoded), &conv(shop_b::veld::Identity::of(&item_b))), "decoded vs constructed");
    // An error against a model with identical data.
    let gone = shop_a::shop::shop::Gone { id: "x".into() }.into_error();
    equal(gone.canonical(), item.canonical(), "identical data");
    that(!sa::same(&gone, &item) && !sa::same(&item, &gone), "error vs model");
    // Lists compare element by element.
    that(sa::same_all(&[item.clone()], &[item.clone()]), "lists");
    that(!sa::same_all(&[item.clone()], &[shop_a::shop::shop::Item { id: "y".into() }]), "lists of different values");
}
