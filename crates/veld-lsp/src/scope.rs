//! Names and types inside rule expressions: what a bare name means in an invariant, an endpoint
//! block or a policy (`spec/rules.md` sections 3.2, 5.1, 9.1 and 11.2), and the type of a path.
//!
//! Everything here reads the syntax tree and the declaration table, never the IR, so it answers
//! on a file that does not compile. Type names are resolved with the scope of the file that
//! writes them (`visible_in`), because `ix.refs` does not cover fact types, policy parameters or
//! the principal.

use std::collections::BTreeMap;

use veld_compiler::index::Index;
use veld_compiler::language;
use veld_compiler::semantic::DKind;
use veld_compiler::syntax::ast::{
    self, ArgValue, BinOp, CallArg, ExprKind, TypeExpr, TypeExprKind, UnOp,
};
use veld_compiler::{FileId, Span};

use crate::query::{decl_fields, service_ast};

/// Names visible in a file: its own declarations, its imports (under their local names) and the
/// prelude. The value is an index into `Index::decls`.
#[must_use]
pub fn visible_in(ix: &Index, file: FileId) -> BTreeMap<String, usize> {
    let mut map = BTreeMap::new();
    for (i, d) in ix.decls.iter().enumerate() {
        if d.is_prelude() || d.file == file {
            map.insert(d.name.clone(), i);
        }
    }
    let Some(f) = ix.file(file) else {
        return map;
    };
    for u in &f.ast.uses {
        let pkg = u
            .package
            .as_ref()
            .map_or(f.package.as_str(), |p| p.name.as_str());
        let module = u.module_path();
        for imp in &u.imports {
            let hit = ix
                .decls
                .iter()
                .position(|d| d.package == pkg && d.module == module && d.name == imp.name.name);
            if let Some(h) = hit {
                let local = imp.alias.as_ref().unwrap_or(&imp.name);
                map.insert(local.name.clone(), h);
            }
        }
    }
    map
}

/// The type of a value, as far as the editor can tell.
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Scalar(String),
    /// A declaration: an index into `Index::decls`.
    Decl(usize),
    List(Box<Ty>),
    Page(Box<Ty>),
    Map(Box<Ty>),
    Duration,
    Unknown,
}

/// A type and whether the value may be absent.
#[derive(Debug, Clone, PartialEq)]
pub struct Val {
    pub ty: Ty,
    pub optional: bool,
}

impl Val {
    #[must_use]
    pub fn new(ty: Ty) -> Self {
        Self {
            ty,
            optional: false,
        }
    }

    #[must_use]
    pub fn scalar(name: &str) -> Self {
        Self::new(Ty::Scalar(name.to_owned()))
    }

    #[must_use]
    pub fn unknown() -> Self {
        Self::new(Ty::Unknown)
    }

    /// The type as written, with `?` when it may be absent.
    #[must_use]
    pub fn show(&self, ix: &Index) -> String {
        let q = if self.optional { "?" } else { "" };
        format!("{q}{}", self.ty.show(ix))
    }
}

impl Ty {
    #[must_use]
    pub fn show(&self, ix: &Index) -> String {
        match self {
            Ty::Scalar(s) => s.clone(),
            Ty::Decl(d) => ix.decls.get(*d).map_or("?".into(), |d| d.name.clone()),
            Ty::List(t) => format!("{}[]", t.show(ix)),
            Ty::Page(t) => format!("Page<{}>", t.show(ix)),
            Ty::Map(t) => format!("Map<string, {}>", t.show(ix)),
            Ty::Duration => "duration".into(),
            Ty::Unknown => "?".into(),
        }
    }

    /// The element of a list.
    #[must_use]
    pub fn elem(&self) -> Ty {
        match self {
            Ty::List(t) => (**t).clone(),
            _ => Ty::Unknown,
        }
    }

    /// The declaration of an enum type, if this is one.
    #[must_use]
    pub fn enum_decl(&self, ix: &Index) -> Option<usize> {
        match self {
            Ty::Decl(d) if ix.decls.get(*d).is_some_and(|e| e.kind == DKind::Enum) => Some(*d),
            _ => None,
        }
    }
}

