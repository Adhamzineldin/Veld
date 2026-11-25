//! The text of a question for fixed contracts. These tests need no solver.

use super::compile_text;
use crate::problem::Problem;
use crate::question::{invariant_questions, policy_questions, require_questions};
use crate::types::OpenPart;

fn problem_of(source: &str, decl: &str) -> String {
    let ir = compile_text(source);
    let q = invariant_questions(&ir)
        .into_iter()
        .find(|q| q.subject.ends_with(decl))
        .unwrap();
    q.encoded.unwrap().script(0)
}

fn open_of(source: &str, decl: &str) -> Vec<OpenPart> {
    let ir = compile_text(source);
    let q = invariant_questions(&ir)
        .into_iter()
        .find(|q| q.subject.ends_with(decl))
        .unwrap();
    let p: Problem<'_> = q.encoded.unwrap();
    p.open_parts().iter().copied().collect()
}

#[test]
fn a_sum_of_ints_is_encoded_with_overflow_as_an_error() {
    let text = problem_of(
        "model AddInt { a: int\n b: int\n rule small: a + b < 100 }",
        "AddInt",
    );
    assert_eq!(
        text,
        "(push 1)\n\
         (declare-const |self.a| Int)\n\
         (assert (and (>= |self.a| (- 2147483648)) (<= |self.a| 2147483647)))\n\
         (declare-const |self.b| Int)\n\
         (assert (and (>= |self.b| (- 2147483648)) (<= |self.b| 2147483647)))\n\
         (define-fun d1 () Int (+ |self.a| |self.b|))\n\
         (define-fun d2 () Bool (or (< d1 (- 2147483648)) (> d1 2147483647)))\n\
         (define-fun d3 () Bool (< d1 100))\n\
         (assert (! (and (not d2) d3) :named n4))\n\
         (echo \"Q0\")\n\
         (check-sat)\n\
         (get-unsat-core)\n\
         (get-value (|self.a| |self.b|))\n\
         (pop 1)\n"
    );
}

#[test]
fn the_text_of_a_question_is_the_same_every_time() {
    let source = "model M { a: int\n b?: string\n rule r: has(b) implies a > len(b) }";
    assert_eq!(problem_of(source, "M"), problem_of(source, "M"));
}

#[test]
fn an_error_is_conditioned_by_short_circuiting() {
    // `b != 0 && a / b > 1`: the division by zero only counts where the left side is true.
    let text = problem_of(
        "model G { a: int\n b: int\n rule safe: b != 0 && a / b > 1 }",
        "G",
    );
    assert!(text.contains("(= |self.b| 0)"), "{text}");
    // The error of the right side is gated by the left side: `(and <left> <err of right>)`.
    assert!(
        text.contains("(define-fun d6 () Bool (and d1 d3))"),
        "{text}"
    );
    // No global assertion that the divisor is not zero.
    assert!(!text.contains("(assert (not (= |self.b| 0)))"), "{text}");
}

#[test]
fn division_truncates_toward_zero() {
    let text = problem_of("model D { a: int\n b: int\n rule h: a / b == -3 }", "D");
    assert!(text.contains("(ite (>= |self.a| 0)"), "{text}");
    assert!(!text.contains("(div |self.a| |self.b|)\n"), "{text}");
}

#[test]
fn the_open_parts_of_a_question_are_recorded() {
    let float = "model F { x: float\n y: float\n rule r: x * y < 1.5 }";
    assert_eq!(open_of(float, "F"), vec![OpenPart::Float]);
    let text = "model S { s: string\n rule r: matches(s, \"a+\") && startsWith(s, \"a\") }";
    assert_eq!(
        open_of(text, "S"),
        vec![OpenPart::StringContent, OpenPart::Matches]
    );
    let list = "model L { xs: int[]\n rule r: all(xs, x => x > 0) && sum(xs, x => x) < 5 }";
    assert_eq!(
        open_of(list, "L"),
        vec![OpenPart::Comprehension, OpenPart::Sum]
    );
    let exact = "model E { a: int\n rule r: a in [1, 2, 3] }";
    assert!(open_of(exact, "E").is_empty());
}

