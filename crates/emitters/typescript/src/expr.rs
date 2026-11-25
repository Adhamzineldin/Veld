//! The TypeScript [`ExprPrinter`]: prints a typed rule expression (`spec/rules.md` section 3)
//! as TypeScript that calls the helpers of `runtime/rules.ts`.
//!
//! Representation of a value while a rule runs: `int` and `float` are `number`; `long`,
//! `duration`, `datetime`, `date` and `time` are `bigint`; `decimal` is the helper's `Decimal`;
//! everything else keeps its wire form (strings, enum wire names, base64 text, lists, maps).
//! A field read converts at the leaf, so a `long` field read is a `bigint`. A list read converts
//! its elements once, so a lambda variable over scalars is a bare identifier.
//!
//! Native operators are used where the representation makes them exact (`===`, `<`, `&&`,
//! `startsWith`); every checked or exact operation is a helper call. Partial checks need an
//! absent required field to be detectable, so a required step of a path goes through
//! `required(...)`; an optional step the compiler proved present uses a non-null assertion.

use std::collections::BTreeSet;

use veld_codegen::canonical::es_number;
use veld_codegen::expr::{ExprPrinter, Printed, ValueKind, print_expr};
use veld_ir::{
    Builtin, Expr, ExprKind, ExprLit, ExprType, Ir, Op, PathRef, Quantifier, Root, Scalar, Type,
};

use crate::util::{is_ident, ts_str};

/// Precedences of the TypeScript operators the printer emits natively.
mod prec {
    pub const OR: u8 = 3;
    pub const AND: u8 = 4;
    pub const EQ: u8 = 8;
    pub const CMP: u8 = 9;
    pub const PREFIX: u8 = 14;
}

/// Prints rule expressions with the helper namespace `ns` (an import of `runtime/rules`).
#[derive(Debug)]
pub struct TsExpr<'a> {
    ir: &'a Ir,
    ns: String,
    scope: Option<EndpointScope>,
    olds: Vec<(String, ExprType)>,
}

/// Prints `expr` with the helpers under the namespace `ns`.
#[must_use]
pub fn print(ir: &Ir, ns: &str, expr: &Expr) -> String {
    let mut p = TsExpr {
        ir,
        ns: ns.to_owned(),
        scope: None,
        olds: Vec::new(),
    };
    print_expr(&mut p, expr).code
}

/// The identifier of a lambda variable. Prefixed, so it can never be a reserved word or collide
/// with a local of the generated check function.
#[must_use]
pub fn var_ident(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c.to_string()
            } else {
                format!("u{:x}", u32::from(c))
            }
        })
        .collect();
    format!("l_{safe}")
}

/// `base.name`, or `base['name']` when `name` is not an identifier.
#[must_use]
pub fn member(base: &str, name: &str) -> String {
    if is_ident(name) {
        format!("{base}.{name}")
    } else {
        format!("{base}[{}]", ts_str(name))
    }
}

