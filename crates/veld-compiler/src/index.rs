//! A queryable, owned snapshot of an analysed workspace, for tools such as the language
//! server. Unlike [`crate::compile`] it keeps going when the workspace has errors, so that
//! names still resolve while the user is typing.
//!
//! The resolution table is built with the compiler's own scopes ([`Program::lookup`]); no
//! resolver is duplicated here.

use crate::diagnostics::{Diagnostic, FileId, Severity, SourceMap, Span, has_errors};
use crate::semantic::{self, DKind, ParsedPackage, Program};
use crate::syntax::ast::{self, DeclKind, LitKind, TypeExpr, TypeExprKind, doc_text};
use crate::{CompileInput, lower};
use veld_ir::{Ir, Method, PackageKind};

/// What a use site points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefTarget {
    /// Index into [`Index::decls`].
    Decl(usize),
    /// An enum variant, by declaration and variant name.
    Variant { decl: usize, name: String },
    /// A module: the file that defines it.
    Module(FileId),
}

/// One resolved use site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub span: Span,
    pub target: RefTarget,
}

/// One parsed file with the package and module it belongs to.
#[derive(Debug)]
pub struct IndexedFile {
    pub id: FileId,
    pub package: String,
    pub package_kind: PackageKind,
    /// Path relative to the package root.
    pub rel_path: String,
    /// Dotted module path (empty when the path is not a valid module path).
    pub module: String,
    pub ast: ast::File,
}

/// A resolved field (own fields only), keyed by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldEntry {
    pub name: String,
    /// The resolved type, for example `User[]` or `Page<Order>`.
    pub ty: String,
    pub optional: bool,
}

/// A top-level declaration other than a service.
#[derive(Debug, Clone)]
pub struct DeclEntry {
    pub kind: DKind,
    pub name: String,
    pub qualified: String,
    pub package: String,
    pub package_kind: PackageKind,
    pub module: String,
    pub file: FileId,
    /// Position in `files[file].ast.decls`.
    pub ast_index: usize,
    pub name_span: Span,
    pub docs: Option<String>,
    pub deprecated: Option<String>,
    pub extends: Option<usize>,
    pub fields: Vec<FieldEntry>,
    pub variants: Vec<String>,
    /// Union arms: label and the arm's declaration.
    pub arms: Vec<(String, usize)>,
    pub const_ty: Option<String>,
    pub default_status: Option<u16>,
}

impl DeclEntry {
    #[must_use]
    pub fn is_prelude(&self) -> bool {
        self.package_kind == PackageKind::Prelude
    }
}

/// A resolved endpoint.
#[derive(Debug, Clone)]
pub struct EndpointEntry {
    pub name: String,
    pub method: Method,
    /// Full path: service prefix plus endpoint path.
    pub path: String,
    pub success_status: u16,
    pub success_ty: Option<String>,
    pub success_stream: bool,
    /// (status, error declaration index), implicit errors included.
    pub errors: Vec<(u16, usize)>,
    pub params: Vec<(String, String)>,
}

/// A service and its resolved endpoints.
#[derive(Debug, Clone)]
pub struct ServiceEntry {
    pub name: String,
    pub package: String,
    pub file: FileId,
    /// Position in `files[file].ast.decls`.
    pub ast_index: usize,
    pub prefix: String,
    pub docs: Option<String>,
    pub endpoints: Vec<EndpointEntry>,
}

/// The analysed workspace.
#[derive(Debug)]
pub struct Index {
    pub sources: SourceMap,
    /// Sorted by file and offset, like [`crate::compile`]. Semantic diagnostics appear only
    /// when no file has syntax errors.
    pub diagnostics: Vec<Diagnostic>,
    /// `Some` only when there are no errors.
    pub ir: Option<Ir>,
    /// Indexed by `FileId.0`; the prelude is last.
    pub files: Vec<IndexedFile>,
    pub decls: Vec<DeclEntry>,
    pub services: Vec<ServiceEntry>,
    pub refs: Vec<Reference>,
}

impl Index {
    #[must_use]
    pub fn file(&self, id: FileId) -> Option<&IndexedFile> {
        self.files
            .get(id.0 as usize)
            .filter(|f| f.id == id)
            .or_else(|| self.files.iter().find(|f| f.id == id))
    }

    /// The syntax node of a declaration entry.
    #[must_use]
    pub fn decl_ast(&self, d: &DeclEntry) -> Option<&ast::Decl> {
        self.file(d.file)?.ast.decls.get(d.ast_index)
    }

