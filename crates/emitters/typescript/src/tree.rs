//! Where generated TypeScript files live, and the per-file import bookkeeping.

use std::collections::{BTreeMap, BTreeSet};

use veld_ir::{Decl, DeclId, DeclKind, Ir, PackageId, PackageKind, ServiceId};

use crate::util::{self, HEADER};

/// A set of generated modules rooted at one directory.
///
/// The client target uses one tree at the output root. The server target uses
/// one tree for itself (`flat_home` set, so the home package is not nested)
/// and one per consumed service under `clients/<service>/`.
#[derive(Debug, Clone)]
pub struct Tree<'a> {
    pub ir: &'a Ir,
    /// Directory prefix inside the output, empty or ending in `/`.
    pub root: String,
    /// Package whose files sit directly under `models/` and `errors/` (server target).
    pub flat_home: Option<PackageId>,
    /// Module path of the runtime relative to the output root, without extension.
    pub runtime: String,
    pub services: Vec<ServiceId>,
    /// False when the target option `validate = false` is set: the generated clients and handlers
    /// default to skipping runtime request validation. The types are unchanged.
    pub validate: bool,
    rules: bool,
    endpoint_rules: bool,
    transitions: bool,
    policies: bool,
    rollout: bool,
    events: bool,
    decls: Vec<DeclId>,
}

impl<'a> Tree<'a> {
    /// Collects the declarations to emit: everything reachable from the services,
    /// the prelude, and every declaration of the services' own packages.
    #[must_use]
    pub fn new(
        ir: &'a Ir,
        root: &str,
        flat_home: Option<PackageId>,
        runtime: &str,
        services: Vec<ServiceId>,
    ) -> Self {
        let mut set: BTreeSet<DeclId> = ir.reachable_decls(&services).into_iter().collect();
        let own: BTreeSet<PackageId> = services.iter().map(|s| ir.service(*s).package).collect();
        for d in &ir.decls {
            let in_scope =
                ir.package(d.package).kind == PackageKind::Prelude || own.contains(&d.package);
            // An event is never emitted: a client generates nothing for events (`spec/events.md` 14).
            if in_scope && !d.is_event() {
                set.insert(d.id);
            }
        }
        let endpoint_rules = services.iter().any(|s| {
            ir.service(*s)
                .endpoints
                .iter()
                .any(|ep| !ep.rules.is_empty())
        });
        let endpoints = || {
            services
                .iter()
                .flat_map(|s| ir.service(*s).endpoints.iter())
        };
        let transitions = endpoints().any(|ep| ep.rules.transition.is_some());
        let policies = endpoints().any(|ep| ep.rules.first_policy_origin().is_some());
        let rules = endpoint_rules || set.iter().any(|id| !ir.decl(*id).rules().is_empty());
        let rollout = set
            .iter()
            .any(|id| ir.decl(*id).rules().iter().any(veld_ir::Rule::is_audited))
            || endpoints().any(|ep| ep.rules.first_audited_origin().is_some());
        Self {
            ir,
            root: root.to_string(),
            flat_home,
            runtime: runtime.to_string(),
            services,
            validate: true,
            rules,
            endpoint_rules,
            transitions,
            policies,
            rollout,
            events: false,
            decls: set.into_iter().collect(),
        }
    }

    /// Also emits the events of the tree's services and the declarations only they reach: the
    /// server target does it for a service that publishes or subscribes (`spec/events.md` section
    /// 14), and a client never does. The rule flags are worked out again, because an invariant of
    /// an event is an invariant of the generated package.
    #[must_use]
    pub fn with_events(mut self) -> Self {
        let ir = self.ir;
        let mut set: BTreeSet<DeclId> = self.decls.iter().copied().collect();
        set.extend(ir.events_of(&self.services));
        set.extend(ir.event_reachable_decls(&self.services));
        self.events = self.services.iter().any(|s| ir.service(*s).uses_events());
        self.rules = self.rules || set.iter().any(|id| !ir.decl(*id).rules().is_empty());
        self.rollout = self.rollout
            || set
                .iter()
                .any(|id| ir.decl(*id).rules().iter().any(veld_ir::Rule::is_audited));
        self.decls = set.into_iter().collect();
        self
    }

