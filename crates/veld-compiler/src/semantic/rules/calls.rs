//! Built-in functions and comprehensions (`spec/rules.md` section 3.5).

use super::checker::{Checker, Phase};
use super::paths::flatten;
use super::types::{as_scalar, boolean, is_bool, is_numeric, is_string, list_elem, node, scalar};
use crate::diagnostics::{Diagnostic, Span};
use crate::semantic::regex;
use crate::syntax::ast::{self, CallArg, ExprKind as A};
use veld_ir::rules::{Builtin, CallExpr, Expr, ExprKind, ExprLit, ExprType, QuantExpr, Quantifier};
use veld_ir::{Scalar, Type};

const FUNCTIONS: [&str; 15] = [
    "has",
    "len",
    "matches",
    "startsWith",
    "endsWith",
    "contains",
    "days",
    "hours",
    "minutes",
    "seconds",
    "ms",
    "all",
    "any",
    "count",
    "sum",
];

fn quantifier(name: &str) -> Option<Quantifier> {
    Some(match name {
        "all" => Quantifier::All,
        "any" => Quantifier::Any,
        "count" => Quantifier::Count,
        "sum" => Quantifier::Sum,
        _ => return None,
    })
}

fn string_function(name: &str) -> Option<Builtin> {
    Some(match name {
        "startsWith" => Builtin::StartsWith,
        "endsWith" => Builtin::EndsWith,
        "contains" => Builtin::Contains,
        _ => return None,
    })
}

fn duration_function(name: &str) -> Option<Builtin> {
    Some(match name {
        "days" => Builtin::Days,
        "hours" => Builtin::Hours,
        "minutes" => Builtin::Minutes,
        "seconds" => Builtin::Seconds,
        "ms" => Builtin::Ms,
        _ => return None,
    })
}

