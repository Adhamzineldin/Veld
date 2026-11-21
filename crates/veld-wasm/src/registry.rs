//! The built-in generators. Keep this list in step with `apps/veld/src/registry.rs`.

use veld_codegen::DynGenerator;
use veld_emit_java::Java;
use veld_emit_node::Node;
use veld_emit_rust::Rust;
use veld_emit_typescript::TypeScript;

/// Every generator compiled into the module, in the CLI's order.
pub fn generators() -> &'static [&'static dyn DynGenerator] {
    &[&Java, &Node, &TypeScript, &Rust]
}

/// The generator with the given id.
pub fn find(id: &str) -> Option<&'static dyn DynGenerator> {
    generators().iter().copied().find(|g| g.id() == id)
}