    /// The declaration with this name visible from `package`, own package first.
    #[must_use]
    pub fn find_decl(&self, package: &str, name: &str) -> Option<usize> {
        let same = |d: &&DeclEntry| d.name == name && d.package == package;
        let pos = self.decls.iter().position(|d| same(&d));
        pos.or_else(|| self.decls.iter().position(|d| d.name == name))
    }

    /// The file of a module.
    #[must_use]
    pub fn module_file(&self, package: &str, module: &str) -> Option<FileId> {
        self.files
            .iter()
            .find(|f| f.package == package && f.module == module)
            .map(|f| f.id)
    }

    /// Number of errors among the diagnostics.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }
}

/// Analyses `input` as [`crate::compile`] does, but also returns the resolution table.
#[must_use]
pub fn index(input: &CompileInput) -> Index {
    index_prefixed(input, &std::collections::BTreeMap::new())
}

/// Like [`index`], with the route prefix per package name that
/// [`crate::compile_prefixed`] takes, so that tools show the same full routes as the generators.
#[must_use]
pub fn index_prefixed(
    input: &CompileInput,
    prefixes: &std::collections::BTreeMap<String, String>,
) -> Index {
    let mut sources = SourceMap::default();
    let mut diags = Vec::new();
    let mut parsed = crate::parse_packages(input, prefixes, &mut sources, &mut diags);
    parsed.push(crate::parse_prelude(&mut sources, &mut diags));
    let syntax_errors = has_errors(&diags);
    let mut semantic_diags = Vec::new();
    let prog = semantic::analyze_lenient(&parsed, &input.consumes, &mut semantic_diags);
    let decls = collect_decls(&prog);
    let services = collect_services(&prog);
    let refs = collect_refs(&prog);
    let semantic_ok = !has_errors(&semantic_diags);
    let ir =
        (!syntax_errors && semantic_ok).then(|| lower::lower(&prog, &sources, &input.consumes));
    drop(prog);
    if !syntax_errors {
        diags.extend(semantic_diags);
    }
    let files = into_files(parsed);
    let out = crate::finish(ir, diags, sources);
    Index {
        sources: out.sources,
        diagnostics: out.diagnostics,
        ir: out.ir,
        files,
        decls,
        services,
        refs,
    }
}

fn into_files(parsed: Vec<ParsedPackage>) -> Vec<IndexedFile> {
    let mut files = Vec::new();
    for pkg in parsed {
        for f in pkg.files {
            files.push(IndexedFile {
                id: f.file,
                package: pkg.name.clone(),
                package_kind: pkg.kind,
                module: semantic::module_path(&f.rel_path).unwrap_or_default(),
                rel_path: f.rel_path,
                ast: f.ast,
            });
        }
    }
    files.sort_by_key(|f| f.id);
    files
}

fn ast_index_of(prog: &Program<'_>, module: usize, name: &str, service: bool) -> usize {
    prog.modules[module]
        .ast
        .decls
        .iter()
        .position(|d| d.name().name == name && matches!(d.kind, DeclKind::Service(_)) == service)
        .unwrap_or(0)
}

fn collect_decls(prog: &Program<'_>) -> Vec<DeclEntry> {
    prog.decls
        .iter()
        .map(|d| {
            let module = &prog.modules[d.module];
            let pkg = &prog.packages[d.pkg];
            DeclEntry {
                kind: d.kind,
                name: d.name.clone(),
                qualified: d.qualified.clone(),
                package: pkg.name.clone(),
                package_kind: pkg.kind,
                module: module.path.clone(),
                file: module.file,
                ast_index: ast_index_of(prog, d.module, &d.name, false),
                name_span: d.name_span,
                docs: doc_text(&d.ast.docs),
                deprecated: d.res.deprecated.clone(),
                extends: d.res.parent,
                fields: d
                    .res
                    .own_fields
                    .iter()
                    .map(|f| FieldEntry {
                        name: f.name.clone(),
                        ty: prog.type_name(&f.ty),
                        optional: f.optional,
                    })
                    .collect(),
                variants: d.res.variants.iter().map(|v| v.name.clone()).collect(),
                arms: d
                    .res
                    .arms
                    .iter()
                    .map(|a| (a.label.clone(), a.decl))
                    .collect(),
                const_ty: d.res.const_ty.as_ref().map(|t| prog.type_name(t)),
                default_status: d.res.default_status,
            }
        })
        .collect()
}

