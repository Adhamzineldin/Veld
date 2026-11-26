//! The on-disk shape of one `veld.toml`, as in `spec/workspace.md`.

use std::collections::BTreeMap;

use serde::Deserialize;
use toml::{Table, Value};

use super::error::ConfigError;
use super::suggest::{closest, hint};

/// One `veld.toml`: a workspace root, a member, or both (section 3 of the spec).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FileConfig {
    pub workspace: Option<WorkspaceSection>,
    pub service: Option<ServiceSection>,
    pub app: Option<AppSection>,
    pub library: Option<LibrarySection>,
    pub server: Option<Targets>,
    pub client: Option<Targets>,
    #[serde(default)]
    pub plugins: BTreeMap<String, PluginSection>,
    pub registry: Option<RegistrySection>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    /// `[lint]`: rule name to level. Rule names are validated by `veld-tools::lint`.
    #[serde(default)]
    pub lint: BTreeMap<String, veld_codegen::LintLevel>,
    /// `[hooks]`: commands run around `veld generate`.
    #[serde(default)]
    pub hooks: Option<HooksSection>,
}

/// `[server]` or `[[server]]` (and `[client]`, `[[client]]`).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Targets {
    One(Box<TargetSection>),
    Many(Vec<TargetSection>),
}

impl Targets {
    pub fn list(&self) -> Vec<&TargetSection> {
        match self {
            Self::One(t) => vec![t.as_ref()],
            Self::Many(ts) => ts.iter().collect(),
        }
    }
}

/// One command or a list of commands.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct HooksSection {
    /// Shell command(s) run after a successful, non-dry-run `veld generate`.
    pub post_generate: Option<OneOrMany>,
    /// A failing hook fails `generate` (default: only a warning).
    #[serde(default)]
    pub fail: bool,
}

/// `[workspace]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct WorkspaceSection {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Route prefix (`/api`) prepended to every service prefix.
    pub prefix: Option<String>,
    /// `[workspace.defaults.<role>.<emitter>]`.
    #[serde(default)]
    pub defaults: BTreeMap<String, BTreeMap<String, Table>>,
}

/// `[service]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ServiceSection {
    pub name: Option<String>,
    pub contracts: Option<String>,
    /// The directory of the message catalogs (`spec/platform.md` section 22); default `messages`.
    pub messages: Option<String>,
    #[serde(default)]
    pub consumes: Vec<String>,
    /// Route prefix (`/v1`) prepended to this service's prefixes, after `[workspace].prefix`.
    pub prefix: Option<String>,
    pub base_url: Option<String>,
    #[serde(default)]
    pub owners: Vec<String>,
    /// Registry name (`@org/name`), version and description used by `veld publish`.
    pub package: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
}

/// `[app]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct AppSection {
    pub name: Option<String>,
    pub uses: Option<Vec<String>>,
    #[serde(default)]
    pub owners: Vec<String>,
}

/// `[library]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct LibrarySection {
    pub name: Option<String>,
    pub contracts: Option<String>,
    #[serde(default)]
    pub owners: Vec<String>,
    pub package: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Server,
    Client,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Client => "client",
        }
    }
}

/// One `[server]` or `[client]` table.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct TargetSection {
    pub name: Option<String>,
    pub emitter: Option<String>,
    pub framework: Option<String>,
    pub out: Option<String>,
    pub clients: Option<bool>,
    /// The app that consumes the output, for `veld setup` (default: nearest package.json or build file).
    pub app: Option<String>,
    #[serde(default)]
    pub options: Table,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct PluginSection {
    pub command: Option<String>,
    pub version: Option<String>,
    pub sha256: Option<String>,
    pub entry: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct RegistrySection {
    pub url: Option<String>,
}

const TOP: &[&str] = &[
    "workspace",
    "service",
    "app",
    "library",
    "server",
    "client",
    "plugins",
    "registry",
    "dependencies",
    "lint",
    "hooks",
];
const WORKSPACE: &[&str] = &[
    "name",
    "description",
    "members",
    "exclude",
    "prefix",
    "defaults",
];
const SERVICE: &[&str] = &[
    "name",
    "contracts",
    "messages",
    "consumes",
    "prefix",
    "base-url",
    "owners",
    "package",
    "version",
    "description",
];
const APP: &[&str] = &["name", "uses", "owners"];
const LIBRARY: &[&str] = &[
    "name",
    "contracts",
    "owners",
    "package",
    "version",
    "description",
];
const TARGET: &[&str] = &[
    "name",
    "emitter",
    "framework",
    "out",
    "clients",
    "app",
    "options",
];
const PLUGIN: &[&str] = &["command", "version", "sha256", "entry"];
const REGISTRY: &[&str] = &["url"];
const HOOKS: &[&str] = &["post-generate", "fail"];

impl FileConfig {
    /// Parses one `veld.toml` file. A file in the earlier layout is `E0709`, an unknown key
    /// is `E0701` with a suggestion.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let table = parse_table(text)?;
        super::legacy::check(&table)?;
        check_unknown_keys(&table)?;
        table
            .try_into()
            .map_err(|e: toml::de::Error| ConfigError::invalid(e.message()))
    }

    /// The member tables this file holds, as `[service]`, `[app]`, `[library]`.
    pub fn member_tables(&self) -> Vec<&'static str> {
        [
            ("[service]", self.service.is_some()),
            ("[app]", self.app.is_some()),
            ("[library]", self.library.is_some()),
        ]
        .into_iter()
        .filter_map(|(n, present)| present.then_some(n))
        .collect()
    }

    /// The root-only tables this file holds, as `[registry]`, `[dependencies]`, and so on.
    pub fn root_only_tables(&self) -> Vec<&'static str> {
        [
            ("[registry]", self.registry.is_some()),
            ("[dependencies]", !self.dependencies.is_empty()),
            ("[plugins]", !self.plugins.is_empty()),
            ("[lint]", !self.lint.is_empty()),
            ("[hooks]", self.hooks.is_some()),
        ]
        .into_iter()
        .filter_map(|(n, present)| present.then_some(n))
        .collect()
    }
}

