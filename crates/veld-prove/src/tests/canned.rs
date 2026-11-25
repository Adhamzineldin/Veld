//! The pipeline from questions to results, with the answers of the solver written by hand: the
//! confirmation step, the four results and the report. No solver is needed.

use std::time::Duration;

use veld_ir::Ir;

use super::compile_text;
use crate::problem::Problem;
use crate::question::{Question, invariant_questions, policy_questions};
use crate::report::{Outcome, QuestionKind};
use crate::run::{Options, ask};
use crate::solver::{Backend, Flavour, RunOutput};

/// A backend that answers from a text, and says what it was asked.
struct Canned {
    output: String,
    asked: std::cell::RefCell<Option<String>>,
}

impl Canned {
    fn new(answers: &[String]) -> Canned {
        let mut output = String::from("(:name \"Z3\")\n(:version \"4.8.12\")\n");
        for (i, a) in answers.iter().enumerate() {
            output.push_str(&format!("Q{i}\n{a}\n"));
        }
        Canned {
            output,
            asked: std::cell::RefCell::new(None),
        }
    }
}

impl Backend for Canned {
    fn flavour(&self) -> Flavour {
        Flavour::Z3
    }
    fn run(&self, script: &str, _questions: usize, _per: Duration) -> RunOutput {
        *self.asked.borrow_mut() = Some(script.to_owned());
        RunOutput {
            stdout: self.output.clone(),
            ..RunOutput::default()
        }
    }
}

struct Refuses;
impl Backend for Refuses {
    fn flavour(&self) -> Flavour {
        Flavour::Z3
    }
    fn run(&self, _: &str, _: usize, _: Duration) -> RunOutput {
        panic!("the solver must not be asked");
    }
}

fn sat(p: &Problem<'_>, values: &[(&str, &str)]) -> String {
    let lines: Vec<String> = p
        .value_keys()
        .iter()
        .map(|k| {
            let id = k.id();
            let v = values
                .iter()
                .find(|(n, _)| *n == id)
                .map_or("0", |(_, v)| *v);
            format!("({} {v})", k.id())
        })
        .collect();
    format!(
        "sat\n(error \"unsat core is not available\")\n({})",
        lines.join("\n ")
    )
}

fn only<'a>(ir: &'a Ir, decl: &str) -> Question<'a> {
    invariant_questions(ir)
        .into_iter()
        .find(|q| q.subject.ends_with(decl))
        .unwrap()
}

const SUM: &str = "model Ok { a: int\n b: int\n rule sum: a + b < 100 }";

