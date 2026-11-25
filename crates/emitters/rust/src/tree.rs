//! Where generated Rust items live: module paths, file paths and names.

use std::collections::BTreeSet;

use veld_ir::{Decl, DeclId, DeclKind, Ir, PackageId, PackageKind, ServiceId, Type};

use crate::util::{self, RESERVED_MODULES};

/// A set of generated packages rooted at one Rust module.
///
/// The client target and the server's own code use one tree each at the crate root. A server adds
/// one tree per consumed service under `clients::<service>`. Every tree uses the single runtime
/// (`crate::veld`) and the single prelude (`crate::prelude`) of the crate.
#[derive(Debug, Clone)]
pub struct Tree<'a> {
    pub ir: &'a Ir,
    /// The Rust path of the tree's root module: `crate` or `crate::clients::iam`.
    pub root: String,
    /// The directory of the root module, relative to the output: `src` or `src/clients/iam`.
    pub dir: String,
    pub services: Vec<ServiceId>,
    decls: Vec<DeclId>,
    /// Whether the prelude declarations are emitted here (the main tree only).
    pub owns_prelude: bool,
    /// False when the target option `validate = false` is set: run-time request validation is
    /// off by default (the types stay).
    pub validate: bool,
    /// True when the crate has declarations with invariants: every schema then carries its
    /// check function (or `None`), and the declarations with rules get their check functions.
    pub rules: bool,
    /// True when the crate has an audited rule (`@rollout(audit)`, `spec/rules.md` section 17.1): the test
    /// handler then answers with `Veld-Test-Audited`.
    pub rollout: bool,
}

impl<'a> Tree<'a> {
    /// Collects the declarations to emit: everything reachable from the services, and every
    /// declaration of the services' own packages.
    #[must_use]
    pub fn new(
        ir: &'a Ir,
        root: &str,
        dir: &str,
        services: Vec<ServiceId>,
        owns_prelude: bool,
    ) -> Self {
        let mut set: BTreeSet<DeclId> = ir.reachable_decls(&services).into_iter().collect();
        let own: BTreeSet<PackageId> = services.iter().map(|s| ir.service(*s).package).collect();
        for d in &ir.decls {
            let in_scope = own.contains(&d.package) || (owns_prelude && is_prelude(ir, d));
            // An event is never emitted: a client generates nothing for events (`spec/events.md` 14).
            if in_scope && !d.is_event() {
                set.insert(d.id);
            }
        }
        Self {
            ir,
            root: root.to_string(),
            dir: dir.to_string(),
            services,
            decls: set.into_iter().collect(),
            owns_prelude,
            validate: true,
            rules: false,
            rollout: false,
        }
    }

    /// Whether any declaration of the tree has invariants.
    #[must_use]
    pub fn has_rules(&self) -> bool {
        self.decls
            .iter()
            .any(|id| !self.ir.decl(*id).rules().is_empty())
    }