pub(super) fn parse_table(text: &str) -> Result<Table, ConfigError> {
    text.parse()
        .map_err(|e: toml::de::Error| ConfigError::invalid(e.message()))
}

fn check_keys(table: &Table, location: &str, valid: &[&str]) -> Result<(), ConfigError> {
    check_keys_with(table, location, valid, "")
}

/// `note` is added to the hint of the first unknown key.
fn check_keys_with(
    table: &Table,
    location: &str,
    valid: &[&str],
    note: &str,
) -> Result<(), ConfigError> {
    let Some(key) = table.keys().find(|k| !valid.contains(&k.as_str())) else {
        return Ok(());
    };
    let suggestion = closest(key, valid.iter().copied());
    let note = note.replace("{key}", key);
    Err(ConfigError::UnknownKey {
        location: location.to_string(),
        key: key.clone(),
        hint: format!("{}{note}", hint(suggestion.as_deref(), valid)),
        suggestion,
    })
}

/// What to add when a target table has a key it does not know. The loader cannot tell the options
/// of an emitter (each emitter reads them into its own type), so the note is the same for every
/// key and says "if".
fn options_note(role: &str) -> String {
    format!(
        ". If `{{key}}` is an option of the emitter (`testing`, `package`, ...), write it under `[{role}.options]`"
    )
}