impl TsExpr<'_> {
    fn helper(&self, name: &str, args: &[&str]) -> Printed {
        Printed::atom(format!("{}.{name}({})", self.ns, args.join(", ")))
    }

    fn kind(&self, ty: &ExprType) -> ValueKind {
        ValueKind::of(ty, self.ir)
    }

    /// The raw TypeScript expression of a path, one step at a time.
    fn chain(&self, path: &PathRef) -> String {
        let Some((mut code, optional)) = self.root_code(&path.root) else {
            return self.unavailable("a root other than `self`");
        };
        if optional {
            code.push('!');
        }
        for step in &path.steps {
            let access = member(&code, &step.name);
            code = if step.optional {
                format!("{access}!")
            } else {
                format!("{}.required({access})", self.ns)
            };
        }
        code
    }

    /// The code of a root and whether it may be absent. `None` for a root the scope does not have.
    fn root_code(&self, root: &Root) -> Option<(String, bool)> {
        match root {
            Root::SelfValue => Some(("self".to_owned(), false)),
            Root::Var(name) => Some((var_ident(name), false)),
            Root::Param(name) => {
                let scope = self.scope.as_ref()?;
                let code = scope
                    .local(root)
                    .map_or_else(|| member("p", name), str::to_owned);
                Some((code, scope.optional_params.contains(name)))
            }
            Root::Fact(name) => {
                let scope = self.scope.as_ref()?;
                let code = scope
                    .local(root)
                    .map_or_else(|| member("f", name), str::to_owned);
                Some((code, scope.optional_facts.contains(name)))
            }
            Root::Result => self.scope.as_ref().map(|_| ("result".to_owned(), false)),
            Root::Principal => {
                let scope = self.scope.as_ref()?;
                let principal = scope.principal.as_ref()?;
                if let Some(code) = scope.local(root) {
                    return Some((code.to_owned(), false));
                }
                // An `ensure` runs after the handler, and the caller does not change while it
                // runs: it reads the caller as a saved value (`EndpointPrinter::print`).
                if scope.ensure {
                    return Some((CALLER.to_owned(), false));
                }
                Some((member("f", &principal.name), false))
            }
        }
    }

    fn unavailable(&self, what: &str) -> String {
        format!("{}.unavailable({})", self.ns, ts_str(what))
    }

    /// Converts the wire representation of `ty` to the rule representation.
    fn convert(&self, code: &str, ty: &Type, depth: usize) -> String {
        let helper = match ty {
            Type::Scalar(Scalar::Long) => "toLong",
            Type::Scalar(Scalar::Decimal) => "dec",
            Type::Scalar(Scalar::Date) => "toDate",
            Type::Scalar(Scalar::Datetime) => "toDatetime",
            Type::Scalar(Scalar::Time) => "toTime",
            Type::Scalar(Scalar::Uuid) => "toUuid",
            Type::List(inner) => {
                let name = format!("e{depth}");
                let item = self.convert(&name, inner, depth + 1);
                return if item == name {
                    code.to_owned()
                } else {
                    format!("{code}.map(({name}) => {item})")
                };
            }
            Type::Scalar(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => {
                return code.to_owned();
            }
        };
        format!("{}.{helper}({code})", self.ns)
    }

    fn arith(&self, op: Op, left: ValueKind, out: ValueKind) -> String {
        let verb = match op {
            Op::Add => "add",
            Op::Sub => "sub",
            Op::Mul => "mul",
            Op::Div => "div",
            Op::Rem => "rem",
            Op::And
            | Op::Or
            | Op::Implies
            | Op::Eq
            | Op::Ne
            | Op::Lt
            | Op::Le
            | Op::Gt
            | Op::Ge
            | Op::In => return self.unavailable("a non-arithmetic operator"),
        };
        if op == Op::Sub && left == ValueKind::Date {
            return "dateDiff".to_owned();
        }
        let suffix = match out {
            ValueKind::Int => "Int",
            ValueKind::Float => "Float",
            ValueKind::Decimal => "Dec",
            ValueKind::Long | ValueKind::Datetime | ValueKind::Duration => "Long",
            ValueKind::Bool
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
            | ValueKind::Page => return self.unavailable("arithmetic on this type"),
        };
        if suffix == "Dec" {
            return format!("dec{}", capitalise(verb));
        }
        format!("{verb}{suffix}")
    }

    fn comparison(&self, op: Op, l: &Printed, r: &Printed, left: ValueKind) -> Printed {
        let symbol = match op {
            Op::Lt => "<",
            Op::Le => "<=",
            Op::Gt => ">",
            Op::Ge => ">=",
            Op::Eq => "===",
            Op::Ne => "!==",
            Op::And
            | Op::Or
            | Op::Implies
            | Op::In
            | Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Rem => return Printed::atom(self.unavailable("a non-comparison operator")),
        };
        if left == ValueKind::Decimal {
            let cmp = format!("{}.decCmp({}, {})", self.ns, l.code, r.code);
            return Printed::new(format!("{cmp} {symbol} 0"), prec_of(op));
        }
        let p = prec_of(op);
        let code = format!("{} {symbol} {}", l.at(p + 1), r.at(p + 1));
        Printed::new(code, p)
    }

    fn contains(&self, l: &Printed, r: &Printed, left: &ExprType, right: &ExprType) -> Printed {
        if self.kind(right) == ValueKind::Map {
            return self.helper("hasKey", &[&r.code, &l.code]);
        }
        if self.kind(left) == ValueKind::Decimal {
            return self.helper("decIn", &[&l.code, &r.code]);
        }
        Printed::atom(format!("{}.includes({})", r.at(Printed::ATOM), l.code))
    }
}

fn capitalise(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
        .unwrap_or_default()
}

fn prec_of(op: Op) -> u8 {
    match op {
        Op::Eq | Op::Ne => prec::EQ,
        Op::And
        | Op::Or
        | Op::Implies
        | Op::Lt
        | Op::Le
        | Op::Gt
        | Op::Ge
        | Op::In
        | Op::Add
        | Op::Sub
        | Op::Mul
        | Op::Div
        | Op::Rem => prec::CMP,
    }
}

