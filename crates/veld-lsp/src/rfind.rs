//! What every word of the rules language denotes: one pass over a file that lists each name
//! with its span, what it is and where it is defined. Hover, go to definition and semantic
//! tokens all read this list, so they never disagree.
//!
//! The pass reads the syntax tree and the declaration table only. A statement that did not parse
//! is not in the tree and so is not listed; the rest of the file still is.

use veld_compiler::index::{Index, IndexedFile};
use veld_compiler::syntax::ast::{self, CallArg, DeclKind, ExprKind, ValueKind};
use veld_compiler::{FileId, Span};

use crate::scope::{
    self, Lambda, Local, LocalKind, Phase, Scope, Ty, Val, endpoint_scope, only_enum_with,
    policy_scope, resolve_type, rule_scope, type_of, visible_in,
};
use crate::words::{self, Word};
use crate::workspace::Analysis;

/// What a word of the rules language denotes.
#[derive(Debug, Clone)]
pub enum What {
    /// An invariant: its name or its `rule` word.
    Rule {
        decl: usize,
        rule: usize,
    },
    /// A `require`: its word or its name. `stmt` is the position in the block.
    Require {
        service: usize,
        endpoint: usize,
        stmt: usize,
    },
    Ensure {
        service: usize,
        endpoint: usize,
        stmt: usize,
    },
    /// A name in scope: a field, a parameter, a fact, the caller, `now`, `result`.
    Local(Local),
    /// A field written in a declaration: `owner` is the declaration, `index` the field.
    Member {
        owner: usize,
        index: usize,
    },
    /// `items`, `total`, `page` or `size` of a `Page`.
    PageMember(String),
    Variant {
        decl: usize,
        variant: usize,
    },
    Const(usize),
    Builtin(&'static Word),
    Keyword(&'static Word),
    Policy {
        file: FileId,
        index: usize,
    },
    /// The name a principal has in blocks.
    Principal {
        file: FileId,
    },
    /// A state of a state machine, in a `flow`, a `final` or a `transition`.
    State {
        decl: usize,
        variant: usize,
    },
    Example {
        file: FileId,
        index: usize,
    },
    Endpoint {
        service: usize,
        endpoint: usize,
    },
    Param {
        service: usize,
        endpoint: usize,
        index: usize,
    },
    /// A declaration named by a type or an error name.
    Decl(usize),
    Lambda(Lambda),
}

/// One word, what it denotes and where that is defined.
#[derive(Debug, Clone)]
pub struct Found {
    pub what: What,
    pub span: Span,
    pub def: Option<Span>,
}

/// The policy a name stands for: in this file, else in this package, else anywhere.
#[must_use]
pub fn policy_named(ix: &Index, file: FileId, name: &str) -> Option<(FileId, usize)> {
    let find_in = |f: &IndexedFile| {
        f.ast
            .policies
            .iter()
            .position(|p| p.name.name == name)
            .map(|i| (f.id, i))
    };
    let pkg = ix.file(file).map(|f| f.package.clone())?;
    ix.file(file)
        .and_then(find_in)
        .or_else(|| {
            ix.files
                .iter()
                .filter(|f| f.package == pkg)
                .find_map(find_in)
        })
        .or_else(|| ix.files.iter().find_map(find_in))
}

/// The endpoint an example calls: `(service, endpoint)`, both positions in the index.
#[must_use]
pub fn endpoint_called(
    ix: &Index,
    file: FileId,
    service: Option<&str>,
    endpoint: &str,
) -> Option<(usize, usize)> {
    let pkg = &ix.file(file)?.package;
    ix.services.iter().enumerate().find_map(|(si, s)| {
        if s.package != *pkg || service.is_some_and(|n| n != s.name) {
            return None;
        }
        let sd = crate::query::service_ast(ix, si)?;
        let ei = sd.endpoints.iter().position(|e| e.name.name == endpoint)?;
        Some((si, ei))
    })
}

/// The smallest word that covers `offset`.
#[must_use]
pub fn find(a: &Analysis, file: FileId, offset: u32) -> Option<Found> {
    collect(a, file, Some(offset))
        .into_iter()
        .filter(|f| f.span.start <= offset && offset <= f.span.end)
        .min_by_key(|f| f.span.end - f.span.start)
}

/// Every word of a file. With `only`, the items that do not cover that offset are skipped.
#[must_use]
pub fn collect(a: &Analysis, file: FileId, only: Option<u32>) -> Vec<Found> {
    let ix = &a.index;
    let Some(f) = ix.file(file) else {
        return Vec::new();
    };
    let mut c = Collector {
        ix,
        file,
        only,
        vis: visible_in(ix, file),
        out: Vec::new(),
    };
    c.run(f);
    c.out
}

struct Collector<'a> {
    ix: &'a Index,
    file: FileId,
    only: Option<u32>,
    vis: std::collections::BTreeMap<String, usize>,
    out: Vec<Found>,
}

impl Collector<'_> {
    fn live(&self, s: Span) -> bool {
        self.only.is_none_or(|o| s.start <= o && o <= s.end)
    }

