//! Invariants (`spec/rules.md`, `spec/targets/rust.md` section 10): the Rust `ExprPrinter` and the
//! generated check functions.
//!
//! A rule becomes plain Rust inside one closure that returns `veld::rules::Res<bool>`: helper
//! calls end in `?`, `&&` and `||` stay lazy, and `veld::rules::record` turns the outcome into a
//! violation. There is no interpreter and no rule data structure at run time.

use veld_codegen::{
    CodeWriter, ExprPrinter, GenDiagnostic, Printed, ValueKind, naming, print_expr,
};
use veld_ir::{
    BinExpr, Builtin, CallExpr, Decl, Expr, ExprKind, ExprLit, ExprType, Field, Ir, Model, Op,
    PathRef, QuantExpr, Quantifier, Root, Rule, Type,
};

use crate::types::{Ctx, rust_type};
use crate::util::{doc, lit, snake, type_name, unsupported, variant};

const OR: u8 = 2;
const AND: u8 = 3;
const CMP: u8 = 4;
const UNARY: u8 = 7;

/// The Rust printer of rule expressions.
#[derive(Debug)]
pub struct RustPrinter<'p, 't, 'a> {
    ctx: &'p Ctx<'t, 'a>,
    /// The lambda variables in scope, innermost last, with the type of their elements.
    vars: Vec<(String, ExprType)>,
    /// What the printer could not express. A non-empty list makes generation fail with `E0801`.
    pub unsupported: Vec<String>,
    /// The names an endpoint block can read; `None` for the invariants of a declaration.
    scope: Option<Scope>,
    /// The helper that reads an optional value known to be present: `some`, or `absent` for the
    /// key of an optional fact, where an absent value means "no fact" (`spec/rules.md` 9.2).
    some_fn: &'static str,
}

/// What an endpoint block reads (`spec/rules.md` section 9.1): the parameters and facts by name
/// with whether they may be absent, `result` in an `ensure`, and `now`.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub params: std::collections::BTreeMap<String, bool>,
    pub facts: std::collections::BTreeMap<String, bool>,
    pub result: bool,
    /// The local that holds the principal (a reference to its model), when the caller is in scope
    /// (`spec/rules.md` section 11.1).
    pub principal: Option<String>,
    /// Parameters and facts are locals of the generated function that hold a reference to the
    /// value, named by [`local_ident`] (policy functions and `can_<endpoint>`), instead of members
    /// of the typed request and `fact_` locals (step 6 and step 9).
    pub locals: bool,
}

/// The local that holds a parameter or fact in a [`Scope`] with `locals`. `now` is the clock.
#[must_use]
pub fn local_ident(name: &str) -> String {
    let id = snake(name);
    if id == "now" { "now_".to_string() } else { id }
}

/// The prefix that marks a name read inside `old(...)`: the value before the handler ran.
pub const OLD: &str = "old.";

/// A copy of `expr` in which every parameter and fact inside an `old(...)` is renamed with the
/// [`OLD`] prefix, so that the printer reads them from the saved copy.
#[must_use]
pub fn mark_old(expr: &Expr) -> Expr {
    walk_old(expr, false)
}

/// A path whose parameter or fact root carries the [`OLD`] prefix.
fn old_path(p: &PathRef) -> PathRef {
    let root = match &p.root {
        Root::Param(n) => Root::Param(format!("{OLD}{n}")),
        Root::Fact(n) => Root::Fact(format!("{OLD}{n}")),
        Root::SelfValue | Root::Principal | Root::Result | Root::Var(_) => p.root.clone(),
    };
    PathRef {
        root,
        steps: p.steps.clone(),
    }
}

fn walk_old(e: &Expr, inside: bool) -> Expr {
    let kind = match &e.e {
        ExprKind::Path(p) if inside => ExprKind::Path(old_path(p)),
        ExprKind::Has(p) if inside => ExprKind::Has(old_path(p)),
        ExprKind::Lit(_) | ExprKind::Path(_) | ExprKind::Has(_) | ExprKind::Now(_) => e.e.clone(),
        ExprKind::Not(x) => ExprKind::Not(Box::new(walk_old(x, inside))),
        ExprKind::Neg(x) => ExprKind::Neg(Box::new(walk_old(x, inside))),
        ExprKind::Widen(x) => ExprKind::Widen(Box::new(walk_old(x, inside))),
        ExprKind::Old(x) => ExprKind::Old(Box::new(walk_old(x, true))),
        ExprKind::Bin(b) => ExprKind::Bin(Box::new(BinExpr {
            op: b.op,
            l: walk_old(&b.l, inside),
            r: walk_old(&b.r, inside),
        })),
        ExprKind::Call(c) => ExprKind::Call(CallExpr {
            f: c.f,
            args: c.args.iter().map(|a| walk_old(a, inside)).collect(),
        }),
        ExprKind::List(items) => {
            ExprKind::List(items.iter().map(|a| walk_old(a, inside)).collect())
        }
        ExprKind::Quant(q) => ExprKind::Quant(Box::new(QuantExpr {
            q: q.q,
            list: walk_old(&q.list, inside),
            var: q.var.clone(),
            body: walk_old(&q.body, inside),
        })),
    };
    Expr {
        t: e.t.clone(),
        e: kind,
    }
}

