//! What the generator learns from the rules of an endpoint.
//!
//! A **task** is one comparison with one side that the generator can write: a path, `len(path)`
//! or the difference of two paths. The other side is evaluated with the reference evaluator on
//! the request as it stands, so a constant, `now`, `room.capacity` and `days(30)` are all handled
//! by one mechanism. A task is used in two ways:
//!
//! * **satisfy**: write the side so that the comparison holds. This is how cross-field rules
//!   (`end > start`) come out true some of the time: the generator runs the reference pipeline,
//!   and for each rule it reports as violated it copies the other side of the comparison into
//!   the path, one step beyond it for a strict comparison.
//! * **boundary**: write the side at the other side's value and one smallest step on either side.

use std::collections::BTreeMap;

use serde_json::{Number, Value as Json};
use veld_ir::{
    Constraints, DeclId, DeclKind, Endpoint, Expr, ExprKind, ExprLit, ExprType, Ir, Op, PathRef,
    Root, Scalar, Type,
};
use veld_rules::helpers::decimal::Decimal;
use veld_rules::helpers::instant::MS_PER_DAY;
use veld_rules::{Env, Request, Value, Violation, eval, format_date, format_datetime};

use super::loc::{Loc, Step, get, is_body, slot};
use super::values::{Gen, needs_example};

/// Where the `self` of a rule is.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Scope {
    /// An endpoint `require`: no `self`.
    Request,
    /// A rule of a model, for the value at this place.
    Instance(Loc),
}

/// The side of a comparison the generator can write.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Target {
    Path(PathRef, Scalar),
    Len(PathRef),
    /// `a - b` of two datetimes or two dates.
    Diff(PathRef, PathRef, Scalar),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Task {
    pub scope: Scope,
    pub target: Target,
    /// The comparison, read as `target op other`.
    pub op: Op,
    pub other: Expr,
}

/// A rule found in the contract: its expression and whether it is an endpoint `require`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RuleRef<'a> {
    pub expr: &'a Expr,
    pub endpoint_rule: bool,
}

pub(crate) type RuleIndex<'a> = BTreeMap<&'a str, RuleRef<'a>>;

/// Every rule that can fire on a request to `ep`, by id.
pub(crate) fn index<'a>(ir: &'a Ir, ep: &'a Endpoint) -> RuleIndex<'a> {
    let mut out = BTreeMap::new();
    for d in &ir.decls {
        for r in d.rules() {
            out.insert(
                r.id.as_str(),
                RuleRef {
                    expr: &r.expr,
                    endpoint_rule: false,
                },
            );
        }
    }
    for r in &ep.rules.requires {
        out.insert(
            r.id.as_str(),
            RuleRef {
                expr: &r.expr,
                endpoint_rule: true,
            },
        );
    }
    out
}

// ---- finding comparisons ------------------------------------------------------------------

fn strip(e: &Expr) -> &Expr {
    if let ExprKind::Widen(inner) = &e.e {
        strip(inner)
    } else {
        e
    }
}

fn scalar_of(e: &Expr) -> Option<Scalar> {
    match &e.t {
        ExprType::Type(Type::Scalar(s)) => Some(*s),
        ExprType::Type(_) | ExprType::Duration => None,
    }
}

fn writable_root(p: &PathRef) -> bool {
    matches!(p.root, Root::Param(_) | Root::Fact(_) | Root::SelfValue)
}

fn path_of(e: &Expr) -> Option<&PathRef> {
    match &strip(e).e {
        ExprKind::Path(p) => writable_root(p).then_some(p),
        ExprKind::Lit(_)
        | ExprKind::Now(_)
        | ExprKind::Not(_)
        | ExprKind::Neg(_)
        | ExprKind::Bin(_)
        | ExprKind::Has(_)
        | ExprKind::Call(_)
        | ExprKind::Quant(_)
        | ExprKind::List(_)
        | ExprKind::Widen(_)
        | ExprKind::Old(_) => None,
    }
}

