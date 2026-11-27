//! Parsing, searching and sampling. No solver is needed.

use std::ffi::OsString;
use std::path::PathBuf;

use serde_json::json;
use veld_ir::{Constraints, Scalar, Type};

use crate::sample::{check_scalar, sample, zero_bytes};
use crate::sexp::{Sexp, parse_all};
use crate::solver::{Flavour, NotFound, Verdict, find, parse_answers, preamble};
use crate::values::{decimal_text, gen_str};

fn one(text: &str) -> Sexp {
    parse_all(text).remove(0)
}

#[test]
fn a_solver_value_is_read_as_a_number() {
    assert_eq!(one("(- 5)").as_int(), Some(-5));
    assert_eq!(one("12").as_int(), Some(12));
    assert_eq!(one("(/ 1.0 3.0)").as_rational(), Some((1, 3)));
    assert_eq!(one("(- (/ 1.0 3.0))").as_rational(), Some((-1, 3)));
    assert_eq!(one("2.50").as_rational(), Some((5, 2)));
    assert_eq!(one("(/ 1.0 (- 2.0))").as_rational(), Some((-1, 2)));
    assert_eq!(one("true").as_bool(), Some(true));
    assert_eq!(one("x").as_int(), None);
}

#[test]
fn quoted_symbols_strings_and_errors_are_read() {
    let all = parse_all("(error \"line 1: a \"\"quoted\"\" word\") |a b| (:name \"Z3\") (open");
    assert_eq!(all.len(), 3, "an unfinished expression is dropped: {all:?}");
    assert!(all[0].is_error());
    assert_eq!(all[1].atom(), Some("a b"));
    assert!(!all[2].is_error());
}

const Z3_OUTPUT: &str = "(:name \"Z3\")\n(:version \"4.8.12\")\nQ0\nunsat\n(n1 n3)\n\
(error \"line 19 column 16: model is not available\")\nQ1\nsat\n\
(error \"line 30 column 15: unsat core is not available\")\n\
((|self.x| 4)\n (|self.s| Str!val!0)\n (|self.r| (/ 1.0 3.0))\n ((slen |self.s|) 0))\n\
Q2\nunknown\n(error \"line 3: unsat core is not available\")\n(error \"model is not available\")\n";

#[test]
fn the_output_of_a_batch_is_split_on_its_markers() {
    let answers = parse_answers(Z3_OUTPUT, 4);
    let info = answers.info.unwrap();
    assert_eq!(
        (info.name.as_str(), info.version.as_str()),
        ("Z3", "4.8.12")
    );
    assert_eq!(answers.verdicts.len(), 4);
    assert_eq!(
        answers.verdicts[0],
        Verdict::Unsat(vec!["n1".into(), "n3".into()])
    );
    let Verdict::Sat(values) = &answers.verdicts[1] else {
        panic!("{:?}", answers.verdicts[1]);
    };
    assert_eq!(values.len(), 4);
    assert_eq!(values[0].as_int(), Some(4));
    assert_eq!(values[1].atom(), Some("Str!val!0"));
    assert!(matches!(answers.verdicts[2], Verdict::Unknown(_)));
    // The solver was killed before it answered the fourth question.
    assert!(matches!(answers.verdicts[3], Verdict::Unknown(_)));
}

#[test]
fn no_output_at_all_is_unknown() {
    let answers = parse_answers("", 2);
    assert!(answers.info.is_none());
    assert!(
        answers
            .verdicts
            .iter()
            .all(|v| matches!(v, Verdict::Unknown(_)))
    );
}