impl<'p, 't, 'a> RustPrinter<'p, 't, 'a> {
    #[must_use]
    pub fn new(ctx: &'p Ctx<'t, 'a>) -> Self {
        Self {
            ctx,
            vars: Vec::new(),
            unsupported: Vec::new(),
            scope: None,
            some_fn: "some",
        }
    }

    /// A printer for the expressions of an endpoint block.
    #[must_use]
    pub fn for_block(ctx: &'p Ctx<'t, 'a>, scope: Scope) -> Self {
        Self {
            scope: Some(scope),
            ..Self::new(ctx)
        }
    }

    /// Reads optional values through `absent`: the key of an optional fact.
    #[must_use]
    pub fn reading_absent(mut self) -> Self {
        self.some_fn = "absent";
        self
    }

    fn ir(&self) -> &'t Ir {
        self.ctx.tree.ir
    }

    fn kind(&self, ty: &ExprType) -> ValueKind {
        ValueKind::of(ty, self.ir())
    }

    fn no(&mut self, what: &str) -> Printed {
        self.unsupported.push(what.to_string());
        Printed::atom("unreachable_rule_expression()")
    }

    /// The value of a path end: the raw field `place` converted to the value form of its type.
    fn read(&self, ty: &Type, place: &str) -> Printed {
        let kind = self.kind(&ExprType::Type(ty.clone()));
        let code = match kind {
            ValueKind::Bool
            | ValueKind::Int
            | ValueKind::Long
            | ValueKind::Float
            | ValueKind::Enum => place.to_string(),
            ValueKind::String | ValueKind::Uuid => format!("{place}.as_str()"),
            ValueKind::Decimal => format!("vr::dec(&{place})?"),
            ValueKind::Date => format!("vr::date(&{place})?"),
            ValueKind::Datetime => format!("vr::datetime(&{place})?"),
            ValueKind::Time => format!("vr::time(&{place})?"),
            ValueKind::Duration
            | ValueKind::Bytes
            | ValueKind::Json
            | ValueKind::Model
            | ValueKind::Map
            | ValueKind::Page => format!("&{place}"),
            ValueKind::List => match ty {
                Type::List(inner) => {
                    let item = self.read(inner, "(*e)").code;
                    format!("vr::list(&{place}, |e| Ok({item}))?")
                }
                Type::Scalar(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => {
                    format!("&{place}")
                }
            },
        };
        // A borrow or a dereference binds like a unary operator, not like an atom.
        let prefixed = code.starts_with('&') || code.starts_with('*');
        Printed::new(code, if prefixed { UNARY } else { Printed::ATOM })
    }

    fn field_chain(&self, base: &str, path: &PathRef) -> (String, bool) {
        let mut expr = base.to_string();
        let mut last_optional = false;
        for step in &path.steps {
            let field = snake(&step.name);
            expr = if step.optional {
                format!("vr::{}(&{expr}.{field})?", self.some_fn)
            } else {
                format!("{expr}.{field}")
            };
            last_optional = step.optional;
        }
        (expr, last_optional)
    }

    /// The place a block root names, and whether it may be absent: `p.body` (a parameter of the
    /// request), `(*fact_room)`, `(*r)` (the result), each from the saved copy inside `old(...)`.
    fn block_root(&self, root: &Root) -> Option<(String, bool)> {
        let scope = self.scope.as_ref()?;
        match root {
            Root::Param(n) if scope.locals => {
                let optional = *scope.params.get(n)?;
                Some((format!("(*{})", local_ident(n)), optional))
            }
            Root::Fact(n) if scope.locals => {
                let optional = *scope.facts.get(n)?;
                Some((format!("(*{})", local_ident(n)), optional))
            }
            Root::Param(n) => {
                let (n, old) = split_old(n);
                let optional = *scope.params.get(&n)?;
                let from = if old { "old_p" } else { "p" };
                Some((format!("{from}.{}", snake(&n)), optional))
            }
            Root::Fact(n) => {
                let (n, old) = split_old(n);
                let optional = *scope.facts.get(&n)?;
                let from = if old { "old_fact_" } else { "fact_" };
                Some((format!("(*{from}{})", fact_ident(&n)), optional))
            }
            Root::Principal => scope
                .principal
                .as_ref()
                .map(|local| (format!("(*{local})"), false)),
            Root::Result => scope.result.then(|| ("(*r)".to_string(), false)),
            Root::SelfValue | Root::Var(_) => None,
        }
    }

    /// A path from a block root: the expression of its place and whether the last read was
    /// through `some`.
    fn block_chain(&self, path: &PathRef) -> Option<(String, bool)> {
        let (base, root_optional) = self.block_root(&path.root)?;
        if !root_optional {
            return Some(self.field_chain(&base, path));
        }
        let start = format!("vr::{}(&{base})?", self.some_fn);
        let (expr, last) = self.field_chain(&start, path);
        Some((expr, last || path.steps.is_empty()))
    }

    /// The place a bare path reads, as a value to clone or serialise (not its rule form): the
    /// raw field. `None` when the expression is not a path from a parameter or a fact.
    pub fn raw_place(&mut self, expr: &Expr) -> Option<String> {
        let ExprKind::Path(path) = &expr.e else {
            return None;
        };
        if !matches!(path.root, Root::Param(_) | Root::Fact(_) | Root::Principal) {
            return None;
        }
        let (code, last_optional) = self.block_chain(path)?;
        Some(if last_optional {
            format!("(*{code})")
        } else {
            code
        })
    }

    fn var_value(&self, name: &str) -> Printed {
        let kind = self
            .vars
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, t)| self.kind(t));
        let v = var_ident(name);
        match kind {
            Some(ValueKind::Decimal | ValueKind::List) => Printed::atom(format!("(*{v}).clone()")),
            Some(_) | None => Printed::new(format!("*{v}"), UNARY),
        }
    }

    fn equality(&mut self, eq: bool, l: &Printed, r: &Printed, kind: ValueKind) -> Printed {
        let sym = if eq { "==" } else { "!=" };
        let native = |l: &Printed, r: &Printed| {
            Printed::new(format!("{} {sym} {}", l.at(CMP + 1), r.at(CMP + 1)), CMP)
        };
        match kind {
            ValueKind::Bool
            | ValueKind::Int
            | ValueKind::Long
            | ValueKind::Float
            | ValueKind::String
            | ValueKind::Date
            | ValueKind::Datetime
            | ValueKind::Time
            | ValueKind::Duration
            | ValueKind::Enum => native(l, r),
            ValueKind::Decimal => {
                let test = if eq { "is_eq" } else { "is_ne" };
                Printed::atom(format!(
                    "{}.compare(&{}).{test}()",
                    l.at(Printed::ATOM),
                    r.at(UNARY)
                ))
            }
            ValueKind::Uuid => {
                let call = format!("vr::text::uuid_eq({}, {})", l.code, r.code);
                if eq {
                    Printed::atom(call)
                } else {
                    Printed::new(format!("!{call}"), UNARY)
                }
            }
            ValueKind::Bytes
            | ValueKind::Json
            | ValueKind::Model
            | ValueKind::List
            | ValueKind::Map
            | ValueKind::Page => self.no("`==` on a value that cannot be compared"),
        }
    }

    fn ordering(&mut self, op: Op, l: &Printed, r: &Printed, kind: ValueKind) -> Printed {
        let (sym, test) = match op {
            Op::Lt => ("<", "is_lt"),
            Op::Le => ("<=", "is_le"),
            Op::Gt => (">", "is_gt"),
            Op::Ge
            | Op::And
            | Op::Or
            | Op::Implies
            | Op::Eq
            | Op::Ne
            | Op::In
            | Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Rem => (">=", "is_ge"),
        };
        match kind {
            ValueKind::Int
            | ValueKind::Long
            | ValueKind::Float
            | ValueKind::Date
            | ValueKind::Datetime
            | ValueKind::Time
            | ValueKind::Duration => {
                Printed::new(format!("{} {sym} {}", l.at(CMP + 1), r.at(CMP + 1)), CMP)
            }
            ValueKind::Decimal => Printed::atom(format!(
                "{}.compare(&{}).{test}()",
                l.at(Printed::ATOM),
                r.at(UNARY)
            )),
            ValueKind::Bool
            | ValueKind::String
            | ValueKind::Uuid
            | ValueKind::Enum
            | ValueKind::Bytes
            | ValueKind::Json
            | ValueKind::Model
            | ValueKind::List
            | ValueKind::Map
            | ValueKind::Page => self.no("an ordering of a value that cannot be ordered"),
        }
    }

    fn membership(
        &mut self,
        l: &Printed,
        r: &Printed,
        elem: ValueKind,
        coll: ValueKind,
    ) -> Printed {
        if coll == ValueKind::Map {
            return Printed::atom(format!("{}.contains_key({})", r.at(Printed::ATOM), l.code));
        }
        let eq = self.equality(true, &Printed::atom("needle"), &Printed::atom("(*e)"), elem);
        Printed::atom(format!(
            "{{ let needle = {}; vr::contains(&{}, |e| {}) }}",
            l.code, r.code, eq.code
        ))
    }

    fn arith(
        &mut self,
        op: Op,
        l: &Printed,
        r: &Printed,
        kinds: (ValueKind, ValueKind, ValueKind),
    ) -> Printed {
        let (lk, rk, out) = kinds;
        let name = match op {
            Op::Add => "add",
            Op::Sub => "sub",
            Op::Mul => "mul",
            Op::Div => "div",
            Op::Rem
            | Op::And
            | Op::Or
            | Op::Implies
            | Op::Eq
            | Op::Ne
            | Op::Lt
            | Op::Le
            | Op::Gt
            | Op::Ge
            | Op::In => "rem",
        };
        match out {
            ValueKind::Int => {
                Printed::atom(format!("vr::num::{name}_i32({}, {})?", l.code, r.code))
            }
            ValueKind::Long => {
                Printed::atom(format!("vr::num::{name}_i64({}, {})?", l.code, r.code))
            }
            ValueKind::Float => {
                Printed::atom(format!("vr::num::{name}_f64({}, {})?", l.code, r.code))
            }
            ValueKind::Decimal if matches!(op, Op::Add | Op::Sub | Op::Mul) => {
                Printed::atom(format!("{}.{name}(&{})", l.at(Printed::ATOM), r.at(UNARY)))
            }
            ValueKind::Datetime | ValueKind::Duration => self.temporal(op, l, r, (lk, rk)),
            ValueKind::Decimal
            | ValueKind::Bool
            | ValueKind::String
            | ValueKind::Uuid
            | ValueKind::Date
            | ValueKind::Time
            | ValueKind::Bytes
            | ValueKind::Json
            | ValueKind::Enum
            | ValueKind::Model
            | ValueKind::List
            | ValueKind::Map
            | ValueKind::Page => self.no("arithmetic that the language does not define"),
        }
    }

    fn temporal(
        &mut self,
        op: Op,
        l: &Printed,
        r: &Printed,
        kinds: (ValueKind, ValueKind),
    ) -> Printed {
        use ValueKind::{Date, Datetime, Duration};
        let call = |f: &str| Printed::atom(format!("{f}({}, {})?", l.code, r.code));
        match (op, kinds) {
            (Op::Add, (Datetime, Duration) | (Duration, Datetime | Duration)) => {
                call("vr::num::add_i64")
            }
            (Op::Sub, (Datetime, Duration | Datetime) | (Duration, Duration)) => {
                call("vr::num::sub_i64")
            }
            (Op::Sub, (Date, Date)) => call("vr::date_diff"),
            _ => self.no("arithmetic on dates and durations that the language does not define"),
        }
    }

    /// `has` from a block root that may itself be absent.
    fn has_root(&self, base: &str, optional: bool, steps: &[veld_ir::Step]) -> String {
        if !optional {
            return self.has_chain(base, steps, 0);
        }
        if steps.iter().all(|s| !s.optional) {
            return format!("{base}.is_some()");
        }
        format!(
            "{base}.as_ref().is_some_and(|h0| {})",
            self.has_chain("h0", steps, 1)
        )
    }

    fn has_chain(&self, base: &str, steps: &[veld_ir::Step], depth: usize) -> String {
        let Some(first) = steps.iter().position(|s| s.optional) else {
            return "true".to_string();
        };
        let mut place = base.to_string();
        for s in steps.iter().take(first + 1) {
            place = format!("{place}.{}", snake(&s.name));
        }
        let rest = steps.get(first + 1..).unwrap_or_default();
        if rest.iter().all(|s| !s.optional) {
            return format!("{place}.is_some()");
        }
        let inner = format!("h{depth}");
        // The remaining steps start from the unwrapped value.
        let tail = self.has_chain(&inner, rest, depth + 1);
        format!("{place}.as_ref().is_some_and(|{inner}| {tail})")
    }
}