fn target_of(e: &Expr) -> Option<Target> {
    let e = strip(e);
    if let Some(p) = path_of(e) {
        return Some(Target::Path(p.clone(), scalar_of(e)?));
    }
    if let ExprKind::Call(c) = &e.e
        && c.f == veld_ir::Builtin::Len
    {
        return path_of(c.args.first()?).map(|p| Target::Len(p.clone()));
    }
    if let ExprKind::Bin(b) = &e.e
        && b.op == Op::Sub
    {
        let s = scalar_of(&b.l)?;
        let (a, c) = path_of(&b.l).zip(path_of(&b.r))?;
        return matches!(s, Scalar::Datetime | Scalar::Date)
            .then(|| Target::Diff(a.clone(), c.clone(), s));
    }
    None
}

fn bool_lit(b: bool) -> Expr {
    Expr {
        t: ExprType::Type(Type::Scalar(Scalar::Bool)),
        e: ExprKind::Lit(ExprLit::Bool(b)),
    }
}

fn flip(op: Op) -> Op {
    match op {
        Op::Lt => Op::Gt,
        Op::Le => Op::Ge,
        Op::Gt => Op::Lt,
        Op::Ge => Op::Le,
        Op::And
        | Op::Or
        | Op::Implies
        | Op::Eq
        | Op::Ne
        | Op::In
        | Op::Add
        | Op::Sub
        | Op::Mul
        | Op::Div
        | Op::Rem => op,
    }
}

/// The comparisons of a rule. With `all`, every one; without, the ones a request must satisfy
/// for the rule to hold (both sides of `&&`, the left of `||`, the consequent of `implies`).
fn comparisons(e: &Expr, all: bool, out: &mut Vec<(Op, Expr, Expr)>) {
    match &e.e {
        ExprKind::Bin(b) => match b.op {
            Op::And => {
                comparisons(&b.l, all, out);
                comparisons(&b.r, all, out);
            }
            Op::Or => {
                comparisons(&b.l, all, out);
                if all {
                    comparisons(&b.r, all, out);
                }
            }
            Op::Implies => {
                if all {
                    comparisons(&b.l, all, out);
                }
                comparisons(&b.r, all, out);
            }
            Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::Eq | Op::Ne => {
                out.push((b.op, b.l.clone(), b.r.clone()));
            }
            Op::In | Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem => {}
        },
        ExprKind::Not(inner) => {
            if path_of(inner).is_some() {
                out.push((Op::Eq, (**inner).clone(), bool_lit(false)));
            }
        }
        ExprKind::Path(p) => {
            if writable_root(p) && scalar_of(e) == Some(Scalar::Bool) {
                out.push((Op::Eq, e.clone(), bool_lit(true)));
            }
        }
        ExprKind::Lit(_)
        | ExprKind::Now(_)
        | ExprKind::Neg(_)
        | ExprKind::Has(_)
        | ExprKind::Call(_)
        | ExprKind::Quant(_)
        | ExprKind::List(_)
        | ExprKind::Widen(_)
        | ExprKind::Old(_) => {}
    }
}

/// The tasks of one rule. `first_only` keeps one task per comparison (for satisfying a rule).
pub(crate) fn tasks(scope: &Scope, rule: &Expr, all: bool, first_only: bool) -> Vec<Task> {
    let mut cmps = Vec::new();
    comparisons(rule, all, &mut cmps);
    let mut out = Vec::new();
    for (op, l, r) in cmps {
        let sides = [(&l, &r, op), (&r, &l, flip(op))];
        for (target, other, op) in sides {
            let Some(t) = target_of(target) else { continue };
            out.push(Task {
                scope: scope.clone(),
                target: t,
                op,
                other: other.clone(),
            });
            if first_only {
                break;
            }
        }
    }
    out
}

/// The places of the parameters that hold a model with rules, below `ep`'s parameters.
pub(crate) fn rule_places(ir: &Ir, ep: &Endpoint) -> Vec<(Loc, DeclId)> {
    let mut out = Vec::new();
    for p in ep.params.iter().filter(|p| !p.stream) {
        let root = Loc {
            root: p.name.clone(),
            fact: false,
            steps: Vec::new(),
        };
        walk(ir, &p.ty, &root, &mut Vec::new(), &mut out);
    }
    out
}