    fn push(&mut self, what: What, span: Span, def: Option<Span>) {
        self.out.push(Found { what, span, def });
    }

    /// The word that starts at byte `start`, as a keyword.
    fn kw(&mut self, start: u32, word: &str) {
        if let Some(w) = words::word(word) {
            let end = start + u32::try_from(word.len()).unwrap_or(0);
            self.push(
                What::Keyword(w),
                Span::new(self.file, start as usize, end as usize),
                None,
            );
        }
    }

    fn run(&mut self, f: &IndexedFile) {
        for (ai, d) in f.ast.decls.iter().enumerate() {
            if !self.live(d.span) {
                continue;
            }
            let entry =
                self.ix.decls.iter().position(|e| {
                    e.file == self.file && e.ast_index == ai && e.name == d.name().name
                });
            match (&d.kind, entry) {
                (DeclKind::Model(m), Some(e)) => self.rules(e, &m.rules),
                (DeclKind::Error(m), Some(e)) => self.rules(e, &m.rules),
                (DeclKind::Event(m), Some(e)) => self.rules(e, &m.rules),
                (DeclKind::Enum(en), Some(e)) => self.machine(e, en),
                (DeclKind::Service(s), _) => self.service(ai, s),
                _ => {}
            }
        }
        for (pi, p) in f.ast.policies.iter().enumerate() {
            if self.live(p.span) {
                self.policy(pi, p);
            }
        }
        for p in &f.ast.principals {
            if self.live(p.span) {
                self.principal(p);
            }
        }
        for (xi, x) in f.ast.examples.iter().enumerate() {
            if self.live(x.span) {
                self.example(xi, x);
            }
        }
    }

    // ---- types --------------------------------------------------------------------------

    fn type_expr(&mut self, t: &ast::TypeExpr) {
        match &t.kind {
            ast::TypeExprKind::List(inner) => self.type_expr(inner),
            ast::TypeExprKind::Named { name, args } => {
                if let Some(&d) = self.vis.get(&name.name) {
                    let def = self.ix.decls.get(d).map(|e| e.name_span);
                    self.push(What::Decl(d), name.span, def);
                }
                for a in args {
                    self.type_expr(a);
                }
            }
        }
    }

    // ---- invariants ---------------------------------------------------------------------

    fn rules(&mut self, decl: usize, rules: &[ast::Rule]) {
        let scope = rule_scope(self.ix, decl);
        for (ri, r) in rules.iter().enumerate() {
            if !self.live(r.span) {
                continue;
            }
            let what = What::Rule { decl, rule: ri };
            let kw_end = r.span.start as usize + "rule".len();
            self.push(
                what.clone(),
                Span::new(self.file, r.span.start as usize, kw_end),
                None,
            );
            self.push(what, r.name.span, Some(r.name.span));
            self.expr(&scope, &r.expr, None);
        }
    }

    // ---- state machines -----------------------------------------------------------------

    fn machine(&mut self, decl: usize, e: &ast::EnumDecl) {
        for fl in &e.flows {
            self.kw(fl.span.start, "flow");
            self.states(decl, e, &fl.states);
        }
        for fi in &e.finals {
            self.kw(fi.span.start, "final");
            self.states(decl, e, &fi.states);
        }
    }

    fn states(&mut self, decl: usize, e: &ast::EnumDecl, names: &[ast::Ident]) {
        for n in names {
            if let Some(i) = e.variants.iter().position(|v| v.name.name == n.name) {
                let def = e.variants.get(i).map(|v| v.name.span);
                self.push(What::State { decl, variant: i }, n.span, def);
            }
        }
    }