fn var_ident(name: &str) -> String {
    format!("v_{}", snake(name).trim_start_matches("r#"))
}

/// The identifier of a fact local: `room` is `fact_room`.
#[must_use]
pub fn fact_ident(name: &str) -> String {
    snake(name).trim_start_matches("r#").to_string()
}

fn split_old(name: &str) -> (String, bool) {
    match name.strip_prefix(OLD) {
        Some(rest) => (rest.to_string(), true),
        None => (name.to_string(), false),
    }
}

impl ExprPrinter for RustPrinter<'_, '_, '_> {
    fn lit(&mut self, l: &ExprLit, ty: &ExprType) -> Printed {
        let kind = self.kind(ty);
        let number = |n: i64| -> Printed {
            let (code, neg) = match kind {
                ValueKind::Long | ValueKind::Duration => (format!("{n}_i64"), n < 0),
                ValueKind::Float => (format!("{n}.0_f64"), n < 0),
                ValueKind::Decimal => {
                    return Printed::atom(format!("vr::dec({})?", lit(&n.to_string())));
                }
                ValueKind::Int
                | ValueKind::Bool
                | ValueKind::String
                | ValueKind::Uuid
                | ValueKind::Date
                | ValueKind::Datetime
                | ValueKind::Time
                | ValueKind::Bytes
                | ValueKind::Json
                | ValueKind::Enum
                | ValueKind::Model
                | ValueKind::List
                | ValueKind::Map
                | ValueKind::Page => (format!("{n}_i32"), n < 0),
            };
            Printed::new(code, if neg { UNARY } else { Printed::ATOM })
        };
        match l {
            ExprLit::Bool(b) => Printed::atom(b.to_string()),
            ExprLit::Int(n) | ExprLit::Long(n) => number(*n),
            ExprLit::Float(x) => Printed::new(
                format!("{x:?}f64"),
                if *x < 0.0 { UNARY } else { Printed::ATOM },
            ),
            ExprLit::Decimal(text) => Printed::atom(format!("vr::dec({})?", lit(text))),
            ExprLit::String(s) => Printed::atom(lit(s)),
            ExprLit::Enum(e) => {
                let path = self.ctx.tree.type_path(e.decl, self.ctx.here);
                Printed::atom(format!("{path}::{}", variant(&e.name)))
            }
        }
    }

    fn path(&mut self, path: &PathRef, ty: &ExprType) -> Printed {
        let ExprType::Type(t) = ty else {
            return self.no("a path of type duration");
        };
        match &path.root {
            Root::SelfValue => {
                let (expr, optional) = self.field_chain("self", path);
                let place = if optional { format!("(*{expr})") } else { expr };
                self.read(t, &place)
            }
            Root::Var(name) if path.steps.is_empty() => self.var_value(name),
            Root::Var(name) => {
                let (expr, optional) = self.field_chain(&var_ident(name), path);
                let place = if optional { format!("(*{expr})") } else { expr };
                self.read(t, &place)
            }
            Root::Result | Root::Param(_) | Root::Fact(_) | Root::Principal => {
                match self.block_chain(path) {
                    Some((expr, optional)) => {
                        let place = if optional { format!("(*{expr})") } else { expr };
                        self.read(t, &place)
                    }
                    None => self.no("a name that is not in scope"),
                }
            }
        }
    }

    fn has(&mut self, path: &PathRef) -> Printed {
        let base = match &path.root {
            Root::SelfValue => "self".to_string(),
            Root::Var(name) => var_ident(name),
            Root::Param(_) | Root::Fact(_) | Root::Principal => {
                let Some((base, optional)) = self.block_root(&path.root) else {
                    return self.no("`has` on a name that is not in scope");
                };
                return Printed::atom(self.has_root(&base, optional, &path.steps));
            }
            Root::Result => {
                return self.no("`has` on the result");
            }
        };
        Printed::atom(self.has_chain(&base, &path.steps, 0))
    }

    fn now(&mut self) -> Printed {
        if self.scope.is_some() {
            Printed::atom("now")
        } else {
            self.no("`now`")
        }
    }

    fn not(&mut self, operand: Printed) -> Printed {
        Printed::new(format!("!{}", operand.at(UNARY)), UNARY)
    }

    fn neg(&mut self, operand: Printed, ty: &ExprType) -> Printed {
        match self.kind(ty) {
            ValueKind::Int => Printed::atom(format!("vr::num::neg_i32({})?", operand.code)),
            ValueKind::Long | ValueKind::Duration => {
                Printed::atom(format!("vr::num::neg_i64({})?", operand.code))
            }
            ValueKind::Float => Printed::new(format!("-{}", operand.at(UNARY)), UNARY),
            ValueKind::Decimal => Printed::atom(format!("{}.negated()", operand.at(Printed::ATOM))),
            ValueKind::Bool
            | ValueKind::String
            | ValueKind::Uuid
            | ValueKind::Date
            | ValueKind::Datetime
            | ValueKind::Time
            | ValueKind::Bytes
            | ValueKind::Json
            | ValueKind::Enum
            | ValueKind::Model
            | ValueKind::List
            | ValueKind::Map
            | ValueKind::Page => self.no("a negation of a value that is not a number"),
        }
    }

    fn bin(
        &mut self,
        op: Op,
        l: Printed,
        r: Printed,
        left: &ExprType,
        right: &ExprType,
        out: &ExprType,
    ) -> Printed {
        let (lk, rk, ok) = (self.kind(left), self.kind(right), self.kind(out));
        match op {
            Op::And => Printed::new(format!("{} && {}", l.at(AND), r.at(AND + 1)), AND),
            Op::Or => Printed::new(format!("{} || {}", l.at(OR), r.at(OR + 1)), OR),
            Op::Implies => Printed::new(format!("!{} || {}", l.at(UNARY), r.at(OR + 1)), OR),
            Op::Eq => self.equality(true, &l, &r, lk),
            Op::Ne => self.equality(false, &l, &r, lk),
            Op::Lt | Op::Le | Op::Gt | Op::Ge => self.ordering(op, &l, &r, lk),
            Op::In => self.membership(&l, &r, lk, rk),
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem => self.arith(op, &l, &r, (lk, rk, ok)),
        }
    }

    fn call(
        &mut self,
        f: Builtin,
        args: Vec<Printed>,
        arg_types: &[&ExprType],
        _out: &ExprType,
    ) -> Printed {
        let kinds: Vec<ValueKind> = arg_types.iter().map(|t| self.kind(t)).collect();
        let code = |i: usize| args.get(i).map(|p| p.code.clone()).unwrap_or_default();
        match f {
            Builtin::Len => match kinds.first() {
                Some(ValueKind::String | ValueKind::Uuid) => {
                    Printed::atom(format!("vr::len_str({})?", code(0)))
                }
                Some(ValueKind::List | ValueKind::Map | ValueKind::Bytes) => {
                    Printed::atom(format!(
                        "vr::len_n({}.len())?",
                        args.first()
                            .map(|p| p.at(Printed::ATOM))
                            .unwrap_or_default()
                    ))
                }
                _ => self.no("`len` of a value that has no length"),
            },
            Builtin::Matches => Printed::atom(format!("vr::matches({}, {})?", code(0), code(1))),
            Builtin::StartsWith => {
                Printed::atom(format!("vr::text::starts_with({}, {})", code(0), code(1)))
            }
            Builtin::EndsWith => {
                Printed::atom(format!("vr::text::ends_with({}, {})", code(0), code(1)))
            }
            Builtin::Contains => {
                Printed::atom(format!("vr::text::contains({}, {})", code(0), code(1)))
            }
            Builtin::Days | Builtin::Hours | Builtin::Minutes | Builtin::Seconds | Builtin::Ms => {
                let name = match f {
                    Builtin::Days => "days_ms",
                    Builtin::Hours => "hours_ms",
                    Builtin::Minutes => "minutes_ms",
                    Builtin::Seconds => "seconds_ms",
                    Builtin::Ms
                    | Builtin::Len
                    | Builtin::Matches
                    | Builtin::StartsWith
                    | Builtin::EndsWith
                    | Builtin::Contains => "ms_ms",
                };
                let n = match kinds.first() {
                    Some(ValueKind::Long) => code(0),
                    _ => format!("i64::from({})", code(0)),
                };
                Printed::atom(format!("vr::num::{name}({n})?"))
            }
        }
    }

    fn enter_var(&mut self, var: &str, elem: &ExprType) {
        self.vars.push((var.to_string(), elem.clone()));
    }

    fn exit_var(&mut self, _var: &str) {
        self.vars.pop();
    }

    fn quant(
        &mut self,
        q: Quantifier,
        list: Printed,
        _elem: &ExprType,
        var: &str,
        body: Printed,
        body_ty: &ExprType,
    ) -> Printed {
        let v = var_ident(var);
        let (func, ret) = match q {
            Quantifier::All => ("all", "bool"),
            Quantifier::Any => ("any", "bool"),
            Quantifier::Count => ("count", "bool"),
            Quantifier::Sum => match self.kind(body_ty) {
                ValueKind::Int => ("sum_i32", "i32"),
                ValueKind::Long => ("sum_i64", "i64"),
                ValueKind::Float => ("sum_f64", "f64"),
                ValueKind::Decimal => ("sum_dec", "vr::Decimal"),
                ValueKind::Bool
                | ValueKind::String
                | ValueKind::Uuid
                | ValueKind::Date
                | ValueKind::Datetime
                | ValueKind::Time
                | ValueKind::Duration
                | ValueKind::Bytes
                | ValueKind::Json
                | ValueKind::Enum
                | ValueKind::Model
                | ValueKind::List
                | ValueKind::Map
                | ValueKind::Page => return self.no("`sum` of something that is not a number"),
            },
        };
        Printed::atom(format!(
            "vr::{func}(&{}, |{v}| -> vr::Res<{ret}> {{ Ok({}) }})?",
            list.code, body.code
        ))
    }

    fn list(&mut self, items: Vec<Printed>, _elem: Option<&ExprType>) -> Printed {
        if items.is_empty() {
            return Printed::atom("Vec::new()");
        }
        let codes: Vec<String> = items.into_iter().map(|p| p.code).collect();
        Printed::atom(format!("vec![{}]", codes.join(", ")))
    }

    fn widen(&mut self, operand: Printed, from: &ExprType, to: &ExprType) -> Printed {
        match (self.kind(from), self.kind(to)) {
            (ValueKind::Int, ValueKind::Long) => {
                Printed::atom(format!("i64::from({})", operand.code))
            }
            (ValueKind::Int, ValueKind::Float) => {
                Printed::atom(format!("f64::from({})", operand.code))
            }
            (ValueKind::Int, ValueKind::Decimal) => Printed::atom(format!(
                "vr::Decimal::from_i64(i64::from({}))",
                operand.code
            )),
            (ValueKind::Long, ValueKind::Decimal) => {
                Printed::atom(format!("vr::Decimal::from_i64({})", operand.code))
            }
            _ => self.no("a conversion that the language does not define"),
        }
    }

    fn old(&mut self, operand: Printed, _ty: &ExprType) -> Printed {
        // The operand was marked by `mark_old`, so it already reads the saved copy.
        if self.scope.is_some() {
            operand
        } else {
            self.no("`old`")
        }
    }
}

