//! `veld init`: scaffold `veld.toml` and an example contract. Never overwrites anything.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use veld_project::config::CONFIG_FILE;

use crate::cli::InitArgs;
use crate::error::{Exit, user};
use crate::ui::Ui;

pub fn run(ui: &Ui, args: &InitArgs) -> Result<Exit> {
    let dir = std::env::current_dir().context("cannot read the current directory")?;
    let name = match &args.name {
        Some(n) => n.clone(),
        None => dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(sanitize)
            .unwrap_or_default(),
    };
    let name = if name.is_empty() {
        "app".to_string()
    } else {
        name
    };
    let files = if args.workspace {
        workspace_files(&name)
    } else {
        single_files(&name)
    };
    let written = scaffold(&dir, &files)?;
    ui.info(&format!("created {}", written.join(", ")));
    ui.info("next: run `veld check`, then `veld generate`");
    Ok(Exit::Ok)
}

/// Letters, digits and `_` only (the name becomes a package name).
fn sanitize(raw: &str) -> String {
    let s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    s.trim_matches('_').to_string()
}

/// Writes every file, or none: refuses if any of them already exists.
fn scaffold(dir: &Path, files: &[(String, String)]) -> Result<Vec<String>> {
    if dir.join(CONFIG_FILE).exists() {
        return Err(user(format!(
            "{CONFIG_FILE} already exists here; nothing was changed"
        )));
    }
    let taken: Vec<&str> = files
        .iter()
        .filter(|(p, _)| dir.join(p).exists())
        .map(|(p, _)| p.as_str())
        .collect();
    if !taken.is_empty() {
        return Err(user(format!(
            "refusing to overwrite existing files: {}",
            taken.join(", ")
        )));
    }
    for (path, text) in files {
        let target = dir.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create `{}`", parent.display()))?;
        }
        fs::write(&target, text).with_context(|| format!("cannot write `{}`", target.display()))?;
    }
    Ok(files.iter().map(|(p, _)| p.clone()).collect())
}

fn single_files(name: &str) -> Vec<(String, String)> {
    let config = format!(
        r#"[project]
name = "{name}"
contracts = "contracts"

# A Node.js server (framework adapters: express, fastify, hono, node-http).
[[targets]]
name = "api"
role = "server"
emitter = "node"
framework = "express"
service = "{name}"
out = "generated/server"

# A typed TypeScript client for the same contract.
[[targets]]
name = "web"
role = "client"
emitter = "typescript"
out = "generated/client"
"#
    );
    vec![
        (CONFIG_FILE.to_string(), config),
        ("contracts/hello.veld".to_string(), HELLO.to_string()),
    ]
}

fn workspace_files(name: &str) -> Vec<(String, String)> {
    let config = format!(
        r#"[project]
name = "{name}"

[services.users]
contracts = "contracts/users"
base-url = "http://localhost:3001"

[services.orders]
contracts = "contracts/orders"
base-url = "http://localhost:3002"
consumes = ["users"]   # orders gets a generated client for users

[[targets]]
name = "users-api"
role = "server"
emitter = "node"
framework = "express"
service = "users"
out = "services/users/generated"

[[targets]]
name = "orders-api"
role = "server"
emitter = "node"
framework = "express"
service = "orders"
out = "services/orders/generated"

[[targets]]
name = "web"
role = "client"
emitter = "typescript"
out = "web/generated"
"#
    );
    vec![
        (CONFIG_FILE.to_string(), config),
        ("contracts/users/users.veld".to_string(), USERS.to_string()),
        (
            "contracts/orders/orders.veld".to_string(),
            ORDERS.to_string(),
        ),
    ]
}

const HELLO: &str = r#"/// A greeting for one person.
model Greeting {
  message: string
}

/// Says hello.
service Hello at "/hello" {
  /// Greets the person with the given name.
  get "/{name}" greet(name: string) -> Greeting | NotFound
}
"#;

const USERS: &str = r#"/// A registered user.
model User {
  id: uuid
  name: string
}

/// User accounts.
service Users at "/users" {
  /// Fetch one user.
  get "/{id}" getUser(id: uuid) -> User | NotFound
}
"#;

const ORDERS: &str = r#"/// An order placed by a user.
model Order {
  id: uuid
  userId: uuid
}

/// Order management.
service Orders at "/orders" {
  /// Fetch one order.
  get "/{id}" getOrder(id: uuid) -> Order | NotFound
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use veld_project::config::load_str;

    #[test]
    fn sanitize_makes_a_package_name() {
        assert_eq!(sanitize("my-shop"), "my_shop");
        assert_eq!(sanitize("--x--"), "x");
        assert_eq!(sanitize("..."), "");
    }

    #[test]
    fn scaffolded_configs_are_valid() {
        let dir = std::env::temp_dir().join("veld-init-unit");
        for files in [single_files("shop"), workspace_files("shop")] {
            let (_, text) = &files[0];
            assert!(load_str(&dir, text).is_ok(), "{text}");
        }
    }

    #[test]
    fn scaffold_refuses_to_overwrite() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("veld.toml"), "keep").unwrap();
        let err = scaffold(tmp.path(), &single_files("a"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("already exists"), "{err}");
        assert_eq!(
            fs::read_to_string(tmp.path().join("veld.toml")).unwrap(),
            "keep"
        );
        assert!(!tmp.path().join("contracts").exists());
    }

    #[test]
    fn scaffold_refuses_when_a_contract_exists_and_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("contracts")).unwrap();
        fs::write(tmp.path().join("contracts/hello.veld"), "mine").unwrap();
        let err = scaffold(tmp.path(), &single_files("a"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("contracts/hello.veld"), "{err}");
        assert!(!tmp.path().join("veld.toml").exists());
        assert_eq!(
            fs::read_to_string(tmp.path().join("contracts/hello.veld")).unwrap(),
            "mine"
        );
    }
}