#[test]
fn a_value_the_evaluator_confirms_is_a_counterexample() {
    let ir = compile_text(SUM);
    let q = only(&ir, "Ok");
    let answer = sat(
        q.encoded.as_ref().unwrap(),
        &[("self.a", "1"), ("self.b", "(- 2)")],
    );
    let report = ask(vec![q], &Canned::new(&[answer]), &Options::default());
    let Outcome::Counterexample { value } = &report.items[0].outcome else {
        panic!("{:?}", report.items[0].outcome);
    };
    assert_eq!(value, r#"{"a":1,"b":-2}"#);
    assert!(!report.items[0].is_finding());
    assert_eq!(report.solver.as_ref().unwrap().name, "Z3");
}

#[test]
fn a_value_the_evaluator_rejects_is_never_shown() {
    let ir = compile_text(SUM);
    let q = only(&ir, "Ok");
    let answer = sat(
        q.encoded.as_ref().unwrap(),
        &[("self.a", "60"), ("self.b", "61")],
    );
    let report = ask(vec![q], &Canned::new(&[answer]), &Options::default());
    let Outcome::Unknown { reason } = &report.items[0].outcome else {
        panic!("{:?}", report.items[0].outcome);
    };
    assert!(reason.contains("not confirmed"), "{reason}");
    let shown = format!("{:?}", report.items);
    assert!(!shown.contains("60") && !shown.contains("61"), "{shown}");
}

#[test]
fn a_value_that_breaks_a_constraint_is_not_confirmed() {
    // The evaluator of rules does not look at `@max`; the confirmation step does.
    let ir = compile_text("model C { n: int @max(5)\n rule r: n > 0 }");
    let q = only(&ir, "C");
    let answer = sat(q.encoded.as_ref().unwrap(), &[("self.n", "9")]);
    let report = ask(vec![q], &Canned::new(&[answer]), &Options::default());
    assert!(
        matches!(report.items[0].outcome, Outcome::Unknown { .. }),
        "{:?}",
        report.items[0].outcome
    );
}

#[test]
fn unsat_is_proved_and_names_the_core() {
    let ir =
        compile_text("model B { a: int\n c: int\n rule x: a > 0\n rule y: a < 0\n rule z: c > 0 }");
    let q = only(&ir, "B");
    let names: Vec<(String, String)> = q.encoded.as_ref().unwrap().named().to_vec();
    let core: Vec<&str> = names
        .iter()
        .filter(|(_, w)| w != "rule `z`")
        .map(|(n, _)| n.as_str())
        .collect();
    let answer = format!(
        "unsat\n({})\n(error \"model is not available\")",
        core.join(" ")
    );
    let report = ask(vec![q], &Canned::new(&[answer]), &Options::default());
    assert_eq!(
        report.items[0].outcome,
        Outcome::Proved {
            core: vec!["rule `x`".into(), "rule `y`".into()]
        }
    );
    assert!(report.items[0].is_finding());
    assert_eq!(report.findings().count(), 1);
}

#[test]
fn unknown_and_a_missing_answer_are_unknown() {
    let ir = compile_text(SUM);
    let report = ask(
        vec![only(&ir, "Ok")],
        &Canned::new(&["unknown\n(error \"x\")".into()]),
        &Options::default(),
    );
    assert!(matches!(report.items[0].outcome, Outcome::Unknown { .. }));
    assert!(!report.items[0].is_finding());
    // The solver was killed: nothing came back.
    let report = ask(
        vec![only(&ir, "Ok")],
        &Canned::new(&[]),
        &Options::default(),
    );
    assert!(matches!(report.items[0].outcome, Outcome::Unknown { .. }));
    assert_eq!(report.undecided().count(), 1);
}

#[test]
fn a_solver_that_cannot_start_leaves_every_question_unknown() {
    struct Broken;
    impl Backend for Broken {
        fn flavour(&self) -> Flavour {
            Flavour::Z3
        }
        fn run(&self, _: &str, _: usize, _: Duration) -> RunOutput {
            RunOutput {
                failed: Some("cannot start z3: not found".into()),
                ..RunOutput::default()
            }
        }
    }
    let ir = compile_text(SUM);
    let report = ask(vec![only(&ir, "Ok")], &Broken, &Options::default());
    let Outcome::Unknown { reason } = &report.items[0].outcome else {
        panic!()
    };
    assert!(reason.contains("cannot start"), "{reason}");
    assert!(report.solver.is_none());
}

#[test]
fn strings_come_back_as_distinct_tokens_of_the_asked_length() {
    let ir = compile_text("model S { s: string\n t: string\n rule r: s != t && len(s) == 2 }");
    let q = only(&ir, "S");
    let answer = sat(
        q.encoded.as_ref().unwrap(),
        &[
            ("self.s", "Str!val!0"),
            ("slen:self.s", "2"),
            ("self.t", "Str!val!1"),
            ("slen:self.t", "2"),
        ],
    );
    let report = ask(vec![q], &Canned::new(&[answer]), &Options::default());
    let Outcome::Counterexample { value } = &report.items[0].outcome else {
        panic!("{:?}", report.items[0].outcome);
    };
    assert_eq!(value, r#"{"s":"aa","t":"ab"}"#);
}

#[test]
fn equal_tokens_are_the_same_string() {
    let ir = compile_text("model S { s: string\n t: string\n rule r: s != t }");
    let q = only(&ir, "S");
    let answer = sat(
        q.encoded.as_ref().unwrap(),
        &[("self.s", "Str!val!0"), ("self.t", "Str!val!0")],
    );
    // The model is inconsistent with the rule, so the evaluator does not confirm it.
    let report = ask(vec![q], &Canned::new(&[answer]), &Options::default());
    assert!(matches!(report.items[0].outcome, Outcome::Unknown { .. }));
}

#[test]
fn an_unsupported_question_never_reaches_the_solver() {
    let old = compile_text("model M { x: float\n rule r: x * 2.0 > 1.0 }");
    let new = compile_text("model M { x: float\n rule r: x * 2.0 > 3.0 }");
    let (dold, dnew) = (&old.decls[0], &new.decls[0]);
    let changed = crate::diff::Changed::Invariant {
        old: (&old, dold, &dold.rules()[0]),
        new: (&new, dnew, &dnew.rules()[0]),
    };
    let q = crate::diff::tightened_question("t.M", "rule `r`", changed);
    let report = ask(vec![q], &Refuses, &Options::default());
    let Outcome::Unsupported { reason } = &report.items[0].outcome else {
        panic!("{:?}", report.items[0].outcome);
    };
    assert!(reason.contains("float"), "{reason}");
    assert_eq!(report.items[0].kind, QuestionKind::Tightened);
    assert!(!report.items[0].is_finding());
}

#[test]
fn a_policy_that_cannot_be_encoded_is_still_one_question() {
    let ir = compile_text(
        "model U { id: int }\nprincipal U\npolicy P(user: U) { permit when user.id > 0 }",
    );
    let questions = policy_questions(&ir);
    assert_eq!(questions.len(), 1);
    let answer = sat(
        questions[0].encoded.as_ref().unwrap(),
        &[("param.user.id", "3")],
    );
    let report = ask(questions, &Canned::new(&[answer]), &Options::default());
    let Outcome::Counterexample { value } = &report.items[0].outcome else {
        panic!("{:?}", report.items[0].outcome);
    };
    assert_eq!(value, r#"{"user":{"id":3}}"#);
}

#[test]
fn the_script_has_one_block_per_question_and_the_options_of_the_solver() {
    let ir = compile_text(SUM);
    let canned = Canned::new(&["unknown".into(), "unknown".into()]);
    let _ = ask(
        vec![only(&ir, "Ok"), only(&ir, "Ok")],
        &canned,
        &Options {
            timeout: Duration::from_secs(7),
        },
    );
    let script = canned.asked.borrow().clone().unwrap();
    assert!(script.contains("(set-option :timeout 7000)"));
    assert!(script.contains("(echo \"Q0\")") && script.contains("(echo \"Q1\")"));
    assert_eq!(script.matches("(push 1)").count(), 2);
    assert_eq!(script.matches("(pop 1)").count(), 2);
}