fn is_scalar(ty: &ExprType, s: Scalar) -> bool {
    matches!(ty, ExprType::Type(Type::Scalar(x)) if *x == s)
}

fn number(text: String, negative: bool) -> Printed {
    if negative {
        Printed::new(text, prec::PREFIX)
    } else {
        Printed::atom(text)
    }
}

impl ExprPrinter for TsExpr<'_> {
    fn lit(&mut self, lit: &ExprLit, ty: &ExprType) -> Printed {
        match lit {
            ExprLit::Bool(b) => Printed::atom(b.to_string()),
            ExprLit::Int(n) if is_scalar(ty, Scalar::Long) => number(format!("{n}n"), *n < 0),
            ExprLit::Int(n) if is_scalar(ty, Scalar::Float) => number(es_number(*n as f64), *n < 0),
            ExprLit::Int(n) if is_scalar(ty, Scalar::Decimal) => {
                self.helper("dec", &[&ts_str(&n.to_string())])
            }
            ExprLit::Int(n) => number(n.to_string(), *n < 0),
            ExprLit::Long(n) => number(format!("{n}n"), *n < 0),
            ExprLit::Float(x) => number(es_number(*x), *x < 0.0),
            ExprLit::Decimal(text) => self.helper("dec", &[&ts_str(text)]),
            ExprLit::String(s) => Printed::atom(ts_str(s)),
            ExprLit::Enum(e) => Printed::atom(ts_str(&e.wire)),
        }
    }

    fn path(&mut self, path: &PathRef, ty: &ExprType) -> Printed {
        let raw = self.chain(path);
        let ExprType::Type(t) = ty else {
            return Printed::atom(raw);
        };
        // A lambda variable with no steps is an element of a list that was already converted.
        if matches!(path.root, Root::Var(_)) && path.steps.is_empty() {
            return Printed::atom(raw);
        }
        Printed::atom(self.convert(&raw, t, 0))
    }

    fn has(&mut self, path: &PathRef) -> Printed {
        let Some((mut code, optional)) = self.root_code(&path.root) else {
            return Printed::atom(self.unavailable("a root other than `self`"));
        };
        let mut present = Vec::new();
        if optional {
            present.push(format!("{code} !== undefined"));
            code.push('!');
        }
        for step in &path.steps {
            let access = member(&code, &step.name);
            if step.optional {
                present.push(format!("{access} !== undefined"));
                code = format!("{access}!");
            } else {
                code = format!("{}.required({access})", self.ns);
            }
        }
        match present.len() {
            0 => Printed::atom("true"),
            1 => Printed::new(present.join(""), prec::EQ),
            _ => Printed::new(present.join(" && "), prec::AND),
        }
    }

    fn now(&mut self) -> Printed {
        if self.scope.is_some() {
            return Printed::atom("now");
        }
        Printed::atom(self.unavailable("now"))
    }

    fn not(&mut self, operand: Printed) -> Printed {
        Printed::new(format!("!{}", operand.at(prec::PREFIX)), prec::PREFIX)
    }

    fn neg(&mut self, operand: Printed, ty: &ExprType) -> Printed {
        match self.kind(ty) {
            ValueKind::Int => self.helper("negInt", &[&operand.code]),
            ValueKind::Long | ValueKind::Duration => self.helper("negLong", &[&operand.code]),
            ValueKind::Decimal => self.helper("decNeg", &[&operand.code]),
            ValueKind::Float
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
            | ValueKind::Page => {
                Printed::new(format!("-{}", operand.at(prec::PREFIX + 1)), prec::PREFIX)
            }
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
        match op {
            Op::And => Printed::new(
                format!("{} && {}", l.at(prec::AND), r.at(prec::AND + 1)),
                prec::AND,
            ),
            Op::Or => Printed::new(
                format!("{} || {}", l.at(prec::OR), r.at(prec::OR + 1)),
                prec::OR,
            ),
            Op::Implies => Printed::new(
                format!("!{} || {}", l.at(prec::PREFIX), r.at(prec::OR + 1)),
                prec::OR,
            ),
            Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                self.comparison(op, &l, &r, self.kind(left))
            }
            Op::In => self.contains(&l, &r, left, right),
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem => {
                let name = self.arith(op, self.kind(left), self.kind(out));
                self.helper(&name, &[&l.code, &r.code])
            }
        }
    }

    fn call(
        &mut self,
        f: Builtin,
        args: Vec<Printed>,
        arg_types: &[&ExprType],
        _out: &ExprType,
    ) -> Printed {
        let code: Vec<&str> = args.iter().map(|a| a.code.as_str()).collect();
        let first = args
            .first()
            .map_or_else(String::new, |a| a.at(Printed::ATOM));
        let second = code.get(1).copied().unwrap_or_default();
        match f {
            Builtin::Len => match arg_types.first().map(|t| self.kind(t)) {
                Some(ValueKind::String) => self.helper("lenString", &code),
                Some(ValueKind::Bytes) => self.helper("lenBytes", &code),
                Some(ValueKind::Map) => self.helper("lenMap", &code),
                _ => Printed::atom(format!("{first}.length")),
            },
            Builtin::Matches => self.helper("matches", &code),
            Builtin::StartsWith => Printed::atom(format!("{first}.startsWith({second})")),
            Builtin::EndsWith => Printed::atom(format!("{first}.endsWith({second})")),
            Builtin::Contains => Printed::atom(format!("{first}.includes({second})")),
            Builtin::Days => self.helper("days", &code),
            Builtin::Hours => self.helper("hours", &code),
            Builtin::Minutes => self.helper("minutes", &code),
            Builtin::Seconds => self.helper("seconds", &code),
            Builtin::Ms => self.helper("ms", &code),
        }
    }

    fn enter_var(&mut self, _var: &str, _elem: &ExprType) {}

    fn exit_var(&mut self, _var: &str) {}

    fn quant(
        &mut self,
        q: Quantifier,
        list: Printed,
        _elem: &ExprType,
        var: &str,
        body: Printed,
        body_ty: &ExprType,
    ) -> Printed {
        let arrow = format!("({}) => {}", var_ident(var), body.code);
        match q {
            Quantifier::All => Printed::atom(format!("{}.every({arrow})", list.at(Printed::ATOM))),
            Quantifier::Any => Printed::atom(format!("{}.some({arrow})", list.at(Printed::ATOM))),
            Quantifier::Count => self.helper("countWhere", &[&list.code, &arrow]),
            Quantifier::Sum => {
                let name = match self.kind(body_ty) {
                    ValueKind::Int => "sumInt",
                    ValueKind::Float => "sumFloat",
                    ValueKind::Decimal => "sumDecimal",
                    ValueKind::Long
                    | ValueKind::Bool
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
                    | ValueKind::Page => "sumLong",
                };
                self.helper(name, &[&list.code, &arrow])
            }
        }
    }

    fn list(&mut self, items: Vec<Printed>, _elem: Option<&ExprType>) -> Printed {
        let items: Vec<&str> = items.iter().map(|i| i.code.as_str()).collect();
        Printed::atom(format!("[{}]", items.join(", ")))
    }

    fn widen(&mut self, operand: Printed, from: &ExprType, to: &ExprType) -> Printed {
        match (self.kind(from), self.kind(to)) {
            (ValueKind::Int, ValueKind::Long) => Printed::atom(format!("BigInt({})", operand.code)),
            (ValueKind::Int, ValueKind::Decimal) => self.helper("decFromInt", &[&operand.code]),
            (ValueKind::Long, ValueKind::Decimal) => self.helper("decFromLong", &[&operand.code]),
            _ => operand,
        }
    }

    fn old(&mut self, operand: Printed, ty: &ExprType) -> Printed {
        let Some(scope) = &self.scope else {
            return Printed::atom(self.unavailable("old"));
        };
        // The operand runs before the handler, where the caller is part of the scope `f`.
        let operand = match &scope.principal {
            Some(principal) => Printed {
                code: operand.code.replace(CALLER, &member("f", &principal.name)),
                ..operand
            },
            None => operand,
        };
        let slot = match self.olds.iter().position(|(code, _)| *code == operand.code) {
            Some(slot) => slot,
            None => {
                self.olds.push((operand.code, ty.clone()));
                self.olds.len() - 1
            }
        };
        Printed::atom(format!("o[{slot}]"))
    }
}

