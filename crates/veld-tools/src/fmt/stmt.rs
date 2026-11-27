//! Text of endpoint statements and examples (`spec/rules.md` sections 9 and 12).
//!
//! A statement is one line, with its `message` on a continuation line like a rule. An example
//! call is one line; with `returns` the call, `returns` and the outcome take a line each.

use veld_compiler::syntax::ast::{
    Else, EventExample, Example, ExampleCall, ExpectEmitted, ExpectKind, Expectation, FlagSuffix,
    Given, IfMatchLine, KeyLine, KeyStoreLine, Outcome, PolicyClause, PolicyDecl, PrincipalDecl,
    Stmt, TransitionStmt, Value, ValueKind,
};

use super::expr::expr_text;
use super::text::{quote, type_expr};
use super::wrap::{Node, expr_lines, node_flat_fits, render, value_node};

fn pad(n: usize) -> String {
    " ".repeat(n)
}

fn payload_text(o: &Else) -> String {
    let mut s = type_expr(&o.error);
    if let Some(fields) = &o.payload {
        if fields.is_empty() {
            s.push_str(" {}");
        } else {
            let items: Vec<String> = fields
                .iter()
                .map(|f| format!("{}: {}", f.name.name, expr_text(&f.value)))
                .collect();
            s.push_str(&format!(" {{ {} }}", items.join(", ")));
        }
    }
    s
}

/// The lines of a statement, indented by `ind`.
pub(super) fn stmt_lines(s: &Stmt, ind: usize) -> Vec<String> {
    let p = pad(ind);
    match s {
        Stmt::Fact(f) => {
            let mut line = format!(
                "{p}fact {}{}: {}",
                f.name.name,
                if f.optional { "?" } else { "" },
                type_expr(&f.ty)
            );
            if let Some(k) = &f.key {
                line.push_str(&format!(" = {}", expr_text(k)));
            }
            vec![line]
        }
        Stmt::Require(r) => {
            let name = r
                .name
                .as_ref()
                .map_or(String::new(), |n| format!("{}: ", n.name));
            let tail = r
                .otherwise
                .as_ref()
                .map_or_else(String::new, |o| format!(" else {}", payload_text(o)));
            let lines = expr_lines(&format!("require {name}"), &r.expr, &tail, ind);
            with_flag(
                with_message(lines, r.message.as_ref().map(|m| m.value.as_str()), &p),
                r.flag.as_ref(),
            )
        }
        Stmt::Ensure(e) => {
            let name = e
                .name
                .as_ref()
                .map_or(String::new(), |n| format!("{}: ", n.name));
            let lines = expr_lines(&format!("ensure {name}"), &e.expr, "", ind);
            with_flag(
                with_message(lines, e.message.as_ref().map(|m| m.value.as_str()), &p),
                e.flag.as_ref(),
            )
        }
        Stmt::Transition(t) => vec![format!("{p}{}", transition_text(t))],
        Stmt::Allow(a) => {
            let args: Vec<String> = a.args.iter().map(expr_text).collect();
            vec![format!("{p}allow {}({})", a.policy.name, args.join(", "))]
        }
    }
}

/// ` when flag("name")` at the end of the last line of a statement (`spec/platform.md` section 11).
pub(super) fn with_flag(mut lines: Vec<String>, flag: Option<&FlagSuffix>) -> Vec<String> {
    if let (Some(f), Some(last)) = (flag, lines.last_mut()) {
        last.push_str(&flag_text(f));
    }
    lines
}

pub(super) fn flag_text(f: &FlagSuffix) -> String {
    format!(" when flag({})", quote(&f.name))
}

fn with_message(mut lines: Vec<String>, message: Option<&str>, p: &str) -> Vec<String> {
    if let Some(m) = message {
        lines.push(format!("{p}  message {}", quote(m)));
    }
    lines
}

/// The lines of a `permit` or `forbid` clause, indented by `ind`.
pub(super) fn clause_lines(c: &PolicyClause, ind: usize) -> Vec<String> {
    let p = pad(ind);
    let word = if c.forbid { "forbid" } else { "permit" };
    let lines = expr_lines(&format!("{word} when "), &c.expr, "", ind);
    with_flag(
        with_message(lines, c.message.as_ref().map(|m| m.value.as_str()), &p),
        c.flag.as_ref(),
    )
}

