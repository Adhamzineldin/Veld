//! The questions of section 18.1 on small contracts, with a real solver. Each test asks all of
//! its questions in one solver call.

use super::{compile_text, solver};
use crate::diff::Changed;
use crate::report::{Item, Outcome, QuestionKind, Report};
use crate::run::{ChangedRule, Options, check, diff};

fn item<'a>(r: &'a Report, kind: QuestionKind, name: &str) -> &'a Item {
    r.items
        .iter()
        .find(|i| i.kind == kind && i.name().contains(name))
        .unwrap_or_else(|| panic!("no question `{name}` in {:#?}", r.items))
}

fn proved(r: &Report, name: &str) -> Vec<String> {
    match &item(r, QuestionKind::Invariants, name).outcome {
        Outcome::Proved { core } => core.clone(),
        other => panic!("{name}: {other:?}"),
    }
}

fn has_value(r: &Report, kind: QuestionKind, name: &str) -> String {
    match &item(r, kind, name).outcome {
        Outcome::Counterexample { value } => value.clone(),
        other => panic!("{name}: {other:?}"),
    }
}

const MODELS: &str = r#"
enum Status { pending, paid, shipped }

model Exact1 { a: int
  b: int
  rule r: a + b < 0 && a > 0 && b > 0 }
model Exact2 { x: int
  rule r: x == 0 || 10 / x > 1 }
model Exact3 { s: string
  rule r: s == "a" && s == "b" }
model Exact4 { s: string
  rule r: len(s) > 3 && len(s) < 2 }
model Exact5 { status: Status
  rule r: status in [.paid] && status == .pending }
model Exact6 { note?: string
  rule r: has(note) && !has(note) }
model Exact7 { p: decimal
  rule r: p > 1.5 && p < 1.25 }
model Exact8 { xs: int[]
  rule r: len(xs) > 3 && len(xs) < 2 }
model Exact9 { n: int @max(9)
  rule r: n > 20 }
model Exact10 { t: datetime
  d: duration2
  rule r: t > t }
model Open1 { x: float
  y: float
  rule r: x * y < 1.5 && x * y > 2.5 }
model Open2 { s: string
  rule r: startsWith(s, "a") && !startsWith(s, "a") }
model Find1 { p: decimal
  rule r: p * 2 == 3 }
model Find2 { x: int
  rule r: x % 7 == -3 }
model Find3 { start: datetime
  end: datetime
  rule r: end - start == hours(3) }
model Find4 { xs: int[]
  rule r: len(xs) == 2 }
model Find5 { tags: string[] @length(max: 3)
  note?: string
  rule r: len(tags) > 1 && has(note) }
"#;

#[test]
fn contradictions_of_invariants_are_proved_and_everything_else_is_not_claimed() {
    let Some(s) = solver("invariants") else {
        return;
    };
    let source = MODELS.replace("  d: duration2\n", "");
    let ir = compile_text(&source);
    let report = check(&ir, &s, &Options::default());
    // The contradictions, proved. `a + b` that stays in range cannot be negative.
    for name in [
        "Exact1", "Exact3", "Exact4", "Exact5", "Exact6", "Exact7", "Exact8", "Exact10",
    ] {
        assert_eq!(proved(&report, name), vec!["rule `r`".to_owned()], "{name}");
    }
    // The constraints are part of the question, and the core says so.
    assert_eq!(
        proved(&report, "Exact9"),
        vec!["constraint @max of `n`".to_owned(), "rule `r`".to_owned()]
    );
    // Short-circuiting: dividing by zero is only an error where it is evaluated.
    let v = has_value(&report, QuestionKind::Invariants, "Exact2");
    assert!(v.contains("\"x\":"), "{v}");
    // A part left open never proves anything: the evaluator does not confirm the value.
    for name in ["Open1", "Open2"] {
        let i = item(&report, QuestionKind::Invariants, name);
        assert!(!i.is_finding(), "{name}: {:?}", i.outcome);
        assert!(!i.open.is_empty(), "{name}");
    }
    // Values the solver found, in the form the evaluator confirmed.
    assert_eq!(
        has_value(&report, QuestionKind::Invariants, "Find1"),
        r#"{"p":"1.5"}"#
    );
    let seven = has_value(&report, QuestionKind::Invariants, "Find2");
    assert!(
        seven.contains("-3") || seven.contains("-10") || seven.contains("-17"),
        "{seven}"
    );
    let find3 = has_value(&report, QuestionKind::Invariants, "Find3");
    assert!(
        find3.contains("\"start\":") && find3.contains("\"end\":"),
        "{find3}"
    );
    assert!(has_value(&report, QuestionKind::Invariants, "Find4").contains("[0,0]"));
    let find5 = has_value(&report, QuestionKind::Invariants, "Find5");
    assert!(
        find5.contains("\"tags\":[") && find5.contains("\"note\":"),
        "{find5}"
    );
    // Nothing the report says is "verified" or "correct".
    let text = format!("{:?}{:?}", report.closing_lines(), report.items).to_lowercase();
    assert!(!text.contains("verified") && !text.contains("correct"));
}

