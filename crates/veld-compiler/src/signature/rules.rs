//! Rule identity, exactly `spec/rules.md` section 6.
//!
//! `rule_id = "veld:" package "/" Decl "." ruleName "@" hex16( sha256( rule_bytes ) )` with
//! `rule_bytes` the RFC 8785 JSON of `{ "d": qualified name, "n": rule name, "e": ExprShape }`.
//! `ExprShape` is the `Expr` JSON made independent of anything that can change without changing
//! the rule's meaning: declaration ids, source names of fields, lambda variable names and the
//! formatting of numbers. Messages, docs, annotations and spans are not part of it.

use veld_ir::rules::{Builtin, Expr, ExprKind, ExprLit, ExprType, Op, PathRef, Quantifier, Root};
use veld_ir::{Decl, Package};

use super::{Json, canonical, hex16, obj, text, tshape};

/// Fills the `id` of every rule of every declaration. An inherited rule has the id it has in
/// the declaration that wrote it, so the id is computed from the declarer in both cases.
pub fn rule_ids(decls: &mut [Decl], packages: &[Package]) {
    let ids: Vec<Vec<String>> = decls
        .iter()
        .enumerate()
        .map(|(i, d)| {
            d.rules()
                .iter()
                .map(|r| {
                    let from = r.inherited_from.map_or(i, veld_ir::DeclId::index);
                    rule_id(decls, packages, from, &r.name, &r.expr)
                })
                .collect()
        })
        .collect();
    for (d, ids) in decls.iter_mut().zip(ids) {
        for (rule, id) in rules_mut(d).iter_mut().zip(ids) {
            rule.id = id;
        }
    }
}

fn rules_mut(d: &mut Decl) -> &mut [veld_ir::rules::Rule] {
    match &mut d.kind {
        veld_ir::DeclKind::Model(m) => &mut m.rules,
        veld_ir::DeclKind::Error(e) => &mut e.fields.rules,
        veld_ir::DeclKind::Enum(_) | veld_ir::DeclKind::Union(_) | veld_ir::DeclKind::Const(_) => {
            &mut []
        }
    }
}

/// The id of rule `name` with expression `expr`, written by `decls[declarer]`.
#[must_use]
pub fn rule_id(
    decls: &[Decl],
    packages: &[Package],
    declarer: usize,
    name: &str,
    expr: &Expr,
) -> String {
    let d = &decls[declarer];
    format!(
        "veld:{}/{}.{name}@{}",
        packages[d.package.index()].name,
        d.name,
        hex16(rule_bytes(decls, declarer, name, expr).as_bytes())
    )
}

/// The `rule_bytes` of `spec/rules.md` section 6: the canonical JSON that the id hashes.
#[must_use]
pub fn rule_bytes(decls: &[Decl], declarer: usize, name: &str, expr: &Expr) -> String {
    canonical(&obj(vec![
        ("d", text(&decls[declarer].qualified)),
        ("n", text(name)),
        ("e", shape(expr, decls, &mut Vec::new())),
    ]))
}

#[cfg(test)]
mod tests;

fn etype(t: &ExprType, decls: &[Decl]) -> Json {
    match t {
        ExprType::Type(t) => tshape(t, decls),
        ExprType::Duration => text("duration"),
    }
}

/// `ExprShape` of one node. `vars` holds the enclosing lambda variables, outermost first; a
/// variable is written `v<depth>`.
fn shape(e: &Expr, decls: &[Decl], vars: &mut Vec<String>) -> Json {
    obj(vec![
        ("t", etype(&e.t, decls)),
        ("e", kind(&e.e, decls, vars)),
    ])
}

fn one(key: &str, v: Json) -> Json {
    obj(vec![(key, v)])
}