/// What stands for the caller in an `ensure` until `EndpointPrinter::print` has saved it.
const CALLER: &str = "\u{1}caller\u{1}";

/// The caller of an endpoint: the name it has in the rules (`user`) and its type.
#[derive(Debug, Clone)]
pub struct Principal {
    pub name: String,
    pub ty: Type,
}

/// What the rules of an endpoint block can read besides the parameters' types: which parameters
/// and facts may be absent. A root that may be absent is asserted present where the compiler
/// proved it (`coupon!`), and `has(coupon)` tests it.
#[derive(Debug, Default, Clone)]
pub struct EndpointScope {
    pub optional_params: BTreeSet<String>,
    pub optional_facts: BTreeSet<String>,
    /// The caller, when the rules of the block may read it (`spec/rules.md` section 11.1). Its
    /// value is in the scope `f` under its name, put there by the server before the facts load.
    pub principal: Option<Principal>,
    /// Roots that are read from a local of the generated function instead of `p.<name>`,
    /// `f.<name>` or the caller in `f`: the arguments of a policy function and of
    /// `can<Endpoint>`.
    pub locals: Vec<(Root, String)>,
    /// The expressions printed are those of an `ensure`: a read of the caller is a saved value.
    pub ensure: bool,
}

impl EndpointScope {
    fn local(&self, root: &Root) -> Option<&str> {
        self.locals
            .iter()
            .find(|(r, _)| r == root)
            .map(|(_, code)| code.as_str())
    }
}

