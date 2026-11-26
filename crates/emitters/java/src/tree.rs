//! Where generated Java files live, and the per-file import bookkeeping.

use std::collections::{BTreeMap, BTreeSet};

use veld_ir::{Decl, DeclId, DeclKind, Ir, PackageId, PackageKind, ServiceId};

use crate::util::{self, HEADER};

/// Package names directly below a tree's base that the generator itself uses.
const RESERVED_DIRS: &[&str] = &["adapters", "clients", "prelude", "runtime", "runtimeconfig"];

/// A set of generated packages rooted at one Java package.
///
/// The client target and the server's own code use one tree each at the base package. A server
/// adds one tree per consumed service under `<base>.clients.<service>`; every tree imports the
/// single runtime of the output.
#[derive(Debug, Clone)]
pub struct Tree<'a> {
    pub ir: &'a Ir,
    /// Java package the tree is rooted at.
    pub base: String,
    /// Java package of the runtime.
    pub runtime: String,
    pub services: Vec<ServiceId>,
    /// False when the target option `validate = false` is set (the types stay).
    pub validate: bool,
    /// True when the contract has invariants: the output then carries the rule classes and the
    /// rule members of the hooks, the options and the schemas (`spec/targets/java.md` section 8).
    pub rules: bool,
    /// True when the target option `testing` is set and the services have endpoint rules, a key, a
    /// guard or an `ETag`: the server then also gets a test handler per such service.
    pub testing: bool,
    /// True when the contract uses policies or reads the principal: a service with a principal
    /// then gets typed auth hooks, and `allow`, the policy methods and `can<Endpoint>` are
    /// generated (`spec/targets/java.md` section 11).
    pub policies: bool,
    /// True when the contract has an audited rule (`@rollout(audit)`, `spec/rules.md` section 17.1): the
    /// output then carries `Audit` and the `rollout` regions of the runtime, and the test handler
    /// answers with `Veld-Test-Audited`.
    pub rollout: bool,
    /// True when a service of the tree publishes or subscribes: the tree then also emits the events
    /// of its channels and the declarations only they reach (`spec/events.md` section 14).
    pub events: bool,
    /// The typed runtime config of the target, when it has one: a server entry then takes it as
    /// its last argument, and a server with `rules.output` takes the mode from it.
    pub runtime_config: Option<crate::config::Info>,
    /// What a server of the tree uses of platform phase P1 (visibility, tenancy, the audit log,
    /// rate limits). Nothing for a client and for a consumed service.
    pub platform: crate::platform::Uses,
    decls: Vec<DeclId>,
}

impl<'a> Tree<'a> {
    /// Collects the declarations to emit: everything reachable from the services, the prelude,
    /// and every declaration of the services' own packages.
    #[must_use]
    pub fn new(ir: &'a Ir, base: &str, runtime: &str, services: Vec<ServiceId>) -> Self {
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
        Self {
            ir,
            base: base.to_string(),
            runtime: runtime.to_string(),
            services,
            validate: true,
            rules: false,
            testing: false,
            policies: false,
            rollout: false,
            events: false,
            runtime_config: None,
            platform: crate::platform::Uses::default(),
            decls: set.into_iter().collect(),
        }
    }

    /// Also emits the events of the tree's services and the declarations only they reach. The
    /// server target calls it for its own tree; a client and a consumed service never do, because
    /// a client generates nothing for events (`spec/events.md` section 14).
    #[must_use]
    pub fn with_events(mut self) -> Self {
        let ir = self.ir;
        let mut set: BTreeSet<DeclId> = self.decls.iter().copied().collect();
        set.extend(ir.events_of(&self.services));
        set.extend(ir.event_reachable_decls(&self.services));
        self.events = self.services.iter().any(|s| ir.service(*s).uses_events());
        self.decls = set.into_iter().collect();
        self
    }

    /// Sets whether the generated code validates requests at run time by default.
    #[must_use]
    pub fn with_validate(mut self, validate: bool) -> Self {
        self.validate = validate;
        self
    }

    /// Sets whether the server also gets a test handler per service that needs one.
    #[must_use]
    pub fn with_testing(mut self, testing: bool) -> Self {
        self.testing = testing;
        self
    }

    /// Sets whether the contract uses policies or reads the principal.
    #[must_use]
    pub fn with_policies(mut self, policies: bool) -> Self {
        self.policies = policies;
        self
    }

    /// Sets whether the contract has an audited rule.
    #[must_use]
    pub fn with_rollout(mut self, rollout: bool) -> Self {
        self.rollout = rollout;
        self
    }

    /// Sets the runtime config of the target.
    #[must_use]
    pub fn with_runtime(mut self, info: Option<crate::config::Info>) -> Self {
        self.runtime_config = info;
        self
    }

    /// Sets what the tree's server uses of platform phase P1.
    #[must_use]
    pub fn with_platform(mut self, platform: crate::platform::Uses) -> Self {
        self.platform = platform;
        self
    }

