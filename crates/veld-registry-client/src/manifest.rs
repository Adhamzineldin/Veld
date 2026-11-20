//! `veld-package.toml`: the manifest at the root of every registry package.

use std::collections::BTreeMap;

use serde::Deserialize;
use veld_project::packages::{CONTRACTS_DIR, split_name};

use crate::error::{RegistryError, Result};
use crate::tarball::sha256_hex;

/// The only package language version this Veld reads and writes.
pub const LANGUAGE: &str = "2";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageManifest {
    /// `@org/name`.
    pub name: String,
    pub version: String,
    pub language: String,
    #[serde(default)]
    pub description: Option<String>,
    /// SHA-256 of every file in the archive except the manifest, keyed by archive path.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

impl PackageManifest {
    /// Builds the manifest for `files` (`(path relative to contracts/, contents)`).
    pub fn new(
        name: &str,
        version: &str,
        description: Option<&str>,
        files: &[(String, Vec<u8>)],
    ) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            language: LANGUAGE.to_string(),
            description: description.map(str::to_string),
            files: files
                .iter()
                .map(|(rel, data)| (format!("{CONTRACTS_DIR}/{rel}"), sha256_hex(data)))
                .collect(),
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        let m: PackageManifest =
            toml::from_str(text).map_err(|e| RegistryError::Manifest(e.message().to_string()))?;
        m.validate()?;
        Ok(m)
    }

    fn validate(&self) -> Result<()> {
        let fail = |m: String| Err(RegistryError::Manifest(m));
        if split_name(&self.name).is_none() {
            return fail(format!(
                "`{}` is not a valid package name (@org/name)",
                self.name
            ));
        }
        if semver::Version::parse(&self.version).is_err() {
            return fail(format!("`{}` is not a semver version", self.version));
        }
        if self.language != LANGUAGE {
            return fail(format!(
                "language is `{}`; this Veld reads language \"{LANGUAGE}\" packages",
                self.language
            ));
        }
        for (path, sum) in &self.files {
            if sum.len() != 64 || !sum.bytes().all(|b| b.is_ascii_hexdigit()) {
                return fail(format!("checksum of `{path}` is not a sha256 hex digest"));
            }
        }
        Ok(())
    }

    /// Deterministic TOML text (sorted keys, fixed layout).
    pub fn to_toml(&self) -> String {
        let q = |s: &str| toml::Value::String(s.to_string()).to_string();
        let mut out = format!(
            "name = {}\nversion = {}\nlanguage = {}\n",
            q(&self.name),
            q(&self.version),
            q(&self.language)
        );
        if let Some(d) = self.description.as_deref().filter(|d| !d.is_empty()) {
            out.push_str(&format!("description = {}\n", q(d)));
        }
        out.push_str("\n[files]\n");
        for (path, sum) in &self.files {
            out.push_str(&format!("{} = {}\n", q(path), q(sum)));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_toml_parses_back() {
        let files = vec![("a.veld".to_string(), b"x".to_vec())];
        let m = PackageManifest::new("@acme/lib", "1.2.3", Some("a \"quoted\" lib"), &files);
        let back = PackageManifest::parse(&m.to_toml()).unwrap();
        assert_eq!(back, m);
        assert!(back.files.contains_key("contracts/a.veld"));
    }

    #[test]
    fn rejects_invalid_manifests() {
        let good = "name = \"@a/b\"\nversion = \"1.0.0\"\nlanguage = \"2\"\n";
        assert!(PackageManifest::parse(good).is_ok());
        for bad in [
            "name = \"a/b\"\nversion = \"1.0.0\"\nlanguage = \"2\"\n",
            "name = \"@a/b\"\nversion = \"1\"\nlanguage = \"2\"\n",
            "name = \"@a/b\"\nversion = \"1.0.0\"\nlanguage = \"1\"\n",
            "name = \"@a/b\"\nversion = \"1.0.0\"\n",
            "name = \"@a/b\"\nversion = \"1.0.0\"\nlanguage = \"2\"\nextra = 1\n",
            "name = \"@a/b\"\nversion = \"1.0.0\"\nlanguage = \"2\"\n[files]\n\"contracts/x.veld\" = \"zz\"\n",
        ] {
            assert!(PackageManifest::parse(bad).is_err(), "{bad}");
        }
    }
}
