//! The framework adapters of the `node` server: one module per framework and ONE registry slice.
//!
//! `server/adapters.ts` is one file: the shared helpers followed by the adapter of every
//! framework, in a fixed order, so the output is identical whichever framework a target names.
//! Each adapter contributes its own part; [`emit`] assembles the file.

mod express;
mod fastify;
mod hono;
mod nest;
mod node_http;

use veld_codegen::{FileSet, FrameworkAdapter, FrameworkInfo, GenDiagnostic, Profile};
use veld_emit_typescript::util::add_file;

/// The Node server as the adapters see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerModel {
    /// Root-relative path of the assembled adapters file (`server/adapters.ts`).
    pub adapters_path: String,
    /// The services the server hosts. Adapters that emit per-service files (`nest`) read it.
    pub services: Vec<ServiceModel>,
}

/// One hosted service, as the framework adapters need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceModel {
    /// The service name as written in the contract (`Orders`).
    pub raw_name: String,
    /// PascalCase name used in generated identifiers (`Orders`).
    pub name: String,
    /// File stem of the service's files (`orders`).
    pub file: String,
    /// Root-relative module path of `create<Service>Handler` (`server/orders`).
    pub module: String,
    /// Route prefix without slashes at the ends (`orders`), in Nest's `:param` syntax.
    pub prefix: String,
    /// Whether the service's hooks have required members (auth or middleware).
    pub hooks_required: bool,
    /// Endpoints in route-registration order: literal routes before parameterised ones.
    pub endpoints: Vec<EndpointModel>,
}

/// One endpoint of a [`ServiceModel`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointModel {
    /// The TypeScript method name.
    pub name: String,
    pub method: veld_ir::Method,
    /// Path below the service prefix, in Nest's `:param` syntax (empty for the root).
    pub path: String,
    pub status: u16,
}

/// Shared helpers every adapter builds on: the head of `server/adapters.ts`.
const SHARED: &str = include_str!("../../runtime/adapters/shared.ts");

/// Every framework the `node` generator supports. This slice is the single source of truth:
/// `frameworks(role)` and the central `E0803` check are derived from it.
pub const ADAPTERS: &[&dyn FrameworkAdapter<ServerModel>] = &[
    &express::Express,
    &fastify::Fastify,
    &hono::Hono,
    &node_http::NodeHttp,
    &nest::Nest,
];

/// `server/ws.ts`: the WebSocket adapter, emitted when the server hosts `ws` endpoints.
const WS: &str = include_str!("../../runtime/ws.ts");

/// Adds `server/ws.ts`. It is the only generated file written against the `ws` package.
pub fn emit_ws(files: &mut FileSet) -> Result<(), Vec<GenDiagnostic>> {
    add_file(files, "server/ws.ts", WS.to_string())
}

/// The registry as the core sees it.
#[must_use]
pub fn infos() -> Vec<FrameworkInfo> {
    veld_codegen::framework_infos(ADAPTERS)
}

/// The path of the part an adapter contributes: `<adapters file>.part-<position>`.
/// Parts are concatenated in the order of `position`.
fn part_path(model: &ServerModel, position: u8) -> String {
    format!("{}.part-{position}", model.adapters_path)
}

/// A one-file set holding an adapter's section of `server/adapters.ts`.
fn part(model: &ServerModel, position: u8, source: &str) -> Result<FileSet, Vec<GenDiagnostic>> {
    let mut files = FileSet::new();
    add_file(&mut files, &part_path(model, position), source.to_string())?;
    Ok(files)
}

/// Assembles `server/adapters.ts` from the shared helpers and every adapter's part.
pub fn emit(
    files: &mut FileSet,
    model: &ServerModel,
    profile: &Profile,
) -> Result<(), Vec<GenDiagnostic>> {
    let mut text = SHARED.to_string();
    let mut parts: Vec<(String, Vec<u8>)> = Vec::new();
    let part_prefix = format!("{}.part-", model.adapters_path);
    for adapter in ADAPTERS {
        let set = adapter.files(model, profile)?;
        for (path, bytes) in set.iter() {
            if path.starts_with(&part_prefix) {
                parts.push((path.to_string(), bytes.to_vec()));
            } else {
                // A file of the adapter's own (`nest`): it goes out as it is.
                add_file(files, path, String::from_utf8_lossy(bytes).into_owned())?;
            }
        }
    }
    parts.sort();
    for (_, bytes) in &parts {
        text.push_str(&String::from_utf8_lossy(bytes));
    }
    add_file(files, &model.adapters_path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_lists_every_framework_once_with_its_minimum_language() {
        let infos = infos();
        let ids: Vec<&str> = infos.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["express", "fastify", "hono", "node-http", "nest"]);
        assert!(infos.iter().all(|i| i.min_language.is_some()));
        // Only the Nest adapter is versioned; the plain adapters take no framework-version.
        for info in &infos {
            let expected: &[&str] = if info.id == "nest" { &["10"] } else { &[] };
            assert_eq!(info.versions, expected, "{}", info.id);
        }
    }

    #[test]
    fn parts_assemble_in_position_order() {
        let model = ServerModel {
            adapters_path: "server/adapters.ts".into(),
            services: Vec::new(),
        };
        let mut files = FileSet::new();
        emit(&mut files, &model, &Profile::unversioned()).unwrap();
        let text = String::from_utf8(files.get("server/adapters.ts").unwrap().to_vec()).unwrap();
        let at = |needle: &str| text.find(needle).unwrap();
        assert!(at("// Node http") < at("// Express"));
        assert!(at("// Express") < at("// Fastify"));
        assert!(at("// Fastify") < at("// Hono"));
        assert_eq!(files.len(), 1);
    }
}