fn collect_services(prog: &Program<'_>) -> Vec<ServiceEntry> {
    prog.services
        .iter()
        .map(|s| ServiceEntry {
            name: s.name.clone(),
            package: prog.packages[s.pkg].name.clone(),
            file: prog.modules[s.module].file,
            ast_index: ast_index_of(prog, s.module, &s.name, true),
            prefix: s.prefix.clone(),
            docs: s.docs.clone(),
            endpoints: s
                .endpoints
                .iter()
                .map(|e| EndpointEntry {
                    name: e.name.clone(),
                    method: e.method,
                    path: e.path.clone(),
                    success_status: e.success_status,
                    success_ty: e.success_ty.as_ref().map(|t| prog.type_name(t)),
                    success_stream: e.success_stream,
                    errors: e.errors.clone(),
                    params: e
                        .params
                        .iter()
                        .map(|p| (p.name.clone(), prog.type_name(&p.ty)))
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

// ---- the resolution table ------------------------------------------------------

struct Walker<'p, 'a> {
    prog: &'p Program<'a>,
    module: usize,
    refs: Vec<Reference>,
}

fn collect_refs(prog: &Program<'_>) -> Vec<Reference> {
    let mut out = Vec::new();
    for module in 0..prog.modules.len() {
        let mut w = Walker {
            prog,
            module,
            refs: Vec::new(),
        };
        w.walk_file();
        out.extend(w.refs);
    }
    out.sort_by_key(|r| (r.span.file, r.span.start, r.span.end));
    out
}

impl Walker<'_, '_> {
    fn push(&mut self, span: Span, target: RefTarget) {
        self.refs.push(Reference { span, target });
    }

    fn walk_file(&mut self) {
        let file = self.prog.modules[self.module].ast;
        for u in &file.uses {
            self.walk_use(u);
        }
        for d in &file.decls {
            self.walk_decl(d);
        }
    }

    fn walk_use(&mut self, u: &ast::Use) {
        let prog = self.prog;
        let pkg = match &u.package {
            None => Some(prog.modules[self.module].pkg),
            Some(id) => prog.packages.iter().position(|p| p.name == id.name),
        };
        let Some(pkg) = pkg else { return };
        let Some(&target) = prog.module_index.get(&(pkg, u.module_path())) else {
            return;
        };
        if let (Some(first), Some(last)) = (u.module.first(), u.module.last()) {
            self.push(
                first.span.to(last.span),
                RefTarget::Module(prog.modules[target].file),
            );
        }
        for imp in &u.imports {
            let hit = prog
                .decls
                .iter()
                .position(|d| d.module == target && d.name == imp.name.name);
            if let Some(d) = hit {
                self.push(imp.name.span, RefTarget::Decl(d));
            }
        }
    }

    fn walk_type(&mut self, t: &TypeExpr) {
        match &t.kind {
            TypeExprKind::List(inner) => self.walk_type(inner),
            TypeExprKind::Named { name, args } => {
                if let Some(d) = self.prog.lookup(self.module, &name.name) {
                    self.push(name.span, RefTarget::Decl(d));
                }
                for a in args {
                    self.walk_type(a);
                }
            }
        }
    }

    /// `.variant` literals whose declared type is an enum.
    fn walk_default(&mut self, ty: Option<&veld_ir::Type>, lit: &ast::Literal) {
        let (Some(veld_ir::Type::Decl(d)), LitKind::Variant(name)) = (ty, &lit.kind) else {
            return;
        };
        let decl = d.index();
        if self
            .prog
            .decls
            .get(decl)
            .is_some_and(|x| x.kind == DKind::Enum)
        {
            self.push(
                lit.span,
                RefTarget::Variant {
                    decl,
                    name: name.clone(),
                },
            );
        }
    }

    fn walk_fields(&mut self, decl: usize, fields: &[ast::Field]) {
        for f in fields {
            self.walk_type(&f.ty);
            if let Some(lit) = &f.default {
                let ty = self.prog.decls[decl]
                    .res
                    .own_fields
                    .iter()
                    .find(|x| x.name == f.name.name)
                    .map(|x| x.ty.clone());
                self.walk_default(ty.as_ref(), lit);
            }
        }
    }

    fn walk_decl(&mut self, d: &ast::Decl) {
        let idx = self
            .prog
            .decls
            .iter()
            .position(|x| x.name_span == d.name().span);
        match (&d.kind, idx) {
            (DeclKind::Model(m), Some(i)) => {
                if let Some(e) = &m.extends {
                    self.walk_type(e);
                }
                self.walk_fields(i, &m.fields);
            }
            (DeclKind::Error(e), Some(i)) => self.walk_fields(i, &e.fields),
            (DeclKind::Union(u), _) => u.arms.iter().for_each(|a| self.walk_type(&a.ty)),
            (DeclKind::Const(c), Some(i)) => {
                self.walk_type(&c.ty);
                let ty = self.prog.decls[i].res.const_ty.clone();
                self.walk_default(ty.as_ref(), &c.value);
            }
            (DeclKind::Service(s), _) => {
                for ep in &s.endpoints {
                    self.walk_endpoint(ep);
                }
            }
            _ => {}
        }
    }

    fn walk_endpoint(&mut self, ep: &ast::Endpoint) {
        for p in &ep.params {
            self.walk_type(&p.ty);
        }
        for item in ep.responses.iter().flat_map(|r| &r.items) {
            if let Some(t) = &item.ty {
                self.walk_type(t);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PackageInput, SourceFile};
    use std::collections::BTreeMap;

    fn input(files: &[(&str, &str, &str)]) -> CompileInput {
        let mut by: BTreeMap<&str, Vec<SourceFile>> = BTreeMap::new();
        for (p, path, text) in files {
            by.entry(p).or_default().push(SourceFile {
                path: (*path).into(),
                text: (*text).into(),
            });
        }
        CompileInput {
            packages: by
                .into_iter()
                .map(|(name, files)| PackageInput {
                    name: name.into(),
                    kind: PackageKind::Service,
                    files,
                    base_url: None,
                })
                .collect(),
            consumes: BTreeMap::new(),
        }
    }

    const USERS: &str = "model User { id: uuid }\nenum Role { admin, guest }\n";
    const APP: &str = "use iam:users.{User, Role}\nmodel Order {\n  owner: User\n  role: Role = .admin\n}\nservice Orders at \"/o\" {\n  get \"/\" list() -> Order[] | NotFound\n}\n";

    #[test]
    fn resolves_imports_types_variants_and_modules() {
        let ix = index(&input(&[
            ("iam", "users.veld", USERS),
            ("app", "app.veld", APP),
        ]));
        assert_eq!(
            ix.error_count(),
            0,
            "{}",
            ix.sources.render_all(&ix.diagnostics)
        );
        assert!(ix.ir.is_some());
        let user = ix.find_decl("iam", "User").unwrap();
        let hits = |t: &RefTarget| ix.refs.iter().filter(|r| &r.target == t).count();
        assert_eq!(hits(&RefTarget::Decl(user)), 2);
        let role = ix.find_decl("iam", "Role").unwrap();
        assert_eq!(
            hits(&RefTarget::Variant {
                decl: role,
                name: "admin".into()
            }),
            1
        );
        let users_file = ix.module_file("iam", "users").unwrap();
        assert_eq!(hits(&RefTarget::Module(users_file)), 1);
        let nf = ix.find_decl("app", "NotFound").unwrap();
        assert!(ix.decls[nf].is_prelude());
        assert_eq!(hits(&RefTarget::Decl(nf)), 1);
    }

    #[test]
    fn keeps_resolving_when_there_are_errors() {
        let broken = "model A {\n  x: Missing\n  y: B\n}\nmodel B { z: int }\n";
        let ix = index(&input(&[("app", "app.veld", broken)]));
        assert!(ix.ir.is_none());
        assert!(ix.error_count() > 0);
        let b = ix.find_decl("app", "B").unwrap();
        assert_eq!(
            ix.refs
                .iter()
                .filter(|r| r.target == RefTarget::Decl(b))
                .count(),
            1
        );
    }

    #[test]
    fn resolves_even_with_syntax_errors_but_reports_only_syntax() {
        let src = "model A { x: B }\nmodel B { y: }\n";
        let ix = index(&input(&[("app", "app.veld", src)]));
        assert!(ix.error_count() > 0);
        assert!(ix.diagnostics.iter().all(|d| d.code.starts_with("E00")));
        let b = ix.find_decl("app", "B").unwrap();
        assert!(ix.refs.iter().any(|r| r.target == RefTarget::Decl(b)));
    }

    #[test]
    fn exposes_endpoints_and_fields() {
        let ix = index(&input(&[
            ("iam", "users.veld", USERS),
            ("app", "app.veld", APP),
        ]));
        let svc = &ix.services[0];
        assert_eq!(svc.name, "Orders");
        let ep = &svc.endpoints[0];
        assert_eq!((ep.method, ep.path.as_str()), (Method::Get, "/o"));
        assert_eq!(ep.success_ty.as_deref(), Some("Order[]"));
        assert!(ep.errors.iter().any(|(s, _)| *s == 404));
        let order = ix.find_decl("app", "Order").unwrap();
        assert_eq!(ix.decls[order].fields[0].ty, "User");
        assert!(ix.decl_ast(&ix.decls[order]).is_some());
    }
}
