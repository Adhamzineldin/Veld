//! The built-in generators: an explicit, static list (no linker tricks, no discovery).

use veld_codegen::DynGenerator;
use veld_emit_java::Java;
use veld_emit_node::Node;
use veld_emit_typescript::TypeScript;

/// Every generator compiled into this binary, in a fixed order.
pub fn generators() -> &'static [&'static dyn DynGenerator] {
    &[&Java, &Node, &TypeScript]
}

/// The built-in generator with the given id.
pub fn find(id: &str) -> Option<&'static dyn DynGenerator> {
    generators().iter().copied().find(|g| g.id() == id)
}

/// The ids of all built-in generators, for error messages.
pub fn ids() -> Vec<&'static str> {
    generators().iter().map(|g| g.id()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use veld_codegen::Role;

    #[test]
    fn lists_the_built_in_generators() {
        assert_eq!(ids(), ["java", "node", "typescript"]);
        assert!(find("node").is_some_and(|g| g.roles() == [Role::Server]));
        assert!(find("typescript").is_some_and(|g| g.roles() == [Role::Client]));
        assert!(find("java").is_some_and(|g| g.roles() == [Role::Server, Role::Client]));
        assert!(find("cobol").is_none());
    }
}