    /// Whether the tree emits events: some service of it publishes or subscribes. It adds
    /// `runtime/events.ts` and `server/<service>-events.ts`.
    #[must_use]
    pub fn uses_events(&self) -> bool {
        self.events
    }

    /// Whether an emitted declaration has an invariant. This one predicate decides whether the
    /// generated package has `runtime/rules.ts`, whether `runtime/veld.ts` keeps its `rules`
    /// regions, and whether models, clients and servers get the rule code and members.
    #[must_use]
    pub fn uses_rules(&self) -> bool {
        self.rules
    }

    /// Whether an endpoint of the tree's services has endpoint rules (`spec/rules.md` section 9).
    /// It switches `uses_rules` on too, and it adds the `endpoint_rules` regions of the runtime,
    /// the fact loaders and the test handler. A contract with invariants only never sees any of it
    /// (the rules digest is not one of them: every service that reaches an invariant has it).
    #[must_use]
    pub fn uses_endpoint_rules(&self) -> bool {
        self.endpoint_rules
    }

    /// Whether an endpoint of the tree's services has a `transition` (`spec/rules.md` section 10).
    /// It adds the `transitions` regions of the runtime, the guard and the `transition` the
    /// handler receives.
    #[must_use]
    pub fn uses_transitions(&self) -> bool {
        self.transitions
    }

    /// Whether an endpoint of the tree's services has an `allow` or reads the principal
    /// (`spec/rules.md` section 11): the `policies` regions of the runtime, the policy functions,
    /// `can<Endpoint>` and the typed auth hook of a service with a principal.
    #[must_use]
    pub fn uses_policies(&self) -> bool {
        self.policies
    }

    /// Whether a declaration or an endpoint of the tree has an audited rule (`@rollout(audit)`,
    /// `spec/rules.md` section 17.1): the `rollout` regions of the runtime and `runtime/rollout.ts`.
    #[must_use]
    pub fn uses_rollout(&self) -> bool {
        self.rollout
    }

    /// The regions of `runtime/veld.ts` this tree needs.
    #[must_use]
    pub fn gates(&self) -> crate::runtime::Gates {
        crate::runtime::Gates {
            rules: self.rules,
            endpoint_rules: self.endpoint_rules,
            transitions: self.transitions,
            policies: self.policies,
            rollout: self.rollout,
        }
    }

    /// Module path of the audit sink: next to the runtime module, without extension.
    #[must_use]
    pub fn rollout_module(&self) -> String {
        sibling(&self.runtime, "rollout")
    }

    /// Module path of the event runtime (`runtime/events.ts`): next to the runtime module.
    #[must_use]
    pub fn events_module(&self) -> String {
        sibling(&self.runtime, "events")
    }

    /// Module path of the rule helpers: next to the runtime module, without extension.
    #[must_use]
    pub fn rules_module(&self) -> String {
        sibling_rules(&self.runtime)
    }

    /// Sets whether the generated code validates requests at run time by default.
    #[must_use]
    pub fn with_validate(mut self, validate: bool) -> Self {
        self.validate = validate;
        self
    }

    #[must_use]
    pub fn decls(&self) -> &[DeclId] {
        &self.decls
    }

    /// Packages of the tree's services, in order of first appearance.
    #[must_use]
    pub fn packages(&self) -> Vec<PackageId> {
        let mut out: Vec<PackageId> = Vec::new();
        for s in &self.services {
            let p = self.ir.service(*s).package;
            if !out.contains(&p) {
                out.push(p);
            }
        }
        out
    }

