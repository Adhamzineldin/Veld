//! Names and paths: lambda variables, fields, inlined constants and the guard analysis of
//! `spec/rules.md` sections 3.2 and 3.6.

use std::collections::BTreeSet;

use super::checker::{Checker, Phase, PrincipalScope};
use super::types::{boolean, lit, node, of, scalar};
use crate::diagnostics::{Diagnostic, Span};
use crate::semantic::{DKind, RField};
use crate::syntax::ast::{self, BinOp, CallArg, ExprKind as A, UnOp};
use veld_ir::rules::{Always, Expr, ExprKind, ExprLit, PathRef, Root, Step};
use veld_ir::{Literal, Scalar, Type};

/// The pieces of a path expression `a.b.c`.
pub(super) struct Flat<'e> {
    pub root: &'e ast::Ident,
    pub steps: Vec<&'e ast::Ident>,
}

/// Flattens `a.b.c`; `None` when `e` is not a plain path.
pub(super) fn flatten(e: &ast::Expr) -> Option<Flat<'_>> {
    match &e.kind {
        A::Name(id) => Some(Flat {
            root: id,
            steps: Vec::new(),
        }),
        A::Field { base, name } => {
            let mut flat = flatten(base)?;
            flat.steps.push(name);
            Some(flat)
        }
        A::Bool(_)
        | A::Int(_)
        | A::Float { .. }
        | A::Str(_)
        | A::EnumLit(_)
        | A::Call { .. }
        | A::List(_)
        | A::Unary { .. }
        | A::Binary { .. } => None,
    }
}

/// `-12`, `3.50`: digits with an optional sign and fraction (what a decimal literal may be).
fn is_decimal_text(s: &str) -> bool {
    let digits = s.strip_prefix('-').unwrap_or(s);
    let (whole, frac) = digits
        .split_once('.')
        .map_or((digits, None), |(w, f)| (w, Some(f)));
    let all = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    all(whole) && frac.is_none_or(all)
}

/// Every prefix of the path as a key (`address`, `address.zip`).
fn prefixes(flat: &Flat<'_>) -> Vec<String> {
    let mut key = flat.root.name.clone();
    let mut out = vec![key.clone()];
    for s in &flat.steps {
        key = format!("{key}.{}", s.name);
        out.push(key.clone());
    }
    out
}

fn has_arg(e: &ast::Expr) -> Option<Flat<'_>> {
    let A::Call { name, args } = &e.kind else {
        return None;
    };
    match args.as_slice() {
        [CallArg::Expr(arg)] if name.name == "has" => flatten(arg),
        // `old(has(p))` in `ensure`: presence before the handler covers the reads inside
        // `old(...)` (`spec/rules.md` section 9.4).
        [CallArg::Expr(arg)] if name.name == "old" => has_arg(arg),
        _ => None,
    }
}

/// Paths known present when `e` is true (`spec/rules.md` section 3.6).
pub(super) fn facts(e: &ast::Expr) -> BTreeSet<String> {
    match &e.kind {
        A::Call { .. } => has_arg(e)
            .map(|f| prefixes(&f))
            .unwrap_or_default()
            .into_iter()
            .collect(),
        A::Binary {
            op: BinOp::And,
            l,
            r,
        } => facts(l).into_iter().chain(facts(r)).collect(),
        A::Unary {
            op: UnOp::Not { .. },
            operand,
        } => absent_facts(operand),
        A::Bool(_)
        | A::Int(_)
        | A::Float { .. }
        | A::Str(_)
        | A::EnumLit(_)
        | A::Name(_)
        | A::Field { .. }
        | A::List(_)
        | A::Unary { .. }
        | A::Binary { .. } => BTreeSet::new(),
    }
}

