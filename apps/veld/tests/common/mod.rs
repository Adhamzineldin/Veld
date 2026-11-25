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
        .env_remove("TERM")
        .env_remove("VELD_ASCII")
        .env("LC_ALL", "C.UTF-8")
        .env_remove("VELD_REQUIRE_TOOLCHAINS");
    cmd
}

/// Every file below `dir` as (relative path with `/`, bytes), sorted.
pub fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    tree_excluding(dir, &[])
}

pub fn tree_excluding(dir: &Path, excluded_dirs: &[&str]) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, excluded: &[&str], out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                if !excluded.contains(&entry.file_name().to_string_lossy().as_ref()) {
                    walk(root, &path, excluded, out);
                }
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(
                    rel,
                    fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, excluded_dirs, &mut out);
    out
}

pub fn assert_same_tree(
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
    context: &str,
) {
    let mut changes = Vec::new();
    for (path, bytes) in before {
        match after.get(path) {
            None => changes.push(format!("missing {path}")),
            Some(other) if other != bytes => changes.push(format!("changed {path}")),
            _ => {}
        }
    }
    for path in after.keys().filter(|path| !before.contains_key(*path)) {
        changes.push(format!("added {path}"));
    }
    assert!(changes.is_empty(), "{context}: {}", changes.join("\n"));
}

pub fn write(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// The root `veld.toml` of the mutual-consumes workspace: a workspace with two services and an
/// app. The contracts stay in `contracts/<service>`; the outputs are `servers/<service>` and `web`.
pub const MUTUAL_TOML: &str = r#"[workspace]
name = "shop"
members = ["services/*", "apps/*"]
"#;

/// `services/orders/veld.toml`.
pub const ORDERS_TOML: &str = r#"[service]
contracts = "../../contracts/orders"
consumes = ["payments"]

[server]
name = "orders-api"
emitter = "node"
out = "../../servers/orders"
"#;

/// `services/payments/veld.toml`.
pub const PAYMENTS_TOML: &str = r#"[service]
contracts = "../../contracts/payments"
consumes = ["orders"]

[server]
name = "payments-api"
emitter = "node"
out = "../../servers/payments"
"#;

/// `apps/web/veld.toml`: a TypeScript client for both services.
pub const WEB_TOML: &str = r#"[app]

[client]
name = "web"
emitter = "typescript"
out = "../../web"
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
    write(dir, "services/orders/veld.toml", ORDERS_TOML);
    write(dir, "services/payments/veld.toml", PAYMENTS_TOML);
    write(dir, "apps/web/veld.toml", WEB_TOML);
    write(dir, "contracts/orders/orders.veld", ORDERS_VELD);
    write(dir, "contracts/payments/payments.veld", PAYMENTS_VELD);
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
