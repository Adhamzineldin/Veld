//! The framework adapters of the Java server: one module per framework and ONE registry slice.
//!
//! The JDK adapter is the default framework. A target emits the files of its own framework only:
//! `jdk` gets no Spring file and `spring` gets no JDK adapter.

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
    /// One entry per service, for adapters that register routes with their framework.
    pub services: Vec<ServiceModel>,
}

/// One service as a framework adapter sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceModel {
    /// The service name in Java case (`Orders`): `<name>Service`, `<name>Hooks`, `<name>Handler`.
    pub name: String,
    /// The Java package of those three classes.
    pub server_package: String,
    /// The Java package an adapter puts this service's framework class in.
    pub adapter_package: String,
    /// Whether the service's endpoints need hooks (auth or middleware).
    pub needs_hooks: bool,
    pub endpoints: Vec<EndpointModel>,
}

/// One endpoint of a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointModel {
    /// The endpoint name, as the runtime knows it.
    pub name: String,
    /// The Java method name.
    pub method_name: String,
    /// The HTTP method, upper case.
    pub http_method: &'static str,
    /// The full path with `{param}` segments.
    pub path: String,
}

/// Every framework the Java server supports. This slice is the single source of truth:
/// `frameworks(role)` and the central `E0803` check are derived from it.
pub const ADAPTERS: &[&dyn FrameworkAdapter<ServerModel>] = &[&jdk::Jdk, &spring::Spring];

/// The framework of a target that names none.
const BASELINE: &str = jdk::ID;

/// The registry as the core sees it.
#[must_use]
pub fn infos() -> Vec<FrameworkInfo> {
    veld_codegen::framework_infos(ADAPTERS)
}

/// Emits the adapter of the profile's framework (`jdk` when none is named) and nothing of the others.
pub fn emit(
    files: &mut FileSet,
    model: &ServerModel,
    profile: &Profile,
) -> Result<(), Vec<GenDiagnostic>> {
    for adapter in ADAPTERS {
        if adapter.id() == profile.framework().unwrap_or(BASELINE) {
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
    fn each_framework_emits_only_its_own_files() {
        let model = ServerModel {
            base: "a.b".into(),
            services: Vec::new(),
        };
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
        assert_eq!(paths(Some("spring")), ["a/b/adapters/SpringAdapter.java"]);
    }
}