    /// Sets whether the output carries the rule classes and members.
    #[must_use]
    pub fn with_rules(mut self, rules: bool) -> Self {
        self.rules = rules;
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

    /// Java package holding everything of one contract package: `<base>.<package>`.
    #[must_use]
    pub fn pkg_package(&self, pkg: PackageId) -> String {
        let name = self
            .ir
            .package(pkg)
            .name
            .trim_start_matches('@')
            .replace('/', "_");
        let mut dir = util::segment(&name);
        if RESERVED_DIRS.contains(&dir.as_str()) {
            dir.push('_');
        }
        format!("{}.{dir}", self.base)
    }

    /// Java package of the types of a declaration.
    #[must_use]
    pub fn decl_package(&self, d: &Decl) -> String {
        if self.ir.package(d.package).kind == PackageKind::Prelude {
            return format!("{}.prelude", self.base);
        }
        self.module_package(d.package, &d.module)
    }

    /// Java package of the declarations of one module of a contract package.
    #[must_use]
    pub fn module_package(&self, package: PackageId, module: &str) -> String {
        let mut pkg = self.pkg_package(package);
        for (i, seg) in module.split('.').filter(|s| !s.is_empty()).enumerate() {
            let mut s = util::segment(seg);
            if i == 0 && (s == "client" || s == "server") {
                s.push('_');
            }
            pkg.push('.');
            pkg.push_str(&s);
        }
        pkg
    }

    /// Java package of a package's client or server classes (`kind` is `client` or `server`).
    #[must_use]
    pub fn api_package(&self, pkg: PackageId, kind: &str) -> String {
        format!("{}.{kind}", self.pkg_package(pkg))
    }

    /// A prelude declaration by name (`NotFound`, `ValidationError`, ...).
    #[must_use]
    pub fn prelude_decl(&self, name: &str) -> Option<DeclId> {
        self.ir
            .decls
            .iter()
            .find(|d| d.name == name && self.ir.package(d.package).kind == PackageKind::Prelude)
            .map(|d| d.id)
    }

    /// The unions of which `model` is an arm.
    #[must_use]
    pub fn unions_of(&self, model: DeclId) -> Vec<DeclId> {
        self.decls
            .iter()
            .copied()
            .filter(|id| matches!(&self.ir.decl(*id).kind, DeclKind::Union(u) if u.arms.iter().any(|a| a.decl == model)))
            .collect()
    }
}

/// `a.b.C` to `a/b/C.java`.
#[must_use]
pub fn java_path(package: &str, type_name: &str) -> String {
    format!("{}/{type_name}.java", package.replace('.', "/"))
}

/// A file being generated: tracks the imports its body needs.
#[derive(Debug)]
pub struct FileCtx<'t, 'a> {
    pub tree: &'t Tree<'a>,
    imports: Imports,
}

/// The imports of one file: simple name to fully qualified name for every type the file refers to
/// by its simple name. A name another type already claimed stays fully qualified.
#[derive(Debug, Clone)]
pub struct Imports {
    package: String,
    names: BTreeMap<String, String>,
}

impl Imports {
    /// The imports of the file declaring `own` in `package`.
    #[must_use]
    pub fn new(package: &str, own: &str) -> Self {
        let mut names = BTreeMap::new();
        names.insert(own.to_string(), format!("{package}.{own}"));
        Self {
            package: package.to_string(),
            names,
        }
    }

    /// The name to write for the type `fqn`: its simple name, importing it when needed, or the
    /// fully qualified name when another type already claimed the simple name.
    pub fn name(&mut self, fqn: &str) -> String {
        let simple = fqn.rsplit('.').next().unwrap_or(fqn).to_string();
        match self.names.get(&simple) {
            Some(existing) if existing == fqn => simple,
            Some(_) => fqn.to_string(),
            None => {
                self.names.insert(simple.clone(), fqn.to_string());
                simple
            }
        }
    }

    /// Complete file text: header, package, sorted imports, then `body`.
    #[must_use]
    pub fn finish(&self, body: &str) -> String {
        let mut imports: Vec<&str> = self
            .names
            .values()
            .map(String::as_str)
            .filter(|fqn| {
                let pkg = fqn.rsplit_once('.').map_or("", |(p, _)| p);
                pkg != self.package && pkg != "java.lang"
            })
            .collect();
        imports.sort_unstable();
        let mut out = format!("{HEADER}\npackage {};\n", self.package);
        if !imports.is_empty() {
            out.push('\n');
            for i in imports {
                out.push_str(&format!("import {i};\n"));
            }
        }
        out.push('\n');
        out.push_str(body);
        out
    }
}

impl<'t, 'a> FileCtx<'t, 'a> {
    /// A context for the file declaring `own` in `package`.
    #[must_use]
    pub fn new(tree: &'t Tree<'a>, package: &str, own: &str) -> Self {
        Self {
            tree,
            imports: Imports::new(package, own),
        }
    }

    /// The name to write for the type `fqn`: its simple name, importing it when needed, or the
    /// fully qualified name when another type already claimed the simple name.
    pub fn name(&mut self, fqn: &str) -> String {
        self.imports.name(fqn)
    }

    /// A runtime type by simple name.
    pub fn rt(&mut self, simple: &str) -> String {
        let fqn = format!("{}.{simple}", self.tree.runtime);
        self.name(&fqn)
    }

    /// A declaration's type name.
    pub fn decl(&mut self, id: DeclId) -> String {
        let d = self.tree.ir.decl(id);
        let fqn = format!("{}.{}", self.tree.decl_package(d), util::type_name(d));
        self.name(&fqn)
    }

    /// Complete file text: header, package, imports, then `body`.
    #[must_use]
    pub fn finish(&self, body: &str) -> String {
        self.imports.finish(body)
    }
}