fn check_table(root: &Table, section: &str, valid: &[&str]) -> Result<(), ConfigError> {
    if let Some(Value::Table(t)) = root.get(section) {
        check_keys(t, &format!("`[{section}]`"), valid)?;
    }
    Ok(())
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

fn check_targets(root: &Table, role: &str) -> Result<(), ConfigError> {
    match root.get(role) {
        Some(Value::Table(t)) => {
            check_keys_with(t, &format!("`[{role}]`"), TARGET, &options_note(role))
        }
        Some(Value::Array(list)) => {
            for (i, value) in list.iter().enumerate() {
                if let Value::Table(t) = value {
                    let place = format!("`[[{role}]]` #{}", i + 1);
                    check_keys_with(t, &place, TARGET, &options_note(role))?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn check_defaults(workspace: &Table) -> Result<(), ConfigError> {
    let Some(Value::Table(defaults)) = workspace.get("defaults") else {
        return Ok(());
    };
    for (role, value) in defaults {
        if role != "server" && role != "client" {
            let suggestion = closest(role, ["server", "client"]);
            return Err(ConfigError::UnknownKey {
                location: "`[workspace.defaults]`".into(),
                key: role.clone(),
                hint: hint(suggestion.as_deref(), &["server", "client"]),
                suggestion,
            });
        }
        if !matches!(value, Value::Table(_)) {
            return Err(ConfigError::invalid(format!(
                "`[workspace.defaults.{role}]` must be a table of emitters"
            )));
        }
    }
    Ok(())
}

fn check_unknown_keys(root: &Table) -> Result<(), ConfigError> {
    check_keys(root, "the top level", TOP)?;
    check_table(root, "workspace", WORKSPACE)?;
    if let Some(Value::Table(w)) = root.get("workspace") {
        check_defaults(w)?;
    }
    check_table(root, "service", SERVICE)?;
    check_table(root, "app", APP)?;
    check_table(root, "library", LIBRARY)?;
    check_table(root, "registry", REGISTRY)?;
    check_table(root, "hooks", HOOKS)?;
    check_named_tables(root, "plugins", PLUGIN)?;
    check_targets(root, "server")?;
    check_targets(root, "client")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"
[workspace]
name = "nexusbank"
members = ["services/*"]
[workspace.defaults.server.node]
framework = "express"
[service]
name = "iam"
base-url = "http://iam:3001"
consumes = ["common"]
owners = ["team-a"]
[server]
name = "orders-api"
emitter = "node"
framework = "express"
out = "gen"
clients = false
[server.options]
validate = true
[plugins.kotlin]
command = "veld-emit-kotlin"
version = "1.2.0"
sha256 = "ab"
entry = "main.js"
[registry]
url = "https://r"
[dependencies]
"@acme/billing" = "^1.2"
"#;

    #[test]
    fn lint_levels_parse_and_reject_unknown_levels() {
        let cfg = FileConfig::parse(
            "[service]\nname = \"x\"\n[lint]\nunused-model = \"off\"\nempty-model = \"error\"\n",
        )
        .unwrap();
        assert_eq!(cfg.lint["unused-model"], veld_codegen::LintLevel::Off);
        assert_eq!(cfg.lint["empty-model"], veld_codegen::LintLevel::Error);
        assert!(
            FileConfig::parse("[service]\nname = \"x\"\n[lint]\nunused-model = \"loud\"\n")
                .is_err()
        );
    }

    #[test]
    fn parses_full_example() {
        let cfg = FileConfig::parse(FULL).unwrap();
        let ws = cfg.workspace.as_ref().unwrap();
        assert_eq!(ws.name.as_deref(), Some("nexusbank"));
        assert_eq!(
            ws.defaults["server"]["node"]["framework"].as_str(),
            Some("express")
        );
        let svc = cfg.service.as_ref().unwrap();
        assert_eq!(svc.base_url.as_deref(), Some("http://iam:3001"));
        assert_eq!(svc.owners, ["team-a"]);
        let server = cfg.server.as_ref().unwrap().list()[0].clone();
        assert_eq!(server.emitter.as_deref(), Some("node"));
        assert_eq!(server.options.get("validate"), Some(&Value::Boolean(true)));
        assert_eq!(cfg.plugins["kotlin"].sha256.as_deref(), Some("ab"));
        assert_eq!(cfg.plugins["kotlin"].entry.as_deref(), Some("main.js"));
        assert_eq!(cfg.dependencies["@acme/billing"], "^1.2");
    }

    #[test]
    fn a_target_can_be_a_table_or_an_array() {
        let one = FileConfig::parse("[service]\nname=\"x\"\n[server]\nemitter=\"node\"\n").unwrap();
        assert_eq!(one.server.unwrap().list().len(), 1);
        let many = FileConfig::parse(
            "[service]\nname=\"x\"\n[[server]]\nemitter=\"node\"\n[[server]]\nemitter=\"java\"\n",
        )
        .unwrap();
        assert_eq!(many.server.unwrap().list().len(), 2);
    }

    #[test]
    fn unknown_target_key_suggests() {
        let err =
            FileConfig::parse("[service]\nname=\"x\"\n[server]\nemiter=\"node\"\n").unwrap_err();
        assert_eq!(err.code(), Some("E0701"));
        let msg = err.to_string();
        assert!(
            msg.contains("`emiter`") && msg.contains("did you mean `emitter`"),
            "{msg}"
        );
    }

    #[test]
    fn an_emitter_option_written_as_a_target_key_points_to_the_options_table() {
        let err =
            FileConfig::parse("[service]\nname=\"x\"\n[server]\nemitter=\"node\"\ntesting=true\n")
                .unwrap_err();
        assert_eq!(err.code(), Some("E0701"));
        let msg = err.to_string();
        assert!(
            msg.contains("`testing`") && msg.contains("under `[server.options]`"),
            "{msg}"
        );
        let err = FileConfig::parse(
            "[service]\nname=\"x\"\n[[client]]\nemitter=\"typescript\"\ntesting=true\n",
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("under `[client.options]`"),
            "{err}"
        );
    }

    #[test]
    fn unknown_keys_at_every_level() {
        for (text, near) in [
            ("[servce]\nname=\"x\"", "service"),
            ("[service]\nnam=\"x\"", "name"),
            ("[service]\ncontract=\"c\"", "contracts"),
            ("[service]\nbase_url=\"c\"", "base-url"),
            ("[workspace]\nmember=[]", "members"),
            ("[app]\nuse=[]", "uses"),
            ("[library]\ncontract=\"c\"", "contracts"),
            ("[plugins.a]\nsha=\"c\"", "sha256"),
            ("[registry]\nurls=\"c\"", "url"),
            ("[[client]]\nout_=\"c\"", "out"),
            ("[workspace.defaults.servr.node]\nframework=\"x\"", "server"),
        ] {
            let err = FileConfig::parse(text).unwrap_err();
            assert_eq!(err.code(), Some("E0701"), "{text}");
            assert!(err.to_string().contains(near), "{text}: {err}");
        }
    }

    #[test]
    fn syntax_and_type_errors_are_e0700() {
        assert_eq!(
            FileConfig::parse("[service").unwrap_err().code(),
            Some("E0700")
        );
        assert_eq!(
            FileConfig::parse("[service]\nname = 3").unwrap_err().code(),
            Some("E0700")
        );
    }
}