/// The printed Rust of one rule's expression.
fn print_rule(ctx: &Ctx<'_, '_>, rule: &Rule) -> Result<String, String> {
    let mut printer = RustPrinter::new(ctx);
    let printed = print_expr(&mut printer, &rule.expr);
    match printer.unsupported.first() {
        Some(what) => Err(format!("rule `{}`: {what}", rule.name)),
        None => Ok(printed.code),
    }
}

/// The wire names of the fields a rule reads (`Rule.reads` holds source names).
fn wire_names(m: &Model, rule: &Rule) -> Vec<String> {
    rule.reads
        .iter()
        .map(|name| {
            m.fields
                .iter()
                .find(|f| &f.name == name)
                .map_or_else(|| name.clone(), |f| f.wire.clone())
        })
        .collect()
}

/// `RULE_END_AFTER_START`, unique within the declaration.
fn const_names(rules: &[Rule]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for rule in rules {
        let base = format!("RULE_{}", naming::screaming_snake(&rule.name));
        let mut name = base.clone();
        let mut n = 2;
        while seen.contains(&name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        seen.push(name);
    }
    seen
}

/// Emits the public surface and the check function of a declaration with rules.
///
/// # Errors
/// `E0801` when a rule uses something the generator cannot express (never for invariants that
/// the compiler accepted).
pub fn emit(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    d: &Decl,
    m: &Model,
) -> Result<(), Vec<GenDiagnostic>> {
    let name = type_name(&d.name);
    let consts = const_names(&m.rules);
    let mut bodies = Vec::new();
    for rule in &m.rules {
        bodies.push(
            print_rule(ctx, rule).map_err(|e| unsupported(format!("generator `rust`: {e}")))?,
        );
    }
    w.open(format!("impl {name}"));
    for (rule, c) in m.rules.iter().zip(&consts) {
        doc(w, rule.docs.as_deref());
        w.line(format!("/// The id of the rule `{}`.", rule.name));
        w.line(format!("pub const {c}: &'static str = {};", lit(&rule.id)));
    }
    w.blank();
    w.line(
        "/// Checks the invariants of this value and of everything inside it. Every violation is",
    );
    w.line("/// reported; none is thrown. Paths start at `$`.");
    w.open("pub fn check(&self) -> Vec<veld::rules::Violation>");
    w.line("veld::rules::check_value(self, veld::rules::Mode::Full)");
    w.close().blank();
    w.line(
        "/// Like [`Self::check`], for a value in request position: rules that read a `@readonly`",
    );
    w.line("/// field are skipped.");
    w.open("pub fn check_request(&self) -> Vec<veld::rules::Violation>");
    w.line("veld::rules::check_value(self, veld::rules::Mode::Request)");
    w.close();
    w.close().blank();
    let derived: Vec<&Field> = if ctx.tree.derived {
        d.derived_fields()
    } else {
        Vec::new()
    };
    w.open(format!("impl veld::rules::Invariants for {name}"));
    if !derived.is_empty() {
        // A rule that reads a derived field that could not be computed is skipped.
        w.open("fn check_rules(&self, mode: veld::rules::Mode, path: &str, out: &mut Vec<veld::rules::Violation>)");
        w.line("self.check_rules_absent(mode, path, &[], out);");
        w.close().blank();
    }
    w.line(
        "#[allow(clippy::all, unused_variables, unused_parens, unused_braces, unused_comparisons)]",
    );
    if derived.is_empty() {
        w.open("fn check_rules(&self, mode: veld::rules::Mode, path: &str, out: &mut Vec<veld::rules::Violation>)");
    } else {
        w.open("fn check_rules_absent(&self, mode: veld::rules::Mode, path: &str, absent: &[&str], out: &mut Vec<veld::rules::Violation>)");
    }
    w.line("use veld::rules as vr;");
    for ((rule, c), body) in m.rules.iter().zip(&consts).zip(&bodies) {
        rule_statement(w, m, rule, c, (body, &derived));
    }
    w.close().close().blank();
    Ok(())
}

fn rule_statement(
    w: &mut CodeWriter,
    m: &Model,
    rule: &Rule,
    id_const: &str,
    (body, derived): (&str, &[&Field]),
) {
    let fields: Vec<String> = wire_names(m, rule).iter().map(|f| lit(f)).collect();
    let message = rule
        .message
        .clone()
        .unwrap_or_else(|| format!("violates rule {}", rule.name));
    w.line(format!("// {}", rule.name));
    // The conditions of the rule: not in a request when it reads a read-only field, and not when
    // it reads a derived field that has no value.
    let mut conditions: Vec<String> = Vec::new();
    if rule.reads_readonly {
        conditions.push("mode != vr::Mode::Request".to_string());
    }
    for f in derived.iter().filter(|f| rule.reads.contains(&f.name)) {
        conditions.push(format!("!absent.contains(&{})", lit(&f.wire)));
    }
    let guarded = !conditions.is_empty();
    if guarded {
        w.open(format!("if {}", conditions.join(" && ")));
    }
    if rule.is_audited() {
        // An audited rule reports to the sink of a server and is skipped when there is none, so a
        // client never evaluates it (`spec/rules.md` section 17.1).
        w.line(format!(
            "vr::audit(path, Self::{id_const}, {}, &[{}], {}, || -> vr::Res<bool> {{ Ok({body}) }});",
            lit(&rule.name),
            fields.join(", "),
            lit(&message)
        ));
    } else {
        w.line(format!(
            "vr::record(out, path, Self::{id_const}, {}, &[{}], {}, (|| -> vr::Res<bool> {{ Ok({body}) }})());",
            lit(&rule.name),
            fields.join(", "),
            lit(&message)
        ));
    }
    if guarded {
        w.close();
    }
}

/// The `ok_<field>` local of a derived field: whether it has a value.
fn ok_ident(f: &Field) -> String {
    format!("ok_{}", snake(&f.name).trim_start_matches("r#"))
}

/// The value of a derived expression as the field's type: the widening the language allows at
/// the root, then the conversion from the value form to the field.
fn derived_value(
    ctx: &Ctx<'_, '_>,
    f: &Field,
    expr: &Expr,
) -> Result<String, String> {
    let mut printer = RustPrinter::new(ctx);
    let mut printed = print_expr(&mut printer, expr);
    let ty = ExprType::Type(f.ty.clone());
    let (from, to) = (printer.kind(&expr.t), printer.kind(&ty));
    if from != to {
        printed = printer.widen(printed, &expr.t, &ty);
    }
    if let Some(what) = printer.unsupported.first() {
        return Err(format!("derived field `{}`: {what}", f.name));
    }
    let atom = printed.at(Printed::ATOM);
    Ok(match to {
        ValueKind::Bool
        | ValueKind::Int
        | ValueKind::Long
        | ValueKind::Float
        | ValueKind::Enum => printed.code,
        ValueKind::String | ValueKind::Uuid | ValueKind::Decimal => format!("{atom}.to_string()"),
        ValueKind::Date => format!("vr::format_date({})", printed.code),
        ValueKind::Datetime => format!("vr::format_datetime({})", printed.code),
        ValueKind::Time => format!("vr::format_time({})", printed.code),
        ValueKind::Duration
        | ValueKind::Bytes
        | ValueKind::Json
        | ValueKind::Model
        | ValueKind::List
        | ValueKind::Map
        | ValueKind::Page => {
            return Err(format!(
                "derived field `{}`: a derived value of this type is not supported",
                f.name
            ));
        }
    })
}

/// The wire names of the fields of `d` that `names` (source names) stand for.
fn wires_of(d: &Decl, names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|n| {
            d.fields()
                .iter()
                .find(|f| &f.name == n)
                .map_or_else(|| n.clone(), |f| f.wire.clone())
        })
        .collect()
}