#[test]
fn an_enum_is_a_finite_set_and_in_is_a_disjunction() {
    let text = problem_of(
        "enum Status { pending, paid, shipped }\n\
         model O { status: Status\n rule r: status in [.paid, .shipped] }",
        "O",
    );
    assert!(
        text.contains("(and (>= |self.status| 0) (<= |self.status| 2))"),
        "{text}"
    );
    assert!(text.contains("(= |self.status| 1)"), "{text}");
    assert!(text.contains("(= |self.status| 2)"), "{text}");
}

#[test]
fn an_optional_field_has_a_boolean_beside_it() {
    let text = problem_of(
        "model O { a: int\n note?: string\n rule r: has(note) implies len(note) > a }",
        "O",
    );
    assert!(
        text.contains("(declare-const |self.note#has| Bool)"),
        "{text}"
    );
    assert!(text.contains("(slen |self.note|)"), "{text}");
}

#[test]
fn constraints_are_named_assertions_of_the_question() {
    let ir = compile_text(
        "model C { n: int @min(1) @max(9)\n s: string @length(min: 2, max: 4)\n rule r: n > len(s) }",
    );
    let q = invariant_questions(&ir).pop().unwrap();
    let p = q.encoded.unwrap();
    let names: Vec<&str> = p.named().iter().map(|(_, w)| w.as_str()).collect();
    assert!(names.contains(&"rule `r`"), "{names:?}");
    assert!(names.contains(&"constraint @min of `n`"), "{names:?}");
    assert!(names.contains(&"constraint @max of `n`"), "{names:?}");
    assert!(
        names.contains(&"constraint @length min of `s`"),
        "{names:?}"
    );
    assert!(
        names.contains(&"constraint @length max of `s`"),
        "{names:?}"
    );
}

#[test]
fn facts_and_the_principal_are_not_assumed_to_keep_their_constraints() {
    let ir = compile_text(
        "model Room { cap: int @min(1)\n }\n\
         model U { id: int @min(1) }\n\
         principal U\n\
         @auth(bearer)\n\
         service S at \"/s\" {\n\
           get \"/\" f(@query @min(5) n: int) -> Room {\n\
             fact room: Room = n\n\
             require ok: room.cap > 0 && user.id > 0 && n > 0\n\
           }\n\
         }",
    );
    let q = require_questions(&ir).pop().unwrap();
    let p = q.encoded.unwrap();
    let names: Vec<&str> = p.named().iter().map(|(_, w)| w.as_str()).collect();
    // The parameter is validated, so its constraint holds; the fact and the principal are
    // trusted values and keep none.
    assert!(names.iter().any(|n| n.contains("@min of `n`")), "{names:?}");
    assert!(!names.iter().any(|n| n.contains("`cap`")), "{names:?}");
    assert!(!names.iter().any(|n| n.contains("`id`")), "{names:?}");
}

#[test]
fn a_policy_permits_when_a_permit_holds_and_no_forbid_does() {
    let ir = compile_text(
        "model U { id: int\n suspended: bool }\n\
         principal U\n\
         policy P(user: U) {\n\
           permit when user.id > 0\n\
           forbid when user.suspended\n\
         }",
    );
    let q = policy_questions(&ir).pop().unwrap();
    let text = q.encoded.unwrap().script(0);
    assert!(
        text.contains("the policy permits") || text.contains(":named"),
        "{text}"
    );
    assert!(
        text.contains("(declare-const |param.user.suspended| Bool)"),
        "{text}"
    );
}

#[test]
fn audited_rules_are_not_assumed() {
    let ir =
        compile_text("model A { n: int\n rule fine: n > 0\n @rollout(audit) rule soft: n < 0 }");
    let q = invariant_questions(&ir).pop().unwrap();
    let p = q.encoded.unwrap();
    let names: Vec<&str> = p.named().iter().map(|(_, w)| w.as_str()).collect();
    assert_eq!(names, vec!["rule `fine`"]);
}
