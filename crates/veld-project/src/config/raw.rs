//! The on-disk shape of `veld.toml`, exactly as in `spec/config.md`.

use std::collections::BTreeMap;

use serde::Deserialize;
use toml::{Table, Value};

use super::error::ConfigError;
use super::suggest::{closest, hint};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    pub project: ProjectSection,
    #[serde(default)]
    pub libraries: BTreeMap<String, LibrarySection>,
    #[serde(default)]
    pub services: BTreeMap<String, ServiceSection>,
    #[serde(default)]
    pub targets: Vec<TargetSection>,
    #[serde(default)]
    pub plugins: BTreeMap<String, PluginSection>,
    pub registry: Option<RegistrySection>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ProjectSection {
    #[serde(default)]
    pub name: String,
    pub contracts: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct LibrarySection {
    pub contracts: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ServiceSection {
    pub contracts: Option<String>,
    pub base_url: Option<String>,
    #[serde(default)]
    pub consumes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Server,
    Client,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct TargetSection {
    pub name: Option<String>,
    pub role: Option<Role>,
    pub emitter: Option<String>,
    pub framework: Option<String>,
    pub service: Option<String>,
    pub services: Option<Vec<String>>,
    pub out: Option<String>,
    pub clients: Option<bool>,
    #[serde(default)]
    pub options: Table,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct PluginSection {
    pub command: Option<String>,
    pub version: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct RegistrySection {
    pub url: Option<String>,
}

const TOP: &[&str] = &[
    "project",
    "libraries",
    "services",
    "targets",
    "plugins",
    "registry",
    "dependencies",
];
const PROJECT: &[&str] = &["name", "contracts"];
const LIBRARY: &[&str] = &["contracts"];
const SERVICE: &[&str] = &["contracts", "base-url", "consumes"];
const TARGET: &[&str] = &[
    "name",
    "role",
    "emitter",
    "framework",
    "service",
    "services",
    "out",
    "clients",
    "options",
];
const PLUGIN: &[&str] = &["command", "version", "sha256"];
const REGISTRY: &[&str] = &["url"];

impl Config {
    /// Parses `veld.toml` text. Unknown keys give E0701 with a suggestion.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let table: Table = text
            .parse()
            .map_err(|e: toml::de::Error| ConfigError::invalid(e.message()))?;
        check_unknown_keys(&table)?;
        table
            .try_into()
            .map_err(|e: toml::de::Error| ConfigError::invalid(e.message()))
    }
}

fn check_keys(table: &Table, location: &str, valid: &[&str]) -> Result<(), ConfigError> {
    let Some(key) = table.keys().find(|k| !valid.contains(&k.as_str())) else {
        return Ok(());
    };
    let suggestion = closest(key, valid.iter().copied());
    Err(ConfigError::UnknownKey {
        location: location.to_string(),
        key: key.clone(),
        hint: hint(suggestion.as_deref(), valid),
        suggestion,
    })
}

fn check_named_tables(root: &Table, section: &str, valid: &[&str]) -> Result<(), ConfigError> {
    let Some(Value::Table(entries)) = root.get(section) else {
        return Ok(());
    };
    for (name, value) in entries {
        if let Value::Table(inner) = value {
            check_keys(inner, &format!("`[{section}.{name}]`"), valid)?;
        }
    }
    Ok(())
}

fn check_unknown_keys(root: &Table) -> Result<(), ConfigError> {
    check_keys(root, "the top level", TOP)?;
    if let Some(Value::Table(t)) = root.get("project") {
        check_keys(t, "`[project]`", PROJECT)?;
    }
    if let Some(Value::Table(t)) = root.get("registry") {
        check_keys(t, "`[registry]`", REGISTRY)?;
    }
    check_named_tables(root, "libraries", LIBRARY)?;
    check_named_tables(root, "services", SERVICE)?;
    check_named_tables(root, "plugins", PLUGIN)?;
    if let Some(Value::Array(targets)) = root.get("targets") {
        for (i, value) in targets.iter().enumerate() {
            if let Value::Table(t) = value {
                check_keys(t, &format!("`[[targets]]` #{}", i + 1), TARGET)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"
[project]
name = "nexusbank"
[libraries.common]
contracts = "contracts/common"
[services.iam]
contracts = "contracts/iam"
base-url = "http://iam:3001"
consumes = ["common"]
[[targets]]
name = "orders-api"
service = "iam"
role = "server"
emitter = "node"
framework = "express"
out = "gen"
clients = false
[targets.options]
validate = true
[plugins.kotlin]
command = "veld-emit-kotlin"
version = "1.2.0"
sha256 = "ab"
[registry]
url = "https://r"
[dependencies]
"@acme/billing" = "^1.2"
"#;

    #[test]
    fn parses_full_example() {
        let cfg = Config::parse(FULL).unwrap();
        assert_eq!(cfg.project.name, "nexusbank");
        assert_eq!(
            cfg.services["iam"].base_url.as_deref(),
            Some("http://iam:3001")
        );
        assert_eq!(cfg.targets[0].role, Some(Role::Server));
        assert_eq!(
            cfg.targets[0].options.get("validate"),
            Some(&Value::Boolean(true))
        );
        assert_eq!(cfg.plugins["kotlin"].sha256.as_deref(), Some("ab"));
        assert_eq!(cfg.dependencies["@acme/billing"], "^1.2");
    }

    #[test]
    fn unknown_target_key_suggests() {
        let err =
            Config::parse("[project]\nname=\"x\"\n[[targets]]\nemiter=\"node\"\n").unwrap_err();
        assert_eq!(err.code(), Some("E0701"));
        let msg = err.to_string();
        assert!(
            msg.contains("`emiter`") && msg.contains("did you mean `emitter`"),
            "{msg}"
        );
    }

    #[test]
    fn unknown_keys_at_every_level() {
        for (text, near) in [
            ("[projct]\nname=\"x\"", "project"),
            ("[project]\nnam=\"x\"", "name"),
            (
                "[project]\nname=\"x\"\n[services.a]\ncontract=\"c\"",
                "contracts",
            ),
            (
                "[project]\nname=\"x\"\n[services.a]\nbase_url=\"c\"",
                "base-url",
            ),
            ("[project]\nname=\"x\"\n[plugins.a]\nsha=\"c\"", "sha256"),
            ("[project]\nname=\"x\"\n[registry]\nurls=\"c\"", "url"),
            (
                "[project]\nname=\"x\"\n[libraries.a]\ncontract=\"c\"",
                "contracts",
            ),
        ] {
            let err = Config::parse(text).unwrap_err();
            assert_eq!(err.code(), Some("E0701"), "{text}");
            assert!(err.to_string().contains(near), "{text}: {err}");
        }
    }

    #[test]
    fn syntax_and_type_errors_are_e0700() {
        assert_eq!(Config::parse("[project").unwrap_err().code(), Some("E0700"));
        assert_eq!(
            Config::parse("[project]\nname = 3").unwrap_err().code(),
            Some("E0700")
        );
        assert_eq!(
            Config::parse("[project]\nname=\"x\"\n[[targets]]\nrole=\"both\"")
                .unwrap_err()
                .code(),
            Some("E0700")
        );
    }
}
