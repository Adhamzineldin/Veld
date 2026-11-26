//! Formatter behaviour for the annotations of platform phase P1 (`spec/platform.md` sections 2 to
//! 5): they survive a round trip, formatting is idempotent, and comments around them stay.

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
fn visible_prints_its_expression_in_the_house_style() {
    let out = stable(
        "principal User\nmodel Booking {\n  guestId: uuid\n  guestPhone?: string   @visible( when:user.id==guestId||\"support\" in user.roles )\n}",
    );
    assert!(
        out.contains(
            "  guestPhone?: string @visible(when: user.id == guestId || \"support\" in user.roles)\n"
        ),
        "{out}"
    );
}

#[test]
fn visible_before_the_field_moves_nowhere_and_comments_stay() {
    let src = "model B {\n  a: int\n  // the phone\n  phone?: string @visible(when: a > 1 && !(a == 3)) // shown to some\n}\n";
    let out = stable(src);
    assert_eq!(out, src);
}

#[test]
fn when_and_visible_stay_names_where_they_are_names() {
    let out = stable("model M {\n  when: string\n  visible: bool\n}");
    assert!(
        out.contains("  when: string\n") && out.contains("  visible: bool\n"),
        "{out}"
    );
}

#[test]
fn tenant_stays_on_its_field_in_one_line() {
    let out = stable(
        "model Booking {\n  id: uuid @readonly\n  tenantId:uuid   @tenant\n  @tenant  other: string\n}",
    );
    assert!(out.contains("  tenantId: uuid @tenant\n"), "{out}");
    assert!(out.contains("  other: string @tenant\n"), "{out}");
}

#[test]
fn tenant_stays_a_name_where_it_is_a_name() {
    let out = stable("model M {\n  tenant: string\n}");
    assert!(out.contains("  tenant: string\n"), "{out}");
}
