//! Packing the current project's library or service for `veld publish`.

use std::path::Path;

use veld_ir::PackageKind;
use veld_project::config::{Config, PackageSpec, Project, collect_sources};
use veld_project::packages::split_name;

use crate::error::{RegistryError, Result, io};
use crate::manifest::PackageManifest;
use crate::tarball::{Packed, pack};

/// A package ready to upload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishPlan {
    /// `@org/name`.
    pub name: String,
    pub version: String,
    /// Number of `.veld` files packed.
    pub files: usize,
    pub packed: Packed,
}

impl PublishPlan {
    /// The org and package name parts of `name`.
    pub fn parts(&self) -> Result<(&str, &str)> {
        split_name(&self.name)
            .ok_or_else(|| RegistryError::Manifest(format!("bad package name `{}`", self.name)))
    }
}

/// What `veld.toml` says about publishing one package.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Meta {
    package: Option<String>,
    version: Option<String>,
    description: Option<String>,
}

fn meta_of(cfg: &Config, pkg: &str) -> (Meta, String) {
    if let Some(l) = cfg.libraries.get(pkg) {
        let meta = Meta {
            package: l.package.clone(),
            version: l.version.clone(),
            description: l.description.clone(),
        };
        return (meta, format!("[libraries.{pkg}]"));
    }
    if let Some(s) = cfg.services.get(pkg) {
        let meta = Meta {
            package: s.package.clone(),
            version: s.version.clone(),
            description: s.description.clone(),
        };
        return (meta, format!("[services.{pkg}]"));
    }
    let p = &cfg.project;
    let meta = Meta {
        package: p.package.clone(),
        version: p.version.clone(),
        description: p.description.clone(),
    };
    (meta, "[project]".to_string())
}

/// The publishable package: the only library or service, or the one named by `package`.
pub fn select_package<'a>(
    project: &'a Project,
    cfg: &Config,
    package: Option<&str>,
) -> Result<&'a PackageSpec> {
    let candidates: Vec<&PackageSpec> = project
        .packages
        .iter()
        .filter(|p| matches!(p.kind, PackageKind::Library | PackageKind::Service))
        .collect();
    let names = || {
        candidates
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    // Without --package, the one package that declares `package = "@org/name"` is meant.
    let marked: Vec<&PackageSpec> = candidates
        .iter()
        .copied()
        .filter(|p| meta_of(cfg, &p.name).0.package.is_some())
        .collect();
    if package.is_none() && candidates.len() > 1 && marked.len() == 1 {
        return Ok(marked[0]);
    }
    match (package, candidates.as_slice()) {
        (Some(want), _) => candidates
            .iter()
            .copied()
            .find(|p| p.name == want)
            .ok_or_else(|| {
                RegistryError::Resolve(format!("no package named `{want}` (have: {})", names()))
            }),
        (None, [only]) => Ok(only),
        (None, []) => Err(RegistryError::Resolve("the project has no packages".into())),
        (None, _) => Err(RegistryError::Resolve(format!(
            "this workspace has several packages; choose one with --package ({})",
            names()
        ))),
    }
}