const ENDPOINTS: &str = r#"
model Room { cap: int @max(5) }
model Account { id: uuid
  age: int @max(120)
  suspended: bool }
principal Account

model Req { n: int @max(3)
  note?: string }

policy NobodyAtAll(user: Account) { permit when user.age > 10 && user.age < 5 }
policy ForbidWins(user: Account) {
  permit when user.age > 0
  forbid when true
}
policy Adults(user: Account) {
  permit when user.age >= 18
  forbid when user.suspended
}
policy Faulty(user: Account) { permit when 10 / user.age > 1 }

@auth(bearer)
service S at "/s" {
  post "/a" never(body: Req) -> 204 {
    require tooBig: body.n > 5
  }
  post "/b" sequence(body: Req) -> 204 {
    require first: body.n > 1
    require second: body.n < 1
  }
  post "/c" facts(body: Req) -> 204 {
    fact room: Room = body.n
    require roomy: room.cap > 100
    require older: user.age > 200
  }
  post "/d" optional(body: Req) -> 204 {
    require noted: has(body.note) && len(body.note) > 2
  }
}
"#;

#[test]
fn a_require_that_can_never_pass_and_a_policy_that_permits_nobody() {
    let Some(s) = solver("endpoints") else {
        return;
    };
    let ir = compile_text(ENDPOINTS);
    let report = check(&ir, &s, &Options::default());
    let core = |name: &str| match &item(&report, QuestionKind::Require, name).outcome {
        Outcome::Proved { core } => core.clone(),
        other => panic!("{name}: {other:?}"),
    };
    // A parameter keeps its constraints, so `n > 5` cannot pass with `@max(3)`.
    assert_eq!(
        core("tooBig"),
        vec![
            "constraint @max of `n`".to_owned(),
            "require `tooBig`".to_owned()
        ]
    );
    // The lines before a require are assumed to have passed.
    assert_eq!(
        core("second"),
        vec!["require `first`".to_owned(), "require `second`".to_owned()]
    );
    // A fact, and the principal, are trusted values of the application: no constraint is
    // assumed for them, so the requires on them can pass.
    let roomy = has_value(&report, QuestionKind::Require, "roomy");
    assert!(roomy.contains("fact room"), "{roomy}");
    let older = has_value(&report, QuestionKind::Require, "older");
    assert!(older.contains("principal"), "{older}");
    // An optional value is read under its guard.
    let noted = has_value(&report, QuestionKind::Require, "noted");
    assert!(noted.contains("\"note\":"), "{noted}");
    // Policies.
    let policy = |name: &str| item(&report, QuestionKind::Policy, name);
    assert!(policy("NobodyAtAll").is_finding());
    assert!(policy("ForbidWins").is_finding());
    assert!(!policy("Adults").is_finding());
    // A clause that cannot be evaluated does not permit.
    assert!(
        !policy("Faulty").is_finding(),
        "{:?}",
        policy("Faulty").outcome
    );
    assert!(has_value(&report, QuestionKind::Policy, "Faulty").contains("\"age\":"));
}

