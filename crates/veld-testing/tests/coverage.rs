//! `veld test --coverage` (`spec/rules.md` section 19.1): the numbers and the exact report text.
//! The contract is `fixtures/coverage.veld`, written for these tests; its comments say which state
//! each rule is in.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_testing::coverage::{Coverage, UnitKind, measure, units};
use veld_testing::select;

const FIXTURE: &str = include_str!("fixtures/coverage.veld");

fn compile_text(text: &str) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text: text.to_owned(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let Some(ir) = out.ir.clone() else {
        panic!("does not compile:\n{}", out.render());
    };
    ir
}

fn coverage_of(text: &str) -> Coverage {
    let ir = compile_text(text);
    let examples = select(&ir, None);
    measure(&ir, &examples, &|_| true)
}

#[test]
fn the_report_names_every_rule_that_is_not_covered_with_what_is_missing() {
    let c = coverage_of(FIXTURE);
    let expected = [
        "coverage: 5 of 12 rules held and violated (41%)",
        "  t/NewStay.atMost30Days                             never violated",
        "  t/Closure.reasonGiven                              never evaluated",
        "  t/Bookings.payBooking allow ManageBooking          never denied",
        "  t/Bookings.payBooking transition pending -> paid   never illegal",
        "  t/Bookings.payBooking ensure paidTotal             never evaluated",
        "  t/Bookings.payBooking ensure #2                    never evaluated",
        "  t/Bookings.closeBooking require reasonNeeded       never evaluated",
    ];
    assert_eq!(c.report(), expected);
}

#[test]
fn the_percentage_is_rounded_down() {
    // 5 of 12 is 41.67 per cent.
    let c = coverage_of(FIXTURE);
    assert_eq!((c.covered(), c.units.len()), (5, 12));
    assert_eq!(c.percent(), 41);
}

#[test]
fn every_kind_of_unit_is_counted_once() {
    let ir = compile_text(FIXTURE);
    let all = units(&ir, &|_| true);
    let count = |k: UnitKind| all.iter().filter(|u| u.kind == k).count();
    assert_eq!(count(UnitKind::Invariant), 4);
    assert_eq!(count(UnitKind::Allow), 2);
    assert_eq!(count(UnitKind::Transition), 2);
    assert_eq!(count(UnitKind::Require), 2);
    assert_eq!(count(UnitKind::Ensure), 2);
}

#[test]
fn an_audited_rule_counts_like_an_enforced_one() {
    let c = coverage_of(FIXTURE);
    let small_party = c
        .units
        .iter()
        .find(|(u, _)| u.label == "t/NewStay.smallParty")
        .unwrap();
    assert!(small_party.1.covered());
}

#[test]
fn a_rule_the_examples_never_reach_is_never_evaluated_and_not_covered() {
    let c = coverage_of(FIXTURE);
    let closure = c
        .units
        .iter()
        .find(|(u, _)| u.label == "t/Closure.reasonGiven")
        .unwrap();
    assert!(!closure.1.held && !closure.1.violated);
}

#[test]
fn an_ensure_is_evaluated_only_by_an_example_with_returns() {
    let more = r#"
example "the guest pays and the handler answers" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa", total: 10.00 }
  as { id: "00000000-0000-4000-8000-0000000000aa", roles: [] }
  payBooking(id: booking.id, body: { amount: 10.00 }) returns { status: .paid, total: 10.00 } -> ok
}

example "a handler that forgets to mark it paid is an internal error" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa", total: 10.00 }
  as { id: "00000000-0000-4000-8000-0000000000aa", roles: [] }
  payBooking(id: booking.id, body: { amount: 10.00 }) returns { status: .pending, total: 10.00 } -> Internal
}
"#;
    let c = coverage_of(&format!("{FIXTURE}{more}"));
    let expected = [
        "coverage: 6 of 12 rules held and violated (50%)",
        "  t/NewStay.atMost30Days                             never violated",
        "  t/Closure.reasonGiven                              never evaluated",
        "  t/Bookings.payBooking allow ManageBooking          never denied",
        "  t/Bookings.payBooking transition pending -> paid   never illegal",
        "  t/Bookings.payBooking ensure paidTotal             never violated",
        "  t/Bookings.closeBooking require reasonNeeded       never evaluated",
    ];
    assert_eq!(c.report(), expected);
}

#[test]
fn a_rule_that_was_only_violated_says_so() {
    // Only the example that refuses the amount: the `require` was violated and never held.
    let ir = compile_text(FIXTURE);
    let examples: Vec<_> = select(&ir, Some("a wrong amount"));
    let c = measure(&ir, &examples, &|_| true);
    let line = c
        .report()
        .into_iter()
        .find(|l| l.contains("require amountMatches"))
        .unwrap();
    assert!(line.ends_with("never held"), "{line}");
}

#[test]
fn a_workspace_without_rules_has_no_coverage_to_report() {
    let c = coverage_of("model A { n: int }\nservice S at \"/s\" { get \"/\" list() -> A }\n");
    assert_eq!(c.report(), ["coverage: no rules"]);
    assert_eq!(c.percent(), 100);
}

#[test]
fn a_workspace_with_rules_and_no_examples_covers_nothing() {
    let text = FIXTURE
        .split("example \"a stay is accepted\"")
        .next()
        .unwrap();
    let c = coverage_of(text);
    assert_eq!(c.covered(), 0);
    assert_eq!(c.percent(), 0);
    assert_eq!(
        c.report()[0],
        "coverage: 0 of 12 rules held and violated (0%)"
    );
    assert_eq!(c.report().len(), 13);
}

#[test]
fn packages_the_selection_leaves_out_have_no_units() {
    let ir = compile_text(FIXTURE);
    let examples = select(&ir, None);
    let c = measure(&ir, &examples, &|package| package != "t");
    assert_eq!(c.report(), ["coverage: no rules"]);
}

#[test]
fn two_runs_give_the_same_report() {
    assert_eq!(coverage_of(FIXTURE).report(), coverage_of(FIXTURE).report());
}
