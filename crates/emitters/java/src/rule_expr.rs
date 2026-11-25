//! The Java [`ExprPrinter`]: rule expressions as Java code (`spec/targets/java.md` section 8).
//!
//! Values inside an expression have one Java form per kind: `boolean`, `int`, `long`, `double`,
//! `BigDecimal`, `String`, `UUID` and enum constants as they are; instants, dates, times and
//! durations as `long` (milliseconds, days, milliseconds, milliseconds). Paths read the record
//! accessors. A value that may be a boxed type (an optional or defaulted field, a list element)
//! is cast to its primitive so that `==` never compares references. Anything the helpers cannot
//! express exactly goes through `Rules` (`runtime/Rules.java`).

use veld_codegen::{ExprPrinter, Printed, ValueKind};
use veld_ir::{Builtin, Decl, DeclId, DeclKind, ExprLit, ExprType, Ir, Op, PathRef, Quantifier};
use veld_ir::{Root, Type};

use crate::models::{enum_constants, field_idents};
use crate::tree::FileCtx;
use crate::types::{java_double, nullable};
use crate::util::java_str;

const OR: u8 = 2;
const AND: u8 = 3;
const EQ: u8 = 4;
const CMP: u8 = 5;
const UNARY: u8 = 8;

/// What a path step reads from: a declaration, a page, or a value that has no members.
#[derive(Clone, Copy)]
enum At<'a> {
    Decl(&'a Decl),
    Page,
    Nothing,
}

/// Prints the expressions of one declaration's rules. `v` is the value being checked.
#[derive(Debug)]
pub struct JavaExpr<'f, 't, 'a> {
    fc: &'f mut FileCtx<'t, 'a>,
    ir: &'a Ir,
    this: DeclId,
    vars: Vec<(String, ExprType)>,
    unsupported: Vec<String>,
}

impl<'f, 't, 'a> JavaExpr<'f, 't, 'a> {
    /// A printer for the rules of `this`.
    pub fn new(fc: &'f mut FileCtx<'t, 'a>, this: DeclId) -> Self {
        let ir = fc.tree.ir;
        Self {
            fc,
            ir,
            this,
            vars: Vec::new(),
            unsupported: Vec::new(),
        }
    }

    /// What the generator cannot express. A non-empty list is an `E0801`.
    #[must_use]
    pub fn unsupported(&self) -> &[String] {
        &self.unsupported
    }

    fn kind(&self, ty: &ExprType) -> ValueKind {
        ValueKind::of(ty, self.ir)
    }

    fn rules(&mut self) -> String {
        self.fc.rt("Rules")
    }

    fn refuse(&mut self, what: &str) -> Printed {
        self.unsupported.push(what.to_string());
        Printed::atom("false")
    }

    fn call_helper(&mut self, name: &str, args: &[&Printed]) -> Printed {
        let rules = self.rules();
        let args: Vec<String> = args.iter().map(|a| a.at(0)).collect();
        Printed::atom(format!("{rules}.{name}({})", args.join(", ")))
    }