fn rule_pair<'a>(old: &'a veld_ir::Ir, new: &'a veld_ir::Ir, decl: &str) -> Changed<'a> {
    let find = |ir: &'a veld_ir::Ir| {
        let d = ir.decls.iter().find(|d| d.name == decl).unwrap();
        (ir, d, &d.rules()[0])
    };
    Changed::Invariant {
        old: find(old),
        new: find(new),
    }
}

fn ask_diff(pairs: &[(&str, &str, &str)]) -> Report {
    let Some(s) = solver("diff") else {
        return Report {
            solver: None,
            seed: 0,
            timeout: Options::default().timeout,
            items: Vec::new(),
        };
    };
    let sources: Vec<(veld_ir::Ir, veld_ir::Ir, &str)> = pairs
        .iter()
        .map(|(decl, old, new)| (compile_text(old), compile_text(new), *decl))
        .collect();
    let rules: Vec<ChangedRule<'_>> = sources
        .iter()
        .map(|(old, new, decl)| ChangedRule {
            subject: (*decl).to_owned(),
            detail: "rule `r`".to_owned(),
            changed: rule_pair(old, new, decl),
        })
        .collect();
    diff(&rules, &s, &Options::default())
}

#[test]
fn a_changed_rule_is_tightened_with_a_confirmed_value_or_proved_not() {
    let report = ask_diff(&[
        (
            "A",
            "model A { x: int\n rule r: x > 0 }",
            "model A { x: int\n rule r: x > 5 }",
        ),
        (
            "B",
            "model B { x: int\n rule r: x > 5 }",
            "model B { x: int\n rule r: x > 0 }",
        ),
        (
            "C",
            "model C { x: int\n rule r: x > 0 }",
            "model C { x: int\n rule r: x >= 1 }",
        ),
        (
            "D",
            "model D { x: int\n rule r: x > 0 }",
            "model D { x: float\n rule r: x > 0.0 }",
        ),
        (
            "E",
            "enum K { a, b, c }\nmodel E { k: K\n rule r: k in [.a, .b] }",
            "enum K { a, b, c }\nmodel E { k: K\n rule r: k in [.a] }",
        ),
        (
            "F",
            "model F { x: int\n rule r: x / 1 > 0 }",
            "model F { x: int\n rule r: x > 0 }",
        ),
        (
            "G",
            "model G { x: float\n rule r: x > 0.0 }",
            "model G { x: float\n rule r: x * 2.0 > 0.0 }",
        ),
    ]);
    if report.items.is_empty() {
        return;
    }
    let value = |name: &str| match &item(&report, QuestionKind::Tightened, name).outcome {
        Outcome::Counterexample { value } => value.clone(),
        other => panic!("{name}: {other:?}"),
    };
    // Tighter: a value the old rule accepts and the new one rejects, confirmed.
    let a = value("A");
    let x: i64 = a
        .trim_start_matches("{\"x\":")
        .trim_end_matches('}')
        .parse()
        .unwrap();
    assert!((1..=5).contains(&x), "{a}");
    // Looser, and the same rule written differently: proved not tightened.
    for name in ["B", "C", "F"] {
        let i = item(&report, QuestionKind::Tightened, name);
        assert!(
            matches!(i.outcome, Outcome::Proved { .. }),
            "{name}: {:?}",
            i.outcome
        );
        assert!(!i.is_finding());
    }
    // An enum literal: `b` was accepted and no longer is.
    assert_eq!(value("E"), r#"{"k":"b"}"#);
    // A float is left open, where a claim about every value would not be sound.
    for name in ["D", "G"] {
        let i = item(&report, QuestionKind::Tightened, name);
        assert!(
            matches!(i.outcome, Outcome::Unsupported { .. }),
            "{name}: {:?}",
            i.outcome
        );
    }
    assert_eq!(report.counts().unsupported, 2);
}
