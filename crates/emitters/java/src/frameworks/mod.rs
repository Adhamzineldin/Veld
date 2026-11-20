//! The framework adapters of the Java server: one module per framework and ONE registry slice.
//!
//! The JDK adapter is the baseline and is always emitted, so the core server works without any
//! framework. A target that names another framework gets that adapter as well.

mod jdk;
mod spring;

use veld_codegen::{FileSet, FrameworkAdapter, FrameworkInfo, GenDiagnostic, Profile};

use crate::runtime;
use crate::tree::java_path;
use crate::util::add_file;

/// The Java server as the adapters see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerModel {
    /// The base Java package; adapters live in `<base>.adapters`.
    pub base: String,
}

/// Every framework the Java server supports. This slice is the single source of truth:
/// `frameworks(role)` and the central `E0803` check are derived from it.
pub const ADAPTERS: &[&dyn FrameworkAdapter<ServerModel>] = &[&jdk::Jdk, &spring::Spring];

/// The adapter that is emitted whatever the target names.
const BASELINE: &str = jdk::ID;

/// The registry as the core sees it.
#[must_use]
pub fn infos() -> Vec<FrameworkInfo> {
    veld_codegen::framework_infos(ADAPTERS)
}

/// Emits the baseline adapter and, when the profile names another one, that adapter too.
pub fn emit(
    files: &mut FileSet,
    model: &ServerModel,
    profile: &Profile,
) -> Result<(), Vec<GenDiagnostic>> {
    for adapter in ADAPTERS {
        if adapter.id() == BASELINE || Some(adapter.id()) == profile.framework() {
            for (path, bytes) in adapter.files(model, profile)?.iter() {
                add_file(files, path, String::from_utf8_lossy(bytes).into_owned())?;
            }
        }
    }
    Ok(())
}

/// One adapter class file in `<base>.adapters`, with the package declarations substituted.
fn class_file(
    model: &ServerModel,
    class: &str,
    source: &str,
) -> Result<FileSet, Vec<GenDiagnostic>> {
    let mut files = FileSet::new();
    let package = runtime::adapter_package(&model.base);
    add_file(
        &mut files,
        &java_path(&package, class),
        runtime::substitute(source, &model.base),
    )?;
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_lists_jdk_and_a_versioned_spring() {
        let infos = infos();
        let ids: Vec<&str> = infos.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["jdk", "spring"]);
        assert!(infos[0].versions.is_empty());
        assert_eq!(infos[1].versions, ["3"]);
    }

    #[test]
    fn jdk_is_always_emitted_and_spring_only_when_named() {
        let model = ServerModel { base: "a.b".into() };
        let paths = |framework: Option<&str>| {
            let fw = infos();
            let lang = veld_codegen::LanguageInfo {
                id: "java".into(),
                versions: vec!["17".into()],
            };
            let offer = veld_codegen::Offer {
                generator: "java",
                role: veld_codegen::Role::Server,
                language: Some(&lang),
                frameworks: &fw,
            };
            let profile = Profile::resolve(&offer, framework, &serde_json::Value::Null).unwrap();
            let mut files = FileSet::new();
            emit(&mut files, &model, &profile).unwrap();
            files.paths().map(str::to_string).collect::<Vec<_>>()
        };
        assert_eq!(paths(None), ["a/b/adapters/JdkHttpServerAdapter.java"]);
        assert_eq!(paths(Some("jdk")), paths(None));
        assert_eq!(
            paths(Some("spring")),
            [
                "a/b/adapters/JdkHttpServerAdapter.java",
                "a/b/adapters/SpringAdapter.java"
            ]
        );
    }
}
