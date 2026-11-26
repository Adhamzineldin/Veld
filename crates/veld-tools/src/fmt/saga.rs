//! Text of sagas (`spec/platform.md` sections 17 and 18): the steps of a `saga post` endpoint and
//! the saga lines of an example.
//!
//! A step is one line, `step name = Service.endpoint(a: x) compensate Service.undo(b: y)`. When
//! that is wider than [`WIDTH`](super::WIDTH) the `compensate` goes on a line of its own, two
//! spaces further in. An object value is `{ a: x, b: y }`, an empty one `{}`.

use veld_compiler::syntax::ast::{
    SagaAnswerLine, SagaCallDecl, SagaExpectKind, SagaExpectLine, SagaGivenLine, SagaStepDecl,
    SagaTimeoutDecl, SagaValueDecl,
};

use super::WIDTH;
use super::expr::expr_text;
use super::stmt::value_text_of_outcome;

fn pad(n: usize) -> String {
    " ".repeat(n)
}

fn value_text(v: &SagaValueDecl) -> String {
    match v {
        SagaValueDecl::Expr(e) => expr_text(e),
        SagaValueDecl::Object { fields, .. } if fields.is_empty() => "{}".to_owned(),
        SagaValueDecl::Object { fields, .. } => {
            let items: Vec<String> = fields
                .iter()
                .map(|f| format!("{}: {}", f.name.name, value_text(&f.value)))
                .collect();
            format!("{{ {} }}", items.join(", "))
        }
    }
}

fn call_text(c: &SagaCallDecl) -> String {
    let args: Vec<String> = c
        .args
        .iter()
        .map(|a| format!("{}: {}", a.name.name, value_text(&a.value)))
        .collect();
    format!(
        "{}.{}({})",
        c.service.name,
        c.endpoint.name,
        args.join(", ")
    )
}

/// The lines of a step, indented by `ind`.
pub(super) fn step_lines(s: &SagaStepDecl, ind: usize) -> Vec<String> {
    let head = format!("{}step {} = {}", pad(ind), s.name.name, call_text(&s.call));
    let Some(undo) = &s.compensate else {
        return vec![head];
    };
    let flat = format!("{head} compensate {}", call_text(undo));
    if flat.chars().count() <= WIDTH {
        return vec![flat];
    }
    vec![
        head,
        format!("{}compensate {}", pad(ind + 2), call_text(undo)),
    ]
}

/// `timeout expr`, indented by `ind`.
pub(super) fn timeout_line(t: &SagaTimeoutDecl, ind: usize) -> String {
    format!("{}timeout {}", pad(ind), expr_text(&t.expr))
}

/// `given step s = ..` or `given undo s = ..`, without indent.
pub(super) fn given_line(g: &SagaGivenLine) -> String {
    let word = if g.undo { "undo" } else { "step" };
    let answer = match &g.answer {
        SagaAnswerLine::Unavailable(_) => "unavailable".to_owned(),
        SagaAnswerLine::Outcome(o) => value_text_of_outcome(o),
        SagaAnswerLine::Value(v) => super::stmt::value_text(v),
    };
    format!("given {word} {} = {answer}", g.step.name)
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