/// Emits `derive`, `derive_request` and the `Derive` impl of a declaration with derived fields
/// (`spec/rules.md` section 17.2).
///
/// # Errors
/// `E0801` when a derived expression, or the type of a derived field, cannot be expressed.
pub fn emit_derived(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    d: &Decl,
) -> Result<(), Vec<GenDiagnostic>> {
    let name = type_name(&d.name);
    let fields = d.derived_fields();
    let mut bodies = Vec::new();
    for f in &fields {
        let Some(expr) = &f.derived else { continue };
        bodies.push(
            derived_value(ctx, f, expr)
                .map_err(|e| unsupported(format!("generator `rust`: {}: {e}", d.name)))?,
        );
    }
    w.open(format!("impl {name}"));
    w.line("/// Computes the derived fields of this value and of everything inside it, in dependency");
    w.line("/// order (`spec/rules.md` section 17.2): the code the server runs, so that a form can show");
    w.line("/// them while the user types. Returns one violation for each field that could not be");
    w.line("/// computed; such a field keeps the value it had.");
    w.open("pub fn derive(&mut self) -> Vec<veld::rules::Violation>");
    w.line("veld::rules::derive_value(self, veld::rules::Mode::Full)");
    w.close().blank();
    w.line("/// Like [`Self::derive`], for a value in request position: a derived field that reads a");
    w.line("/// `@readonly` field is left alone.");
    w.open("pub fn derive_request(&mut self) -> Vec<veld::rules::Violation>");
    w.line("veld::rules::derive_value(self, veld::rules::Mode::Request)");
    w.close();
    w.close().blank();
    w.open(format!("impl veld::rules::Derive for {name}"));
    w.line(
        "#[allow(clippy::all, unused_variables, unused_parens, unused_braces, unused_comparisons)]",
    );
    w.open("fn derive_fields(&mut self, node: &mut serde_json::Map<String, serde_json::Value>, mode: veld::rules::Mode, path: &str, out: &mut Vec<veld::rules::Violation>)");
    w.line("use veld::rules as vr;");
    for (f, body) in fields.iter().zip(&bodies) {
        derived_statement(w, ctx, d, f, body);
    }
    w.close().close().blank();
    Ok(())
}

