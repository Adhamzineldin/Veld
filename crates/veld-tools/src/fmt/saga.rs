//! Text of sagas (`spec/platform.md` sections 17 and 18): the steps of a `saga post` endpoint and
//! the saga lines of an example.
//!
//! A step is one line, `step name = Service.endpoint(a: x) compensate Service.undo(b: y)`. When
//! that is wider than [`WIDTH`](super::WIDTH) the `compensate` goes on a line of its own, two
//! spaces further in. An object value is `{ a: x, b: y }`, an empty one `{}`.

use veld_compiler::syntax::ast::{
    SagaAnswerLine, SagaCallDecl, SagaExpectKind, SagaExpectLine, SagaGivenLine, SagaStepDecl,
    SagaTimeoutDecl,
};

use super::WIDTH;
use super::expr::expr_text;
use super::wrap::{Node, render, saga_node, value_node};

fn pad(n: usize) -> String {
    " ".repeat(n)
}

fn call_node(c: &SagaCallDecl) -> Node {
    let args = c
        .args
        .iter()
        .map(|a| (Some(a.name.name.clone()), saga_node(&a.value)))
        .collect();
    Node::call(format!("{}.{}", c.service.name, c.endpoint.name), args)
}

/// The lines of a step, indented by `ind`. One line when it fits. Otherwise the `compensate` goes
/// on a line of its own, two spaces further in, and a call that is still too wide has one
/// argument per line.
pub(super) fn step_lines(s: &SagaStepDecl, ind: usize) -> Vec<String> {
    let prefix = format!("step {} = ", s.name.name);
    let head = call_node(&s.call);
    let Some(undo) = &s.compensate else {
        return render(&prefix, &head, ind, "");
    };
    let undo = call_node(undo);
    let flat = format!(
        "{}{prefix}{} compensate {}",
        pad(ind),
        head.flat_text(),
        undo.flat_text()
    );
    if flat.chars().count() <= WIDTH {
        return vec![flat];
    }
    let mut lines = render(&prefix, &head, ind, "");
    lines.extend(render("compensate ", &undo, ind + 2, ""));
    lines
}

/// `timeout expr`, indented by `ind`.
pub(super) fn timeout_line(t: &SagaTimeoutDecl, ind: usize) -> String {
    format!("{}timeout {}", pad(ind), expr_text(&t.expr))
}

/// `given step s = ..` or `given undo s = ..`, the first line at `ind`.
pub(super) fn given_lines(g: &SagaGivenLine, ind: usize) -> Vec<String> {
    let word = if g.undo { "undo" } else { "step" };
    let before = format!("given {word} {} = ", g.step.name);
    match &g.answer {
        SagaAnswerLine::Unavailable(_) => vec![format!("{}{before}unavailable", pad(ind))],
        SagaAnswerLine::Outcome(o) => super::stmt::outcome_lines(&before, o, ind),
        SagaAnswerLine::Value(v) => render(&before, &value_node(v), ind, ""),
    }
}

/// `expect compensated a, b` or `expect stuck`, without indent.
pub(super) fn expect_line(e: &SagaExpectLine) -> String {
    match &e.kind {
        SagaExpectKind::Stuck => "expect stuck".to_owned(),
        SagaExpectKind::Compensated(steps) => {
            let names: Vec<&str> = steps.iter().map(|s| s.name.as_str()).collect();
            format!("expect compensated {}", names.join(", "))
        }
    }
}