impl Checker<'_, '_, '_> {
    pub fn call(&mut self, name: &ast::Ident, args: &[CallArg], span: Span) -> Option<Expr> {
        let n = name.name.as_str();
        if let Some(q) = quantifier(n) {
            return self.quant(q, name, args, span);
        }
        if let Some(f) = string_function(n) {
            return self.string_call(f, name, args, span);
        }
        if let Some(f) = duration_function(n) {
            return self.duration_call(f, name, args, span);
        }
        match n {
            "has" => self.has_call(name, args, span),
            "len" => self.len_call(name, args, span),
            "matches" => self.matches_call(name, args, span),
            "old" => self.old_call(name, args, span),
            _ => self.unknown_function(name),
        }
    }

    /// `old(e)`: the value of `e` before the handler ran (`spec/rules.md` section 9.4).
    fn old_call(&mut self, name: &ast::Ident, args: &[CallArg], span: Span) -> Option<Expr> {
        let state = self.ep.as_ref().map(|e| (e.phase, e.in_old));
        let msg = match state {
            None => {
                "`old(...)` is not available in an invariant: there is no operation to compare with"
            }
            Some((phase, _)) if phase != Phase::Ensure => {
                "`old(...)` is only available in `ensure`"
            }
            Some((_, true)) => "`old(...)` cannot be nested",
            Some((_, false)) => "",
        };
        if !msg.is_empty() {
            self.err("E0904", msg, name.span);
            return None;
        }
        let [arg] = self.plain_args(name, args, 1, span)?[..] else {
            return None;
        };
        self.set_in_old(true);
        let inner = self.expr(arg, None);
        self.set_in_old(false);
        let inner = inner?;
        Some(node(inner.t.clone(), ExprKind::Old(Box::new(inner))))
    }

    fn set_in_old(&mut self, on: bool) {
        if let Some(ep) = &mut self.ep {
            ep.in_old = on;
        }
    }

    fn unknown_function(&mut self, name: &ast::Ident) -> Option<Expr> {
        let help = if matches!(name.name.as_str(), "min" | "max") {
            "`min` and `max` are not part of rules v1; `all(items, i => i.qty <= 10)` states a bound"
                .to_string()
        } else {
            format!("functions: {}", FUNCTIONS.join(", "))
        };
        let diag = Diagnostic::error(
            "E0906",
            format!("unknown function `{}`", name.name),
            name.span,
        );
        let diag = if help.starts_with("`min`") {
            diag.with_help(help)
        } else {
            diag.did_you_mean(&name.name, FUNCTIONS.iter().copied(), Some(help))
        };
        self.report(diag);
        None
    }

    /// Checks the argument count and rejects a lambda; returns the plain expressions.
    fn plain_args<'e>(
        &mut self,
        name: &ast::Ident,
        args: &'e [CallArg],
        count: usize,
        span: Span,
    ) -> Option<Vec<&'e ast::Expr>> {
        if args.len() != count {
            let s = if count == 1 { "" } else { "s" };
            self.err(
                "E0906",
                format!(
                    "`{}` takes {count} argument{s}, found {}",
                    name.name,
                    args.len()
                ),
                span,
            );
            return None;
        }
        let mut out = Vec::new();
        for a in args {
            match a {
                CallArg::Expr(e) => out.push(e),
                CallArg::Lambda { span, .. } => {
                    self.err(
                        "E0906",
                        format!("`{}` does not take a lambda", name.name),
                        *span,
                    );
                    return None;
                }
            }
        }
        Some(out)
    }

    fn has_call(&mut self, name: &ast::Ident, args: &[CallArg], span: Span) -> Option<Expr> {
        let [arg] = self.plain_args(name, args, 1, span)?[..] else {
            return None;
        };
        if flatten(arg).is_none() {
            self.err(
                "E0906",
                "`has` takes a field path such as `has(address.zip)`",
                arg.span,
            );
            return None;
        }
        self.typed_path(arg, true).map(|t| t.expr)
    }

    fn len_call(&mut self, name: &ast::Ident, args: &[CallArg], span: Span) -> Option<Expr> {
        let [arg] = self.plain_args(name, args, 1, span)?[..] else {
            return None;
        };
        let a = self.expr(arg, None)?;
        let sized = is_string(&a.t)
            || as_scalar(&a.t) == Some(Scalar::Bytes)
            || matches!(&a.t, ExprType::Type(Type::List(_) | Type::Map(_)));
        if !sized {
            let shown = self.tname(&a.t);
            self.err(
                "E0906",
                format!("`len` takes a string, list, map or `bytes`, found `{shown}`"),
                arg.span,
            );
            return None;
        }
        Some(call(Builtin::Len, scalar(Scalar::Int), vec![a]))
    }

    fn string_call(
        &mut self,
        f: Builtin,
        name: &ast::Ident,
        args: &[CallArg],
        span: Span,
    ) -> Option<Expr> {
        let [s, p] = self.plain_args(name, args, 2, span)?[..] else {
            return None;
        };
        let (a, b) = (self.string_arg(name, s), self.string_arg(name, p));
        Some(call(f, boolean(), vec![a?, b?]))
    }

    fn string_arg(&mut self, name: &ast::Ident, e: &ast::Expr) -> Option<Expr> {
        let a = self.expr(e, None)?;
        if is_string(&a.t) {
            return Some(a);
        }
        let shown = self.tname(&a.t);
        self.err(
            "E0906",
            format!("`{}` takes strings, found `{shown}`", name.name),
            e.span,
        );
        None
    }

    fn matches_call(&mut self, name: &ast::Ident, args: &[CallArg], span: Span) -> Option<Expr> {
        let [s, re] = self.plain_args(name, args, 2, span)?[..] else {
            return None;
        };
        let subject = self.string_arg(name, s);
        let A::Str(pattern) = &re.kind else {
            self.err(
                "E0906",
                "the second argument of `matches` must be a string literal",
                re.span,
            );
            return None;
        };
        if let Err(reason) = regex::check(pattern) {
            self.report(
                Diagnostic::error(
                    "E0603",
                    format!("pattern `{pattern}` is outside the portable regex subset: {reason}"),
                    re.span,
                )
                .with_help("allowed: literals, `.`, `[...]`, `\\d \\w \\s`, `* + ? {n,m}`, `( )`, `|`, `^ $`"),
            );
            return None;
        }
        let lit = node(
            scalar(Scalar::String),
            ExprKind::Lit(ExprLit::String(pattern.clone())),
        );
        Some(call(Builtin::Matches, boolean(), vec![subject?, lit]))
    }

    fn duration_call(
        &mut self,
        f: Builtin,
        name: &ast::Ident,
        args: &[CallArg],
        span: Span,
    ) -> Option<Expr> {
        let [n] = self.plain_args(name, args, 1, span)?[..] else {
            return None;
        };
        let a = self.expr(n, None)?;
        if !matches!(as_scalar(&a.t), Some(Scalar::Int | Scalar::Long)) {
            let shown = self.tname(&a.t);
            self.err(
                "E0906",
                format!(
                    "`{}` takes an `int` or a `long`, found `{shown}`",
                    name.name
                ),
                n.span,
            );
            return None;
        }
        Some(call(f, ExprType::Duration, vec![a]))
    }

    fn quant(
        &mut self,
        q: Quantifier,
        name: &ast::Ident,
        args: &[CallArg],
        span: Span,
    ) -> Option<Expr> {
        let [CallArg::Expr(list), CallArg::Lambda { var, body, .. }] = args else {
            self.err(
                "E0906",
                format!(
                    "`{}` takes a list and a lambda: `{}(list, x => ...)`",
                    name.name, name.name
                ),
                span,
            );
            return None;
        };
        let (typed, bounded) = self.quant_list(name, list)?;
        let elem = list_elem(&typed.t)?;
        self.check_shadow(var)?;
        self.check_cost(bounded, span);
        self.lambdas.push((var.name.clone(), elem));
        self.bounded.push(bounded);
        let body_t = self.quant_body(q, body);
        self.bounded.pop();
        self.lambdas.pop();
        let body_t = body_t?;
        let t = match q {
            Quantifier::All | Quantifier::Any => boolean(),
            Quantifier::Count => scalar(Scalar::Int),
            Quantifier::Sum => body_t.t.clone(),
        };
        Some(node(
            t,
            ExprKind::Quant(Box::new(QuantExpr {
                q,
                list: typed,
                var: var.name.clone(),
                body: body_t,
            })),
        ))
    }

    fn quant_list(&mut self, name: &ast::Ident, list: &ast::Expr) -> Option<(Expr, bool)> {
        let (typed, bounded) = if flatten(list).is_some() {
            let t = self.typed_path(list, false)?;
            (t.expr, t.bounded)
        } else {
            let e = self.expr(list, None)?;
            let bounded = matches!(e.e, ExprKind::List(_));
            (e, bounded)
        };
        if list_elem(&typed.t).is_none() {
            let shown = self.tname(&typed.t);
            self.err(
                "E0906",
                format!("`{}` takes a list, found `{shown}`", name.name),
                list.span,
            );
            return None;
        }
        Some((typed, bounded))
    }

    fn quant_body(&mut self, q: Quantifier, body: &ast::Expr) -> Option<Expr> {
        if q == Quantifier::Sum {
            let b = self.expr(body, None)?;
            if is_numeric(&b.t) {
                return Some(b);
            }
            let shown = self.tname(&b.t);
            self.err(
                "E0902",
                format!("the lambda of `sum` must give a number, found `{shown}`"),
                body.span,
            );
            return None;
        }
        let b = self.expr(body, Some(&boolean()))?;
        if is_bool(&b.t) {
            return Some(b);
        }
        let shown = self.tname(&b.t);
        self.err(
            "E0902",
            format!("the lambda must give a `bool`, found `{shown}`"),
            body.span,
        );
        None
    }

    /// A lambda variable must not shadow the local scope or an enclosing lambda.
    fn check_shadow(&mut self, var: &ast::Ident) -> Option<()> {
        let what = if self.lambdas.iter().any(|(v, _)| v == &var.name) {
            "an enclosing lambda variable"
        } else if self.fields.iter().any(|f| f.name == var.name) {
            "a field of this declaration"
        } else {
            return Some(());
        };
        self.report(
            Diagnostic::error(
                "E0901",
                format!("lambda variable `{}` shadows {what}", var.name),
                var.span,
            )
            .with_help("choose another name"),
        );
        None
    }

    /// `W0902`: a comprehension inside another one over a list without `@length(max: n)`.
    fn check_cost(&mut self, bounded: bool, span: Span) {
        if self.bounded.is_empty() || (bounded && self.bounded.iter().all(|b| *b)) {
            return;
        }
        self.report(
            Diagnostic::warning(
                "W0902",
                "nested comprehension over a list without `@length(max: n)`",
                span,
            )
            .with_help(
                "the cost grows with the product of both lists; bound them with `@length(max: n)`",
            ),
        );
    }
}

fn call(f: Builtin, t: ExprType, args: Vec<Expr>) -> Expr {
    node(t, ExprKind::Call(CallExpr { f, args }))
}