/// `policy Name(a: T, b: U)`, the head of the block.
pub(super) fn policy_head(p: &PolicyDecl) -> String {
    let params: Vec<String> = p
        .params
        .iter()
        .map(|x| format!("{}: {}", x.name.name, type_expr(&x.ty)))
        .collect();
    format!("policy {}({})", p.name.name, params.join(", "))
}

/// `principal User [as name]`.
pub(super) fn principal_line(p: &PrincipalDecl) -> String {
    let mut s = format!("principal {}", type_expr(&p.ty));
    if let Some(n) = &p.name {
        s.push_str(&format!(" as {}", n.name));
    }
    s
}

/// `transition booking.status: a, b -> c`.
fn transition_text(t: &TransitionStmt) -> String {
    let path: Vec<&str> = t.path.iter().map(|i| i.name.as_str()).collect();
    let from: Vec<&str> = t.from.iter().map(|i| i.name.as_str()).collect();
    let since = t.since.as_ref().map_or_else(String::new, |s| {
        let names: Vec<&str> = s.iter().map(|i| i.name.as_str()).collect();
        format!(" since {}", names.join("."))
    });
    format!(
        "transition {}: {} -> {}{since}",
        path.join("."),
        from.join(", "),
        t.to.name
    )
}

/// The printing group of a statement: facts, `allow`, `transition`, then `require`, then `ensure`
/// (`spec/rules.md` 9).
pub(super) fn stmt_group(s: &Stmt) -> u8 {
    match s {
        Stmt::Fact(_) => 0,
        Stmt::Allow(_) => 1,
        Stmt::Transition(_) => 2,
        Stmt::Require(_) => 3,
        Stmt::Ensure(_) => 4,
    }
}

pub(super) fn stmt_docs(s: &Stmt) -> &[veld_compiler::syntax::ast::Doc] {
    match s {
        Stmt::Fact(_) | Stmt::Transition(_) | Stmt::Allow(_) => &[],
        Stmt::Require(r) => &r.docs,
        Stmt::Ensure(e) => &e.docs,
    }
}

pub(super) fn stmt_annotations(s: &Stmt) -> &[veld_compiler::syntax::ast::Annotation] {
    match s {
        Stmt::Fact(_) | Stmt::Transition(_) | Stmt::Allow(_) => &[],
        Stmt::Require(r) => &r.annotations,
        Stmt::Ensure(e) => &e.annotations,
    }
}

pub(super) fn value_text(v: &Value) -> String {
    match &v.kind {
        ValueKind::Bool(b) => b.to_string(),
        ValueKind::Int(n) => n.to_string(),
        ValueKind::Float { text, .. } => text.clone(),
        ValueKind::Str(s) => quote(s),
        ValueKind::EnumLit(id) => format!(".{}", id.name),
        ValueKind::Path(p) => p
            .iter()
            .map(|i| i.name.as_str())
            .collect::<Vec<_>>()
            .join("."),
        ValueKind::Object(fields) if fields.is_empty() => "{}".to_string(),
        ValueKind::Object(fields) => {
            let items: Vec<String> = fields
                .iter()
                .map(|f| format!("{}: {}", f.name.name, value_text(&f.value)))
                .collect();
            format!("{{ {} }}", items.join(", "))
        }
        ValueKind::List(items) => {
            let items: Vec<String> = items.iter().map(value_text).collect();
            format!("[{}]", items.join(", "))
        }
    }
}

/// The lines of a `given`, the first at `ind`; a record that is too wide goes one member per line.
pub(super) fn given_lines(g: &Given, ind: usize) -> Vec<String> {
    if let Some(on) = g.flag {
        let off = if on { "" } else { " off" };
        return vec![format!(
            "{}given flag {}{off}",
            pad(ind),
            quote(&g.name.name)
        )];
    }
    let prefix = format!("given {} = ", g.name.name);
    match &g.value {
        Some(v) => render(&prefix, &value_node(v), ind, ""),
        None => vec![format!("{}{prefix}absent", pad(ind))],
    }
}

/// `as value`, the caller of an example.
pub(super) fn principal_lines(v: &Value, ind: usize) -> Vec<String> {
    render("as ", &value_node(v), ind, "")
}

/// The words of an outcome before its value: `ok` or the error type.
pub(super) fn outcome_word(o: &Outcome) -> String {
    o.error.as_ref().map_or_else(|| "ok".to_string(), type_expr)
}

/// An outcome after `before`, its record broken like any other value when it is too wide.
pub(super) fn outcome_lines(before: &str, o: &Outcome, ind: usize) -> Vec<String> {
    let word = format!("{before}{}", outcome_word(o));
    match &o.value {
        Some(v) => render(&format!("{word} "), &value_node(v), ind, ""),
        None => vec![format!("{}{word}", pad(ind))],
    }
}

