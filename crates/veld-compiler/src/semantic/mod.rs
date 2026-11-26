//! Semantic analysis: module graph, name resolution, inheritance, type checks,
//! services and annotations. Each pass reports diagnostics and never panics.

pub mod annotations;
mod collect;
#[cfg(test)]
mod delivery_tests;
mod events;
#[cfg(test)]
mod events_groundwork_tests;
#[cfg(test)]
mod events_tests;
mod examples;
pub use collect::module_path;
mod inherit;
mod machine;
#[cfg(test)]
mod machine_tests;
mod policy;
#[cfg(test)]
mod policy_tests;
pub mod regex;
mod resolve;
#[cfg(test)]
mod rollout_tests;
pub mod rules;
mod services;
#[cfg(test)]
mod tests;
mod typeck;
#[cfg(test)]
mod version_tests;

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostics::{Diagnostic, FileId, Span, has_errors};
use crate::syntax::ast;
pub use events::{
    ChannelEntry, ChannelInfo, RChannel, RChannelKey, RHandler, RSubscription, legal_name,
    topic_is_legal,
};
pub use examples::{REventExample, REventExpect, RExample, RKey, RKeyStore, ROutcome};
pub use policy::{PolicyEntry, PolicyInfo, RClause, RPolicy, RPolicyParam, RPrincipal};
pub use rules::RRule;
pub(crate) use rules::{RAllow, RBlock, RElse, REnsure, RRequire, RTransition};
use veld_ir::{Auth, Constraints, Literal, Method, PackageKind, Type};

/// One parsed `.veld` file of a package.
#[derive(Debug)]
pub struct ParsedFile {
    pub file: FileId,
    /// Path relative to the package root, `/` separators.
    pub rel_path: String,
    pub ast: ast::File,
}

/// One package with all of its parsed files.
#[derive(Debug)]
pub struct ParsedPackage {
    pub name: String,
    pub kind: PackageKind,
    pub base_url: Option<String>,
    /// Route prefix prepended to every service prefix of the package (`/` or empty: none).
    pub prefix: String,
    pub files: Vec<ParsedFile>,
}

#[derive(Debug)]
pub struct PkgInfo {
    pub name: String,
    pub kind: PackageKind,
    pub base_url: Option<String>,
    pub prefix: String,
}

#[derive(Debug)]
pub struct ModuleInfo<'a> {
    pub pkg: usize,
    /// Dotted module path.
    pub path: String,
    pub file: FileId,
    pub rel_path: &'a str,
    pub ast: &'a ast::File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DKind {
    Model,
    Error,
    Enum,
    Union,
    Const,
    Event,
}

#[derive(Debug, Clone)]
pub struct RField {
    pub name: String,
    pub wire: String,
    pub ty: Type,
    pub optional: bool,
    pub default: Option<Literal>,
    pub readonly: bool,
    /// `@version` (`spec/rules.md` section 11.7).
    pub version: bool,
    pub inherited_from: Option<usize>,
    pub constraints: Constraints,
    pub example: Option<Literal>,
    pub docs: Option<String>,
    pub deprecated: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RVariant {
    pub name: String,
    pub wire: String,
    pub docs: Option<String>,
    pub deprecated: Option<String>,
    pub span: Span,
}

/// One edge of a state machine.
#[derive(Debug, Clone)]
pub struct RFlow {
    pub from: String,
    pub to: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RArm {
    pub label: String,
    pub decl: usize,
    pub span: Span,
}

/// What resolution learned about a declaration.
#[derive(Debug, Clone, Default)]
pub struct Resolved {
    pub parent: Option<usize>,
    pub own_fields: Vec<RField>,
    /// Parent-first, including inherited fields.
    pub fields: Vec<RField>,
    pub default_status: Option<u16>,
    pub variants: Vec<RVariant>,
    /// The machine of an enum with flows (`spec/rules.md` section 10).
    pub machine: machine::RMachine,
    pub discriminator: String,
    pub arms: Vec<RArm>,
    pub const_ty: Option<Type>,
    pub const_value: Option<Literal>,
    pub deprecated: Option<String>,
    pub cyclic: bool,
    /// Checked rules the declaration writes itself, in source order (`spec/rules.md` section 5).
    pub own_rules: Vec<RRule>,
}

#[derive(Debug)]
pub struct DeclInfo<'a> {
    pub pkg: usize,
    pub module: usize,
    pub name: String,
    pub qualified: String,
    pub kind: DKind,
    pub ast: &'a ast::Decl,
    pub name_span: Span,
    pub res: Resolved,
}

#[derive(Debug)]
pub struct RParam {
    pub name: String,
    pub binding: veld_ir::Binding,
    pub header: Option<String>,
    pub ty: Type,
    pub optional: bool,
    pub stream: bool,
    pub constraints: Constraints,
}

#[derive(Debug)]
pub struct REndpoint {
    pub name: String,
    pub method: Method,
    pub path: String,
    pub docs: Option<String>,
    pub deprecated: Option<String>,
    pub auth: Auth,
    pub middleware: Vec<String>,
    pub params: Vec<RParam>,
    pub success_status: u16,
    pub success_ty: Option<Type>,
    pub success_stream: bool,
    /// (status, error declaration), deduplicated, in source order with the
    /// implicit `ValidationError`, `Unauthorized` and `Internal` last.
    pub errors: Vec<(u16, usize)>,
    /// The checked endpoint block (`spec/rules.md` section 9); empty without one.
    pub rules: RBlock,
    /// `@idempotent` (`spec/rules.md` section 11.8).
    pub idempotent: Option<RIdempotent>,
    pub span: Span,
}

/// A checked `@idempotent`.
#[derive(Debug, Clone)]
pub struct RIdempotent {
    pub header: String,
    pub required: bool,
    /// The annotation.
    pub span: Span,
}

#[derive(Debug)]
pub struct ServiceInfo {
    pub pkg: usize,
    pub module: usize,
    pub name: String,
    pub prefix: String,
    pub docs: Option<String>,
    pub auth: Auth,
    pub middleware: Vec<String>,
    pub endpoints: Vec<REndpoint>,
    /// Channels the service publishes to (indices into `Program::channels`), in source order.
    pub publishes: Vec<usize>,
    pub publish_spans: Vec<Span>,
    pub subscriptions: Vec<RSubscription>,
    pub span: Span,
}

#[derive(Debug)]
pub struct ImportRec {
    pub local: String,
    pub span: Span,
    pub used: Cell<bool>,
}

#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub decl: usize,
    pub import: Option<usize>,
}

