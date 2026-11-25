//! Expressions. Every node becomes a pair: its value, and the condition under which evaluating
//! it is an error (`spec/rules.md` section 3.7). The error condition follows short-circuiting, so
//! `x == 0 || 1 / x > 2` is not an error where `x` is 0. A rule holds when its error condition is
//! false and its value is true.

use veld_ir::{
    BinExpr, Builtin, CallExpr, Expr, ExprKind, ExprLit, ExprType, Op, QuantExpr, Quantifier,
    Scalar, Type,
};
use veld_rules::helpers::instant::MS_PER_DAY;

use crate::problem::Problem;
use crate::smt;
use crate::sorts;
use crate::types::{EncodeError, OpenPart, Sort, unsupported};

/// The value of an expression and the condition under which it is an evaluation error.
#[derive(Debug, Clone)]
pub struct Pair {
    pub v: String,
    pub err: String,
}

impl Pair {
    fn ok(v: String) -> Pair {
        Pair {
            v,
            err: "false".to_owned(),
        }
    }
}

impl Problem<'_> {
    /// Encodes `e`.
    ///
    /// # Errors
    /// [`EncodeError::Unsupported`] for a construct that has no place in a question (`old`, a
    /// lambda variable outside a comprehension).
    pub fn encode(&mut self, e: &Expr) -> Result<Pair, EncodeError> {
        match &e.e {
            ExprKind::Lit(l) => self.literal_value(l, &e.t),
            ExprKind::Path(p) => Ok(Pair::ok(self.read_path(p, &e.t)?)),
            ExprKind::Now(_) => Ok(Pair::ok(self.now_value())),
            ExprKind::Not(x) => {
                let x = self.encode(x)?;
                Ok(Pair {
                    v: smt::not(&x.v),
                    err: x.err,
                })
            }
            ExprKind::Neg(x) => self.negate(e, x),
            ExprKind::Bin(b) => self.binary(e, b),
            ExprKind::Has(p) => Ok(Pair::ok(self.has_term(p)?)),
            ExprKind::Call(c) => self.call(e, c),
            ExprKind::Quant(q) => self.quantifier(e, q),
            ExprKind::Widen(x) => self.widen(e, x),
            ExprKind::List(_) => unsupported("a list outside `in`"),
            ExprKind::Old(_) => unsupported("`old(...)`"),
        }
    }

    /// The term for a whole rule: true when it holds, so with no error and a true value.
    ///
    /// # Errors
    /// As [`Problem::encode`].
    pub fn holds(&mut self, e: &Expr) -> Result<String, EncodeError> {
        let p = self.encode(e)?;
        Ok(smt::and2(&smt::not(&p.err), &p.v))
    }

    fn sort_of(&self, t: &ExprType) -> Result<Sort, EncodeError> {
        match sorts::sort_of_expr(self.ir, t) {
            Some(s) => Ok(s),
            None => unsupported("an expression of a type that has no value in a question"),
        }
    }

    /// A named definition of `body`, unless it is already an atom.
    fn atom(&mut self, sort: Sort, body: String) -> String {
        if body.contains(' ') {
            self.define(sort, &body)
        } else {
            body
        }
    }

    fn literal_value(&mut self, l: &ExprLit, t: &ExprType) -> Result<Pair, EncodeError> {
        let v = match l {
            ExprLit::Bool(b) => b.to_string(),
            ExprLit::Int(n) => self.int_literal(i128::from(*n), t),
            ExprLit::Long(n) => smt::int(i128::from(*n)),
            ExprLit::Float(x) => self.float_literal(*x),
            ExprLit::Decimal(text) => smt::real_text(text),
            ExprLit::String(s) => self.literal(Sort::Str, s),
            ExprLit::Enum(e) => {
                let variants = self.enum_wires(e.decl);
                let idx = variants.iter().position(|v| *v == e.wire);
                smt::int(idx.map_or(-1, |i| i as i128))
            }
        };
        Ok(Pair::ok(v))
    }

    fn int_literal(&mut self, n: i128, t: &ExprType) -> String {
        if sorts::is_float(t) && n.abs() > (1 << 53) {
            return self.open_value(OpenPart::Float, Sort::Real, None);
        }
        if sorts::is_float(t) || sorts::is_decimal(t) {
            smt::real_int(n)
        } else {
            smt::int(n)
        }
    }

    /// A float that is a whole number below 2^53 is exact; any other is left open.
    fn float_literal(&mut self, x: f64) -> String {
        if x.is_finite() && x.fract() == 0.0 && x.abs() < 9.0e15 {
            smt::real_int(x as i128)
        } else {
            self.open_value(OpenPart::Float, Sort::Real, None)
        }
    }

    fn negate(&mut self, e: &Expr, x: &Expr) -> Result<Pair, EncodeError> {
        let x = self.encode(x)?;
        let v = self.atom(self.sort_of(&e.t)?, format!("(- {})", x.v));
        let err = match sorts::checked_range(&e.t) {
            Some((lo, hi)) => smt::or2(&x.err, &smt::outside(&v, lo, hi)),
            None => x.err,
        };
        Ok(Pair { v, err })
    }

    fn widen(&mut self, e: &Expr, x: &Expr) -> Result<Pair, EncodeError> {
        let x = self.encode(x)?;
        let v = match self.sort_of(&e.t)? {
            Sort::Real => self.atom(Sort::Real, format!("(to_real {})", x.v)),
            Sort::Int | Sort::Bool | Sort::Str | Sort::Uuid => x.v,
        };
        Ok(Pair { v, err: x.err })
    }

    fn binary(&mut self, e: &Expr, b: &BinExpr) -> Result<Pair, EncodeError> {
        match b.op {
            Op::And | Op::Or | Op::Implies => self.logic(b),
            Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => self.compare(b),
            Op::In => self.member(b),
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem => self.arith(e, b),
        }
    }

    /// `&&`, `||` and `implies` short-circuit: the right side is evaluated only when it can
    /// still change the result, so only then can it be an error.
    fn logic(&mut self, b: &BinExpr) -> Result<Pair, EncodeError> {
        let l = self.encode(&b.l)?;
        let r = self.encode(&b.r)?;
        let (value, reached) = match b.op {
            Op::And => (smt::and2(&l.v, &r.v), l.v.clone()),
            Op::Or => (smt::or2(&l.v, &r.v), smt::not(&l.v)),
            Op::Implies => (smt::or2(&smt::not(&l.v), &r.v), l.v.clone()),
            Op::Eq
            | Op::Ne
            | Op::Lt
            | Op::Le
            | Op::Gt
            | Op::Ge
            | Op::In
            | Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Rem => return unsupported("an operator that is not logic"),
        };
        let err = smt::or2(&l.err, &smt::and2(&reached, &r.err));
        Ok(Pair {
            v: self.atom(Sort::Bool, value),
            err: self.atom(Sort::Bool, err),
        })
    }

    fn compare(&mut self, b: &BinExpr) -> Result<Pair, EncodeError> {
        let l = self.encode(&b.l)?;
        let r = self.encode(&b.r)?;
        let (l, r) = self.same_sort(&b.l, &l, &b.r, &r)?;
        let body = match b.op {
            Op::Eq => format!("(= {} {})", l.v, r.v),
            Op::Ne => format!("(not (= {} {}))", l.v, r.v),
            Op::Lt => format!("(< {} {})", l.v, r.v),
            Op::Le => format!("(<= {} {})", l.v, r.v),
            Op::Gt => format!("(> {} {})", l.v, r.v),
            Op::Ge => format!("(>= {} {})", l.v, r.v),
            Op::And
            | Op::Or
            | Op::Implies
            | Op::In
            | Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Rem => return unsupported("an operator that does not compare"),
        };
        let v = self.atom(Sort::Bool, body);
        Ok(Pair {
            v,
            err: smt::or2(&l.err, &r.err),
        })
    }

    /// Both sides as one sort: an integer next to a real is converted.
    fn same_sort(
        &mut self,
        le: &Expr,
        l: &Pair,
        re: &Expr,
        r: &Pair,
    ) -> Result<(Pair, Pair), EncodeError> {
        let (ls, rs) = (self.sort_of(&le.t)?, self.sort_of(&re.t)?);
        let conv = |me: &mut Self, p: &Pair| Pair {
            v: me.atom(Sort::Real, format!("(to_real {})", p.v)),
            err: p.err.clone(),
        };
        Ok(match (ls, rs) {
            (Sort::Int, Sort::Real) => (conv(self, l), r.clone()),
            (Sort::Real, Sort::Int) => (l.clone(), conv(self, r)),
            _ => (l.clone(), r.clone()),
        })
    }

    /// `x in [a, b]` is a disjunction. Membership in anything else is left open.
    fn member(&mut self, b: &BinExpr) -> Result<Pair, EncodeError> {
        let needle = self.encode(&b.l)?;
        let ExprKind::List(items) = &b.r.e else {
            let v = self.open_value(OpenPart::Membership, Sort::Bool, None);
            return Ok(Pair { v, err: needle.err });
        };
        let (mut hits, mut errs) = (Vec::new(), vec![needle.err.clone()]);
        for item in items {
            let p = self.encode(item)?;
            let (n, p2) = self.same_sort(&b.l, &needle, item, &p)?;
            hits.push(format!("(= {} {})", n.v, p2.v));
            errs.push(p.err);
        }
        let v = self.atom(Sort::Bool, smt::or_all(&hits));
        Ok(Pair {
            v,
            err: self.atom(Sort::Bool, smt::or_all(&errs)),
        })
    }

    fn arith(&mut self, e: &Expr, b: &BinExpr) -> Result<Pair, EncodeError> {
        let l = self.encode(&b.l)?;
        let r = self.encode(&b.r)?;
        let err = smt::or2(&l.err, &r.err);
        if sorts::is_float(&e.t) {
            let v = self.open_value(OpenPart::Float, Sort::Real, None);
            return Ok(Pair { v, err });
        }
        if self.sort_of(&e.t)? == Sort::Real {
            let op = match b.op {
                Op::Add => "+",
                Op::Sub => "-",
                Op::Mul => "*",
                Op::Div | Op::Rem => return unsupported("`/` and `%` on decimal"),
                Op::And
                | Op::Or
                | Op::Implies
                | Op::Eq
                | Op::Ne
                | Op::Lt
                | Op::Le
                | Op::Gt
                | Op::Ge
                | Op::In => return unsupported("an operator that is not arithmetic"),
            };
            let v = self.atom(Sort::Real, format!("({op} {} {})", l.v, r.v));
            return Ok(Pair { v, err });
        }
        self.int_arith(e, b, &l, &r, err)
    }

    fn int_arith(
        &mut self,
        e: &Expr,
        b: &BinExpr,
        l: &Pair,
        r: &Pair,
        err: String,
    ) -> Result<Pair, EncodeError> {
        let (v, extra) = match b.op {
            Op::Add => (format!("(+ {} {})", l.v, r.v), None),
            Op::Sub if is_date(&b.l.t) => (format!("(* (- {} {}) {MS_PER_DAY})", l.v, r.v), None),
            Op::Sub => (format!("(- {} {})", l.v, r.v), None),
            Op::Mul => (format!("(* {} {})", l.v, r.v), None),
            Op::Div | Op::Rem => {
                let q = self.define(Sort::Int, &smt::trunc_div(&l.v, &r.v));
                let zero = format!("(= {} 0)", r.v);
                if b.op == Op::Div {
                    (q, Some(zero))
                } else {
                    (format!("(- {} (* {} {q}))", l.v, r.v), Some(zero))
                }
            }
            Op::And
            | Op::Or
            | Op::Implies
            | Op::Eq
            | Op::Ne
            | Op::Lt
            | Op::Le
            | Op::Gt
            | Op::Ge
            | Op::In => return unsupported("an operator that is not arithmetic"),
        };
        let v = self.atom(Sort::Int, v);
        let mut err = err;
        if let Some(z) = extra {
            err = smt::or2(&err, &z);
        }
        if b.op != Op::Rem
            && let Some((lo, hi)) = sorts::checked_range(&e.t)
        {
            err = smt::or2(&err, &smt::outside(&v, lo, hi));
        }
        Ok(Pair {
            v,
            err: self.atom(Sort::Bool, err),
        })
    }

    fn call(&mut self, e: &Expr, c: &CallExpr) -> Result<Pair, EncodeError> {
        match c.f {
            Builtin::Len => match c.args.as_slice() {
                [arg] => Ok(Pair::ok(self.len_term(arg)?)),
                _ => unsupported("`len` with the wrong number of arguments"),
            },
            Builtin::Matches => Ok(Pair::ok(self.open_bool(OpenPart::Matches))),
            Builtin::StartsWith | Builtin::EndsWith | Builtin::Contains => {
                Ok(Pair::ok(self.open_bool(OpenPart::StringContent)))
            }
            Builtin::Days | Builtin::Hours | Builtin::Minutes | Builtin::Seconds | Builtin::Ms => {
                self.duration(e, c)
            }
        }
    }

    fn open_bool(&mut self, part: OpenPart) -> String {
        self.open_value(part, Sort::Bool, None)
    }

    fn duration(&mut self, e: &Expr, c: &CallExpr) -> Result<Pair, EncodeError> {
        let factor: i128 = match c.f {
            Builtin::Days => 86_400_000,
            Builtin::Hours => 3_600_000,
            Builtin::Minutes => 60_000,
            Builtin::Seconds => 1_000,
            Builtin::Ms
            | Builtin::Len
            | Builtin::Matches
            | Builtin::StartsWith
            | Builtin::EndsWith
            | Builtin::Contains => 1,
        };
        let [arg] = c.args.as_slice() else {
            return unsupported("a duration function with the wrong number of arguments");
        };
        let a = self.encode(arg)?;
        let v = self.atom(Sort::Int, format!("(* {} {factor})", a.v));
        let (lo, hi) = sorts::checked_range(&e.t).unwrap_or(sorts::I64);
        let err = smt::or2(&a.err, &smt::outside(&v, lo, hi));
        Ok(Pair { v, err })
    }

    /// A comprehension is left open: an unconstrained value of its type.
    fn quantifier(&mut self, e: &Expr, q: &QuantExpr) -> Result<Pair, EncodeError> {
        let sort = self.sort_of(&e.t)?;
        let range = match q.q {
            Quantifier::Count => Some((0, sorts::I32.1)),
            Quantifier::Sum => sorts::checked_range(&e.t),
            Quantifier::All | Quantifier::Any => None,
        };
        if q.q == Quantifier::Sum {
            self.open.insert(OpenPart::Sum);
        }
        let v = self.open_value(OpenPart::Comprehension, sort, range);
        Ok(Pair::ok(v))
    }
}

fn is_date(t: &ExprType) -> bool {
    matches!(t, ExprType::Type(Type::Scalar(Scalar::Date)))
}