    // ---- endpoint blocks ----------------------------------------------------------------

    fn service(&mut self, ast_index: usize, sd: &ast::ServiceDecl) {
        let Some(si) = self
            .ix
            .services
            .iter()
            .position(|s| s.file == self.file && s.ast_index == ast_index)
        else {
            return;
        };
        for (ei, ep) in sd.endpoints.iter().enumerate() {
            if let Some(block) = ep.block.as_ref().filter(|b| self.live(b.span)) {
                self.block(si, ei, block);
            }
        }
    }

    fn block(&mut self, si: usize, ei: usize, block: &ast::EndpointBlock) {
        for (st, s) in block.stmts.iter().enumerate() {
            let at = (si, ei, st);
            match s {
                ast::Stmt::Require(r) if self.live(r.span) => self.require(at, r),
                ast::Stmt::Ensure(e) if self.live(e.span) => self.ensure(at, e),
                ast::Stmt::Fact(f) if self.live(f.span) => self.fact(at, f),
                ast::Stmt::Transition(t) if self.live(t.span) => self.transition(at, t),
                ast::Stmt::Allow(a) if self.live(a.span) => self.allow(at, a),
                ast::Stmt::Require(_)
                | ast::Stmt::Ensure(_)
                | ast::Stmt::Fact(_)
                | ast::Stmt::Transition(_)
                | ast::Stmt::Allow(_) => {}
            }
        }
    }

    fn statement_words(&mut self, what: &What, start: u32, word: &str, name: Option<&ast::Ident>) {
        let end = start as usize + word.len();
        self.push(
            what.clone(),
            Span::new(self.file, start as usize, end),
            None,
        );
        if let Some(n) = name {
            self.push(what.clone(), n.span, Some(n.span));
        }
    }

    fn require(&mut self, at: (usize, usize, usize), r: &ast::RequireStmt) {
        let what = What::Require {
            service: at.0,
            endpoint: at.1,
            stmt: at.2,
        };
        self.statement_words(&what, r.span.start, "require", r.name.as_ref());
        let scope = endpoint_scope(self.ix, at.0, at.1, Phase::Require);
        self.expr(&scope, &r.expr, None);
        let Some(o) = &r.otherwise else { return };
        self.type_expr(&o.error);
        let err = match &o.error.kind {
            ast::TypeExprKind::Named { name, .. } => self.vis.get(&name.name).copied(),
            ast::TypeExprKind::List(_) => None,
        };
        for pf in o.payload.iter().flatten() {
            let m = err.and_then(|d| scope::member(self.ix, &Ty::Decl(d), &pf.name.name));
            let hint = m.as_ref().map(|m| m.val.ty.clone());
            if let Some(m) = m
                && let Some((owner, index)) = m.origin
            {
                self.push(What::Member { owner, index }, pf.name.span, m.span);
            }
            self.expr(&scope, &pf.value, hint.as_ref());
        }
    }

    fn ensure(&mut self, at: (usize, usize, usize), e: &ast::EnsureStmt) {
        let what = What::Ensure {
            service: at.0,
            endpoint: at.1,
            stmt: at.2,
        };
        self.statement_words(&what, e.span.start, "ensure", e.name.as_ref());
        let scope = endpoint_scope(self.ix, at.0, at.1, Phase::Ensure);
        self.expr(&scope, &e.expr, None);
    }

    fn fact(&mut self, at: (usize, usize, usize), f: &ast::FactStmt) {
        self.kw(f.span.start, "fact");
        let all = endpoint_scope(self.ix, at.0, at.1, Phase::Require);
        if let Some(l) = all.get(&f.name.name) {
            let def = l.def;
            self.push(What::Local(l.clone()), f.name.span, def);
        }
        self.type_expr(&f.ty);
        if let Some(key) = &f.key {
            let scope = endpoint_scope(self.ix, at.0, at.1, Phase::FactKey(f.span.start));
            let ty = scope::resolve_type(self.ix, scope.file.unwrap_or(self.file), &f.ty);
            self.expr(&scope, key, Some(&ty));
        }
    }