/// Resolves a written type in the scope of `file`.
#[must_use]
pub fn resolve_type(ix: &Index, file: FileId, t: &TypeExpr) -> Ty {
    match &t.kind {
        TypeExprKind::List(inner) => Ty::List(Box::new(resolve_type(ix, file, inner))),
        TypeExprKind::Named { name, args } => {
            let n = name.name.as_str();
            if language::SCALARS.contains(&n) {
                return Ty::Scalar(n.to_owned());
            }
            let arg = |i: usize| {
                args.get(i)
                    .map_or(Ty::Unknown, |a| resolve_type(ix, file, a))
            };
            match n {
                "Page" => Ty::Page(Box::new(arg(0))),
                "Map" => Ty::Map(Box::new(arg(args.len().saturating_sub(1)))),
                _ => visible_in(ix, file)
                    .get(n)
                    .map_or(Ty::Unknown, |&d| Ty::Decl(d)),
            }
        }
    }
}

/// A scalar type from the name a constant was resolved to.
fn scalar_named(name: &str) -> Ty {
    if language::SCALARS.contains(&name) {
        Ty::Scalar(name.to_owned())
    } else {
        Ty::Unknown
    }
}

/// A declaration and its parents, the root first. A cycle in a broken file ends the walk.
#[must_use]
pub fn chain(ix: &Index, decl: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut cur = Some(decl);
    while let Some(d) = cur {
        if out.contains(&d) || out.len() > 16 {
            break;
        }
        out.push(d);
        cur = ix.decls.get(d).and_then(|e| e.extends);
    }
    out.reverse();
    out
}

/// A field of a value: where it is written, or a built-in member of `Page`.
#[derive(Debug, Clone)]
pub struct Member {
    pub name: String,
    pub val: Val,
    /// The declaration that writes the field and its position there; `None` for `Page`.
    pub origin: Option<(usize, usize)>,
    pub span: Option<Span>,
}

/// The fields of a value of type `ty`, inherited ones first.
#[must_use]
pub fn members(ix: &Index, ty: &Ty) -> Vec<Member> {
    match ty {
        Ty::Decl(d) => decl_members(ix, *d),
        Ty::Page(inner) => {
            let item = |name: &str, ty: Ty| Member {
                name: name.to_owned(),
                val: Val::new(ty),
                origin: None,
                span: None,
            };
            vec![
                item("items", Ty::List(inner.clone())),
                item("total", Ty::Scalar("long".into())),
                item("page", Ty::Scalar("int".into())),
                item("size", Ty::Scalar("int".into())),
            ]
        }
        _ => Vec::new(),
    }
}

fn decl_members(ix: &Index, d: usize) -> Vec<Member> {
    let record = ix
        .decls
        .get(d)
        .is_some_and(|e| matches!(e.kind, DKind::Model | DKind::Error | DKind::Event));
    if !record {
        return Vec::new();
    }
    let mut out: Vec<Member> = Vec::new();
    for owner in chain(ix, d) {
        let Some(od) = ix.decls.get(owner) else {
            continue;
        };
        for (i, f) in decl_fields(ix, od).iter().enumerate() {
            out.retain(|m| m.name != f.name.name);
            out.push(Member {
                name: f.name.name.clone(),
                val: Val {
                    ty: resolve_type(ix, od.file, &f.ty),
                    optional: f.optional,
                },
                origin: Some((owner, i)),
                span: Some(f.name.span),
            });
        }
    }
    out
}

/// What a name in scope is.
#[derive(Debug, Clone)]
pub enum LocalKind {
    Field {
        owner: usize,
        index: usize,
    },
    Param {
        service: usize,
        endpoint: usize,
        index: usize,
    },
    Fact {
        service: usize,
        endpoint: usize,
        stmt: usize,
    },
    Principal {
        file: FileId,
    },
    PolicyParam {
        file: FileId,
        policy: usize,
        index: usize,
    },
    Now,
    Result,
}

/// A name in scope.
#[derive(Debug, Clone)]
pub struct Local {
    pub name: String,
    pub kind: LocalKind,
    pub val: Val,
    /// Where the name is declared, when it is in the source.
    pub def: Option<Span>,
}

/// A lambda variable: `x` in `all(list, x => ..)`.
#[derive(Debug, Clone)]
pub struct Lambda {
    pub name: String,
    pub val: Val,
    pub span: Span,
}

/// The names a rule expression may read.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub file: Option<FileId>,
    pub locals: Vec<Local>,
}

impl Scope {
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Local> {
        self.locals.iter().find(|l| l.name == name)
    }
}