    /// Directory prefix (ending in `/` or empty) for a package's files.
    #[must_use]
    pub fn pkg_prefix(&self, pkg: PackageId) -> String {
        let package = self.ir.package(pkg);
        if package.kind == PackageKind::Prelude || self.flat_home == Some(pkg) {
            return self.root.clone();
        }
        let dir = package.name.trim_start_matches('@').replace('/', "-");
        if self.flat_home.is_some() {
            format!("{}deps/{dir}/", self.root)
        } else {
            format!("{}{dir}/", self.root)
        }
    }

    /// Module path (root-relative, no extension) holding `d`.
    #[must_use]
    pub fn decl_module(&self, d: &Decl) -> String {
        if self.ir.package(d.package).kind == PackageKind::Prelude {
            return format!("{}prelude", self.root);
        }
        let kind = match d.kind {
            DeclKind::Error(_) => "errors",
            // Events have a tree of their own: `models/` is the same in both roles.
            DeclKind::Event(_) => "events",
            DeclKind::Model(_) | DeclKind::Enum(_) | DeclKind::Union(_) | DeclKind::Const(_) => {
                "models"
            }
        };
        let module = if d.module.is_empty() {
            "root".to_string()
        } else {
            d.module.replace('.', "/")
        };
        format!("{}{kind}/{module}", self.pkg_prefix(d.package))
    }

    /// Module path (root-relative, no extension) holding the functions of `policy`.
    #[must_use]
    pub fn policy_module(&self, policy: &veld_ir::Policy) -> String {
        let module = if policy.module.is_empty() {
            "root".to_string()
        } else {
            policy.module.replace('.', "/")
        };
        format!("{}policies/{module}", self.pkg_prefix(policy.package))
    }

    /// Module path of a package's `client.ts`.
    #[must_use]
    pub fn client_module(&self, pkg: PackageId) -> String {
        format!("{}client", self.pkg_prefix(pkg))
    }
}

/// The module path of the rule helpers: next to the runtime module.
fn sibling_rules(runtime_module: &str) -> String {
    sibling(runtime_module, "rules")
}

/// The module path of a helper file `name` next to the runtime module.
fn sibling(runtime_module: &str, name: &str) -> String {
    match runtime_module.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{name}"),
        None => name.to_string(),
    }
}

/// Imports of one generated file.
#[derive(Debug, Default)]
pub struct Imports {
    /// module -> name -> is a value import
    named: BTreeMap<String, BTreeMap<String, bool>>,
    runtime: bool,
    rules: bool,
    rollout: bool,
    events: bool,
}

impl Imports {
    fn add(&mut self, module: &str, name: &str, value: bool) {
        let entry = self
            .named
            .entry(module.to_string())
            .or_default()
            .entry(name.to_string())
            .or_insert(false);
        *entry |= value;
    }

    /// Import lines for the file `from_file`, sorted by specifier.
    #[must_use]
    pub fn render(&self, from_file: &str, runtime_module: &str) -> Vec<String> {
        let mut lines: Vec<(String, String)> = Vec::new();
        if self.runtime {
            let spec = util::rel_import(from_file, runtime_module);
            lines.push((spec.clone(), format!("import * as veld from '{spec}';")));
        }
        if self.rules {
            let spec = util::rel_import(from_file, &sibling_rules(runtime_module));
            lines.push((
                spec.clone(),
                format!("import * as veldRules from '{spec}';"),
            ));
        }
        if self.rollout {
            let spec = util::rel_import(from_file, &sibling(runtime_module, "rollout"));
            lines.push((
                spec.clone(),
                format!("import * as veldRollout from '{spec}';"),
            ));
        }
        if self.events {
            let spec = util::rel_import(from_file, &sibling(runtime_module, "events"));
            lines.push((
                spec.clone(),
                format!("import * as veldEvents from '{spec}';"),
            ));
        }
        for (module, names) in &self.named {
            let spec = util::rel_import(from_file, module);
            let values: Vec<&str> = names
                .iter()
                .filter(|(_, v)| **v)
                .map(|(n, _)| n.as_str())
                .collect();
            let types: Vec<&str> = names
                .iter()
                .filter(|(_, v)| !**v)
                .map(|(n, _)| n.as_str())
                .collect();
            if !values.is_empty() {
                lines.push((
                    spec.clone(),
                    format!("import {{ {} }} from '{spec}';", values.join(", ")),
                ));
            }
            if !types.is_empty() {
                lines.push((
                    spec.clone(),
                    format!("import type {{ {} }} from '{spec}';", types.join(", ")),
                ));
            }
        }
        lines.sort();
        lines.into_iter().map(|(_, l)| l).collect()
    }
}