/// Paths known present when `e` is false.
pub(super) fn absent_facts(e: &ast::Expr) -> BTreeSet<String> {
    match &e.kind {
        A::Unary {
            op: UnOp::Not { .. },
            operand,
        } => facts(operand),
        A::Binary {
            op: BinOp::Or,
            l,
            r,
        } => absent_facts(l).into_iter().chain(absent_facts(r)).collect(),
        A::Bool(_)
        | A::Int(_)
        | A::Float { .. }
        | A::Str(_)
        | A::EnumLit(_)
        | A::Name(_)
        | A::Field { .. }
        | A::Call { .. }
        | A::List(_)
        | A::Unary { .. }
        | A::Binary { .. } => BTreeSet::new(),
    }
}

/// Where a path starts: a field of the declaration or a lambda variable of this type.
enum Start {
    Field(Box<RField>),
    /// A parameter or fact of an endpoint block: the root itself, not a step. The flag says
    /// whether reading it counts as a read of a parameter.
    Root(Box<RField>, bool),
    Var(Type),
}

/// A typed path and whether the list it ends in is bounded by `@length(max: n)`.
pub(super) struct Typed {
    pub expr: Expr,
    pub bounded: bool,
}

impl Checker<'_, '_, '_> {
    pub fn path_expr(&mut self, e: &ast::Expr) -> Option<Expr> {
        self.typed_path(e, false).map(|t| t.expr)
    }

    /// Resolves a name or a field path. With `in_has` an optional step is allowed.
    pub fn typed_path(&mut self, e: &ast::Expr, in_has: bool) -> Option<Typed> {
        let Some(flat) = flatten(e) else {
            self.err("E0902", "cannot read a field of this expression", e.span);
            return None;
        };
        let name = &flat.root.name;
        if let Some(i) = self.lambdas.iter().rposition(|(v, _)| v == name) {
            let ty = self.lambdas[i].1.clone();
            return self.walk(Root::Var(name.clone()), &flat, Start::Var(ty), in_has);
        }
        if self.ep.is_some() {
            if let Some(found) = self.endpoint_name(&flat, in_has) {
                return found;
            }
            let fields_too = self.ep.as_ref().is_some_and(|e| e.fields_in_scope);
            if let Some(f) = self.fields.iter().find(|f| &f.name == name).cloned()
                && fields_too
            {
                return self.walk(Root::SelfValue, &flat, Start::Field(Box::new(f)), in_has);
            }
        } else if let Some(f) = self.fields.iter().find(|f| &f.name == name).cloned() {
            return self.walk(Root::SelfValue, &flat, Start::Field(Box::new(f)), in_has);
        }
        self.constant_or_unknown(&flat)
    }

    /// A name of an endpoint block (`spec/rules.md` section 9.1). `None` when `flat` starts with
    /// none of them; `Some(None)` when it does and the use was reported.
    fn endpoint_name(&mut self, flat: &Flat<'_>, in_has: bool) -> Option<Option<Typed>> {
        let name = flat.root.name.clone();
        let ep = self.ep.as_ref()?;
        let param = ep.params.iter().find(|f| f.name == name).cloned();
        let fact = ep.facts.iter().find(|f| f.name == name).cloned();
        if let Some(f) = param {
            return Some(self.walk(
                Root::Param(name),
                flat,
                Start::Root(Box::new(f), true),
                in_has,
            ));
        }
        if let Some(f) = fact {
            if !self.fact_readable(&name, flat.root.span) {
                return Some(None);
            }
            return Some(self.walk(
                Root::Fact(name),
                flat,
                Start::Root(Box::new(f), false),
                in_has,
            ));
        }
        let principal = ep.principal.clone().filter(|p| p.field.name == name);
        if let Some(p) = principal {
            return Some(self.principal_value(flat, in_has, p));
        }
        match name.as_str() {
            "now" => Some(self.now_value(flat, in_has)),
            "result" => Some(self.result_value(flat, in_has)),
            _ => None,
        }
    }

    /// The caller: a root of its own, never a parameter (`spec/rules.md` section 11.1).
    fn principal_value(
        &mut self,
        flat: &Flat<'_>,
        in_has: bool,
        p: PrincipalScope,
    ) -> Option<Typed> {
        if !p.available {
            self.report(
                Diagnostic::error(
                    "E0904",
                    format!(
                        "`{}` is the principal, which this endpoint does not have: its auth is `none`",
                        p.field.name
                    ),
                    flat.root.span,
                )
                .with_help("give the endpoint or its service an `@auth`, or do not read the caller"),
            );
            return None;
        }
        self.walk(
            Root::Principal,
            flat,
            Start::Root(Box::new(p.field), false),
            in_has,
        )
    }

    /// `E0923`: a fact is read in `ensure` only inside `old(...)`.
    fn fact_readable(&mut self, name: &str, span: Span) -> bool {
        let in_ensure_bare = self
            .ep
            .as_ref()
            .is_some_and(|e| e.phase == Phase::Ensure && !e.in_old);
        if in_ensure_bare {
            self.report(
                Diagnostic::error(
                    "E0923",
                    format!("fact `{name}` is read in `ensure` outside `old(...)`"),
                    span,
                )
                .with_help(format!(
                    "the handler may have changed it: write `old({name}...)` for its value before the handler ran"
                )),
            );
        }
        !in_ensure_bare
    }

    fn now_value(&mut self, flat: &Flat<'_>, in_has: bool) -> Option<Typed> {
        if in_has {
            self.err(
                "E0906",
                "`now` is never absent, so `has` has nothing to ask",
                flat.root.span,
            );
            return None;
        }
        if let Some(step) = flat.steps.first() {
            self.err(
                "E0902",
                format!("cannot read `.{}` of a `datetime`", step.name),
                step.span,
            );
            return None;
        }
        Some(Typed {
            expr: node(scalar(Scalar::Datetime), ExprKind::Now(Always)),
            bounded: false,
        })
    }

    fn result_value(&mut self, flat: &Flat<'_>, in_has: bool) -> Option<Typed> {
        let span = flat.root.span;
        let (phase, result) = self.ep.as_ref().map(|e| (e.phase, e.result.clone()))?;
        if phase != Phase::Ensure {
            self.err("E0904", "`result` is only available in `ensure`", span);
            return None;
        }
        let Some(ty) = result else {
            self.report(
                Diagnostic::error(
                    "E0904",
                    "`result` is not available: this endpoint has no success body to read",
                    span,
                )
                .with_help("give the endpoint a success type, e.g. `-> 200 Booking`"),
            );
            return None;
        };
        self.walk(Root::Result, flat, Start::Var(ty), in_has)
    }

    fn walk(&mut self, root: Root, flat: &Flat<'_>, start: Start, in_has: bool) -> Option<Typed> {
        let mut steps = Vec::new();
        let mut bounded = false;
        let mut key = flat.root.name.clone();
        let mut any_optional = false;
        let mut ty = match start {
            Start::Var(ty) => ty,
            Start::Field(f) => {
                self.note_read(&f.name, f.readonly);
                bounded = f.constraints.max_len.is_some();
                any_optional |= f.optional;
                self.require_present(f.optional, &key, flat.root.span, in_has)?;
                steps.push(Step {
                    name: f.name,
                    wire: f.wire,
                    optional: f.optional,
                });
                f.ty
            }
            Start::Root(f, is_param) => {
                if is_param {
                    self.note_read(&f.name, false);
                }
                bounded = f.constraints.max_len.is_some();
                any_optional |= f.optional;
                self.require_present(f.optional, &key, flat.root.span, in_has)?;
                f.ty
            }
        };
        for id in &flat.steps {
            key = format!("{key}.{}", id.name);
            let (step, next, max) = self.step(&ty, id)?;
            any_optional |= step.optional;
            self.require_present(step.optional, &key, id.span, in_has)?;
            bounded = max;
            steps.push(step);
            ty = next;
        }
        if in_has && !any_optional {
            self.report(
                Diagnostic::error(
                    "E0906",
                    format!("`{key}` is never absent, so `has` has nothing to ask"),
                    flat.root.span,
                )
                .with_help("`has` takes an optional field (`name?: T`) or a path through one"),
            );
            return None;
        }
        let path = PathRef { root, steps };
        let t = of(ty);
        let kind = if in_has {
            ExprKind::Has(path)
        } else {
            ExprKind::Path(path)
        };
        let out_t = if in_has { boolean() } else { t };
        Some(Typed {
            expr: node(out_t, kind),
            bounded,
        })
    }

    fn note_read(&mut self, name: &str, readonly: bool) {
        if !self.reads.iter().any(|r| r == name) {
            self.reads.push(name.to_string());
        }
        self.reads_readonly |= readonly;
    }

    fn require_present(
        &mut self,
        optional: bool,
        key: &str,
        span: Span,
        in_has: bool,
    ) -> Option<()> {
        let lenient = self.ep.as_ref().is_some_and(|e| e.lenient);
        if !optional || in_has || lenient || self.known.contains(key) {
            return Some(());
        }
        self.report(
            Diagnostic::error(
                "E0903",
                format!("`{key}` is optional and may be absent where it is read here"),
                span,
            )
            .with_help(format!(
                "guard it: `has({key}) && ...`, `has({key}) implies ...` or `!has({key}) || ...`"
            )),
        );
        None
    }

    /// One `.name` step on a value of type `ty`: the step, the type after it and whether the
    /// field carries `@length(max: n)`.
    fn step(&mut self, ty: &Type, id: &ast::Ident) -> Option<(Step, Type, bool)> {
        match ty {
            Type::Decl(d)
                if matches!(self.prog.decls[d.index()].kind, DKind::Model | DKind::Error) =>
            {
                let info = &self.prog.decls[d.index()];
                let Some(f) = info.res.fields.iter().find(|f| f.name == id.name) else {
                    let names: Vec<&str> =
                        info.res.fields.iter().map(|f| f.name.as_str()).collect();
                    let diag = Diagnostic::error(
                        "E0901",
                        format!("`{}` has no field `{}`", info.name, id.name),
                        id.span,
                    )
                    .did_you_mean(
                        &id.name,
                        names.iter().copied(),
                        Some(format!("fields: {}", names.join(", "))),
                    );
                    self.report(diag);
                    return None;
                };
                let step = Step {
                    name: f.name.clone(),
                    wire: f.wire.clone(),
                    optional: f.optional,
                };
                Some((step, f.ty.clone(), f.constraints.max_len.is_some()))
            }
            Type::Page(item) => self.page_step(item, id),
            Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Decl(_) => {
                let shown = self.prog.type_name(ty);
                self.err(
                    "E0902",
                    format!("cannot read `.{}` of a value of type `{shown}`", id.name),
                    id.span,
                );
                None
            }
        }
    }

    fn page_step(&mut self, item: &Type, id: &ast::Ident) -> Option<(Step, Type, bool)> {
        let ty = match id.name.as_str() {
            "items" => Type::List(Box::new(item.clone())),
            "total" => Type::Scalar(Scalar::Long),
            "page" | "size" => Type::Scalar(Scalar::Int),
            _ => {
                self.report(
                    Diagnostic::error(
                        "E0901",
                        format!("a `Page` has no field `{}`", id.name),
                        id.span,
                    )
                    .with_help("a `Page` has `items`, `total`, `page` and `size`"),
                );
                return None;
            }
        };
        let step = Step {
            name: id.name.clone(),
            wire: id.name.clone(),
            optional: false,
        };
        Some((step, ty, false))
    }

    fn constant_or_unknown(&mut self, flat: &Flat<'_>) -> Option<Typed> {
        let id = flat.root;
        if self.poisoned.contains(&id.name) {
            return None;
        }
        let hit = self.prog.lookup(self.module, &id.name);
        if let Some(d) = hit.filter(|d| self.prog.decls[*d].kind == DKind::Const) {
            if let Some(step) = flat.steps.first() {
                self.err(
                    "E0902",
                    format!("constant `{}` has no fields", id.name),
                    step.span,
                );
                return None;
            }
            return self.const_literal(d, id.span).map(|expr| Typed {
                expr,
                bounded: false,
            });
        }
        self.unknown_name(id)
    }

    fn unknown_name(&mut self, id: &ast::Ident) -> Option<Typed> {
        let msg = match id.name.as_str() {
            "now" => {
                "`now` is not available in an invariant: data that was valid yesterday stays valid today"
            }
            "result" => "`result` is not available in an invariant",
            _ => "",
        };
        if !msg.is_empty() {
            self.err("E0904", msg, id.span);
            return None;
        }
        let mut names: Vec<String> = self.lambdas.iter().map(|(v, _)| v.clone()).collect();
        names.extend(self.scope_names());
        names.extend(self.visible_consts());
        let diag = Diagnostic::error("E0901", format!("unknown name `{}`", id.name), id.span)
            .did_you_mean(
                &id.name,
                names.iter().map(String::as_str),
                Some(self.scope_hint()),
            );
        self.report(diag);
        None
    }

    /// The local names of the scope: fields of the declaration, or the parameters, facts, `now`
    /// and `result` of an endpoint block.
    fn scope_names(&self) -> Vec<String> {
        let Some(ep) = &self.ep else {
            return self.fields.iter().map(|f| f.name.clone()).collect();
        };
        let mut names: Vec<String> = ep.params.iter().map(|f| f.name.clone()).collect();
        names.extend(ep.facts.iter().map(|f| f.name.clone()));
        names.push("now".to_string());
        if let Some(p) = ep.principal.as_ref().filter(|p| p.available) {
            names.push(p.field.name.clone());
        }
        if ep.phase == Phase::Ensure && ep.result.is_some() {
            names.push("result".to_string());
        }
        names
    }

    fn scope_hint(&self) -> String {
        let what = if self.ep.is_some() {
            "names in scope"
        } else {
            "fields in scope"
        };
        format!("{what}: {}", self.scope_names().join(", "))
    }

    fn visible_consts(&self) -> Vec<String> {
        self.prog.scopes[self.module]
            .names
            .iter()
            .filter(|(_, e)| self.prog.decls[e.decl].kind == DKind::Const)
            .map(|(n, _)| n.clone())
            .collect()
    }

    /// A `const` replaced by its literal (`spec/rules.md` section 3.2).
    fn const_literal(&mut self, d: usize, span: Span) -> Option<Expr> {
        let info = &self.prog.decls[d];
        let (Some(ty), Some(value)) = (&info.res.const_ty, &info.res.const_value) else {
            return None;
        };
        let name = info.name.clone();
        let Type::Scalar(s) = ty else {
            return self.bad_const(&name, span);
        };
        let literal = match (s, value) {
            (Scalar::Bool, Literal::Bool(b)) => ExprLit::Bool(*b),
            (Scalar::String, Literal::String(v)) => ExprLit::String(v.clone()),
            (Scalar::Int, Literal::Int(n)) => ExprLit::Int(*n),
            (Scalar::Long, Literal::Int(n)) => ExprLit::Long(*n),
            (Scalar::Float, Literal::Float(f)) => ExprLit::Float(*f),
            (Scalar::Decimal, Literal::String(v)) if is_decimal_text(v) => {
                ExprLit::Decimal(v.clone())
            }
            _ => return self.bad_const(&name, span),
        };
        Some(lit(scalar(*s), literal))
    }

    fn bad_const(&mut self, name: &str, span: Span) -> Option<Expr> {
        self.report(
            Diagnostic::error(
                "E0905",
                format!("constant `{name}` cannot be used in a rule"),
                span,
            )
            .with_help("only `bool`, `string`, `int`, `long`, `float` and `decimal` constants can"),
        );
        None
    }

    /// Runs `f` with `extra` added to the known-present paths, then restores them.
    pub fn with_known<T>(&mut self, extra: BTreeSet<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        if extra.is_empty() {
            return f(self);
        }
        let saved = self.known.clone();
        self.known.extend(extra);
        let out = f(self);
        self.known = saved;
        out
    }
}
