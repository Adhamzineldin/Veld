//! Wire goldens: `tests/wire/*.json`, checked through the generated runtime and declarations. The
//! same goldens are checked against the generated TypeScript and Java, so every language produces
//! the same canonical bytes for the same values.

use harness::check::{equal, that};
use orders_client::orders::catalog::{Money, Payment};
use orders_client::orders::orders::{CreateOrder, Order, OrderFilter};
use orders_client::veld::canonical::canonical;
use orders_client::veld::{VeldDecl, VeldValue};
use serde_json::Value;

/// Typed route: decode the wire value into the generated type and take its canonical bytes.
fn typed(decl: &str, wire: &Value) -> Option<(Vec<u8>, Vec<String>)> {
    fn go<T: VeldDecl>(wire: &Value) -> (Vec<u8>, Vec<String>) {
        let issues = T::validate(wire).into_iter().map(|i| format!("{}:{}", i.path, i.code)).collect();
        let bytes = T::from_wire(wire).map(|v| v.canonical()).unwrap_or_default();
        (bytes, issues)
    }
    Some(match decl {
        "Money" => go::<Money>(wire),
        "CreateOrder" => go::<CreateOrder>(wire),
        "Order" => go::<Order>(wire),
        "OrderFilter" => go::<OrderFilter>(wire),
        "Payment" => go::<Payment>(wire),
        _ => return None,
    })
}

fn main() {
    let dir = std::env::var("WIRE_DIR").unwrap_or_else(|_| panic!("set WIRE_DIR"));
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    let mut checked = 0;
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let cases: Vec<Value> = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        for case in cases {
            let name = format!("{}: {}", file.file_name().and_then(|n| n.to_str()).unwrap_or("?"), case["name"].as_str().unwrap_or("?"));
            let expected = case["canonical"].as_str().unwrap_or_default();
            if case["kind"] == "json" {
                equal(expected, canonical(&case["input"]).as_str(), &name);
            } else {
                let decl = case["decl"].as_str().unwrap_or_default();
                let (bytes, issues) = typed(decl, &case["input"]).unwrap_or_else(|| panic!("{name}: no declaration {decl}"));
                that(issues.is_empty(), &format!("{name} validates: {issues:?}"));
                equal(expected, String::from_utf8_lossy(&bytes).as_ref(), &format!("{name} (typed)"));
            }
            checked += 1;
        }
    }
    that(checked > 0, "no goldens found");
    println!("wire ok ({checked} cases)");
}
