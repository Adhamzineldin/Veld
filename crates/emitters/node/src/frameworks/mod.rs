//! The framework adapters of the `node` server: one module per framework and ONE registry slice.
//!
//! `server/adapters.ts` is one file: the shared helpers followed by the adapter of every
//! framework, in a fixed order, so the output is identical whichever framework a target names.
//! Each adapter contributes its own part; [`emit`] assembles the file.

mod express;
mod fastify;
mod hono;
mod node_http;

use veld_codegen::{FileSet, FrameworkAdapter, FrameworkInfo, GenDiagnostic, Profile};
use veld_emit_typescript::util::add_file;

/// The Node server as the adapters see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerModel {
    /// Root-relative path of the assembled adapters file (`server/adapters.ts`).
    pub adapters_path: String,
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
];

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
    for adapter in ADAPTERS {
        let set = adapter.files(model, profile)?;
        parts.extend(set.iter().map(|(p, b)| (p.to_string(), b.to_vec())));
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
        assert_eq!(ids, ["express", "fastify", "hono", "node-http"]);
        assert!(
            infos
                .iter()
                .all(|i| i.versions.is_empty() && i.min_language.is_some())
        );
    }

    #[test]
    fn parts_assemble_in_position_order() {
        let model = ServerModel {
            adapters_path: "server/adapters.ts".into(),
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
