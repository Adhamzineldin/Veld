//! Formatter behaviour for `@pii`, `@secret` and `@retain` (`spec/rules.md` section 19.3 and
//! `spec/platform.md` section 23): they survive a round trip, formatting is idempotent, and
//! comments around them stay.

use super::format;

fn f(src: &str) -> String {
    format(src)
        .unwrap_or_else(|d| format!("ERR {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>()))
}

fn stable(src: &str) -> String {
    let once = f(src);
    assert_eq!(
        f(&once),
        once,
        "not idempotent for:\n{src}\n--- first pass:\n{once}"
    );
    once
}

#[test]
fn pii_and_secret_stay_on_their_field_in_one_line() {
    let out = stable(
        "model M {\n id: uuid\n name:string   @pii\n  @secret  key: string\n both: string @secret   @pii\n}",
    );
    assert!(out.contains("  name: string @pii\n"), "{out}");
    assert!(out.contains("  key: string @secret\n"), "{out}");
    assert!(out.contains("  both: string @secret @pii\n"), "{out}");
}

#[test]
fn retain_keeps_its_arguments_and_its_place() {
    let out = stable(
        "@retain( days( 30 ) ,since:checkedOutAt,   then:anonymize )\nmodel Guest {\n  checkedOutAt?: datetime\n  name: string @pii\n}",
    );
    assert!(
        out.starts_with("@retain(days(30), since: checkedOutAt, then: anonymize)\nmodel Guest {"),
        "{out}"
    );
}

#[test]
fn comments_around_the_annotations_stay() {
    let src = "// the clock starts at checkout\n/// A guest.\n@retain(days(1), since: day, then: delete)\nmodel Log {\n  // the day\n  day: date // when\n  who: string @pii // personal\n}\n";
    let out = stable(src);
    assert_eq!(out, src);
}

#[test]
fn the_words_stay_names_where_they_are_names() {
    let out = stable("model M {\n  pii: string\n  secret: int\n  retain: bool\n}");
    assert!(out.contains("  pii: string\n"), "{out}");
}