/// A file being generated: tracks the imports its body needs.
#[derive(Debug)]
pub struct FileCtx<'t, 'a> {
    pub tree: &'t Tree<'a>,
    /// Root-relative module path of this file, without extension.
    pub module: String,
    pub imports: Imports,
}

impl<'t, 'a> FileCtx<'t, 'a> {
    #[must_use]
    pub fn new(tree: &'t Tree<'a>, module: &str) -> Self {
        Self {
            tree,
            module: module.to_string(),
            imports: Imports::default(),
        }
    }

    /// `veld.<name>`, recording the runtime import.
    pub fn rt(&mut self, name: &str) -> String {
        self.imports.runtime = true;
        format!("veld.{name}")
    }

    /// The namespace of `runtime/rules.ts` (`veldRules`), recording the import.
    pub fn rules_ns(&mut self) -> String {
        self.imports.rules = true;
        "veldRules".to_string()
    }

    /// The namespace of `runtime/rollout.ts` (`veldRollout`), recording the import.
    pub fn rollout_ns(&mut self) -> String {
        self.imports.rollout = true;
        "veldRollout".to_string()
    }

    /// The namespace of `runtime/events.ts` (`veldEvents`), recording the import.
    pub fn events_ns(&mut self) -> String {
        self.imports.events = true;
        "veldEvents".to_string()
    }

    /// Name of `id` used as a type; records a type-only import when it lives elsewhere.
    pub fn type_ref(&mut self, id: DeclId) -> String {
        self.decl_ref(id, false)
    }

    /// Name of `id` used as a value (companion or class).
    pub fn value_ref(&mut self, id: DeclId) -> String {
        self.decl_ref(id, true)
    }

    fn decl_ref(&mut self, id: DeclId, value: bool) -> String {
        let d = self.tree.ir.decl(id);
        let module = self.tree.decl_module(d);
        let name = util::type_name(d);
        if module != self.module {
            self.imports.add(&module, &name, value);
        }
        name
    }

    /// Imports a named value or type from an arbitrary module.
    pub fn import(&mut self, module: &str, name: &str, value: bool) {
        if module != self.module {
            self.imports.add(module, name, value);
        }
    }

    /// Complete file text: header, imports, then `body`.
    #[must_use]
    pub fn finish(&self, body: &str) -> String {
        let from = format!("{}.ts", self.module);
        let mut out = String::from(HEADER);
        out.push('\n');
        let imports = self.imports.render(&from, &self.tree.runtime);
        if !imports.is_empty() {
            out.push('\n');
            for l in imports {
                out.push_str(&l);
                out.push('\n');
            }
        }
        out.push('\n');
        out.push_str(body);
        out
    }
}

impl FileCtx<'_, '_> {
    /// The class and data-interface names of an error declaration, importing both.
    pub fn error_refs(&mut self, id: DeclId) -> (String, String) {
        let class = self.value_ref(id);
        let data = format!("{class}Data");
        let module = self.tree.decl_module(self.tree.ir.decl(id));
        self.import(&module, &data, false);
        (class, data)
    }
}

impl Tree<'_> {
    /// A prelude declaration by name (`NotFound`, `ValidationError`, ...).
    #[must_use]
    pub fn prelude_decl(&self, name: &str) -> Option<DeclId> {
        self.ir
            .decls
            .iter()
            .find(|d| d.name == name && self.ir.package(d.package).kind == PackageKind::Prelude)
            .map(|d| d.id)
    }
}
