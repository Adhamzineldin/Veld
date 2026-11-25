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
    /// Whether the target has rules: such a target also routes the path with a trailing slash.
    pub rules: bool,
    /// The fact-loader interface of the service (`<Service>FactLoaders`), when it has facts.
    pub loaders: Option<String>,
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
    /// What the endpoint speaks.
    pub kind: EndpointKind,
}

/// The transport of an endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointKind {
    /// A request and a response.
    Http,
    /// Server-Sent Events: a `get` that streams its response.
    Sse,
    /// A WebSocket.
    Ws,
}

impl ServiceModel {
    /// Whether any endpoint of the service is a WebSocket.
    #[must_use]
    pub fn has_ws(&self) -> bool {
        self.endpoints.iter().any(|e| e.kind == EndpointKind::Ws)
    }
}

impl ServerModel {
    /// The streaming transports the served services use: what an adapter must carry code for.
    #[must_use]
    pub fn streaming(&self) -> runtime::Streaming {
        let any = |kind: EndpointKind| {
            self.services
                .iter()
                .any(|s| s.endpoints.iter().any(|e| e.kind == kind))
        };
        runtime::Streaming {
            sse: any(EndpointKind::Sse),
            ws: any(EndpointKind::Ws),
        }
    }

    /// Whether any service has a WebSocket endpoint.
    #[must_use]
    pub fn has_ws(&self) -> bool {
        self.services.iter().any(ServiceModel::has_ws)
    }
}

/// Every framework the Java server supports. This slice is the single source of truth:
/// `frameworks(role)` and the central `E0803` check are derived from it.
pub const ADAPTERS: &[&dyn FrameworkAdapter<ServerModel>] = &[&jdk::Jdk, &spring::Spring];

/// The framework of a target that names none.
pub(crate) const BASELINE: &str = jdk::ID;

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
        runtime::substitute(source, &model.base, model.streaming()),
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
