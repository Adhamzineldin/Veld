//! `veld.toml` loading: parse, validate, resolve to absolute paths.

mod error;
mod paths;
mod raw;
mod resolve;
mod sources;
mod suggest;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use veld_ir::PackageKind;

pub use error::{ConfigError, Warning};
pub use paths::{CONFIG_FILE, find_config};
pub use raw::{
    Config, HooksSection, LibrarySection, OneOrMany, PluginSection, ProjectSection,
    RegistrySection, Role, ServiceSection, TargetSection,
};
pub use sources::collect_sources;

/// A resolved contracts package (a service or a library).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSpec {
    pub name: String,
    pub kind: PackageKind,
    /// Absolute contracts directory.
    pub root: PathBuf,
    pub base_url: Option<String>,
    /// Names of consumed services (self-references removed, duplicates dropped).
    pub consumes: Vec<String>,
    /// Route prefix prepended to every service prefix of this package (`[project].prefix`
    /// then `[services.x].prefix`); empty when none.
    pub prefix: String,
}

/// Who an emitter id refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitterRef {
    Builtin(String),
    Plugin(String),
}

/// A resolved `[[targets]]` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetSpec {
    pub name: String,
    pub role: Role,
    pub emitter: EmitterRef,
    pub framework: Option<String>,
    /// The served service (servers only).
    pub service: Option<String>,
    /// Services a client covers (clients only; defaults to all services).
    pub services: Vec<String>,
    /// Absolute output directory.
    pub out: PathBuf,
    /// Whether a server also emits clients for its `consumes`.
    pub clients: bool,
    /// Absolute directory of the app that consumes the output (`app` in `veld.toml`), if set.
    pub app: Option<PathBuf>,
    /// Emitter-specific options, validated later by the emitter.
    pub options: toml::Table,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSpec {
    pub name: String,
    pub command: String,
    pub version: Option<String>,
    pub sha256: Option<String>,
    /// Script the interpreter in `command` runs. When set, `sha256` covers this file instead
    /// of the executable.
    pub entry: Option<String>,
}

impl PluginSpec {
    /// The entry script as a path: absolute, or relative to the project `root`.
    pub fn entry_path(&self, root: &Path) -> Option<PathBuf> {
        let entry = self.entry.as_deref()?;
        let p = Path::new(entry);
        Some(if p.is_absolute() {
            p.to_path_buf()
        } else {
            root.join(p)
        })
    }

    /// Refuses (E0705) to run a pinned file (the entry script, else the executable) whose
    /// SHA-256 differs from `sha256`, if one is pinned.
    pub fn verify_executable(&self, executable: &Path) -> Result<(), ConfigError> {
        let Some(expected) = &self.sha256 else {
            return Ok(());
        };
        let bytes = fs::read(executable).map_err(|source| ConfigError::Io {
            path: executable.to_path_buf(),
            source,
        })?;
        let actual: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if actual.eq_ignore_ascii_case(expected.trim()) {
            Ok(())
        } else {
            Err(ConfigError::PluginHashMismatch {
                plugin: self.name.clone(),
                expected: expected.clone(),
                actual,
            })
        }
    }
}

/// Resolved `[hooks]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hooks {
    /// Shell commands run after a successful, non-dry-run generate, in order.
    pub post_generate: Vec<String>,
    /// A failing hook fails the run (otherwise it is only a warning).
    pub fail: bool,
}

/// A fully resolved project.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    /// Absolute directory containing `veld.toml`.
    pub root: PathBuf,
    pub name: String,
    /// `[project].description`, written by `veld init`.
    pub description: Option<String>,
    pub packages: Vec<PackageSpec>,
    pub targets: Vec<TargetSpec>,
    pub plugins: Vec<PluginSpec>,
    pub registry: Option<String>,
    pub dependencies: BTreeMap<String, String>,
    /// `[lint]` rule levels by rule name (unvalidated names; see `veld_tools::lint::LintConfig`).
    pub lint: BTreeMap<String, veld_codegen::LintLevel>,
    /// `[hooks]`.
    pub hooks: Hooks,
    pub warnings: Vec<Warning>,
}