#[test]
fn the_preamble_depends_on_the_solver() {
    let t = std::time::Duration::from_secs(5);
    let z3 = preamble(Flavour::Z3, t);
    assert!(z3.contains("(set-option :timeout 5000)"));
    assert!(z3.contains("(set-option :random-seed 0)"));
    let cvc5 = preamble(Flavour::Cvc5, t);
    assert!(cvc5.contains("(set-option :tlimit-per 5000)"));
    assert!(cvc5.contains("(set-option :seed 0)"));
    assert!(z3.contains("(declare-fun slen (Str) Int)"));
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("veld-prove-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_solver_is_found_in_the_order_the_spec_gives() {
    let dir = scratch_dir("find");
    let (a, b) = (dir.join("named-solver"), dir.join("z3.exe"));
    std::fs::write(&a, "").unwrap();
    std::fs::write(&b, "").unwrap();
    let path = OsString::from(dir.as_os_str());
    // `--solver` first, then VELD_SOLVER, then PATH.
    let env = OsString::from(b.as_os_str());
    assert_eq!(find(Some(&a), Some(&env), Some(&path)).unwrap().program, a);
    assert_eq!(find(None, Some(&env), Some(&path)).unwrap().program, b);
    assert_eq!(find(None, None, Some(&path)).unwrap().program, b);
    // Nothing on PATH and nothing named: the one-line answer.
    let empty = OsString::new();
    assert_eq!(find(None, None, Some(&empty)), Err(NotFound::None));
    assert_eq!(find(None, None, None), Err(NotFound::None));
    // A named solver that is not there is an error, not a silent search.
    let missing = dir.join("nope");
    assert!(matches!(
        find(Some(&missing), None, Some(&path)),
        Err(NotFound::Missing(_))
    ));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_cvc5_is_recognised_by_its_file_name() {
    let dir = scratch_dir("cvc5");
    let program = dir.join("cvc5");
    std::fs::write(&program, "").unwrap();
    let path = OsString::from(dir.as_os_str());
    assert_eq!(
        find(None, None, Some(&path)).unwrap().flavour,
        Flavour::Cvc5
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn z3_is_looked_up_before_cvc5() {
    let dir = scratch_dir("both");
    std::fs::write(dir.join("cvc5"), "").unwrap();
    std::fs::write(dir.join("z3"), "").unwrap();
    let path = OsString::from(dir.as_os_str());
    assert_eq!(find(None, None, Some(&path)).unwrap().flavour, Flavour::Z3);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_filler_respects_constraints() {
    let ir = super::compile_text("model X { a: int }");
    let int = Type::Scalar(Scalar::Int);
    let c = Constraints {
        min: Some(5.0),
        ..Constraints::default()
    };
    assert_eq!(sample(&ir, &int, &c, 0).unwrap(), json!(5));
    let c = Constraints {
        max: Some(-2.0),
        ..Constraints::default()
    };
    assert_eq!(sample(&ir, &int, &c, 0).unwrap(), json!(-2));
    let s = Type::Scalar(Scalar::String);
    let c = Constraints {
        min_len: Some(3),
        pattern: Some("^a+$".into()),
        ..Constraints::default()
    };
    assert_eq!(sample(&ir, &s, &c, 0).unwrap(), json!("aaa"));
    let c = Constraints {
        pattern: Some("^[0-9]+$".into()),
        ..Constraints::default()
    };
    assert_eq!(sample(&ir, &s, &c, 0).unwrap(), json!("0"));
}

#[test]
fn constraints_are_checked_on_a_value() {
    let int = Type::Scalar(Scalar::Int);
    let c = Constraints {
        min: Some(1.0),
        max: Some(9.0),
        ..Constraints::default()
    };
    assert!(check_scalar(&int, &c, &json!(5)).is_ok());
    assert!(check_scalar(&int, &c, &json!(0)).is_err());
    assert!(check_scalar(&int, &c, &json!(10)).is_err());
    let s = Type::Scalar(Scalar::String);
    let c = Constraints {
        min_len: Some(2),
        max_len: Some(3),
        ..Constraints::default()
    };
    assert!(check_scalar(&s, &c, &json!("ab")).is_ok());
    assert!(check_scalar(&s, &c, &json!("a")).is_err());
    assert!(check_scalar(&s, &c, &json!("abcd")).is_err());
    // Code points, not bytes.
    assert!(check_scalar(&s, &c, &json!("\u{1F600}\u{1F600}")).is_ok());
}

#[test]
fn zero_bytes_have_the_length_asked_for() {
    for n in 0..8 {
        let text = zero_bytes(n);
        assert_eq!(veld_rules::helpers::text::base64_byte_len(&text), Some(n));
    }
}

#[test]
fn a_real_becomes_exact_decimal_text_when_it_can() {
    assert_eq!(decimal_text(3, 2).as_deref(), Some("1.5"));
    assert_eq!(decimal_text(-1, 4).as_deref(), Some("-0.25"));
    assert_eq!(decimal_text(7, 1).as_deref(), Some("7"));
    assert_eq!(decimal_text(10, 30), None);
    assert_eq!(decimal_text(1, 8).as_deref(), Some("0.125"));
}

#[test]
fn distinct_tokens_get_distinct_texts_of_the_same_length() {
    assert_eq!(gen_str(0, 3).as_deref(), Some("aaa"));
    assert_eq!(gen_str(1, 3).as_deref(), Some("aab"));
    assert_eq!(gen_str(26, 3).as_deref(), Some("aba"));
    assert_eq!(gen_str(26, 1), None);
    assert_eq!(gen_str(0, 0).as_deref(), Some(""));
    assert_eq!(gen_str(1, 0), None);
}