fn local_now() -> Local {
    Local {
        name: "now".into(),
        kind: LocalKind::Now,
        val: Val::scalar("datetime"),
        def: None,
    }
}

/// The scope of an invariant of declaration `decl`: its fields, inherited ones included. `now` is
/// not in it (`spec/rules.md` section 5.1).
#[must_use]
pub fn rule_scope(ix: &Index, decl: usize) -> Scope {
    let locals = decl_members(ix, decl)
        .into_iter()
        .filter_map(|m| {
            let (owner, index) = m.origin?;
            Some(Local {
                name: m.name,
                kind: LocalKind::Field { owner, index },
                val: m.val,
                def: m.span,
            })
        })
        .collect();
    Scope {
        file: ix.decls.get(decl).map(|d| d.file),
        locals,
    }
}

/// The scope of a policy: its parameters and `now`.
#[must_use]
pub fn policy_scope(ix: &Index, file: FileId, policy: usize) -> Scope {
    let mut scope = Scope {
        file: Some(file),
        locals: Vec::new(),
    };
    let Some(p) = ix.file(file).and_then(|f| f.ast.policies.get(policy)) else {
        return scope;
    };
    for (index, param) in p.params.iter().enumerate() {
        scope.locals.push(Local {
            name: param.name.name.clone(),
            kind: LocalKind::PolicyParam {
                file,
                policy,
                index,
            },
            val: Val::new(resolve_type(ix, file, &param.ty)),
            def: Some(param.name.span),
        });
    }
    scope.locals.push(local_now());
    scope
}

/// Where in an endpoint block an expression stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// A fact key: it sees the facts that end before this offset.
    FactKey(u32),
    Require,
    /// An `ensure`: `result` and `old` are in scope.
    Ensure,
}

/// The package's principal: its file, the declaration and the name it has in blocks.
#[must_use]
pub fn principal_of(ix: &Index, file: FileId) -> Option<(FileId, &ast::PrincipalDecl)> {
    let pkg = &ix.file(file)?.package;
    ix.files
        .iter()
        .filter(|f| f.package == *pkg)
        .find_map(|f| f.ast.principals.first().map(|p| (f.id, p)))
}

/// True when the endpoint's effective auth is `none`, where the principal is not in scope.
fn is_public(ix: &Index, service: usize, ep: &ast::Endpoint) -> bool {
    let own = ep.annotations.iter().find(|a| a.name.name == "auth");
    let svc = ix.services.get(service).and_then(|s| {
        ix.file(s.file)
            .and_then(|f| f.ast.decls.get(s.ast_index))
            .and_then(|d| d.annotations.iter().find(|a| a.name.name == "auth"))
    });
    let Some(auth) = own.or(svc) else {
        return false;
    };
    auth.args
        .iter()
        .flatten()
        .any(|arg| matches!(&arg.value, ArgValue::Ident(i) if i.name == "none"))
}

/// The type of the success value of an endpoint, when it has one.
fn success_type(ix: &Index, file: FileId, ep: &ast::Endpoint) -> Option<Ty> {
    let items = &ep.responses.as_ref()?.items;
    items.iter().filter(|i| !i.stream).find_map(|i| {
        let ty = resolve_type(ix, file, i.ty.as_ref()?);
        let is_error =
            matches!(&ty, Ty::Decl(d) if ix.decls.get(*d).is_some_and(|e| e.kind == DKind::Error));
        (!is_error).then_some(ty)
    })
}

/// The scope of an endpoint block. `service` is an index into `Index::services` and `endpoint` a
/// position in the syntax tree of the service.
#[must_use]
pub fn endpoint_scope(ix: &Index, service: usize, endpoint: usize, phase: Phase) -> Scope {
    let mut scope = Scope::default();
    let (Some(entry), Some(sd)) = (ix.services.get(service), service_ast(ix, service)) else {
        return scope;
    };
    let Some(ep) = sd.endpoints.get(endpoint) else {
        return scope;
    };
    let file = entry.file;
    scope.file = Some(file);
    for (index, p) in ep.params.iter().enumerate().filter(|(_, p)| !p.stream) {
        scope.locals.push(Local {
            name: p.name.name.clone(),
            kind: LocalKind::Param {
                service,
                endpoint,
                index,
            },
            val: Val {
                ty: resolve_type(ix, file, &p.ty),
                optional: p.optional,
            },
            def: Some(p.name.span),
        });
    }
    add_facts(ix, &mut scope, (service, endpoint), ep, phase);
    if !is_public(ix, service, ep)
        && let Some((pf, p)) = principal_of(ix, file)
    {
        scope.locals.push(Local {
            name: p
                .name
                .as_ref()
                .map_or("user", |n| n.name.as_str())
                .to_owned(),
            kind: LocalKind::Principal { file: pf },
            val: Val::new(resolve_type(ix, pf, &p.ty)),
            def: Some(p.ty.span),
        });
    }
    scope.locals.push(local_now());
    if phase == Phase::Ensure
        && let Some(ty) = success_type(ix, file, ep)
    {
        scope.locals.push(Local {
            name: "result".into(),
            kind: LocalKind::Result,
            val: Val::new(ty),
            def: None,
        });
    }
    scope
}