fn kind(k: &ExprKind, decls: &[Decl], vars: &mut Vec<String>) -> Json {
    match k {
        ExprKind::Lit(l) => one("lit", literal(l, decls)),
        ExprKind::Path(p) => one("path", path(p, vars)),
        ExprKind::Has(p) => one("has", path(p, vars)),
        ExprKind::Now(_) => one("now", Json::Bool(true)),
        ExprKind::Not(x) => one("not", shape(x, decls, vars)),
        ExprKind::Neg(x) => one("neg", shape(x, decls, vars)),
        ExprKind::Widen(x) => one("widen", shape(x, decls, vars)),
        ExprKind::Old(x) => one("old", shape(x, decls, vars)),
        ExprKind::Bin(b) => one(
            "bin",
            obj(vec![
                ("op", text(op_name(b.op))),
                ("l", shape(&b.l, decls, vars)),
                ("r", shape(&b.r, decls, vars)),
            ]),
        ),
        ExprKind::Call(c) => one(
            "call",
            obj(vec![
                ("f", text(builtin_name(c.f))),
                (
                    "args",
                    Json::Arr(c.args.iter().map(|a| shape(a, decls, vars)).collect()),
                ),
            ]),
        ),
        ExprKind::Quant(q) => {
            let list = shape(&q.list, decls, vars);
            let var = format!("v{}", vars.len());
            vars.push(q.var.clone());
            let body = shape(&q.body, decls, vars);
            vars.pop();
            one(
                "quant",
                obj(vec![
                    ("q", text(quantifier_name(q.q))),
                    ("list", list),
                    ("var", Json::Str(var)),
                    ("body", body),
                ]),
            )
        }
        ExprKind::List(items) => one(
            "list",
            Json::Arr(items.iter().map(|a| shape(a, decls, vars)).collect()),
        ),
    }
}

/// Every number is written as a string, so no number formatting is involved.
fn literal(l: &ExprLit, decls: &[Decl]) -> Json {
    match l {
        ExprLit::Bool(b) => one("bool", Json::Bool(*b)),
        ExprLit::Int(n) => one("int", Json::Str(n.to_string())),
        ExprLit::Long(n) => one("long", Json::Str(n.to_string())),
        ExprLit::Float(f) => one("float", Json::Str(format!("{:016x}", f.to_bits()))),
        ExprLit::Decimal(s) => one("decimal", text(s)),
        ExprLit::String(s) => one("string", text(s)),
        ExprLit::Enum(e) => one(
            "enum",
            obj(vec![
                ("ref", text(&decls[e.decl.index()].qualified)),
                ("wire", text(&e.wire)),
            ]),
        ),
    }
}

fn path(p: &PathRef, vars: &[String]) -> Json {
    let root = match &p.root {
        Root::SelfValue => text("self"),
        Root::Principal => text("principal"),
        Root::Result => text("result"),
        Root::Var(v) => {
            let depth = vars.iter().rposition(|x| x == v).unwrap_or(vars.len());
            one("var", Json::Str(format!("v{depth}")))
        }
        Root::Param(n) => one("param", text(n)),
        Root::Fact(n) => one("fact", text(n)),
    };
    let steps = p.steps.iter().map(|s| one("w", text(&s.wire))).collect();
    obj(vec![("root", root), ("steps", Json::Arr(steps))])
}

fn op_name(op: Op) -> &'static str {
    match op {
        Op::And => "and",
        Op::Or => "or",
        Op::Implies => "implies",
        Op::Eq => "eq",
        Op::Ne => "ne",
        Op::Lt => "lt",
        Op::Le => "le",
        Op::Gt => "gt",
        Op::Ge => "ge",
        Op::In => "in",
        Op::Add => "add",
        Op::Sub => "sub",
        Op::Mul => "mul",
        Op::Div => "div",
        Op::Rem => "rem",
    }
}

fn builtin_name(f: Builtin) -> &'static str {
    match f {
        Builtin::Len => "len",
        Builtin::Matches => "matches",
        Builtin::StartsWith => "starts_with",
        Builtin::EndsWith => "ends_with",
        Builtin::Contains => "contains",
        Builtin::Days => "days",
        Builtin::Hours => "hours",
        Builtin::Minutes => "minutes",
        Builtin::Seconds => "seconds",
        Builtin::Ms => "ms",
    }
}

fn quantifier_name(q: Quantifier) -> &'static str {
    match q {
        Quantifier::All => "all",
        Quantifier::Any => "any",
        Quantifier::Count => "count",
        Quantifier::Sum => "sum",
    }
}