    fn at_of_type(&self, ty: &Type) -> At<'a> {
        match ty {
            Type::Decl(id) => self.at_of_decl(*id),
            Type::Page(_) => At::Page,
            Type::Scalar(_) | Type::List(_) | Type::Map(_) => At::Nothing,
        }
    }

    fn at_of_decl(&self, id: DeclId) -> At<'a> {
        let d = self.ir.decl(id);
        match &d.kind {
            DeclKind::Model(_) | DeclKind::Error(_) => At::Decl(d),
            DeclKind::Enum(_) | DeclKind::Union(_) | DeclKind::Const(_) => At::Nothing,
        }
    }

    fn at_of_expr_type(&self, ty: &ExprType) -> At<'a> {
        match ty {
            ExprType::Type(t) => self.at_of_type(t),
            ExprType::Duration => At::Nothing,
        }
    }

    /// The Java code of every prefix of a path and the declaration or page it ends in. The
    /// third value says whether the last step may be a boxed type.
    fn walk(&mut self, path: &PathRef) -> (Vec<String>, bool) {
        let (mut code, mut at, mut boxed) = match &path.root {
            Root::SelfValue => ("v".to_string(), self.at_of_decl(self.this), false),
            Root::Var(name) => {
                let ty = self
                    .vars
                    .iter()
                    .rev()
                    .find(|(n, _)| n == name)
                    .map(|(_, t)| t.clone());
                let at = ty.as_ref().map_or(At::Nothing, |t| self.at_of_expr_type(t));
                (format!("it_{name}"), at, true)
            }
            Root::Param(_) | Root::Fact(_) | Root::Principal | Root::Result => {
                self.unsupported.push(
                    "a rule that reads a parameter, a fact, the principal or the result"
                        .to_string(),
                );
                return (vec!["null".to_string()], false);
            }
        };
        let mut prefixes = vec![code.clone()];
        for step in &path.steps {
            let (accessor, next, step_boxed) = self.step(at, &step.name);
            code = format!("{code}.{accessor}()");
            at = next;
            boxed = step_boxed;
            prefixes.push(code.clone());
        }
        (prefixes, boxed)
    }

    fn step(&mut self, at: At<'a>, name: &str) -> (String, At<'a>, bool) {
        match at {
            At::Decl(d) => {
                let idents = field_idents(d.fields());
                match d.fields().iter().position(|f| f.name == name) {
                    Some(i) => {
                        let f = &d.fields()[i];
                        (idents[i].clone(), self.at_of_type(&f.ty), nullable(f))
                    }
                    None => {
                        self.unsupported
                            .push(format!("a path step `{name}` that is not a field"));
                        (name.to_string(), At::Nothing, false)
                    }
                }
            }
            At::Page => (name.to_string(), At::Nothing, false),
            At::Nothing => {
                self.unsupported
                    .push(format!("a path step `{name}` into a value without members"));
                (name.to_string(), At::Nothing, false)
            }
        }
    }

    /// An operator on two values of a kind whose Java operators are exact.
    fn native(&mut self, op: Op, l: &Printed, r: &Printed) -> Printed {
        let (sym, prec) = match op {
            Op::Eq => ("==", EQ),
            Op::Ne => ("!=", EQ),
            Op::Lt => ("<", CMP),
            Op::Le => ("<=", CMP),
            Op::Gt => (">", CMP),
            Op::Ge => (">=", CMP),
            Op::And
            | Op::Or
            | Op::Implies
            | Op::In
            | Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Rem => return self.refuse("an operator that is not a comparison"),
        };
        Printed::new(format!("{} {sym} {}", l.at(prec), r.at(prec + 1)), prec)
    }

    fn compare(&mut self, op: Op, l: &Printed, r: &Printed, kind: ValueKind) -> Printed {
        let equality = matches!(op, Op::Eq | Op::Ne);
        match kind {
            ValueKind::Bool | ValueKind::Enum if equality => self.native(op, l, r),
            ValueKind::Int
            | ValueKind::Long
            | ValueKind::Float
            | ValueKind::Date
            | ValueKind::Datetime
            | ValueKind::Time
            | ValueKind::Duration => self.native(op, l, r),
            ValueKind::Decimal => {
                let sym = match op {
                    Op::Eq => "==",
                    Op::Ne => "!=",
                    Op::Lt => "<",
                    Op::Le => "<=",
                    Op::Gt => ">",
                    Op::Ge => ">=",
                    Op::And
                    | Op::Or
                    | Op::Implies
                    | Op::In
                    | Op::Add
                    | Op::Sub
                    | Op::Mul
                    | Op::Div
                    | Op::Rem => return self.refuse("an operator that is not a comparison"),
                };
                let prec = if equality { EQ } else { CMP };
                Printed::new(
                    format!("{}.compareTo({}) {sym} 0", l.at(Printed::ATOM), r.at(0)),
                    prec,
                )
            }
            ValueKind::String | ValueKind::Uuid if equality => {
                let call = format!("{}.equals({})", l.at(Printed::ATOM), r.at(0));
                if op == Op::Eq {
                    Printed::atom(call)
                } else {
                    Printed::new(format!("!{call}"), UNARY)
                }
            }
            ValueKind::Bool
            | ValueKind::Enum
            | ValueKind::String
            | ValueKind::Uuid
            | ValueKind::Bytes
            | ValueKind::Json
            | ValueKind::Model
            | ValueKind::List
            | ValueKind::Map
            | ValueKind::Page => self.refuse("a comparison of this type"),
        }
    }

    fn contains(
        &mut self,
        item: &Printed,
        collection: &Printed,
        left: ValueKind,
        right: ValueKind,
    ) -> Printed {
        match right {
            ValueKind::Map => Printed::atom(format!(
                "{}.containsKey({})",
                collection.at(Printed::ATOM),
                item.at(0)
            )),
            ValueKind::List => {
                let helper = match left {
                    ValueKind::Decimal => "inDecimal",
                    ValueKind::Float => "inDouble",
                    ValueKind::Date | ValueKind::Datetime | ValueKind::Time => "inTemporal",
                    ValueKind::Bool
                    | ValueKind::Int
                    | ValueKind::Long
                    | ValueKind::String
                    | ValueKind::Uuid
                    | ValueKind::Duration
                    | ValueKind::Bytes
                    | ValueKind::Json
                    | ValueKind::Enum
                    | ValueKind::Model
                    | ValueKind::List
                    | ValueKind::Map
                    | ValueKind::Page => "in",
                };
                self.call_helper(helper, &[collection, item])
            }
            ValueKind::Bool
            | ValueKind::Int
            | ValueKind::Long
            | ValueKind::Float
            | ValueKind::Decimal
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
            | ValueKind::Page => self.refuse("`in` on this type"),
        }
    }

    fn arith(
        &mut self,
        op: Op,
        l: &Printed,
        r: &Printed,
        left: ValueKind,
        out: ValueKind,
    ) -> Printed {
        let name = match op {
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
            | Op::In => return self.refuse("an operator that is not arithmetic"),
        };
        match out {
            ValueKind::Int => self.call_helper(&format!("{name}Int"), &[l, r]),
            ValueKind::Duration if left == ValueKind::Date && op == Op::Sub => {
                self.call_helper("dateDiff", &[l, r])
            }
            ValueKind::Long
            | ValueKind::Datetime
            | ValueKind::Duration
            | ValueKind::Date
            | ValueKind::Time => self.call_helper(&format!("{name}Long"), &[l, r]),
            ValueKind::Float => self.call_helper(&format!("{name}Double"), &[l, r]),
            ValueKind::Decimal => match op {
                Op::Add => Printed::atom(format!("{}.add({})", l.at(Printed::ATOM), r.at(0))),
                Op::Sub => Printed::atom(format!("{}.subtract({})", l.at(Printed::ATOM), r.at(0))),
                Op::Mul => Printed::atom(format!("{}.multiply({})", l.at(Printed::ATOM), r.at(0))),
                Op::Div
                | Op::Rem
                | Op::And
                | Op::Or
                | Op::Implies
                | Op::Eq
                | Op::Ne
                | Op::Lt
                | Op::Le
                | Op::Gt
                | Op::Ge
                | Op::In => self.refuse("`/` or `%` on decimal"),
            },
            ValueKind::Bool
            | ValueKind::String
            | ValueKind::Uuid
            | ValueKind::Bytes
            | ValueKind::Json
            | ValueKind::Enum
            | ValueKind::Model
            | ValueKind::List
            | ValueKind::Map
            | ValueKind::Page => self.refuse("arithmetic on this type"),
        }
    }

    fn enum_constant(&mut self, decl: DeclId, name: &str) -> Printed {
        let d = self.ir.decl(decl);
        let DeclKind::Enum(e) = &d.kind else {
            return self.refuse("an enum literal of a declaration that is not an enum");
        };
        let consts = enum_constants(e);
        match e.variants.iter().position(|v| v.name == name) {
            Some(i) => {
                let class = self.fc.decl(decl);
                Printed::atom(format!("{class}.{}", consts[i]))
            }
            None => self.refuse("an enum literal that is not a variant"),
        }
    }
}