impl Project {
    pub fn package(&self, name: &str) -> Option<&PackageSpec> {
        self.packages.iter().find(|p| p.name == name)
    }

    pub fn services(&self) -> impl Iterator<Item = &PackageSpec> {
        self.packages
            .iter()
            .filter(|p| p.kind == PackageKind::Service)
    }

    /// Package name to route prefix, for `veld_compiler::compile_prefixed` (packages without a
    /// prefix are left out).
    #[must_use]
    pub fn prefixes(&self) -> BTreeMap<String, String> {
        self.packages
            .iter()
            .filter(|p| !p.prefix.is_empty())
            .map(|p| (p.name.clone(), p.prefix.clone()))
            .collect()
    }
}

/// Reads and resolves the `veld.toml` at `path`.
pub fn load(path: &Path) -> Result<Project, ConfigError> {
    let path = paths::absolutize(path)?;
    let text = fs::read_to_string(&path).map_err(|source| ConfigError::Io {
        path: path.clone(),
        source,
    })?;
    let root = path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| ConfigError::invalid("config path has no parent directory"))?;
    resolve::resolve(&root, Config::parse(&text)?)
}

/// Parses and resolves config text as if it lived in `root`. Useful for tests and tools.
pub fn load_str(root: &Path, text: &str) -> Result<Project, ConfigError> {
    resolve::resolve(&paths::absolutize(root)?, Config::parse(text)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_hash_verification() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("plug");
        fs::write(&exe, b"abc").unwrap();
        let mut spec = PluginSpec {
            name: "p".into(),
            command: "plug".into(),
            version: None,
            sha256: None,
            entry: None,
        };
        assert!(spec.verify_executable(&exe).is_ok());
        spec.sha256 =
            Some("BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD".into());
        assert!(spec.verify_executable(&exe).is_ok());
        spec.sha256 = Some("00".into());
        let err = spec.verify_executable(&exe).unwrap_err();
        assert_eq!(err.code(), Some("E0705"));
    }

    #[test]
    fn entry_path_is_relative_to_the_root() {
        let mut spec = PluginSpec {
            name: "p".into(),
            command: "node".into(),
            version: None,
            sha256: None,
            entry: None,
        };
        assert!(spec.entry_path(Path::new("/r")).is_none());
        spec.entry = Some("dist/main.js".into());
        assert_eq!(
            spec.entry_path(Path::new("/r")),
            Some(Path::new("/r").join("dist/main.js"))
        );
    }

    #[test]
    fn hooks_accept_a_string_or_a_list_and_default_to_warning() {
        let one = load_str(
            Path::new("/p"),
            "[project]\nname = \"s\"\n[hooks]\npost-generate = \"prettier --write gen\"\n",
        )
        .unwrap();
        assert_eq!(one.hooks.post_generate, ["prettier --write gen"]);
        assert!(!one.hooks.fail);
        let many = load_str(
            Path::new("/p"),
            "[project]\nname = \"s\"\n[hooks]\npost-generate = [\"a\", \"b\"]\nfail = true\n",
        )
        .unwrap();
        assert_eq!(many.hooks.post_generate, ["a", "b"]);
        assert!(many.hooks.fail);
        let none = load_str(Path::new("/p"), "[project]\nname = \"s\"\n").unwrap();
        assert_eq!(none.hooks, Hooks::default());
    }

    #[test]
    fn an_unknown_hooks_key_suggests_the_right_one() {
        let err = load_str(
            Path::new("/p"),
            "[project]\nname = \"s\"\n[hooks]\npost-generat = \"x\"\n",
        )
        .unwrap_err();
        assert_eq!(err.code(), Some("E0701"));
        assert!(
            err.to_string().contains("did you mean `post-generate`"),
            "{err}"
        );
    }
    #[test]
    fn load_reads_a_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join(CONFIG_FILE);
        fs::write(&file, "[project]\nname = \"shop\"\n").unwrap();
        let project = load(&file).unwrap();
        assert_eq!(project.name, "shop");
        assert!(project.root.is_absolute());
        assert!(load(&tmp.path().join("missing.toml")).is_err());
    }
}