fn outcome_text(o: &Outcome) -> String {
    let mut s = o.error.as_ref().map_or_else(|| "ok".to_string(), type_expr);
    if let Some(v) = &o.value {
        s.push(' ');
        s.push_str(&value_text(v));
    }
    s
}

/// `ifMatch 3` or `ifMatch any`.
pub(super) fn if_match_line(i: &IfMatchLine) -> String {
    match i.version {
        Some(n) => format!("ifMatch {n}"),
        None => "ifMatch any".to_string(),
    }
}

/// `key "k"` and what the store holds.
pub(super) fn key_line(k: &KeyLine) -> String {
    let key = quote(&k.key.value);
    match &k.store {
        KeyStoreLine::New => format!("key {key}"),
        KeyStoreLine::Other => format!("key {key} other"),
        KeyStoreLine::Running => format!("key {key} running"),
        KeyStoreLine::Stored(o) => format!("key {key} stored {}", outcome_text(o)),
    }
}

fn call_node(c: &ExampleCall) -> Node {
    let service = c
        .service
        .as_ref()
        .map_or(String::new(), |s| format!("{}.", s.name));
    let args = c
        .args
        .iter()
        .map(|a| (Some(a.name.name.clone()), value_node(&a.value)))
        .collect();
    Node::call(format!("{service}{}", c.endpoint.name), args)
}

/// The call of an example, indented by `ind`. On one line when that fits (with `returns`, the
/// call, `returns` and the outcome take a line each); otherwise the arguments go one per line
/// and the outcome follows on its own line, or hangs on the call when the call fits.
pub(super) fn call_lines(c: &ExampleCall, ind: usize) -> Vec<String> {
    let node = call_node(c);
    let outcome = outcome_text(&c.outcome);
    let Some(r) = &c.returns else {
        return call_and_outcome(c, &node, &outcome, ind);
    };
    let mut lines = render("", &node, ind, "");
    lines.extend(render("returns ", &value_node(r), ind + 2, ""));
    lines.extend(outcome_lines("-> ", &c.outcome, ind + 2));
    lines
}

fn call_and_outcome(c: &ExampleCall, node: &Node, outcome: &str, ind: usize) -> Vec<String> {
    let flat = format!("{}{} -> {outcome}", pad(ind), node.flat_text());
    if flat.chars().count() <= super::WIDTH {
        return vec![flat];
    }
    let hang = format!("{} -> ", node.flat_text());
    let hung = outcome_lines(&hang, &c.outcome, ind);
    if node_flat_fits(node, ind)
        && hung
            .first()
            .is_some_and(|l| l.chars().count() <= super::WIDTH)
    {
        return hung;
    }
    if !node_flat_fits(node, ind) {
        let closed = render("", node, ind, &format!(" -> {outcome}"));
        if closed
            .last()
            .is_some_and(|l| l.chars().count() <= super::WIDTH)
        {
            return closed;
        }
    }
    let mut lines = render("", node, ind, "");
    lines.extend(outcome_lines("-> ", &c.outcome, ind + 2));
    lines
}

/// `expect emitted E` or `expect not emitted`, after the call of an example.
pub(super) fn emitted_line(x: &ExpectEmitted) -> String {
    match &x.event {
        Some(e) => format!("expect emitted {}", type_expr(e)),
        None => "expect not emitted".to_string(),
    }
}

/// `given event E { .. } [on C]`, the first line of an event example.
pub(super) fn given_event_line(e: &EventExample) -> String {
    let mut s = format!(
        "given event {} {}",
        type_expr(&e.event),
        value_text(&e.value)
    );
    if let Some(c) = &e.channel {
        s.push_str(&format!(" on {}", type_expr(c)));
    }
    s
}

/// `expect S.handler [not] called [once]` or `expect rejected`.
pub(super) fn expect_line(x: &Expectation) -> String {
    match &x.kind {
        ExpectKind::Rejected => "expect rejected".to_string(),
        ExpectKind::Called {
            service,
            handler,
            not,
        } => format!(
            "expect {}.{}{} called{}",
            service.name,
            handler.name,
            if *not { " not" } else { "" },
            if x.once.is_some() { " once" } else { "" }
        ),
    }
}

/// `example "name"`, the head of the block.
pub(super) fn example_head(x: &Example) -> String {
    format!("example {}", quote(&x.name.value))
}