impl ExprPrinter for JavaExpr<'_, '_, '_> {
    fn lit(&mut self, lit: &ExprLit, ty: &ExprType) -> Printed {
        let kind = self.kind(ty);
        match lit {
            ExprLit::Bool(b) => Printed::atom(b.to_string()),
            ExprLit::Int(n) => match kind {
                ValueKind::Long => number(format!("{n}L"), *n < 0),
                ValueKind::Float => number(java_double(*n as f64), *n < 0),
                ValueKind::Decimal => self.decimal(&n.to_string()),
                ValueKind::Bool
                | ValueKind::Int
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
                | ValueKind::Page => number(n.to_string(), *n < 0),
            },
            ExprLit::Long(n) => number(format!("{n}L"), *n < 0),
            ExprLit::Float(x) => number(java_double(*x), *x < 0.0),
            ExprLit::Decimal(text) => self.decimal(text),
            ExprLit::String(s) => Printed::atom(java_str(s)),
            ExprLit::Enum(e) => self.enum_constant(e.decl, &e.name),
        }
    }

    fn path(&mut self, path: &PathRef, ty: &ExprType) -> Printed {
        let (prefixes, boxed) = self.walk(path);
        let code = prefixes.last().cloned().unwrap_or_default();
        match self.kind(ty) {
            ValueKind::Datetime => self.call_helper("instantMs", &[&Printed::atom(code)]),
            ValueKind::Date => self.call_helper("dateDays", &[&Printed::atom(code)]),
            ValueKind::Time => self.call_helper("timeMs", &[&Printed::atom(code)]),
            ValueKind::Int if boxed => Printed::new(format!("(int) {code}"), UNARY),
            ValueKind::Long if boxed => Printed::new(format!("(long) {code}"), UNARY),
            ValueKind::Float if boxed => Printed::new(format!("(double) {code}"), UNARY),
            ValueKind::Bool if boxed => Printed::new(format!("(boolean) {code}"), UNARY),
            ValueKind::Bool
            | ValueKind::Int
            | ValueKind::Long
            | ValueKind::Float
            | ValueKind::Decimal
            | ValueKind::String
            | ValueKind::Uuid
            | ValueKind::Duration
            | ValueKind::Bytes
            | ValueKind::Json
            | ValueKind::Enum
            | ValueKind::Model
            | ValueKind::List
            | ValueKind::Map
            | ValueKind::Page => Printed::atom(code),
        }
    }

    fn has(&mut self, path: &PathRef) -> Printed {
        let (prefixes, _) = self.walk(path);
        let mut checks: Vec<String> = Vec::new();
        let mut at = match &path.root {
            Root::SelfValue => self.at_of_decl(self.this),
            Root::Var(name) => {
                let ty = self
                    .vars
                    .iter()
                    .rev()
                    .find(|(n, _)| n == name)
                    .map(|(_, t)| t.clone());
                ty.as_ref().map_or(At::Nothing, |t| self.at_of_expr_type(t))
            }
            Root::Param(_) | Root::Fact(_) | Root::Principal | Root::Result => At::Nothing,
        };
        for (i, step) in path.steps.iter().enumerate() {
            let (_, next, _) = self.step(at, &step.name);
            let declared_optional = match at {
                At::Decl(d) => d.fields().iter().any(|f| f.name == step.name && f.optional),
                At::Page | At::Nothing => false,
            };
            if step.optional || declared_optional {
                checks.push(format!("{} != null", prefixes[i + 1]));
            }
            at = next;
        }
        match checks.len() {
            0 => Printed::atom("true"),
            1 => Printed::new(checks.remove(0), EQ),
            _ => Printed::new(checks.join(" && "), AND),
        }
    }

    fn now(&mut self) -> Printed {
        self.refuse("`now`")
    }

    fn not(&mut self, operand: Printed) -> Printed {
        Printed::new(format!("!{}", operand.at(UNARY)), UNARY)
    }

    fn neg(&mut self, operand: Printed, ty: &ExprType) -> Printed {
        match self.kind(ty) {
            ValueKind::Int => self.call_helper("negInt", &[&operand]),
            ValueKind::Long | ValueKind::Duration => self.call_helper("negLong", &[&operand]),
            ValueKind::Float => Printed::new(format!("-{}", operand.at(UNARY + 1)), UNARY),
            ValueKind::Decimal => Printed::atom(format!("{}.negate()", operand.at(Printed::ATOM))),
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
            | ValueKind::Page => self.refuse("unary `-` on this type"),
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
            Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => self.compare(op, &l, &r, lk),
            Op::In => self.contains(&l, &r, lk, rk),
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem => self.arith(op, &l, &r, lk, ok),
        }
    }

    fn call(
        &mut self,
        f: Builtin,
        args: Vec<Printed>,
        arg_types: &[&ExprType],
        _out: &ExprType,
    ) -> Printed {
        let first = arg_types.first().map_or(ValueKind::Json, |t| self.kind(t));
        let refs: Vec<&Printed> = args.iter().collect();
        let method = |name: &str, args: &[Printed]| match args {
            [a, b] => Printed::atom(format!("{}.{name}({})", a.at(Printed::ATOM), b.at(0))),
            _ => Printed::atom("false"),
        };
        match (f, args.as_slice()) {
            (Builtin::Len, [a]) => match first {
                ValueKind::String => self.call_helper("len", &[a]),
                ValueKind::List | ValueKind::Map => {
                    Printed::atom(format!("{}.size()", a.at(Printed::ATOM)))
                }
                ValueKind::Bytes => Printed::atom(format!("{}.length", a.at(Printed::ATOM))),
                ValueKind::Bool
                | ValueKind::Int
                | ValueKind::Long
                | ValueKind::Float
                | ValueKind::Decimal
                | ValueKind::Uuid
                | ValueKind::Date
                | ValueKind::Datetime
                | ValueKind::Time
                | ValueKind::Duration
                | ValueKind::Json
                | ValueKind::Page
                | ValueKind::Enum
                | ValueKind::Model => self.refuse("`len` of this type"),
            },
            (Builtin::Matches, [_, _]) => self.call_helper("matches", &refs),
            (Builtin::StartsWith, [_, _]) => method("startsWith", &args),
            (Builtin::EndsWith, [_, _]) => method("endsWith", &args),
            (Builtin::Contains, [_, _]) => method("contains", &args),
            (Builtin::Days, [_]) => self.call_helper("days", &refs),
            (Builtin::Hours, [_]) => self.call_helper("hours", &refs),
            (Builtin::Minutes, [_]) => self.call_helper("minutes", &refs),
            (Builtin::Seconds, [_]) => self.call_helper("seconds", &refs),
            (Builtin::Ms, [_]) => self.call_helper("ms", &refs),
            (
                Builtin::Len
                | Builtin::Matches
                | Builtin::StartsWith
                | Builtin::EndsWith
                | Builtin::Contains
                | Builtin::Days
                | Builtin::Hours
                | Builtin::Minutes
                | Builtin::Seconds
                | Builtin::Ms,
                _,
            ) => self.refuse("a built-in function with the wrong number of arguments"),
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
        let lambda = Printed::atom(format!("it_{var} -> {}", body.code));
        let helper = match q {
            Quantifier::All => "all",
            Quantifier::Any => "any",
            Quantifier::Count => "count",
            Quantifier::Sum => match self.kind(body_ty) {
                ValueKind::Int => "sumInt",
                ValueKind::Long | ValueKind::Duration => "sumLong",
                ValueKind::Float => "sumDouble",
                ValueKind::Decimal => "sumDecimal",
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
                | ValueKind::Page => return self.refuse("`sum` of this type"),
            },
        };
        self.call_helper(helper, &[&list, &lambda])
    }

    fn list(&mut self, items: Vec<Printed>, _elem: Option<&ExprType>) -> Printed {
        let list = self.fc.name("java.util.List");
        let items: Vec<String> = items.iter().map(|i| i.at(0)).collect();
        Printed::atom(format!("{list}.of({})", items.join(", ")))
    }

    fn widen(&mut self, operand: Printed, from: &ExprType, to: &ExprType) -> Printed {
        match (self.kind(from), self.kind(to)) {
            (ValueKind::Int, ValueKind::Long) => {
                Printed::new(format!("(long) {}", operand.at(UNARY)), UNARY)
            }
            (ValueKind::Int, ValueKind::Float) => {
                Printed::new(format!("(double) {}", operand.at(UNARY)), UNARY)
            }
            (ValueKind::Int | ValueKind::Long, ValueKind::Decimal) => {
                let big = self.fc.name("java.math.BigDecimal");
                Printed::atom(format!("{big}.valueOf({})", operand.at(0)))
            }
            _ => self.refuse("a conversion that is not an exact widening"),
        }
    }

    fn old(&mut self, _operand: Printed, _ty: &ExprType) -> Printed {
        self.refuse("`old`")
    }
}

impl JavaExpr<'_, '_, '_> {
    fn decimal(&mut self, text: &str) -> Printed {
        let big = self.fc.name("java.math.BigDecimal");
        Printed::atom(format!("new {big}({})", java_str(text)))
    }
}

/// A numeric literal: negative ones bind like a unary minus.
fn number(code: String, negative: bool) -> Printed {
    if negative {
        Printed::new(code, UNARY)
    } else {
        Printed::atom(code)
    }
}

#[cfg(test)]
mod tests;