/// Packs the selected package of the project described by `config_path`.
pub fn plan(config_path: &Path, project: &Project, package: Option<&str>) -> Result<PublishPlan> {
    let text = std::fs::read_to_string(config_path).map_err(io(config_path))?;
    let cfg = Config::parse(&text).map_err(|e| RegistryError::Other(e.to_string()))?;
    let spec = select_package(project, &cfg, package)?;
    let (meta, section) = meta_of(&cfg, &spec.name);
    let name = meta.package.ok_or_else(|| {
        RegistryError::Resolve(format!(
            "{section} needs `package = \"@org/name\"` and `version = \"1.0.0\"` to be published"
        ))
    })?;
    let version = meta.version.ok_or_else(|| {
        RegistryError::Resolve(format!(
            "{section} needs a `version = \"1.0.0\"` to be published"
        ))
    })?;
    if split_name(&name).is_none() {
        return Err(RegistryError::Resolve(format!(
            "{section} package `{name}` is not a valid name (use @org/name)"
        )));
    }
    if semver::Version::parse(&version).is_err() {
        return Err(RegistryError::Resolve(format!(
            "{section} version `{version}` is not semver (MAJOR.MINOR.PATCH)"
        )));
    }
    let sources = collect_sources(spec).map_err(|e| RegistryError::Other(e.to_string()))?;
    if sources.is_empty() {
        return Err(RegistryError::Resolve(format!(
            "package `{}` has no .veld files to publish",
            spec.name
        )));
    }
    let files: Vec<(String, Vec<u8>)> = sources
        .into_iter()
        .map(|(path, text)| (path, text.into_bytes()))
        .collect();
    // The package's own `[dependencies]` travel in its manifest, so installing it can resolve
    // its whole graph. For a workspace, every package shares the project's dependencies.
    let manifest = PackageManifest::new(&name, &version, meta.description.as_deref(), &files)
        .with_dependencies(project.dependencies.clone());
    let packed = pack(&manifest, &files)?;
    Ok(PublishPlan {
        name,
        version,
        files: files.len(),
        packed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use veld_project::config::load;

    fn project(dir: &Path, toml: &str) -> (std::path::PathBuf, Project) {
        std::fs::create_dir_all(dir.join("contracts")).unwrap();
        std::fs::write(dir.join("contracts/m.veld"), "model M { id: uuid }\n").unwrap();
        let path = dir.join("veld.toml");
        std::fs::write(&path, toml).unwrap();
        let p = load(&path).unwrap();
        (path, p)
    }

    #[test]
    fn packs_a_single_service_project() {
        let tmp = tempfile::tempdir().unwrap();
        let (path, p) = project(
            tmp.path(),
            "[project]\nname = \"lib\"\npackage = \"@acme/lib\"\nversion = \"1.2.3\"\ndescription = \"d\"\n",
        );
        let plan = plan(&path, &p, None).unwrap();
        assert_eq!(
            (plan.name.as_str(), plan.version.as_str()),
            ("@acme/lib", "1.2.3")
        );
        assert_eq!(plan.files, 1);
        assert_eq!(plan.parts().unwrap(), ("acme", "lib"));
        let again = super::plan(&path, &p, None).unwrap();
        assert_eq!(plan.packed, again.packed, "deterministic");
    }

    #[test]
    fn dependencies_are_written_to_the_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let (path, p) = project(
            tmp.path(),
            "[project]
name = \"lib\"
package = \"@acme/billing\"
version = \"1.0.0\"
[dependencies]
\"@acme/money\" = \"^1.0\"
",
        );
        let plan = plan(&path, &p, None).unwrap();
        let files =
            crate::tarball::read_archive(&plan.packed.bytes, &crate::tarball::Limits::default())
                .unwrap();
        let text = String::from_utf8(files[veld_project::packages::MANIFEST_FILE].clone()).unwrap();
        let m = PackageManifest::parse(&text).unwrap();
        assert_eq!(m.dependencies["@acme/money"], "^1.0");
    }

    #[test]
    fn missing_metadata_is_explained() {
        let tmp = tempfile::tempdir().unwrap();
        let (path, p) = project(tmp.path(), "[project]\nname = \"lib\"\n");
        let err = plan(&path, &p, None).unwrap_err().to_string();
        assert!(err.contains("package = \"@org/name\""), "{err}");
        let (path, p) = project(
            tmp.path(),
            "[project]\nname = \"lib\"\npackage = \"@acme/lib\"\nversion = \"one\"\n",
        );
        assert!(
            plan(&path, &p, None)
                .unwrap_err()
                .to_string()
                .contains("semver")
        );
    }

    #[test]
    fn workspaces_need_a_package_choice() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        for d in ["common", "iam"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
            std::fs::write(dir.join(d).join("m.veld"), "model M { id: uuid }\n").unwrap();
        }
        let path = dir.join("veld.toml");
        std::fs::write(
            &path,
            "[project]\nname = \"w\"\n[libraries.common]\ncontracts = \"common\"\npackage = \"@acme/common\"\nversion = \"0.1.0\"\n[services.iam]\ncontracts = \"iam\"\n",
        )
        .unwrap();
        let p = load(&path).unwrap();
        // Only `common` declares a package name, so it is the default choice.
        assert_eq!(plan(&path, &p, None).unwrap().name, "@acme/common");
        let two = std::fs::read_to_string(&path).unwrap().replace(
            "[services.iam]
contracts = \"iam\"
",
            "[services.iam]
contracts = \"iam\"
package = \"@acme/iam\"
version = \"1.0.0\"
",
        );
        std::fs::write(&path, two).unwrap();
        let p2 = load(&path).unwrap();
        let err = plan(&path, &p2, None).unwrap_err().to_string();
        assert!(err.contains("--package"), "{err}");
        assert_eq!(plan(&path, &p2, Some("iam")).unwrap().name, "@acme/iam");
        assert_eq!(
            plan(&path, &p, Some("common")).unwrap().name,
            "@acme/common"
        );
        assert!(plan(&path, &p, Some("nope")).is_err());
    }
}
