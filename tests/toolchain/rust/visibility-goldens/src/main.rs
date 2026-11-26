//! The goldens of `tests/wire/visibility/cases.json` through the generated code: for each case the
//! value, the caller and the clock go through `veld::platform::show_value`, and the canonical bytes
//! of what is shown and the paths that were dropped must be the case's. The contract is the
//! goldens' own with a service appended that returns each of its declarations (so that the server
//! has the feature). Usage: `visibility-goldens <cases.json>`; prints `visibility goldens ok: <n>`.

use guests_server::guests::guests::{Address, Card, Guest, Order, Payment, Rejected, User, Vip, Wire};
use guests_server::veld::VeldDecl;
use guests_server::veld::canonical::canonical;
use guests_server::veld::platform::show_value;
use guests_server::veld::rules::instant::parse_datetime;
use guests_server::veld::schema::Schema;
use guests_server::veld::server::Principal;
use serde_json::Value;

fn schema_of(name: &str) -> &'static Schema {
    match name {
        "Address" => Address::schema(),
        "Guest" => Guest::schema(),
        "Vip" => Vip::schema(),
        "Card" => Card::schema(),
        "Wire" => Wire::schema(),
        "Payment" => Payment::schema(),
        "Order" => Order::schema(),
        "Rejected" => Rejected::schema(),
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
        let caller = match &case["caller"] {
            Value::Null => None,
            user => Some(Principal::new(serde_json::from_value::<User>(user.clone()).expect("a caller"))),
        };
        let now = parse_datetime(case["now"].as_str().expect("now")).expect("a time");
        let shown = show_value(schema, &case["value"], caller.as_ref(), now, "$");
        let want = case["shown"].as_str().expect("shown");
        assert_eq!(canonical(&shown.value), want, "{name}: the bytes of what is shown");
        let hidden: Vec<String> = case["hidden"].as_array().expect("hidden").iter().filter_map(|p| p.as_str().map(str::to_string)).collect();
        assert_eq!(shown.hidden, hidden, "{name}: the paths that were dropped");
    }
    println!("visibility goldens ok: {}", cases.len());
}