    /// Whether a value of type `ty` can hold a declaration that has invariants.
    #[must_use]
    pub fn type_reaches_rules(&self, ty: &Type) -> bool {
        let mut stack = Vec::new();
        ty.collect_decls(&mut stack);
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let d = self.ir.decl(id);
            if !d.rules().is_empty() {
                return true;
            }
            refs(d, &mut stack);
        }
        false
    }

    /// Sets whether the crate has an audited rule.
    #[must_use]
    pub fn with_rollout(mut self, rollout: bool) -> Self {
        self.rollout = rollout;
        self
    }

    /// Sets whether the crate carries the invariants runtime.
    #[must_use]
    pub fn with_rules(mut self, rules: bool) -> Self {
        self.rules = rules;
        self
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

    /// The packages of the tree's services, in order of first appearance.
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

    /// The module name of a package (a directory below the root).
    #[must_use]
    pub fn pkg_mod(&self, pkg: PackageId) -> String {
        let m = util::module(&self.ir.package(pkg).name);
        if RESERVED_MODULES.contains(&util::bare(&m)) {
            format!("{}_", util::bare(&m))
        } else {
            m
        }
    }

    /// The module name of the contract module of a declaration.
    #[must_use]
    pub fn decl_mod(d: &Decl) -> String {
        let m = util::module(&d.module);
        if m.ends_with("_client") || m.ends_with("_server") {
            format!("{}_", util::bare(&m))
        } else {
            m
        }
    }

    /// The Rust path of the module that holds a declaration.
    #[must_use]
    pub fn module_path(&self, d: &Decl) -> String {
        if is_prelude(self.ir, d) {
            return "crate::prelude".to_string();
        }
        format!(
            "{}::{}::{}",
            self.root,
            self.pkg_mod(d.package),
            Self::decl_mod(d)
        )
    }

    /// The file of the module that holds a declaration.
    #[must_use]
    pub fn module_file(&self, d: &Decl) -> String {
        if is_prelude(self.ir, d) {
            return "src/prelude.rs".to_string();
        }
        format!(
            "{}/{}/{}.rs",
            self.dir,
            self.pkg_mod(d.package),
            Self::decl_mod(d)
        )
    }

    /// The path of a declaration's type as seen from the module `here`.
    #[must_use]
    pub fn type_path(&self, id: DeclId, here: &str) -> String {
        let d = self.ir.decl(id);
        let name = util::type_name(&d.name);
        let module = self.module_path(d);
        if module == here {
            name
        } else {
            format!("{module}::{name}")
        }
    }

    /// The path of a declaration's static schema as seen from the module `here`.
    #[must_use]
    pub fn schema_path(&self, id: DeclId, here: &str) -> String {
        let d = self.ir.decl(id);
        let name = schema_name(&d.name);
        let module = self.module_path(d);
        if module == here {
            name
        } else {
            format!("{module}::{name}")
        }
    }

    /// A prelude declaration by name (`NotFound`, `ValidationError`, ...).
    #[must_use]
    pub fn prelude_decl(&self, name: &str) -> Option<DeclId> {
        self.ir
            .decls
            .iter()
            .find(|d| d.name == name && is_prelude(self.ir, d))
            .map(|d| d.id)
    }

    /// Whether `target` can reach `owner` through declaration references (so a field of type
    /// `target` inside `owner` needs a `Box`).
    #[must_use]
    pub fn reaches(&self, target: DeclId, owner: DeclId) -> bool {
        let mut seen = BTreeSet::new();
        let mut stack = vec![target];
        while let Some(id) = stack.pop() {
            if id == owner {
                return true;
            }
            if !seen.insert(id) {
                continue;
            }
            refs(self.ir.decl(id), &mut stack);
        }
        false
    }
}

/// Whether a declaration belongs to the prelude package.
#[must_use]
pub fn is_prelude(ir: &Ir, d: &Decl) -> bool {
    ir.package(d.package).kind == PackageKind::Prelude
}

fn refs(d: &Decl, out: &mut Vec<DeclId>) {
    match &d.kind {
        DeclKind::Model(m) => {
            for f in &m.fields {
                f.ty.collect_decls(out);
            }
        }
        DeclKind::Error(e) => {
            for f in &e.fields.fields {
                f.ty.collect_decls(out);
            }
        }
        DeclKind::Union(u) => out.extend(u.arms.iter().map(|a| a.decl)),
        DeclKind::Const(_) | DeclKind::Enum(_) => {}
        DeclKind::Event(e) => {
            for f in &e.fields {
                f.ty.collect_decls(out);
            }
        }
    }
}

/// The static that holds a declaration's schema: `ORDER_ITEM_SCHEMA`.
#[must_use]
pub fn schema_name(decl_name: &str) -> String {
    let n = veld_codegen::naming::screaming_snake(decl_name);
    format!("{}_SCHEMA", util::bare(&util::escape(&n)))
}

/// The constant of a `const` declaration.
#[must_use]
pub fn const_name(decl_name: &str) -> String {
    let n = veld_codegen::naming::screaming_snake(decl_name);
    util::bare(&util::escape(&n)).to_string()
}
