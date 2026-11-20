use super::format;

fn f(src: &str) -> String {
    format(src).unwrap_or_else(|d| format!("ERR {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>()))
}

fn assert_stable(src: &str) -> String {
    let once = f(src);
    assert_eq!(f(&once), once, "not idempotent for:\n{src}");
    once
}

const MESSY: &str = r#"// file header

// about users
use  users.{User,   Role as R}
use iam:users.{Zed, Alpha} // trailing on use
use @acme/billing:invoices.{Invoice,}
use a.b.{X}

/// A thing.
@deprecated("old")
model Thing extends Base { // header note
    id:uuid @readonly   // trailing id
    // leading name
    name?:string @length(min:1,max:5)  @pattern("^a$")
    @wire("t") tag: string[]
    m: Map<string,int[]> = .x /* inline */
    /* tail before close */
} // after model

enum Status { pending, paid = "PAID", shipped }
enum Long { aaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccc, dddddddddddddddddddd, eeeeeeeeeeeeeeeeee }
enum Noted {
  a, // first
  /// documented
  b
}
enum Empty {}
union Pay { card: Card, bank: Bank }
error Oops 409 { sku: string }
error Bare {}
const MAX: int = 3 // limit
const RATE: float = 2
const NAME: string = "a\"b\n"

service Orders at "/orders" {
  get "/{id}" getOrder(id: uuid) -> Order | NotFound
  post "/" createOrder(@header("X-Key") key?: string, body: CreateOrder) -> 201 Order | OutOfStock | Conflict | BadRequest | Forbidden | Unauthorized
  /// Health.
  @auth(none)
  get "/health" health() -> 204
  ws "/chat" chat(body: stream Cmd) -> stream Tick
  delete "/{id}" remove(id: uuid)
  // nothing more
}
service Empty at "/e" {}
// the end
"#;

#[test]
fn messy_source_gets_canonical_style() {
    let out = assert_stable(MESSY);
    insta::assert_snapshot!("messy", out);
}

#[test]
fn empty_and_comment_only_files() {
    assert_eq!(f(""), "");
    assert_eq!(f("\n\n"), "");
    assert_eq!(assert_stable("// only\n\n\n/* a */"), "// only\n\n/* a */\n");
}

#[test]
fn trailing_newline_is_exactly_one() {
    assert_eq!(f("model A { a: int }\n\n\n"), "model A {\n  a: int\n}\n");
    assert_eq!(f("model A { a: int }"), "model A {\n  a: int\n}\n");
}

#[test]
fn uses_are_sorted_by_package_then_module_and_names_sorted() {
    let out = f("use z:b.{Q}\nuse a:c.{B, A}\nuse x.{X}\nuse a:b.{Y}\n");
    assert_eq!(out, "use x.{X}\nuse a:b.{Y}\nuse a:c.{A, B}\nuse z:b.{Q}\n");
}

#[test]
fn comments_attached_to_uses_move_with_them() {
    let out = f("use b.{B} // bee\n// about a\nuse a.{A}\n");
    assert_eq!(out, "// about a\nuse a.{A}\nuse b.{B} // bee\n");
}

#[test]
fn annotations_go_after_the_field_type() {
    let out = f("model A {\n @readonly @wire(\"i\") id: uuid\n n: int @min(1)\n}");
    assert_eq!(out, "model A {\n  id: uuid @readonly @wire(\"i\")\n  n: int @min(1)\n}\n");
}

#[test]
fn enum_one_line_when_it_fits_else_one_per_line_with_trailing_commas() {
    let short = f("enum S { a,\n b }");
    assert_eq!(short, "enum S { a, b }\n");
    let names: Vec<String> = (0..12).map(|i| format!("variant_number_{i}")).collect();
    let long = f(&format!("enum L {{ {} }}", names.join(", ")));
    assert!(long.starts_with("enum L {\n  variant_number_0,\n"), "{long}");
    assert!(long.ends_with("  variant_number_11,\n}\n"), "{long}");
}

#[test]
fn wide_endpoints_wrap_their_responses() {
    let out = f("service S at \"/s\" {\n get \"/aaaaaaaaaaaaaaaaaaaaaaaa\" operationWithLongName(id: uuid) -> LongResponseType | NotFound | Conflict | BadRequest\n}");
    assert_eq!(
        out,
        "service S at \"/s\" {\n  get \"/aaaaaaaaaaaaaaaaaaaaaaaa\" operationWithLongName(id: uuid) -> LongResponseType\n      | NotFound\n      | Conflict\n      | BadRequest\n}\n"
    );
}

#[test]
fn blank_lines_separate_only_heavy_endpoints() {
    let out = f("service S at \"/\" {\n get \"/a\" a()\n get \"/b\" b()\n /// d\n get \"/c\" c()\n get \"/d\" d()\n}");
    assert_eq!(
        out,
        "service S at \"/\" {\n  get \"/a\" a()\n  get \"/b\" b()\n\n  /// d\n  get \"/c\" c()\n\n  get \"/d\" d()\n}\n"
    );
}

#[test]
fn comments_inside_leaf_items_become_trailing() {
    let out = assert_stable("model A {\n a: /* c1 */ int // c2\n}");
    assert_eq!(out, "model A {\n  a: int /* c1 */ // c2\n}\n");
}

#[test]
fn two_line_comments_in_one_item_do_not_merge() {
    let out = assert_stable("model A {\n a: int // one\n b: int\n}\nservice S at \"/\" {\n get \"/\" g( // x\n ) // y\n}");
    assert!(out.contains("// x") && out.contains("// y"), "{out}");
}

#[test]
fn header_and_tail_comments_survive() {
    let out = assert_stable("model A { // head\n a: int\n // tail\n}");
    assert_eq!(out, "model A { // head\n  a: int\n  // tail\n}\n");
    let out = assert_stable("model A {\n // only\n}");
    assert_eq!(out, "model A {\n  // only\n}\n");
}

#[test]
fn detached_leading_comment_keeps_its_blank_line() {
    let out = assert_stable("// header\n\nmodel A {}\n");
    assert_eq!(out, "// header\n\nmodel A {}\n");
    let out = assert_stable("// attached\nmodel A {}\n");
    assert_eq!(out, "// attached\nmodel A {}\n");
}

#[test]
fn refuses_dangling_doc_and_syntax_errors() {
    assert!(f("model A {}\n/// dangling").starts_with("ERR"));
    assert!(f("model A {").starts_with("ERR"));
    assert!(f("import \"a.veld\"").starts_with("ERR"));
}

#[test]
fn doc_comments_round_trip() {
    let out = assert_stable("///no space\n///\n///  indented\nmodel A {}");
    assert_eq!(out, "/// no space\n///\n///  indented\nmodel A {}\n");
}

#[test]
fn floats_and_negative_numbers_print_as_literals() {
    let out = assert_stable("model A { a: float @min(-1) @max(2.50) }");
    assert_eq!(out, "model A {\n  a: float @min(-1) @max(2.5)\n}\n");
}

#[test]
fn crlf_and_bom_are_accepted() {
    let out = f("\u{feff}model A {\r\n a: int\r\n}\r\n");
    assert_eq!(out, "model A {\n  a: int\n}\n");
}
