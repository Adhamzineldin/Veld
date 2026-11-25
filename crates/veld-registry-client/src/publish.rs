//! Packing a library or service of the workspace for `veld publish`.

use veld_ir::PackageKind;
use veld_project::config::{MemberKind, MemberSpec, PackageSpec, Project, collect_sources};
use veld_project::packages::split_name;

use crate::error::{RegistryError, Result};
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

fn publishable(m: &MemberSpec) -> bool {
    matches!(m.kind, MemberKind::Library | MemberKind::Service)
}

/// The publishable member: the one named by `package`, else the current member, else the only
/// library or service, else the only one that declares `package = "@org/name"`.
pub fn select_package<'a>(project: &'a Project, package: Option<&str>) -> Result<&'a MemberSpec> {
    let candidates: Vec<&MemberSpec> = project.members.iter().filter(|m| publishable(m)).collect();
    let names = || {
        candidates
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if let Some(want) = package {
        return candidates
            .iter()
            .copied()
            .find(|m| m.name == want)
            .ok_or_else(|| {
                RegistryError::Resolve(format!("no package named `{want}` (have: {})", names()))
            });
    }
    if let Some(current) = project
        .current
        .as_deref()
        .and_then(|c| candidates.iter().copied().find(|m| m.name == c))
    {
        return Ok(current);
    }
    // Without --package, the one package that declares `package = "@org/name"` is meant.
    let marked: Vec<&MemberSpec> = candidates
        .iter()
        .copied()
        .filter(|m| m.package.is_some())
        .collect();
    match (candidates.as_slice(), marked.as_slice()) {
        ([only], _) => Ok(only),
        ([], _) => Err(RegistryError::Resolve("the project has no packages".into())),
        (_, [one]) => Ok(one),
        _ => Err(RegistryError::Resolve(format!(
            "this workspace has several packages; choose one with --package ({})",
            names()
        ))),
    }
}

/// `[service]` in `services/x/veld.toml`, for messages.
fn section_of(project: &Project, m: &MemberSpec) -> String {
    let table = if m.kind == MemberKind::Library {
        "[library]"
    } else {
        "[service]"
    };
    let file = m.file.strip_prefix(&project.root).unwrap_or(&m.file);
    format!("{table} in `{}`", file.to_string_lossy().replace('\\', "/"))
}

fn contracts_of<'a>(project: &'a Project, m: &MemberSpec) -> Result<&'a PackageSpec> {
    project
        .packages
        .iter()
        .find(|p| p.name == m.name && matches!(p.kind, PackageKind::Library | PackageKind::Service))
        .ok_or_else(|| RegistryError::Resolve(format!("`{}` has no contracts", m.name)))
}

/// Packs the selected package of the workspace.
pub fn plan(project: &Project, package: Option<&str>) -> Result<PublishPlan> {
    let member = select_package(project, package)?;
    let section = section_of(project, member);
    let name = member.package.clone().ok_or_else(|| {
        RegistryError::Resolve(format!(
            "{section} needs `package = \"@org/name\"` and `version = \"1.0.0\"` to be published"
        ))
    })?;
    let version = member.version.clone().ok_or_else(|| {
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
    let spec = contracts_of(project, member)?;
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
    // its whole graph. For a workspace, every package shares the root's dependencies.
    let manifest = PackageManifest::new(&name, &version, member.description.as_deref(), &files)
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
    use std::path::Path;
    use veld_project::config::load;

    fn write(dir: &Path, rel: &str, text: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn single(dir: &Path, toml: &str) -> Project {
        write(dir, "contracts/m.veld", "model M { id: uuid }\n");
        write(dir, "veld.toml", toml);
        load(&dir.join("veld.toml")).unwrap()
    }

    #[test]
    fn packs_a_single_file_project() {
        let tmp = tempfile::tempdir().unwrap();
        let p = single(
            tmp.path(),
            "[service]\nname = \"lib\"\npackage = \"@acme/lib\"\nversion = \"1.2.3\"\ndescription = \"d\"\n",
        );
        let plan = plan(&p, None).unwrap();
        assert_eq!(
            (plan.name.as_str(), plan.version.as_str()),
            ("@acme/lib", "1.2.3")
        );
        assert_eq!(plan.files, 1);
        assert_eq!(plan.parts().unwrap(), ("acme", "lib"));
        let again = super::plan(&p, None).unwrap();
        assert_eq!(plan.packed, again.packed, "deterministic");
    }

    #[test]
    fn dependencies_are_written_to_the_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let p = single(
            tmp.path(),
            "[service]
name = \"lib\"
package = \"@acme/billing\"
version = \"1.0.0\"
[dependencies]
\"@acme/money\" = \"^1.0\"
",
        );
        let plan = plan(&p, None).unwrap();
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
        let p = single(tmp.path(), "[service]\nname = \"lib\"\n");
        let err = plan(&p, None).unwrap_err().to_string();
        assert!(err.contains("package = \"@org/name\""), "{err}");
        assert!(err.contains("`veld.toml`"), "{err}");
        let tmp = tempfile::tempdir().unwrap();
        let p = single(
            tmp.path(),
            "[service]\nname = \"lib\"\npackage = \"@acme/lib\"\nversion = \"one\"\n",
        );
        assert!(
            plan(&p, None)
                .unwrap_err()
                .to_string()
                .contains("semver")
        );
    }

    fn workspace(dir: &Path, iam_meta: &str) -> std::path::PathBuf {
        write(
            dir,
            "veld.toml",
            "[workspace]\nname = \"w\"\nmembers = [\"libs/*\", \"services/*\"]\n",
        );
        write(
            dir,
            "libs/common/veld.toml",
            "[library]\npackage = \"@acme/common\"\nversion = \"0.1.0\"\n",
        );
        write(dir, "libs/common/contracts/m.veld", "model M { id: uuid }\n");
        write(dir, "services/iam/veld.toml", &format!("[service]\n{iam_meta}"));
        write(dir, "services/iam/contracts/m.veld", "model N { id: uuid }\n");
        dir.join("veld.toml")
    }

    #[test]
    fn workspaces_need_a_package_choice() {
        let tmp = tempfile::tempdir().unwrap();
        let path = workspace(tmp.path(), "");
        let p = load(&path).unwrap();
        // Only `common` declares a package name, so it is the default choice.
        assert_eq!(plan(&p, None).unwrap().name, "@acme/common");
        let tmp = tempfile::tempdir().unwrap();
        let path = workspace(tmp.path(), "package = \"@acme/iam\"\nversion = \"1.0.0\"\n");
        let p2 = load(&path).unwrap();
        let err = plan(&p2, None).unwrap_err().to_string();
        assert!(err.contains("--package"), "{err}");
        assert_eq!(plan(&p2, Some("iam")).unwrap().name, "@acme/iam");
        assert_eq!(plan(&p2, Some("common")).unwrap().name, "@acme/common");
        assert!(plan(&p2, Some("nope")).is_err());
    }

    #[test]
    fn the_current_member_is_the_default_choice() {
        let tmp = tempfile::tempdir().unwrap();
        workspace(tmp.path(), "package = \"@acme/iam\"\nversion = \"1.0.0\"\n");
        let p = load(&tmp.path().join("services/iam/veld.toml")).unwrap();
        assert_eq!(plan(&p, None).unwrap().name, "@acme/iam");
    }
}