fn add_facts(ix: &Index, scope: &mut Scope, at: (usize, usize), ep: &ast::Endpoint, phase: Phase) {
    let Some(file) = scope.file else { return };
    let Some(block) = &ep.block else { return };
    for (stmt, s) in block.stmts.iter().enumerate() {
        let ast::Stmt::Fact(f) = s else { continue };
        if matches!(phase, Phase::FactKey(off) if f.span.end > off) {
            continue;
        }
        scope.locals.push(Local {
            name: f.name.name.clone(),
            kind: LocalKind::Fact {
                service: at.0,
                endpoint: at.1,
                stmt,
            },
            val: Val {
                ty: resolve_type(ix, file, &f.ty),
                optional: f.optional,
            },
            def: Some(f.name.span),
        });
    }
}

/// The value a name has: a lambda variable, a name of the scope, or a constant.
#[must_use]
pub fn lookup(ix: &Index, scope: &Scope, lams: &[Lambda], name: &str) -> Option<Val> {
    if let Some(l) = lams.iter().rev().find(|l| l.name == name) {
        return Some(l.val.clone());
    }
    if let Some(l) = scope.get(name) {
        return Some(l.val.clone());
    }
    let file = scope.file?;
    let d = *visible_in(ix, file).get(name)?;
    let e = ix.decls.get(d)?;
    (e.kind == DKind::Const)
        .then(|| Val::new(e.const_ty.as_deref().map_or(Ty::Unknown, scalar_named)))
}

/// The member `name` of a value of type `ty`.
#[must_use]
pub fn member(ix: &Index, ty: &Ty, name: &str) -> Option<Member> {
    members(ix, ty).into_iter().find(|m| m.name == name)
}

/// The type of an expression. `hint` is the type the context expects, which gives an enum
/// literal its enum.
#[must_use]
pub fn type_of(
    ix: &Index,
    scope: &Scope,
    lams: &[Lambda],
    e: &ast::Expr,
    hint: Option<&Ty>,
) -> Val {
    match &e.kind {
        ExprKind::Bool(_) => Val::scalar("bool"),
        ExprKind::Int(_) => Val::scalar("int"),
        ExprKind::Float { .. } => Val::scalar("decimal"),
        ExprKind::Str(_) => Val::scalar("string"),
        ExprKind::EnumLit(_) => hint.map_or_else(Val::unknown, |t| Val::new(t.clone())),
        ExprKind::Name(id) => lookup(ix, scope, lams, &id.name).unwrap_or_else(Val::unknown),
        ExprKind::Field { base, name } => {
            let b = type_of(ix, scope, lams, base, None);
            member(ix, &b.ty, &name.name).map_or_else(Val::unknown, |m| m.val)
        }
        ExprKind::Call { name, args } => call_type(ix, scope, lams, &name.name, args),
        ExprKind::List(items) => {
            let first = items
                .first()
                .map_or(Ty::Unknown, |i| type_of(ix, scope, lams, i, hint).ty);
            Val::new(Ty::List(Box::new(first)))
        }
        ExprKind::Unary { op, operand } => match op {
            UnOp::Not { .. } => Val::scalar("bool"),
            UnOp::Neg => type_of(ix, scope, lams, operand, None),
        },
        ExprKind::Binary { op, l, r } => binary_type(ix, scope, lams, *op, l, r),
    }
}

