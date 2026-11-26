//! Flags (`spec/platform.md` section 11, `spec/targets/rust.md` section 17.2): which flags an
//! endpoint can reach, the provider in the hooks, and the reading of a request.

use std::collections::BTreeSet;

use veld_codegen::CodeWriter;
use veld_ir::{DeclKind, Endpoint, Ir, Service};

use crate::tree::Tree;
use crate::util::lit;

/// The flags of every flagged invariant of the declarations of the tree: any request type, result,
/// error or event can be checked on any endpoint, so every endpoint reads them.
fn invariant_flags(tree: &Tree<'_>) -> BTreeSet<String> {
    tree.decls()
        .iter()
        .flat_map(|id| tree.ir.decl(*id).rules())
        .filter_map(|r| r.flag.clone())
        .collect()
}

/// The flags of the endpoint's own `require` and `ensure`, of the clauses of the policies its
/// `allow` lines name and of the edges its `transition` can take.
fn statement_flags(ir: &Ir, ep: &Endpoint) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    out.extend(ep.rules.requires.iter().filter_map(|r| r.flag.clone()));
    out.extend(ep.rules.ensures.iter().filter_map(|e| e.flag.clone()));
    for a in &ep.rules.allows {
        if let Some(policy) = ir.policies.get(a.policy.index()) {
            out.extend(policy.clauses.iter().filter_map(|c| c.flag.clone()));
        }
    }
    out.extend(transition_edge_flags(ir, ep));
    out
}

/// The flags of the edges from a source of the transition to its target.
pub(crate) fn transition_edge_flags(ir: &Ir, ep: &Endpoint) -> Vec<String> {
    let Some(t) = &ep.rules.transition else {
        return Vec::new();
    };
    match &ir.decl(t.decl).kind {
        DeclKind::Enum(e) => e
            .flows
            .iter()
            .filter(|f| f.to == t.to && t.from.contains(&f.from))
            .filter_map(|f| f.flag.clone())
            .collect(),
        DeclKind::Model(_)
        | DeclKind::Error(_)
        | DeclKind::Union(_)
        | DeclKind::Const(_)
        | DeclKind::Event(_) => Vec::new(),
    }
}

/// The flags an endpoint may touch, sorted: what the server reads once per request.
#[must_use]
pub fn endpoint_flags(tree: &Tree<'_>, ep: &Endpoint) -> Vec<String> {
    let mut all = invariant_flags(tree);
    all.extend(statement_flags(tree.ir, ep));
    all.into_iter().collect()
}

/// Whether an endpoint of the service can reach a flag: the hooks then name a provider.
#[must_use]
pub fn svc_reads_flags(tree: &Tree<'_>, svc: &Service) -> bool {
    tree.flags
        && svc
            .endpoints
            .iter()
            .any(|ep| !endpoint_flags(tree, ep).is_empty())
}

/// The members of the hooks of a service that reads flags: the provider.
pub fn hooks(w: &mut CodeWriter) {
    w.blank();
    w.line("/// The provider of the flags (`spec/platform.md` section 11). Implement");
    w.line("/// `veld::flags::FlagProvider` over your flag service, or use `veld::flags::MemoryFlagProvider`");
    w.line("/// in tests. The server asks each flag an endpoint can reach once per request, after authentication.");
    w.line("type Flags: veld::flags::FlagProvider;");
    w.blank();
    w.line("/// The provider the endpoints read their flags from.");
    w.line("fn flags(&self) -> &Self::Flags;");
}

/// `flag_names` of the module: the flags each endpoint reads, by index.
pub fn names_fn(w: &mut CodeWriter, tree: &Tree<'_>, svc: &Service) {
    w.line("/// The flags the endpoint with this index reads once per request, sorted.");
    w.line("#[doc(hidden)]");
    w.open("pub fn flag_names(endpoint: usize) -> &'static [&'static str]");
    w.open("match endpoint");
    for (i, ep) in svc.endpoints.iter().enumerate() {
        let names = endpoint_flags(tree, ep);
        if !names.is_empty() {
            let list: Vec<String> = names.iter().map(|n| lit(n)).collect();
            w.line(format!("{i} => &[{}],", list.join(", ")));
        }
    }
    w.line("_ => &[],");
    w.close().close().blank();
}

/// `read_flags` of a dispatch: asks the provider of the hooks for each flag of the endpoint.
pub fn forwards(w: &mut CodeWriter) {
    w.open("async fn read_flags(&self, endpoint: usize, ctx: &veld::server::RequestContext) -> veld::rules::FlagView");
    w.line("let mut view = veld::rules::FlagView::new();");
    w.line("let names = flag_names(endpoint);");
    w.line("if names.is_empty() { return view; }");
    w.line("let context = veld::flags::FlagContext { principal: ctx.principal.clone(), tenant: None, operation: ENDPOINTS[endpoint].sig.to_string() };");
    w.open("for name in names");
    w.open("match veld::flags::FlagProvider::is_on(self.hooks.flags(), name, &context).await");
    w.line("Ok(true) => view.insert(name, veld::rules::Reading::On),");
    w.line("Ok(false) => view.insert(name, veld::rules::Reading::Off),");
    w.open("Err(err) =>");
    w.line("self.hooks.on_error(&err, ctx);");
    w.line("view.insert(name, veld::rules::Reading::Failed);");
    w.close();
    w.close().close();
    w.line("view");
    w.close().blank();
}