fn derived_statement(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, d: &Decl, f: &Field, body: &str) {
    let reads = f.derived_reads();
    let id = f.derived_id.clone().unwrap_or_default();
    let ident = snake(&f.name);
    let ty = rust_type(ctx, &f.ty, false);
    let wire_reads: Vec<String> = wires_of(d, &reads).iter().map(|r| lit(r)).collect();
    // Blocked: it reads a derived field that has no value, or (in a request) a read-only field.
    let mut conditions: Vec<String> = Vec::new();
    if reads.iter().any(|r| d.fields().iter().any(|o| &o.name == r && o.readonly)) {
        conditions.push("mode != vr::Mode::Request".to_string());
    }
    for r in &reads {
        if let Some(o) = d.fields().iter().find(|o| &o.name == r && o.derived.is_some()) {
            conditions.push(ok_ident(o));
        }
    }
    w.line(format!("// {}", f.name));
    let guarded = !conditions.is_empty();
    if guarded {
        w.line(format!("let {} = if {} {{", ok_ident(f), conditions.join(" && ")));
        w.indent();
        w.line("match");
    } else {
        w.line(format!("let {} = match", ok_ident(f)));
    }
    w.indent();
    w.line(format!(
        "vr::derived(out, path, {}, {}, &[{}], (|| -> vr::Res<{ty}> {{ Ok({body}) }})())",
        lit(&id),
        lit(&f.name),
        wire_reads.join(", ")
    ));
    w.dedent();
    w.line("{");
    w.indent();
    w.line("Some(v) => {");
    w.indent();
    w.line(format!("self.{ident} = v;"));
    w.line(format!("vr::put(node, {}, &self.{ident});", lit(&f.wire)));
    w.line("true");
    w.dedent();
    w.line("}");
    w.line("None => false,");
    w.dedent();
    if guarded {
        w.line("}");
        w.dedent();
        w.line("} else {");
        w.indent();
        w.line("false");
        w.dedent();
        w.line("};");
    } else {
        w.line("};");
    }
}

#[cfg(test)]
mod tests;
