//! `veld diff` and `@version`, `@idempotent` (`spec/rules.md` sections 11.7 and 11.8): every row of
//! the table in `conditional.rs`, and the errors the features add, which are not reported twice.

use super::ChangeKind::{self, *};
use super::Severity::{self, Breaking as B, NonBreaking as N};
use super::diff;
use crate::testutil::ir;

fn kinds(old: &str, new: &str) -> Vec<(ChangeKind, Severity)> {
    let mut v: Vec<_> = diff(&ir(old), &ir(new))
        .into_iter()
        .map(|c| (c.kind, c.severity))
        .collect();
    v.sort();
    v
}

fn expect(old: &str, new: &str, want: &[(ChangeKind, Severity)]) {
    let mut w = want.to_vec();
    w.sort();
    assert_eq!(kinds(old, new), w, "old:\n{old}\nnew:\n{new}");
}

fn room(version: &str) -> String {
    format!(
        "model Room {{\n  id: uuid\n  name: string\n  {version}\n  revision: long\n  other: int\n}}\n"
    )
}

const PLAIN: &str = "";
const MARKED: &str = "@version";

const GET: &str = "service S at \"/s\" { get \"/{id}\" one(id: uuid) -> Room | NotFound }";
const GUARDED: &str = "service S at \"/s\" {\n  put \"/{id}\" save(id: uuid) -> Room | NotFound {\n    fact room: Room = id\n  }\n}";

// --- the field ---------------------------------------------------------------------------------

#[test]
fn a_version_added_to_a_field_is_non_breaking_and_removing_it_is_breaking() {
    let (plain, marked) = (
        format!("{}{GET}", room(PLAIN)),
        format!("{}{GET}", room(MARKED)),
    );
    expect(&plain, &marked, &[(VersionAdded, N)]);
    expect(&marked, &plain, &[(VersionRemoved, B)]);
}

#[test]
fn another_field_becoming_the_version_is_breaking() {
    let old = "model Room { id: uuid\n @version\n a: int\n b: int }\nservice S at \"/s\" { get \"/\" one() -> Room }";
    let new = "model Room { id: uuid\n a: int\n @version\n b: int }\nservice S at \"/s\" { get \"/\" one() -> Room }";
    expect(old, new, &[(VersionFieldChanged, B)]);
}

// --- the guard -----------------------------------------------------------------------------------

#[test]
fn a_guard_that_arrives_with_the_field_is_non_breaking_and_brings_no_error_change() {
    // `PreconditionFailed` and the `ValidationError` of a malformed header are the guard's own.
    let (plain, marked) = (
        format!("{}{GUARDED}", room(PLAIN)),
        format!("{}{GUARDED}", room(MARKED)),
    );
    expect(&plain, &marked, &[(VersionAdded, N), (GuardAdded, N)]);
}

#[test]
fn a_guard_that_goes_with_the_field_is_breaking_once_per_cause() {
    let (plain, marked) = (
        format!("{}{GUARDED}", room(PLAIN)),
        format!("{}{GUARDED}", room(MARKED)),
    );
    expect(&marked, &plain, &[(VersionRemoved, B), (GuardRemoved, B)]);
}

#[test]
fn a_guard_that_follows_another_fact_is_breaking() {
    let two = |first: &str, second: &str| {
        format!(
            "{}model Plan {{ id: uuid
 @version
 step: int }}
service S at \"/s\" {{
  put \"/{{id}}\" save(id: uuid) -> Room {{
    fact {first}
    fact {second}
  }}
}}",
            room(MARKED)
        )
    };
    let (a, b) = (
        two("a: Room = id", "b: Plan = id"),
        two("b: Plan = id", "a: Room = id"),
    );
    expect(&a, &a, &[]);
    let found = kinds(&a, &b);
    assert!(found.contains(&(GuardChanged, B)), "{found:?}");
}

#[test]
fn an_error_that_is_not_the_guards_is_still_reported() {
    let with_error = format!(
        "{}error Full 409 {{ n: int }}\nservice S at \"/s\" {{\n  put \"/{{id}}\" save(id: uuid) -> Room | NotFound | Full {{\n    fact room: Room = id\n  }}\n}}",
        room(MARKED)
    );
    let plain = with_error.replace(" | Full", "");
    expect(&plain, &with_error, &[(ErrorAdded, B)]);
    // And the guard arriving next to it still says nothing of its own errors.
    let unmarked = with_error.replace("@version\n", "").replace(" | Full", "");
    let found = kinds(&unmarked, &with_error);
    assert!(
        found.contains(&(GuardAdded, N)) && found.contains(&(ErrorAdded, B)),
        "{found:?}"
    );
    assert_eq!(
        found.iter().filter(|(k, _)| *k == ErrorAdded).count(),
        1,
        "{found:?}"
    );
}

// --- idempotency ---------------------------------------------------------------------------------

fn keyed(annotation: &str, errors: &str) -> String {
    format!(
        "model M {{ a: int }}\nerror Full 409 {{ n: int }}\nservice S at \"/s\" {{\n  {annotation}\n  post \"/\" make(body: M) -> 201 M{errors}\n}}"
    )
}

#[test]
fn adding_idempotent_is_non_breaking_unless_the_key_is_required() {
    expect(
        &keyed("", ""),
        &keyed("@idempotent", ""),
        &[(IdempotencyAdded, N)],
    );
    expect(
        &keyed("", ""),
        &keyed("@idempotent(required: true)", ""),
        &[(IdempotencyAdded, B)],
    );
}

#[test]
fn removing_idempotent_is_breaking_and_brings_no_error_change() {
    expect(
        &keyed("@idempotent", ""),
        &keyed("", ""),
        &[(IdempotencyRemoved, B)],
    );
    expect(
        &keyed("@idempotent(required: true)", ""),
        &keyed("", ""),
        &[(IdempotencyRemoved, B)],
    );
}

#[test]
fn required_and_the_header_are_judged_by_what_a_caller_sends() {
    expect(
        &keyed("@idempotent", ""),
        &keyed("@idempotent(required: true)", ""),
        &[(IdempotencyRequiredChanged, B)],
    );
    expect(
        &keyed("@idempotent(required: true)", ""),
        &keyed("@idempotent", ""),
        &[(IdempotencyRequiredChanged, N)],
    );
    expect(
        &keyed("@idempotent", ""),
        &keyed("@idempotent(header: \"X-Request-Id\")", ""),
        &[(IdempotencyHeaderChanged, B)],
    );
    expect(
        &keyed("@idempotent", ""),
        &keyed("@idempotent(header: \"X-Request-Id\", required: true)", ""),
        &[
            (IdempotencyHeaderChanged, B),
            (IdempotencyRequiredChanged, B),
        ],
    );
}

#[test]
fn a_conflict_the_endpoint_already_declared_is_no_change_and_another_error_is() {
    // `Conflict` was listed by hand before: adding `@idempotent` changes no error.
    expect(
        &keyed("", " | Conflict"),
        &keyed("@idempotent", " | Conflict"),
        &[(IdempotencyAdded, N)],
    );
    expect(
        &keyed("", ""),
        &keyed("@idempotent", " | Full"),
        &[(IdempotencyAdded, N), (ErrorAdded, B)],
    );
}

#[test]
fn a_contract_that_does_not_change_has_no_changes() {
    let src = format!("{}{GUARDED}", room(MARKED));
    expect(&src, &src, &[]);
    expect(&keyed("@idempotent", ""), &keyed("@idempotent", ""), &[]);
}