fn walk(ir: &Ir, ty: &Type, at: &Loc, seen: &mut Vec<DeclId>, out: &mut Vec<(Loc, DeclId)>) {
    match ty {
        Type::List(t) => walk(ir, t, &at.child(Step::Item(0)), seen, out),
        Type::Decl(id) if !seen.contains(id) => {
            let Some(decl) = ir.decls.get(id.index()) else {
                return;
            };
            if !matches!(decl.kind, DeclKind::Model(_) | DeclKind::Error(_)) {
                return;
            }
            if !decl.rules().is_empty() {
                out.push((at.clone(), *id));
            }
            seen.push(*id);
            for f in decl.fields().iter().filter(|f| !f.readonly) {
                walk(ir, &f.ty, &at.child(Step::Field(f.wire.clone())), seen, out);
            }
            seen.pop();
        }
        Type::Scalar(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => {}
    }
}

// ---- places of paths ----------------------------------------------------------------------

fn loc_of(p: &PathRef, scope: &Scope) -> Option<Loc> {
    let mut loc = match (&p.root, scope) {
        (Root::Param(n), _) => Loc {
            root: n.clone(),
            fact: false,
            steps: Vec::new(),
        },
        (Root::Fact(n), _) => Loc {
            root: n.clone(),
            fact: true,
            steps: Vec::new(),
        },
        (Root::SelfValue, Scope::Instance(at)) => at.clone(),
        _ => return None,
    };
    loc.steps
        .extend(p.steps.iter().map(|s| Step::Field(s.wire.clone())));
    Some(loc)
}

/// Every writable path an expression reads.
fn paths_in<'a>(e: &'a Expr, out: &mut Vec<&'a PathRef>) {
    match &e.e {
        ExprKind::Path(p) | ExprKind::Has(p) => out.push(p),
        ExprKind::Not(x) | ExprKind::Neg(x) | ExprKind::Widen(x) | ExprKind::Old(x) => {
            paths_in(x, out);
        }
        ExprKind::Bin(b) => {
            paths_in(&b.l, out);
            paths_in(&b.r, out);
        }
        ExprKind::Call(c) => c.args.iter().for_each(|a| paths_in(a, out)),
        ExprKind::List(items) => items.iter().for_each(|a| paths_in(a, out)),
        ExprKind::Quant(q) => {
            paths_in(&q.list, out);
            paths_in(&q.body, out);
        }
        ExprKind::Lit(_) | ExprKind::Now(_) => {}
    }
}

/// Makes every path of `e` present in the request, and the keys of the facts it reads (an
/// optional fact whose key is absent is not loaded).
fn ensure_paths(g: &mut Gen<'_>, ep: &Endpoint, req: &mut Request, scope: &Scope, e: &Expr) {
    let mut paths = Vec::new();
    paths_in(e, &mut paths);
    for p in paths {
        ensure_path(g, ep, req, scope, p);
    }
}

fn ensure_path(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &mut Request,
    scope: &Scope,
    p: &PathRef,
) -> Option<()> {
    let loc = loc_of(p, scope)?;
    slot(g, ep, req, &loc)?;
    if let Root::Fact(name) = &p.root
        && let Some(key) = ep
            .rules
            .facts
            .iter()
            .find(|f| &f.name == name)
            .and_then(|f| f.key.as_ref())
    {
        ensure_paths(g, ep, req, &Scope::Request, key);
    }
    Some(())
}

/// Makes everything a task reads and writes present in the request.
pub(crate) fn prepare(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &mut Request,
    task: &Task,
) -> Option<()> {
    ensure_paths(g, ep, req, &task.scope, &task.other);
    match &task.target {
        Target::Path(p, _) | Target::Len(p) => ensure_path(g, ep, req, &task.scope, p),
        Target::Diff(a, b, _) => {
            ensure_path(g, ep, req, &task.scope, a)?;
            ensure_path(g, ep, req, &task.scope, b)
        }
    }
}

// ---- numbers ------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Num {
    Exact(Decimal),
    Float(f64),
}

fn num_of(v: &Value) -> Option<Num> {
    Some(match v {
        Value::Int(n) => Num::Exact(Decimal::from_i64(i64::from(*n))),
        Value::Long(n) | Value::Date(n) | Value::Datetime(n) | Value::Duration(n) => {
            Num::Exact(Decimal::from_i64(*n))
        }
        Value::Decimal(d) => Num::Exact(d.clone()),
        Value::Float(f) => Num::Float(*f),
        Value::Bool(_)
        | Value::Str(_)
        | Value::Uuid(_)
        | Value::Bytes(_)
        | Value::Time(_)
        | Value::Enum(_)
        | Value::List(_)
        | Value::Map(_)
        | Value::Raw(_) => return None,
    })
}