    fn transition(&mut self, at: (usize, usize, usize), t: &ast::TransitionStmt) {
        self.kw(t.span.start, "transition");
        let scope = endpoint_scope(self.ix, at.0, at.1, Phase::Require);
        let mut ty = Ty::Unknown;
        for (i, id) in t.path.iter().enumerate() {
            if i == 0 {
                if let Some(l) = scope.get(&id.name) {
                    ty = l.val.ty.clone();
                    let def = l.def;
                    self.push(What::Local(l.clone()), id.span, def);
                }
            } else if let Some(m) = scope::member(self.ix, &ty, &id.name) {
                ty = m.val.ty.clone();
                if let Some((owner, index)) = m.origin {
                    self.push(What::Member { owner, index }, id.span, m.span);
                }
            }
        }
        let Some(d) = ty.enum_decl(self.ix) else {
            return;
        };
        let Some(DeclKind::Enum(en)) = self
            .ix
            .decls
            .get(d)
            .and_then(|e| self.ix.decl_ast(e))
            .map(|a| &a.kind)
        else {
            return;
        };
        let names: Vec<ast::Ident> = t.from.iter().chain([&t.to]).cloned().collect();
        self.states(d, en, &names);
    }

    fn allow(&mut self, at: (usize, usize, usize), a: &ast::AllowStmt) {
        self.kw(a.span.start, "allow");
        let policy = policy_named(self.ix, self.file, &a.policy.name);
        if let Some((pf, pi)) = policy {
            let def = self
                .ix
                .file(pf)
                .and_then(|f| f.ast.policies.get(pi))
                .map(|p| p.name.span);
            self.push(
                What::Policy {
                    file: pf,
                    index: pi,
                },
                a.policy.span,
                def,
            );
        }
        let scope = endpoint_scope(self.ix, at.0, at.1, Phase::Require);
        for (i, arg) in a.args.iter().enumerate() {
            let hint = policy.and_then(|(pf, pi)| {
                let p = self.ix.file(pf)?.ast.policies.get(pi)?;
                Some(resolve_type(self.ix, pf, &p.params.get(i)?.ty))
            });
            self.expr(&scope, arg, hint.as_ref());
        }
    }

    // ---- policies and the principal -----------------------------------------------------

    fn policy(&mut self, pi: usize, p: &ast::PolicyDecl) {
        self.kw(p.span.start, "policy");
        self.push(
            What::Policy {
                file: self.file,
                index: pi,
            },
            p.name.span,
            Some(p.name.span),
        );
        let scope = policy_scope(self.ix, self.file, pi);
        for param in &p.params {
            if let Some(l) = scope.get(&param.name.name) {
                let def = l.def;
                self.push(What::Local(l.clone()), param.name.span, def);
            }
            self.type_expr(&param.ty);
        }
        for c in &p.clauses {
            self.kw(c.span.start, if c.forbid { "forbid" } else { "permit" });
            self.expr(&scope, &c.expr, None);
        }
    }

    fn principal(&mut self, p: &ast::PrincipalDecl) {
        self.kw(p.span.start, "principal");
        self.type_expr(&p.ty);
        if let Some(n) = &p.name {
            self.push(What::Principal { file: self.file }, n.span, Some(n.span));
        }
    }

    // ---- examples -----------------------------------------------------------------------

    fn example(&mut self, index: usize, x: &ast::Example) {
        self.kw(x.span.start, "example");
        self.push(
            What::Example {
                file: self.file,
                index,
            },
            x.name.span,
            Some(x.name.span),
        );
        let Some(call) = x.body.as_call() else { return };
        let target = endpoint_called(
            self.ix,
            self.file,
            call.service.as_ref().map(|s| s.name.as_str()),
            &call.endpoint.name,
        );
        let scope = target.map_or_else(Scope::default, |(s, e)| {
            endpoint_scope(self.ix, s, e, Phase::Require)
        });
        for g in &x.givens {
            self.kw(g.span.start, "given");
            let fact = scope.get(&g.name.name).cloned();
            let ty = fact.as_ref().map_or(Ty::Unknown, |l| l.val.ty.clone());
            if let Some(l) = fact {
                let def = l.def;
                self.push(What::Local(l), g.name.span, def);
            }
            if let Some(v) = &g.value {
                self.value(v, &ty, &scope);
            }
        }
        if let Some(p) = &x.principal {
            self.kw(p.span.start, "as");
        }
        if let Some(at) = &x.at {
            self.kw(at.span.start, "at");
        }
        self.call(call, target, &scope);
    }

