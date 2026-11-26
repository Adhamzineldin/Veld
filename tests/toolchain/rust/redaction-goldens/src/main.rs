//! The goldens of `tests/wire/redaction/cases.json` through `veld::platform::redact` of the
//! generated crate: the canonical bytes of what is kept and the paths that were left out. The
//! contract is the goldens' own with an audited service appended (a crate that does not audit has
//! no `@pii` marks in its schemas). Usage: `redaction-goldens <cases.json>`; prints
//! `redaction goldens ok: <n>`.

use std::collections::BTreeSet;

use guests_server::guests::guests::{Address, Guest, GuestRegistered, Order, Rejected, Staff};
use guests_server::veld::VeldDecl;
use guests_server::veld::canonical::canonical;
use guests_server::veld::platform::redact;
use guests_server::veld::schema::Schema;
use serde_json::Value;

fn schema_of(name: &str) -> &'static Schema {
    match name {
        "Address" => Address::schema(),
        "Guest" => Guest::schema(),
        "Staff" => Staff::schema(),
        "Order" => Order::schema(),
        "Rejected" => Rejected::schema(),
        "GuestRegistered" => GuestRegistered::schema(),
        other => panic!("the contract has no declaration `{other}`"),
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("the path of cases.json");
    let file: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("cases.json")).expect("JSON");
    let cases = file["cases"].as_array().expect("cases");
    for case in cases {
        let name = case["name"].as_str().unwrap_or("?");
        let schema = schema_of(case["type"].as_str().expect("type"));
        let mut paths = BTreeSet::new();
        let kept = redact(schema, &case["value"], "$", &mut paths).unwrap_or_else(|why| panic!("{name}: {why}"));
        assert_eq!(canonical(&kept), case["redacted"].as_str().expect("redacted"), "{name}: the bytes of what is kept");
        let want: Vec<String> = case["paths"].as_array().expect("paths").iter().filter_map(|p| p.as_str().map(str::to_string)).collect();
        assert_eq!(paths.into_iter().collect::<Vec<_>>(), want, "{name}: the paths that were left out");
    }
    println!("redaction goldens ok: {}", cases.len());
}