fn to_f64(n: &Num) -> Option<f64> {
    match n {
        Num::Float(f) => Some(*f),
        Num::Exact(d) => d.to_string().parse().ok(),
    }
}

/// `n` moved one smallest step (`delta` is -1, 0 or 1). A decimal step is `0.01`, or finer when
/// the number is.
fn shift(n: &Num, delta: i32, target: Scalar) -> Option<Num> {
    Some(match (n, target) {
        (Num::Float(f), _) => Num::Float(match delta.signum() {
            1 => f.next_up(),
            -1 => f.next_down(),
            _ => *f,
        }),
        (Num::Exact(d), Scalar::Float) => {
            return shift(&Num::Float(to_f64(&Num::Exact(d.clone()))?), delta, target);
        }
        (Num::Exact(d), Scalar::Decimal) => {
            let text = d.to_string();
            let scale = text.split_once('.').map_or(0, |(_, f)| f.len()).max(2);
            let step = Decimal::parse(&format!("0.{}1", "0".repeat(scale - 1)))?;
            Num::Exact(match delta.signum() {
                1 => d.add(&step),
                -1 => d.sub(&step),
                _ => d.clone(),
            })
        }
        (Num::Exact(d), _) => {
            let one = Decimal::from_i64(1);
            Num::Exact(match delta.signum() {
                1 => d.add(&one),
                -1 => d.sub(&one),
                _ => d.clone(),
            })
        }
    })
}

fn whole(n: &Num) -> Option<i64> {
    match n {
        Num::Exact(d) => d.to_string().parse().ok(),
        Num::Float(_) => None,
    }
}

/// The wire form of `n` as a value of `target`.
fn write(target: Scalar, n: &Num) -> Option<Json> {
    match target {
        Scalar::Int => i32::try_from(whole(n)?).ok().map(Json::from),
        Scalar::Long => Some(Json::String(whole(n)?.to_string())),
        Scalar::Decimal => match n {
            Num::Exact(d) => Some(Json::String(d.to_string())),
            Num::Float(_) => None,
        },
        Scalar::Float => Number::from_f64(to_f64(n)?).map(Json::Number),
        Scalar::Datetime => Some(Json::String(format_datetime(whole(n)?))),
        Scalar::Date => Some(Json::String(format_date(whole(n)?))),
        Scalar::String
        | Scalar::Bool
        | Scalar::Time
        | Scalar::Uuid
        | Scalar::Bytes
        | Scalar::Json => None,
    }
}

fn inside(c: &Constraints, n: &Num) -> bool {
    let Some(x) = to_f64(n) else { return false };
    c.min.is_none_or(|m| x >= m) && c.max.is_none_or(|m| x <= m)
}

// ---- applying a task ----------------------------------------------------------------------

/// The `delta` that makes `target op other` true.
pub(crate) fn satisfying(op: Op) -> i32 {
    match op {
        Op::Lt => -1,
        Op::Gt | Op::Ne => 1,
        Op::And
        | Op::Or
        | Op::Implies
        | Op::Eq
        | Op::Le
        | Op::Ge
        | Op::In
        | Op::Add
        | Op::Sub
        | Op::Mul
        | Op::Div
        | Op::Rem => 0,
    }
}

fn env_of<'a>(ir: &'a Ir, req: &Request, this: Option<&'a Json>) -> Env<'a> {
    let mut env = Env::new(ir);
    env.this = this;
    env.params.clone_from(&req.params);
    env.facts = req
        .facts
        .iter()
        .filter_map(|(k, v)| v.clone().map(|v| (k.clone(), v)))
        .collect();
    env.now = Some(req.now);
    env
}

/// Writes the target of `task` so that it stands `delta` steps from the other side
/// (`-1`, `0` or `1`). `None` when the request does not allow it (a value outside the field's
/// constraints, a side that cannot be evaluated, a type the generator does not write).
pub(crate) fn apply(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &mut Request,
    task: &Task,
    delta: i32,
) -> Option<()> {
    ensure_paths(g, ep, req, &task.scope, &task.other);
    let instance = match &task.scope {
        Scope::Instance(at) => Some(get(req, at)?.clone()),
        Scope::Request => None,
    };
    let other = eval(&env_of(g.ir, req, instance.as_ref()), &task.other).ok()?;
    match &task.target {
        Target::Path(p, s) => set_path(g, ep, req, task, (p, *s), &other, delta),
        Target::Len(p) => set_len(g, ep, req, task, p, &other, delta),
        Target::Diff(a, b, s) => {
            let base = {
                let this = instance.as_ref();
                let b_expr = path_expr(b, *s);
                eval(&env_of(g.ir, req, this), &b_expr).ok()?
            };
            set_diff(g, ep, req, task, (a, *s), (&base, &other), delta)
        }
    }
}