    fn call(&mut self, call: &ast::ExampleCall, target: Option<(usize, usize)>, scope: &Scope) {
        if let Some((si, ei)) = target {
            let def = crate::query::service_ast(self.ix, si)
                .and_then(|s| s.endpoints.get(ei))
                .map(|e| e.name.span);
            self.push(
                What::Endpoint {
                    service: si,
                    endpoint: ei,
                },
                call.endpoint.span,
                def,
            );
        }
        for arg in &call.args {
            let param = scope
                .locals
                .iter()
                .find(|l| l.name == arg.name.name && matches!(l.kind, LocalKind::Param { .. }));
            let ty = param.map_or(Ty::Unknown, |l| l.val.ty.clone());
            if let Some(l) = param
                && let LocalKind::Param {
                    service,
                    endpoint,
                    index,
                } = l.kind
            {
                self.push(
                    What::Param {
                        service,
                        endpoint,
                        index,
                    },
                    arg.name.span,
                    l.def,
                );
            }
            self.value(&arg.value, &ty, scope);
        }
        if let Some(e) = &call.outcome.error {
            self.type_expr(e);
        }
    }

    /// A value of an example: a path starts at a given, and an object names the fields of the
    /// type that is expected.
    fn value(&mut self, v: &ast::Value, expected: &Ty, scope: &Scope) {
        match &v.kind {
            ValueKind::Path(ids) => self.path_value(ids, scope),
            ValueKind::EnumLit(id) => {
                if let Some(d) = expected.enum_decl(self.ix) {
                    self.variant(d, id);
                }
            }
            ValueKind::List(items) => {
                for i in items {
                    self.value(i, &expected.elem(), scope);
                }
            }
            ValueKind::Object(fields) => {
                for f in fields {
                    let m = scope::member(self.ix, expected, &f.name.name);
                    let ty = m.as_ref().map_or(Ty::Unknown, |m| m.val.ty.clone());
                    if let Some(m) = m
                        && let Some((owner, index)) = m.origin
                    {
                        self.push(What::Member { owner, index }, f.name.span, m.span);
                    }
                    self.value(&f.value, &ty, scope);
                }
            }
            ValueKind::Bool(_)
            | ValueKind::Int(_)
            | ValueKind::Float { .. }
            | ValueKind::Str(_) => {}
        }
    }

    fn path_value(&mut self, ids: &[ast::Ident], scope: &Scope) {
        let mut ty = Ty::Unknown;
        for (i, id) in ids.iter().enumerate() {
            if i == 0 {
                if let Some(l) = scope.get(&id.name) {
                    ty = l.val.ty.clone();
                    let def = l.def;
                    self.push(What::Local(l.clone()), id.span, def);
                }
            } else if let Some(m) = scope::member(self.ix, &ty, &id.name) {
                ty = m.val.ty.clone();
                if let Some((owner, index)) = m.origin {
                    self.push(What::Member { owner, index }, id.span, m.span);
                }
            }
        }
    }

    // ---- expressions --------------------------------------------------------------------

    fn expr(&mut self, scope: &Scope, e: &ast::Expr, hint: Option<&Ty>) {
        self.walk(scope, &mut Vec::new(), e, hint);
    }

    fn variant(&mut self, decl: usize, id: &ast::Ident) {
        let Some(entry) = self.ix.decls.get(decl) else {
            return;
        };
        if let Some(i) = entry.variants.iter().position(|v| *v == id.name) {
            let def = match self.ix.decl_ast(entry).map(|a| &a.kind) {
                Some(DeclKind::Enum(e)) => e.variants.get(i).map(|v| v.name.span),
                _ => None,
            };
            self.push(What::Variant { decl, variant: i }, id.span, def);
        }
    }