fn binary_type(
    ix: &Index,
    scope: &Scope,
    lams: &[Lambda],
    op: BinOp,
    l: &ast::Expr,
    r: &ast::Expr,
) -> Val {
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
            let lt = type_of(ix, scope, lams, l, None);
            let rt = type_of(ix, scope, lams, r, None);
            let moment = |t: &Ty| matches!(t, Ty::Scalar(s) if s == "datetime" || s == "date");
            if op == BinOp::Sub && moment(&lt.ty) && moment(&rt.ty) {
                Val::new(Ty::Duration)
            } else if lt.ty == Ty::Unknown {
                rt
            } else {
                lt
            }
        }
        _ => Val::scalar("bool"),
    }
}

fn call_type(ix: &Index, scope: &Scope, lams: &[Lambda], name: &str, args: &[CallArg]) -> Val {
    match name {
        "has" | "matches" | "startsWith" | "endsWith" | "contains" | "all" | "any" => {
            Val::scalar("bool")
        }
        "len" | "count" => Val::scalar("int"),
        "days" | "hours" | "minutes" | "seconds" | "ms" => Val::new(Ty::Duration),
        "old" => match args.first() {
            Some(CallArg::Expr(e)) => type_of(ix, scope, lams, e, None),
            _ => Val::unknown(),
        },
        _ => Val::unknown(),
    }
}

/// The type of the elements the first argument of a comprehension holds.
#[must_use]
pub fn element_type(ix: &Index, scope: &Scope, lams: &[Lambda], args: &[CallArg]) -> Val {
    match args.first() {
        Some(CallArg::Expr(e)) => Val::new(type_of(ix, scope, lams, e, None).ty.elem()),
        _ => Val::unknown(),
    }
}

/// The enum that has a variant called `name`, when exactly one visible enum has one. Used for a
/// literal whose context gives no enum.
#[must_use]
pub fn only_enum_with(ix: &Index, file: FileId, name: &str) -> Option<usize> {
    let mut found = visible_in(ix, file)
        .into_values()
        .filter(|&d| {
            ix.decls
                .get(d)
                .is_some_and(|e| e.kind == DKind::Enum && e.variants.iter().any(|v| v == name))
        })
        .collect::<Vec<_>>();
    found.sort_unstable();
    found.dedup();
    (found.len() == 1).then(|| found[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::build;

    const SRC: &str = "model A { id: uuid\n tags: string[]\n note?: string }\nmodel B extends A { n: int\n a: A }\nenum E { x, y }\nconst LIMIT: int = 3\n";

    #[test]
    fn members_include_inherited_fields_parent_first() {
        let ix = build(&[("app", "app.veld", SRC)]);
        let b = ix.find_decl("app", "B").unwrap();
        let names: Vec<_> = members(&ix, &Ty::Decl(b))
            .into_iter()
            .map(|m| m.name)
            .collect();
        assert_eq!(names, ["id", "tags", "note", "n", "a"]);
        let m = member(&ix, &Ty::Decl(b), "note").unwrap();
        assert!(m.val.optional);
        assert_eq!(m.val.show(&ix), "?string");
        let a = member(&ix, &Ty::Decl(b), "a").unwrap();
        assert_eq!(a.val.ty.show(&ix), "A");
    }

    #[test]
    fn a_cycle_in_extends_does_not_hang() {
        let ix = build(&[(
            "app",
            "app.veld",
            "model A extends B { x: int }\nmodel B extends A { y: int }\n",
        )]);
        let a = ix.find_decl("app", "A").unwrap();
        assert!(!members(&ix, &Ty::Decl(a)).is_empty());
    }

    #[test]
    fn page_has_items_total_page_and_size() {
        let ix = build(&[("app", "app.veld", SRC)]);
        let a = ix.find_decl("app", "A").unwrap();
        let names: Vec<_> = members(&ix, &Ty::Page(Box::new(Ty::Decl(a))))
            .into_iter()
            .map(|m| m.name)
            .collect();
        assert_eq!(names, ["items", "total", "page", "size"]);
    }

    #[test]
    fn a_rule_scope_has_the_fields_and_no_clock() {
        let ix = build(&[("app", "app.veld", SRC)]);
        let b = ix.find_decl("app", "B").unwrap();
        let s = rule_scope(&ix, b);
        assert!(s.get("id").is_some() && s.get("n").is_some());
        assert!(s.get("now").is_none());
        let c = lookup(&ix, &s, &[], "LIMIT").unwrap();
        assert_eq!(c.ty, Ty::Scalar("int".into()));
    }
}
