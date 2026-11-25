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

use veld_codegen::canonical::es_number;
use veld_codegen::expr::{ExprPrinter, Printed, ValueKind, print_expr};
use veld_ir::{Builtin, Expr, ExprLit, ExprType, Ir, Op, PathRef, Quantifier, Root, Scalar, Type};

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
}

/// Prints `expr` with the helpers under the namespace `ns`.
#[must_use]
pub fn print(ir: &Ir, ns: &str, expr: &Expr) -> String {
    let mut p = TsExpr {
        ir,
        ns: ns.to_owned(),
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
        let mut code = match &path.root {
            Root::SelfValue => "self".to_owned(),
            Root::Var(name) => var_ident(name),
            Root::Param(_) | Root::Fact(_) | Root::Principal | Root::Result => {
                return self.unavailable("a root other than `self`");
            }
        };
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
            Type::Scalar(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => return code.to_owned(),
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
        _ => prec::CMP,
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
            ExprLit::Int(n) if is_scalar(ty, Scalar::Float) => {
                number(es_number(*n as f64), *n < 0)
            }
            ExprLit::Int(n) if is_scalar(ty, Scalar::Decimal) => self.helper("dec", &[&ts_str(&n.to_string())]),
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
        let mut code = match &path.root {
            Root::SelfValue => "self".to_owned(),
            Root::Var(name) => var_ident(name),
            Root::Param(_) | Root::Fact(_) | Root::Principal | Root::Result => {
                return Printed::atom(self.unavailable("a root other than `self`"));
            }
        };
        let mut present = Vec::new();
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
            _ => Printed::new(format!("-{}", operand.at(prec::PREFIX + 1)), prec::PREFIX),
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
        let first = args.first().map_or_else(String::new, |a| a.at(Printed::ATOM));
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
                    _ => "sumLong",
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
            (ValueKind::Int, ValueKind::Long) => {
                Printed::atom(format!("BigInt({})", operand.code))
            }
            (ValueKind::Int, ValueKind::Decimal) => self.helper("decFromInt", &[&operand.code]),
            (ValueKind::Long, ValueKind::Decimal) => self.helper("decFromLong", &[&operand.code]),
            _ => operand,
        }
    }

    fn old(&mut self, _operand: Printed, _ty: &ExprType) -> Printed {
        Printed::atom(self.unavailable("old"))
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