/// Names visible in one module (local declarations plus imports).
#[derive(Debug, Default)]
pub struct Scope {
    pub names: BTreeMap<String, Entry>,
    pub imports: Vec<ImportRec>,
    /// Local names of imports that failed to resolve; uses of them stay silent.
    pub poisoned: BTreeSet<String>,
    /// The policies visible here: declared in this module, or imported.
    pub policy_names: BTreeMap<String, PolicyEntry>,
    /// The channels visible here: declared in this module, or imported.
    pub channel_names: BTreeMap<String, ChannelEntry>,
}

/// The analysed workspace.
#[derive(Debug)]
pub struct Program<'a> {
    pub packages: Vec<PkgInfo>,
    pub modules: Vec<ModuleInfo<'a>>,
    pub decls: Vec<DeclInfo<'a>>,
    pub services: Vec<ServiceInfo>,
    /// Checked examples (`spec/rules.md` section 12).
    pub examples: Vec<RExample>,
    /// Checked event examples (`spec/events.md` section 12).
    pub event_examples: Vec<REventExample>,
    /// Every `channel` of the workspace (`spec/events.md` section 4).
    pub channels: Vec<ChannelInfo<'a>>,
    /// Every `policy` of the workspace (`spec/rules.md` section 11.2).
    pub policies: Vec<PolicyInfo<'a>>,
    /// The principal of each package that declares one, by package index.
    pub principals: BTreeMap<usize, RPrincipal>,
    pub scopes: Vec<Scope>,
    /// Prelude name -> declaration index.
    pub prelude: BTreeMap<String, usize>,
    pub prelude_pkg: usize,
    /// (package, module path) -> module index.
    pub module_index: BTreeMap<(usize, String), usize>,
}

impl Program<'_> {
    #[must_use]
    pub fn is_prelude(&self, decl: usize) -> bool {
        self.decls[decl].pkg == self.prelude_pkg
    }

    /// Looks `name` up as seen from `module`, marking imports as used.
    #[must_use]
    pub fn lookup(&self, module: usize, name: &str) -> Option<usize> {
        if let Some(e) = self.scopes.get(module).and_then(|s| s.names.get(name)) {
            if let (Some(i), Some(scope)) = (e.import, self.scopes.get(module)) {
                scope.imports[i].used.set(true);
            }
            return Some(e.decl);
        }
        self.prelude.get(name).copied()
    }
}

/// Runs every semantic pass. Returns the program only when no errors occurred.
pub fn analyze<'a>(
    packages: &'a [ParsedPackage],
    consumes: &BTreeMap<String, Vec<String>>,
    diags: &mut Vec<Diagnostic>,
) -> Option<Program<'a>> {
    let prog = analyze_lenient(packages, consumes, diags);
    if has_errors(diags) { None } else { Some(prog) }
}

/// Runs every semantic pass and returns the program even when there are errors, so that
/// tools (the language server) can still resolve names in a broken workspace.
pub fn analyze_lenient<'a>(
    packages: &'a [ParsedPackage],
    consumes: &BTreeMap<String, Vec<String>>,
    diags: &mut Vec<Diagnostic>,
) -> Program<'a> {
    let mut prog = collect::collect(packages, diags);
    resolve::build_scopes(&mut prog, diags);
    resolve::resolve_decls(&mut prog, diags);
    inherit::flatten(&mut prog, diags);
    typeck::check(&mut prog, diags);
    events::check_channels(&mut prog, diags);
    rules::check(&mut prog, diags);
    rules::check_policies(&mut prog, diags);
    services::analyze(&mut prog, diags);
    examples::check(&mut prog, diags);
    events::check_graph(&prog, diags);
    resolve::report_unused_imports(&prog, diags);
    collect::check_consumes(&prog, consumes, diags);
    prog
}