/// The printer of the expressions of one endpoint block (`spec/rules.md` section 9).
///
/// The generated functions take `p` (the parameters, as the handler receives them), `f` (the
/// loaded facts), `now` (milliseconds since the epoch, a `bigint`) and, in an `ensure`, `result`
/// and `o` (the saved `old(...)` values). All of them hold values in the developer-facing
/// representation; a read converts at the leaf like in a check function.
#[derive(Debug)]
pub struct EndpointPrinter<'a> {
    inner: TsExpr<'a>,
}

impl<'a> EndpointPrinter<'a> {
    #[must_use]
    pub fn new(ir: &'a Ir, ns: &str, scope: EndpointScope) -> Self {
        Self {
            inner: TsExpr {
                ir,
                ns: ns.to_owned(),
                scope: Some(scope),
                olds: Vec::new(),
            },
        }
    }

    /// Prints `expr`; the value is in the rule representation.
    pub fn print(&mut self, expr: &Expr) -> String {
        let code = print_expr(&mut self.inner, expr).code;
        self.save_caller(code)
    }

    /// A read of the caller outside `old(...)` in an `ensure` becomes a slot of the saved
    /// values: the operand `f.user` is evaluated with the rest at the end of step 6.
    fn save_caller(&mut self, code: String) -> String {
        let Some(principal) = self.inner.scope.as_ref().and_then(|s| s.principal.clone()) else {
            return code;
        };
        if !code.contains(CALLER) {
            return code;
        }
        let operand = member("f", &principal.name);
        let slot = match self.inner.olds.iter().position(|(c, _)| *c == operand) {
            Some(slot) => slot,
            None => {
                self.inner
                    .olds
                    .push((operand, ExprType::Type(principal.ty)));
                self.inner.olds.len() - 1
            }
        };
        code.replace(CALLER, &format!("o[{slot}]"))
    }

    /// The raw code of the field the `steps` reach below `root`, in the developer-facing
    /// representation: `f.booking.status`. `None` when the scope has no such root.
    #[must_use]
    pub fn field_code(&self, root: &Root, steps: &[veld_ir::Step]) -> Option<String> {
        let (mut code, _) = self.inner.root_code(root)?;
        for step in steps {
            code = member(&code, &step.name);
        }
        Some(code)
    }

    /// The value of a plain path in the developer-facing representation, `undefined` when an
    /// optional step is absent. This is what a fact loader takes as its key and what an `else`
    /// payload carries. `None` when `expr` is not a plain path.
    #[must_use]
    pub fn raw_path(&self, expr: &Expr) -> Option<String> {
        let ExprKind::Path(path) = &expr.e else {
            return None;
        };
        let (mut code, optional) = self.inner.root_code(&path.root)?;
        let mut guarded = optional;
        for step in &path.steps {
            let dot = if guarded { "?." } else { "." };
            code = if is_ident(&step.name) {
                format!("{code}{dot}{}", step.name)
            } else if guarded {
                format!("{code}?.[{}]", ts_str(&step.name))
            } else {
                format!("{code}[{}]", ts_str(&step.name))
            };
            guarded |= step.optional;
        }
        Some(code)
    }

    /// The `old(...)` operands printed so far: the code to evaluate before the handler runs, and
    /// the type of the saved value, by slot.
    #[must_use]
    pub fn olds(&self) -> &[(String, ExprType)] {
        &self.inner.olds
    }
}

/// The wire name of the source field `name` of `fields`, or `name` itself when it is unknown.
#[must_use]
pub fn wire_name(fields: &[veld_ir::Field], name: &str) -> String {
    fields
        .iter()
        .find(|f| f.name == name)
        .map_or_else(|| name.to_owned(), |f| f.wire.clone())
}

#[cfg(test)]
mod tests;
