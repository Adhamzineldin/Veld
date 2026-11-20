//! Helpers shared by the CLI tests.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

/// A `veld` command running in `dir`, with colour and the host environment out of the way.
pub fn veld(dir: &Path) -> Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("veld");
    cmd.current_dir(dir)
        .env("NO_COLOR", "1")
        .env_remove("VELD_REQUIRE_TOOLCHAINS");
    cmd
}

/// Every file below `dir` as (relative path with `/`, bytes), sorted.
pub fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else if let Ok(rel) = path.strip_prefix(root) {
                out.insert(
                    rel.to_string_lossy().replace('\\', "/"),
                    fs::read(&path).unwrap_or_default(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

pub fn write(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A workspace where `orders` and `payments` consume each other, with two Node servers and a
/// TypeScript client for both.
pub const MUTUAL_TOML: &str = r#"[project]
name = "shop"

[services.orders]
contracts = "contracts/orders"
consumes = ["payments"]

[services.payments]
contracts = "contracts/payments"
consumes = ["orders"]

[[targets]]
name = "orders-api"
role = "server"
emitter = "node"
service = "orders"
out = "servers/orders"

[[targets]]
name = "payments-api"
role = "server"
emitter = "node"
service = "payments"
out = "servers/payments"

[[targets]]
name = "web"
role = "client"
emitter = "typescript"
out = "web"
"#;

pub const ORDERS_VELD: &str = r#"/// An order and the state of its payment.
model Order {
  id: uuid
  payment: string
}

/// The part of an order other services may see.
model Summary {
  id: uuid
}

/// Order management.
service Orders at "/orders" {
  /// Fetch an order, asking payments how it was paid.
  get "/{id}" getOrder(id: uuid) -> Order | NotFound

  /// Whether an order exists. Does not call payments, so the two services cannot recurse.
  get "/{id}/summary" getSummary(id: uuid) -> Summary | NotFound
}
"#;

pub const PAYMENTS_VELD: &str = r#"/// A payment taken for an order.
model Payment {
  id: uuid
  orderId: uuid
  amount: int
}

/// Payment processing.
service Payments at "/payments" {
  /// Look up the payment for an order, checking with orders that the order exists.
  get "/by-order/{orderId}" getPaymentByOrder(orderId: uuid) -> Payment | NotFound
}
"#;

/// Writes the mutual-consumes workspace into `dir`.
pub fn write_mutual_workspace(dir: &Path) {
    write(dir, "veld.toml", MUTUAL_TOML);
    write(dir, "contracts/orders/orders.veld", ORDERS_VELD);
    write(dir, "contracts/payments/payments.veld", PAYMENTS_VELD);
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