fn path_expr(p: &PathRef, s: Scalar) -> Expr {
    Expr {
        t: ExprType::Type(Type::Scalar(s)),
        e: ExprKind::Path(p.clone()),
    }
}

fn set_path(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &mut Request,
    task: &Task,
    (p, s): (&PathRef, Scalar),
    other: &Value,
    delta: i32,
) -> Option<()> {
    let loc = loc_of(p, &task.scope)?;
    let json = match (s, other) {
        (Scalar::Bool, Value::Bool(b)) if delta == 0 => Json::Bool(*b == (task.op != Op::Ne)),
        (Scalar::String, Value::Str(t)) | (Scalar::Uuid, Value::Uuid(t)) if task.op == Op::Eq => {
            Json::String(t.clone())
        }
        (_, Value::Enum(w)) if task.op == Op::Eq => Json::String(w.clone()),
        _ => {
            let want = shift(&num_of(other)?, delta, s)?;
            let json = write(s, &want)?;
            let slot_ok = slot(g, ep, req, &loc)?;
            if !inside(&slot_ok.constraints, &want) {
                return None;
            }
            *slot_ok.json = json;
            return Some(());
        }
    };
    *slot(g, ep, req, &loc)?.json = json;
    Some(())
}

fn set_len(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &mut Request,
    task: &Task,
    p: &PathRef,
    other: &Value,
    delta: i32,
) -> Option<()> {
    let want = whole(&shift(&num_of(other)?, delta, Scalar::Int)?)?;
    let n = usize::try_from(want).ok()?;
    let loc = loc_of(p, &task.scope)?;
    let flat = !loc.fact && ep.params.iter().any(|q| q.name == loc.root && !is_body(q));
    if n == 0 && flat {
        return None;
    }
    let s = slot(g, ep, req, &loc)?;
    let (ty, c) = (s.ty.clone(), s.constraints.clone());
    if c.min_len.is_some_and(|m| (n as u64) < m) || c.max_len.is_some_and(|m| n as u64 > m) {
        return None;
    }
    if needs_example(&c) {
        return None;
    }
    let value = g.sized(&ty, n).ok()?;
    *slot(g, ep, req, &loc)?.json = value;
    Some(())
}

fn set_diff(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &mut Request,
    task: &Task,
    (a, s): (&PathRef, Scalar),
    (base, other): (&Value, &Value),
    delta: i32,
) -> Option<()> {
    let (Value::Duration(d), unit) = (other, if s == Scalar::Date { MS_PER_DAY } else { 1 }) else {
        return None;
    };
    let diff = d.checked_add(i64::from(delta) * unit)?;
    let json = match (s, base) {
        (Scalar::Datetime, Value::Datetime(b)) => {
            Json::String(format_datetime(b.checked_add(diff)?))
        }
        (Scalar::Date, Value::Date(b)) => {
            Json::String(format_date(b.checked_add(diff / MS_PER_DAY)?))
        }
        _ => return None,
    };
    let loc = loc_of(a, &task.scope)?;
    *slot(g, ep, req, &loc)?.json = json;
    Some(())
}

/// Makes every comparison of the violated rules true, where the generator can.
pub(crate) fn repair(
    g: &mut Gen<'_>,
    ep: &Endpoint,
    req: &mut Request,
    violations: &[Violation],
    rules: &RuleIndex<'_>,
) -> bool {
    let mut changed = false;
    for v in violations {
        let Some(rule) = rules.get(v.rule_id.as_str()) else {
            continue;
        };
        let scope = match (rule.endpoint_rule, Loc::parse(&v.path)) {
            (true, _) => Scope::Request,
            (false, Some(at)) => Scope::Instance(at),
            (false, None) => continue,
        };
        for task in tasks(&scope, rule.expr, false, true) {
            changed |= apply(g, ep, req, &task, satisfying(task.op)).is_some();
        }
    }
    changed
}