    fn name(&mut self, scope: &Scope, lams: &[Lambda], id: &ast::Ident) {
        if let Some(l) = lams.iter().rev().find(|l| l.name == id.name) {
            self.push(What::Lambda(l.clone()), id.span, Some(l.span));
        } else if let Some(l) = scope.get(&id.name) {
            let def = l.def;
            self.push(What::Local(l.clone()), id.span, def);
        } else if let Some(&d) = self.vis.get(&id.name)
            && let Some(e) = self.ix.decls.get(d)
            && e.kind == veld_compiler::semantic::DKind::Const
        {
            self.push(What::Const(d), id.span, Some(e.name_span));
        }
    }

    fn field(&mut self, ty: &Ty, name: &ast::Ident) {
        if let Some(m) = scope::member(self.ix, ty, &name.name) {
            match m.origin {
                Some((owner, index)) => self.push(What::Member { owner, index }, name.span, m.span),
                None => self.push(What::PageMember(name.name.clone()), name.span, None),
            }
        }
    }

    fn walk(&mut self, scope: &Scope, lams: &mut Vec<Lambda>, e: &ast::Expr, hint: Option<&Ty>) {
        match &e.kind {
            ExprKind::Name(id) => self.name(scope, lams, id),
            ExprKind::Field { base, name } => {
                self.walk(scope, lams, base, None);
                let b = type_of(self.ix, scope, lams, base, None);
                self.field(&b.ty, name);
            }
            ExprKind::EnumLit(id) => {
                let by_hint = hint.and_then(|t| t.enum_decl(self.ix));
                let by_name = || only_enum_with(self.ix, scope.file.unwrap_or(self.file), &id.name);
                if let Some(d) = by_hint.or_else(by_name) {
                    self.variant(d, id);
                }
            }
            ExprKind::Call { name, args } => self.call_expr(scope, lams, name, args),
            ExprKind::List(items) => {
                let elem = hint.map(Ty::elem);
                for i in items {
                    self.walk(scope, lams, i, elem.as_ref());
                }
            }
            ExprKind::Unary { operand, .. } => self.walk(scope, lams, operand, None),
            ExprKind::Binary { op, l, r } => self.binary(scope, lams, *op, l, r),
            ExprKind::Bool(_) | ExprKind::Int(_) | ExprKind::Float { .. } | ExprKind::Str(_) => {}
        }
    }

    fn binary(
        &mut self,
        scope: &Scope,
        lams: &mut Vec<Lambda>,
        op: ast::BinOp,
        l: &ast::Expr,
        r: &ast::Expr,
    ) {
        use ast::BinOp::{Eq, Ge, Gt, In, Le, Lt, Ne};
        let ix = self.ix;
        match op {
            Eq | Ne | Lt | Le | Gt | Ge => {
                let (lt, rt) = (
                    type_of(ix, scope, lams, l, None),
                    type_of(ix, scope, lams, r, None),
                );
                self.walk(scope, lams, l, Some(&rt.ty));
                self.walk(scope, lams, r, Some(&lt.ty));
            }
            In => {
                let lt = type_of(ix, scope, lams, l, None);
                let rt = type_of(ix, scope, lams, r, None);
                self.walk(scope, lams, l, Some(&rt.ty.elem()));
                self.walk(scope, lams, r, Some(&Ty::List(Box::new(lt.ty))));
            }
            ast::BinOp::And
            | ast::BinOp::Or
            | ast::BinOp::Implies
            | ast::BinOp::Add
            | ast::BinOp::Sub
            | ast::BinOp::Mul
            | ast::BinOp::Div
            | ast::BinOp::Rem => {
                self.walk(scope, lams, l, None);
                self.walk(scope, lams, r, None);
            }
        }
    }

    fn call_expr(
        &mut self,
        scope: &Scope,
        lams: &mut Vec<Lambda>,
        name: &ast::Ident,
        args: &[CallArg],
    ) {
        if let Some(w) = words::builtin(&name.name) {
            self.push(What::Builtin(w), name.span, None);
        }
        for arg in args {
            match arg {
                CallArg::Expr(x) => self.walk(scope, lams, x, None),
                CallArg::Lambda { var, body, .. } => {
                    let val: Val = scope::element_type(self.ix, scope, lams, args);
                    let l = Lambda {
                        name: var.name.clone(),
                        val,
                        span: var.span,
                    };
                    self.push(What::Lambda(l.clone()), var.span, Some(var.span));
                    lams.push(l);
                    self.walk(scope, lams, body, None);
                    lams.pop();
                }
            }
        }
    }
}
