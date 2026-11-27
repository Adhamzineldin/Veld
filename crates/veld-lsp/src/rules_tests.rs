//! The language server on the rules language (`spec/rules.md`): hover, definition, completion,
//! the outline and semantic tokens, on a copy of the bookings contract and on a second contract
//! with the words bookings does not use (`has`, `len`, lambdas, `old`, a constant, `@rollout`).

use std::fs;

use lsp_types::{CompletionItem, SymbolKind};
use veld_compiler::FileId;

use crate::workspace::{Analysis, Workspace};
use crate::{complete, hover, nav, outline, words};

const TOML: &str = "[service]\nname = \"s\"\ncontracts = \"c\"\n";
const BOOKINGS: &str = include_str!("../tests/fixtures/bookings.veld");
const EXTRAS: &str = include_str!("../tests/fixtures/extras.veld");
const CARET: char = '§';

struct Fx {
    _tmp: tempfile::TempDir,
    a: Analysis,
    file: FileId,
    text: String,
}

fn fx(src: &str) -> Fx {
    let tmp = tempfile::tempdir().unwrap();
    for (p, t) in [("veld.toml", TOML), ("c/m.veld", src)] {
        let path = tmp.path().join(p);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, t).unwrap();
    }
    let a = Workspace::new(Some(tmp.path().to_path_buf())).analyze(None);
    let file = a.file_for(&tmp.path().join("c/m.veld")).unwrap();
    Fx {
        _tmp: tmp,
        a,
        file,
        text: src.to_owned(),
    }
}

/// `BOOKINGS` with one piece of text replaced.
fn bookings_with(old: &str, new: &str) -> String {
    assert!(BOOKINGS.contains(old), "the contract has no `{old}`");
    BOOKINGS.replacen(old, new, 1)
}

/// The labels of the completion at the caret of `src`, on an analysis of `src` without it.
fn complete_at(src: &str) -> Vec<CompletionItem> {
    let at = src.find(CARET).unwrap();
    let text = src.replace(CARET, "");
    let f = fx(&text);
    complete::complete(&f.a, f.file, at)
}

fn labels(src: &str) -> Vec<String> {
    complete_at(src).into_iter().map(|i| i.label).collect()
}

impl Fx {
    /// One byte into `word`, the first `word` inside the first `context`.
    fn at(&self, context: &str, word: &str) -> u32 {
        let c = self
            .text
            .find(context)
            .unwrap_or_else(|| panic!("no `{context}`"));
        let w = context
            .find(word)
            .unwrap_or_else(|| panic!("no `{word}` in `{context}`"));
        u32::try_from(c + w + 1).unwrap()
    }

    fn hover(&self, context: &str, word: &str) -> String {
        hover::hover(&self.a, self.file, self.at(context, word))
            .unwrap_or_else(|| panic!("no hover on `{word}` in `{context}`"))
            .markdown
    }

    fn def_line(&self, context: &str, word: &str) -> u32 {
        nav::definition(&self.a, self.file, self.at(context, word))
            .unwrap_or_else(|| panic!("no definition for `{word}` in `{context}`"))
            .range
            .start
            .line
    }

    /// The line of the first `needle` after the first `anchor`.
    fn line_after(&self, anchor: &str, needle: &str) -> u32 {
        let a = self.text.find(anchor).unwrap();
        let i = a + self.text[a..].find(needle).unwrap();
        u32::try_from(self.text[..i].matches('\n').count()).unwrap()
    }

    fn line_of(&self, needle: &str) -> u32 {
        let i = self.text.find(needle).unwrap();
        u32::try_from(self.text[..i].matches('\n').count()).unwrap()
    }
}

// ---- the contracts are valid ----------------------------------------------------------------

#[test]
fn the_bookings_contract_compiles_without_errors() {
    let f = fx(BOOKINGS);
    let errors: Vec<_> =
        f.a.index
            .diagnostics
            .iter()
            .filter(|d| d.severity == veld_compiler::Severity::Error)
            .map(|d| d.message.clone())
            .collect();
    assert!(errors.is_empty(), "{errors:?}");
    assert!(f.a.index.ir.is_some());
}

#[test]
fn the_extras_contract_parses() {
    let f = fx(EXTRAS);
    let syntax: Vec<_> =
        f.a.index
            .diagnostics
            .iter()
            .filter(|d| d.code.starts_with("E000"))
            .map(|d| d.message.clone())
            .collect();
    assert!(syntax.is_empty(), "{syntax:?}");
}

// ---- hover: rules and statements -------------------------------------------------------------

#[test]
fn a_rule_name_shows_its_text_message_audit_state_and_id() {
    let f = fx(BOOKINGS);
    let md = f.hover("rule endAfterStart: end > start", "endAfterStart");
    assert!(md.contains("rule endAfterStart: end > start"), "{md}");
    assert!(
        md.contains("message \"The stay must end after it starts\""),
        "{md}"
    );
    assert!(
        md.contains("- message: \"The stay must end after it starts\""),
        "{md}"
    );
    assert!(md.contains("- enforced"), "{md}");
    assert!(md.contains("- id: `veld:"), "{md}");
    assert!(md.contains("invariant of `NewBooking`"), "{md}");
    let md = f.hover("rule atMost30Days", "rule");
    assert!(md.contains("rule atMost30Days"), "{md}");
}

#[test]
fn an_audited_rule_says_it_never_rejects() {
    let f = fx(EXTRAS);
    let md = f.hover("rule nickFits", "nickFits");
    assert!(md.contains("audited (`@rollout(audit)`)"), "{md}");
    assert!(md.contains("never rejects"), "{md}");
    assert!(
        md.contains("the default is \"violates rule nickFits\""),
        "{md}"
    );
}

#[test]
fn a_require_shows_its_text_error_message_and_id() {
    let f = fx(BOOKINGS);
    let md = f.hover("require not room.closed else RoomClosed", "require");
    assert!(
        md.contains("require not room.closed else RoomClosed"),
        "{md}"
    );
    assert!(md.contains("precondition of `createBooking`"), "{md}");
    assert!(md.contains("fails with: `RoomClosed`"), "{md}");
    assert!(md.contains("- enforced"), "{md}");
    assert!(md.contains("- id: `veld:"), "{md}");
    let md = f.hover("require notInPast: body.start >= now", "notInPast");
    assert!(
        md.contains("- message: \"A booking cannot start in the past\""),
        "{md}"
    );
    assert!(md.contains("422 ValidationError"), "{md}");
}

#[test]
fn an_ensure_shows_its_text_and_default_message() {
    let f = fx(BOOKINGS);
    let md = f.hover(
        "ensure keepsRoom: result.roomId == body.roomId",
        "keepsRoom",
    );
    assert!(
        md.contains("ensure keepsRoom: result.roomId == body.roomId"),
        "{md}"
    );
    assert!(md.contains("postcondition of `createBooking`"), "{md}");
    assert!(
        md.contains("the default is \"violates rule keepsRoom\""),
        "{md}"
    );
    let md = f.hover("ensure keepsRoom", "ensure");
    assert!(md.contains("keepsRoom"), "{md}");
}

#[test]
fn the_id_of_the_last_good_build_is_shown_while_the_file_does_not_compile() {
    let tmp = tempfile::tempdir().unwrap();
    for (p, t) in [("veld.toml", TOML), ("c/m.veld", BOOKINGS)] {
        let path = tmp.path().join(p);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, t).unwrap();
    }
    let mut ws = Workspace::new(Some(tmp.path().to_path_buf()));
    let good = ws.analyze(None);
    let broken = format!("{BOOKINGS}\nmodel Oops {{\n");
    ws.open(&tmp.path().join("c/m.veld"), broken);
    let a = ws.analyze(Some(&good.sigs));
    assert!(a.index.ir.is_none());
    let file = a.file_for(&tmp.path().join("c/m.veld")).unwrap();
    let off = BOOKINGS.find("rule endAfterStart").unwrap() + "rule ".len() + 1;
    let md = hover::hover(&a, file, u32::try_from(off).unwrap())
        .unwrap()
        .markdown;
    assert!(md.contains("- id: `veld:"), "{md}");
    assert!(md.contains("(last successful build)"), "{md}");
}

// ---- hover: inside expressions ---------------------------------------------------------------

#[test]
fn a_field_in_an_invariant_shows_its_type_and_where_it_is_declared() {
    let f = fx(BOOKINGS);
    let md = f.hover("rule endAfterStart: end > start", "end >");
    assert!(md.contains("end: datetime"), "{md}");
    assert!(md.contains("field of `NewBooking`"), "{md}");
    let md = f.hover("end > start\n    message", "start");
    assert!(md.contains("start: datetime"), "{md}");
}

#[test]
fn an_inherited_field_names_the_declaration_that_writes_it_and_its_constraints() {
    let f = fx(BOOKINGS);
    let md = f.hover("ensure keepsGuests: result.guests", "guests");
    assert!(md.contains("guests: int"), "{md}");
    assert!(md.contains("field of `NewBooking`"), "{md}");
    assert!(md.contains("@min(1)"), "{md}");
}

#[test]
fn an_optional_field_says_it_needs_a_guard() {
    let f = fx(EXTRAS);
    let md = f.hover("has(nickname) implies len(nickname)", "nickname");
    assert!(md.contains("nickname?: string"), "{md}");
    assert!(md.contains("has(..)"), "{md}");
}

#[test]
fn a_parameter_a_fact_result_now_and_the_caller_have_hover_text() {
    let f = fx(BOOKINGS);
    let md = f.hover("require body.guests <= room.capacity", "body");
    assert!(md.contains("body: NewBooking"), "{md}");
    assert!(md.contains("parameter of `createBooking`"), "{md}");
    let md = f.hover("require not room.closed", "room");
    assert!(md.contains("fact room: Room = body.roomId"), "{md}");
    assert!(md.contains("404 NotFound"), "{md}");
    let md = f.hover("ensure keepsRoom: result.roomId", "result");
    assert!(md.contains("result: Booking"), "{md}");
    let md = f.hover("body.start >= now", "now");
    assert!(md.contains("datetime") && md.contains("clock"), "{md}");
    let md = f.hover("result.guestId == user.id", "user");
    assert!(md.contains("user: User"), "{md}");
    assert!(md.contains("The caller"), "{md}");
}

#[test]
fn a_field_reached_through_a_fact_shows_the_field_of_its_type() {
    let f = fx(BOOKINGS);
    let md = f.hover("require body.guests <= room.capacity", "capacity");
    assert!(md.contains("capacity: int"), "{md}");
    assert!(md.contains("field of `Room`"), "{md}");
    assert!(md.contains("@min(1)"), "{md}");
}

#[test]
fn an_optional_fact_and_old_have_hover_text() {
    let f = fx(EXTRAS);
    let md = f.hover("require has(spouse) implies", "spouse");
    assert!(md.contains("may be absent"), "{md}");
    let md = f.hover("result.name == old(guest.name)", "old");
    assert!(md.contains("old(e)"), "{md}");
    let md = f.hover("require admin.id != id", "admin");
    assert!(md.contains("admin: Admin"), "{md}");
}

#[test]
fn an_enum_literal_shows_the_variant_of_the_enum_the_context_expects() {
    let f = fx(BOOKINGS);
    let md = f.hover("result.status == .pending", "pending");
    assert!(md.contains("Status.pending"), "{md}");
    assert!(md.contains("wire value"), "{md}");
    let f = fx(EXTRAS);
    let md = f.hover("tier in [.basic, .gold]", "gold");
    assert!(md.contains("Tier.gold"), "{md}");
}

#[test]
fn a_constant_shows_its_declaration() {
    let f = fx(EXTRAS);
    let md = f.hover("<= MAX_GUESTS", "MAX_GUESTS");
    assert!(md.contains("const MAX_GUESTS: int"), "{md}");
    assert!(md.contains("The most guests"), "{md}");
}

#[test]
fn builtins_show_a_signature_and_one_line_taken_from_the_table() {
    let f = fx(BOOKINGS);
    let md = f.hover("end - start <= days(30)", "days");
    let w = words::builtin("days").unwrap();
    assert_eq!(md, words::describe(w));
    assert!(md.contains("days(n: int) -> duration"), "{md}");
    let f = fx(EXTRAS);
    for name in ["len", "has", "all", "count", "matches"] {
        let md = f.hover(&format!("{name}("), name);
        assert_eq!(md, words::describe(words::builtin(name).unwrap()), "{name}");
    }
}

#[test]
fn a_lambda_variable_shows_its_type() {
    let f = fx(EXTRAS);
    let md = f.hover("all(stays, s => s.nights <= 30)", "s =>");
    assert!(md.contains("s: Stay"), "{md}");
    let md = f.hover("s => s.nights <= 30", "nights");
    assert!(md.contains("nights: int"), "{md}");
}

// ---- hover: policies, the principal, states, examples ----------------------------------------

#[test]
fn a_policy_shows_its_parameters_and_clauses_where_it_is_declared_and_where_it_is_used() {
    let f = fx(BOOKINGS);
    for md in [
        f.hover(
            "policy ManageBooking(user: User, booking: Booking)",
            "ManageBooking",
        ),
        f.hover("allow ManageBooking(user, booking)", "ManageBooking"),
    ] {
        assert!(
            md.contains("policy ManageBooking(user: User, booking: Booking)"),
            "{md}"
        );
        assert!(
            md.contains("permit when booking.guestId == user.id"),
            "{md}"
        );
        assert!(md.contains("permit when \"staff\" in user.roles"), "{md}");
        assert!(md.contains("forbid when user.suspended"), "{md}");
        assert!(
            md.contains("A suspended account cannot manage bookings"),
            "{md}"
        );
        assert!(md.contains("- id: `veld:"), "{md}");
    }
}

#[test]
fn a_policy_parameter_and_clause_word_have_hover_text() {
    let f = fx(BOOKINGS);
    let md = f.hover("permit when booking.guestId == user.id", "user");
    assert!(md.contains("parameter of policy `ManageBooking`"), "{md}");
    let md = f.hover("permit when booking.guestId == user.id", "permit");
    assert!(md.contains("permit when expr"), "{md}");
    let md = f.hover("\"staff\" in user.roles", "roles");
    assert!(md.contains("roles: string[]"), "{md}");
}

#[test]
fn the_principal_has_hover_text() {
    let f = fx(BOOKINGS);
    let md = f.hover("principal User", "principal");
    assert!(md.contains("principal Type [as name]"), "{md}");
    let md = f.hover("principal User", "User");
    assert!(md.contains("model User"), "{md}");
}

#[test]
fn a_state_shows_the_moves_that_leave_it_and_whether_it_is_final() {
    let f = fx(BOOKINGS);
    let md = f.hover("flow pending -> paid -> checkedIn", "paid");
    assert!(md.contains("Status.paid"), "{md}");
    assert!(
        md.contains("moves out to: checkedIn, cancelled, refunded"),
        "{md}"
    );
    assert!(md.contains("reached from: pending"), "{md}");
    assert!(md.contains("- not final"), "{md}");
    let md = f.hover("final checkedIn, cancelled, refunded", "checkedIn");
    assert!(md.contains("moves out to: none"), "{md}");
    assert!(md.contains("final: it has no way out"), "{md}");
    let md = f.hover("transition booking.status: pending -> paid", "pending");
    assert!(md.contains("Status.pending"), "{md}");
    let md = f.hover("transition booking.status: pending -> paid", "status");
    assert!(md.contains("status: Status"), "{md}");
    let md = f.hover("flow pending -> paid", "flow");
    assert!(md.contains("flow a -> b -> c"), "{md}");
}

#[test]
fn an_example_shows_its_call_and_what_it_expects() {
    let f = fx(BOOKINGS);
    let md = f.hover("example \"a valid booking is accepted\"", "valid");
    assert!(md.contains("a valid booking is accepted"), "{md}");
    assert!(md.contains("calls `createBooking`"), "{md}");
    assert!(md.contains("expects:"), "{md}");
    assert!(md.contains("2026-06-01T00:00:00Z"), "{md}");
    let md = f.hover("createBooking(body: {", "createBooking");
    assert!(
        md.contains("createBooking") && md.contains("/bookings"),
        "{md}"
    );
    let md = f.hover("given room = { capacity: 2, closed: false }", "room");
    assert!(md.contains("fact room: Room"), "{md}");
    let md = f.hover("roomId: room.id, guests: 2", "id,");
    assert!(md.contains("id: uuid"), "{md}");
}

// ---- go to definition ------------------------------------------------------------------------

#[test]
fn definition_from_an_expression_reaches_fields_parameters_facts_and_constants() {
    let f = fx(BOOKINGS);
    assert_eq!(
        f.def_line("rule endAfterStart: end > start", "end >"),
        f.line_of("  end: datetime")
    );
    assert_eq!(
        f.def_line("require body.guests <= room.capacity", "body"),
        f.line_of("createBooking(body: NewBooking)")
    );
    assert_eq!(
        f.def_line("require not room.closed", "room"),
        f.line_of("fact room: Room")
    );
    assert_eq!(
        f.def_line("require body.guests <= room.capacity", "capacity"),
        f.line_of("capacity: int @min(1)")
    );
    assert_eq!(
        f.def_line("ensure keepsGuests: result.guests", "guests"),
        f.line_of("guests: int @min(1)")
    );
    let g = fx(EXTRAS);
    assert_eq!(
        g.def_line("<= MAX_GUESTS", "MAX_GUESTS"),
        g.line_of("const MAX_GUESTS")
    );
    assert_eq!(
        g.def_line("s => s.nights <= 30", "s.nights"),
        g.line_of("all(stays, s =>")
    );
}

#[test]
fn definition_reaches_variants_policies_errors_and_endpoints() {
    let f = fx(BOOKINGS);
    assert_eq!(
        f.def_line("result.status == .pending", "pending"),
        f.line_of("  pending,\n")
    );
    assert_eq!(
        f.def_line("allow ManageBooking(user, booking)", "ManageBooking"),
        f.line_of("policy ManageBooking")
    );
    assert_eq!(
        f.def_line("else RoomClosed", "RoomClosed"),
        f.line_of("error RoomClosed 409")
    );
    assert_eq!(
        f.def_line("payBooking(id: booking.id", "payBooking"),
        f.line_of("payBooking(id: uuid")
    );
    assert_eq!(
        f.def_line("flow pending -> paid -> checkedIn", "paid"),
        f.line_of("  paid,\n")
    );
    assert_eq!(
        f.def_line("transition booking.status: pending -> paid", "booking"),
        f.line_after("payBooking(id: uuid", "fact booking: Booking = id")
    );
}

// ---- completion ------------------------------------------------------------------------------

fn has(l: &[String], names: &[&str]) {
    for n in names {
        assert!(l.iter().any(|x| x == n), "`{n}` is missing from {l:?}");
    }
}

fn lacks(l: &[String], names: &[&str]) {
    for n in names {
        assert!(!l.iter().any(|x| x == n), "`{n}` must not be in {l:?}");
    }
}

#[test]
fn after_a_dot_the_fields_of_that_value_are_offered_through_facts_and_parameters() {
    let src = bookings_with(
        "require not room.closed else RoomClosed",
        "require body.guests <= room.§",
    );
    let l = labels(&src);
    has(&l, &["id", "name", "capacity", "closed", "nightly"]);
    lacks(&l, &["guests", "roomId"]);
    let f = fx(&src.replace(CARET, ""));
    assert!(f.a.index.error_count() > 0, "the contract is broken here");
    let l = labels(&bookings_with(
        "require body.guests <= room.capacity else RoomFull { capacity: room.capacity }",
        "require body.§",
    ));
    has(&l, &["roomId", "guests", "start", "end"]);
    lacks(&l, &["id", "capacity"]);
}

#[test]
fn after_a_dot_on_result_the_inherited_fields_are_offered() {
    let l = labels(&bookings_with(
        "ensure keepsRoom: result.roomId == body.roomId",
        "ensure keepsRoom: result.§",
    ));
    has(
        &l,
        &[
            "id", "guestId", "total", "status", "roomId", "guests", "start", "end",
        ],
    );
}

#[test]
fn after_a_dot_the_fields_of_an_optional_value_and_of_a_lambda_variable_are_offered() {
    let src = EXTRAS.replace(
        "require has(spouse) implies spouse.tier == .gold",
        "require has(spouse) implies spouse.§",
    );
    let l = labels(&src);
    has(&l, &["name", "nickname", "tier", "tags", "stays"]);
    let l = labels(
        "model Stay { nights: int }\nmodel G {\n  stays: Stay[]\n  rule r: all(stays, s => s.§\n}\n",
    );
    assert_eq!(l, ["nights"]);
}

#[test]
fn a_page_has_items_total_page_and_size() {
    let l = labels("model Row { n: int }\nmodel G {\n  rows: Page<Row>\n  rule r: rows.§\n}\n");
    assert_eq!(l, ["items", "total", "page", "size"]);
}

#[test]
fn an_operand_position_offers_names_in_scope_builtins_and_constants() {
    let l = labels(&bookings_with(
        "require not room.closed else RoomClosed",
        "require §",
    ));
    has(
        &l,
        &[
            "body", "room", "user", "now", "has", "len", "matches", "days", "true", "not",
        ],
    );
    lacks(&l, &["result", "old", "guests"]);
    let l = labels(&bookings_with(
        "ensure startsPending: result.status == .pending",
        "ensure §",
    ));
    has(&l, &["result", "old", "body", "user", "now"]);
    let l = labels(&EXTRAS.replace("require admin.id != id", "require §"));
    has(&l, &["MAX_GUESTS", "id", "guest", "spouse", "admin"]);
    let l = labels("model A {\n  x: int\n  y: int\n  rule r: x > §\n}\n");
    has(&l, &["x", "y", "len"]);
    lacks(&l, &["now"]);
}

#[test]
fn every_detail_of_a_builtin_comes_from_the_table() {
    let items = complete_at(&bookings_with(
        "require not room.closed else RoomClosed",
        "require §",
    ));
    for b in words::BUILTINS.iter().filter(|b| b.word != "old") {
        let i = items.iter().find(|i| i.label == b.word).unwrap();
        assert_eq!(i.detail.as_deref(), Some(b.signature), "{}", b.word);
    }
    let policy = complete_at(&bookings_with("allow MayBook(user)", "allow §"));
    assert!(policy.iter().any(|i| i.label == "MayBook"));
}

#[test]
fn an_enum_literal_is_completed_with_the_variants_of_the_enum_that_is_expected() {
    let l = labels(&bookings_with(
        "ensure startsPending: result.status == .pending",
        "ensure startsPending: result.status == .§",
    ));
    assert_eq!(l, ["pending", "paid", "checkedIn", "cancelled", "refunded"]);
    let l = labels(&bookings_with(
        "ensure startsPending: result.status == .pending",
        "ensure startsPending: result.status in [.pending, .§",
    ));
    assert_eq!(l, ["pending", "paid", "checkedIn", "cancelled", "refunded"]);
    let l = labels(&EXTRAS.replace("tier in [.basic, .gold]", "tier in [.§"));
    assert_eq!(l, ["basic", "gold"]);
    let l = labels(&EXTRAS.replace("tier == .gold implies", "tier == .§ implies"));
    assert_eq!(l, ["basic", "gold"]);
}

#[test]
fn the_statement_words_are_offered_at_the_start_of_a_line_in_an_endpoint_block() {
    let l = labels(&bookings_with("    fact room: Room = body.roomId", "    §"));
    has(&l, &["fact", "require", "ensure", "allow", "transition"]);
    lacks(&l, &["get", "post"]);
}

#[test]
fn else_and_the_errors_come_after_a_require() {
    let l = labels(&bookings_with(
        "require not room.closed else RoomClosed",
        "require not room.closed §",
    ));
    has(&l, &["else", "message", "in"]);
    let items = complete_at(&bookings_with(
        "require not room.closed else RoomClosed",
        "require not room.closed else §",
    ));
    let l: Vec<String> = items.iter().map(|i| i.label.clone()).collect();
    has(
        &l,
        &["RoomFull", "RoomClosed", "AmountMismatch", "NotFound"],
    );
    lacks(&l, &["Booking", "string"]);
    let full = items.iter().find(|i| i.label == "RoomFull").unwrap();
    assert!(full.sort_text.as_deref().unwrap().starts_with("00"));
    let other = items.iter().find(|i| i.label == "AmountMismatch").unwrap();
    assert!(!other.sort_text.as_deref().unwrap().starts_with("00"));
}

#[test]
fn a_payload_offers_the_fields_of_the_error_and_then_expressions() {
    let src = "require body.guests <= room.capacity else RoomFull { capacity: room.capacity }";
    let l = labels(&bookings_with(
        src,
        "require body.guests <= room.capacity else RoomFull { §",
    ));
    assert_eq!(l, ["capacity"]);
    let l = labels(&bookings_with(
        src,
        "require body.guests <= room.capacity else RoomFull { capacity: room.§",
    ));
    has(&l, &["capacity", "closed"]);
}

#[test]
fn a_policy_is_offered_after_allow_and_its_arguments_are_expressions() {
    let l = labels(&bookings_with("allow MayBook(user)", "allow §"));
    has(&l, &["ManageBooking", "MayBook", "StaffOnly"]);
    let l = labels(&bookings_with(
        "allow ManageBooking(user, booking)",
        "allow ManageBooking(user, §",
    ));
    has(&l, &["booking", "user", "id"]);
    let l = labels(&bookings_with(
        "allow ManageBooking(user, booking)",
        "allow ManageBooking(user, booking.§",
    ));
    has(&l, &["guestId", "status", "total"]);
}

#[test]
fn states_are_offered_after_transition_and_in_a_flow() {
    let all = ["pending", "paid", "checkedIn", "cancelled", "refunded"];
    let l = labels(&bookings_with(
        "transition booking.status: pending -> paid",
        "transition booking.status: §",
    ));
    assert_eq!(l, all);
    let l = labels(&bookings_with(
        "transition booking.status: pending -> paid",
        "transition booking.status: pending -> §",
    ));
    assert_eq!(l, all);
    let l = labels(&bookings_with(
        "transition booking.status: pending -> paid",
        "transition §",
    ));
    assert_eq!(l, ["booking"]);
    let l = labels(&bookings_with(
        "transition booking.status: pending -> paid",
        "transition booking.§",
    ));
    has(&l, &["status", "total"]);
    let l = labels(&bookings_with(
        "flow pending -> paid -> checkedIn",
        "flow pending -> §",
    ));
    assert_eq!(l, all);
    let l = labels(&bookings_with("  flow paid -> refunded", "  §"));
    has(&l, &["flow", "final"]);
    let l = labels(&bookings_with(
        "final checkedIn, cancelled, refunded",
        "final checkedIn, §",
    ));
    assert_eq!(l, all);
}

#[test]
fn rule_and_policy_words_are_offered_where_they_may_stand() {
    let l = labels("model A {\n  x: int\n  §\n}\n");
    assert_eq!(l, ["rule"]);
    let l = labels("model A {\n  x: int\n  rule r: x > 0\n  §\n}\n");
    has(&l, &["rule", "message"]);
    let l = labels("§");
    has(&l, &["policy", "principal", "example", "model"]);
    let l = labels("model U { id: uuid }\npolicy P(u: U) {\n  §\n}\n");
    has(&l, &["permit", "forbid"]);
    let l = labels("model U { id: uuid }\npolicy P(u: U) {\n  permit §\n}\n");
    assert_eq!(l, ["when"]);
    let l = labels("model U { id: uuid\n ok: bool }\npolicy P(u: U) {\n  permit when u.§\n}\n");
    has(&l, &["id", "ok"]);
    let l = labels("model U { id: uuid }\npolicy P(u: U) {\n  permit when §\n}\n");
    has(&l, &["u", "now", "len"]);
}

const MINI: &str = "model Room { id: uuid }\nservice S at \"/s\" {\n  get \"/{id}\" look(id: uuid) -> Room {\n    fact room: Room = id\n  }\n}\n";

#[test]
fn the_words_of_an_example_are_offered_and_so_are_the_endpoint_and_its_parameters() {
    let l = labels(&format!("{MINI}example \"x\" {{\n  §\n}}\n"));
    has(&l, &["given", "as", "at", "look"]);
    let l = labels(&format!("{MINI}example \"x\" {{\n  given §\n}}\n"));
    assert_eq!(l, ["room"]);
    let l = labels(&format!("{MINI}example \"x\" {{\n  given room = §\n}}\n"));
    assert_eq!(l, ["absent"]);
    let l = labels(&format!("{MINI}example \"x\" {{\n  look(§\n}}\n"));
    assert_eq!(l, ["id"]);
    let l = labels(&format!(
        "{MINI}example \"x\" {{\n  look(id: room.id) §\n}}\n"
    ));
    has(&l, &["returns", "->"]);
    let l = labels(&format!(
        "{MINI}example \"x\" {{\n  look(id: room.id) -> §\n}}\n"
    ));
    has(&l, &["ok", "NotFound", "Forbidden"]);
    let l = labels(&format!("{MINI}example \"x\" {{\n  S.§\n}}\n"));
    assert_eq!(l, ["look"]);
}

#[test]
fn completion_works_while_an_expression_is_unfinished_and_the_file_does_not_parse() {
    for tail in [
        "rule r: x > §\n}\n",
        "rule r: x >= has(§\n}\n",
        "rule r: (x > 0 && §\n}\n",
    ] {
        let src = format!("model A {{\n  x: int\n  y: int\n  {tail}");
        let l = labels(&src);
        has(&l, &["x", "y", "len"]);
        let f = fx(&src.replace(CARET, ""));
        assert!(f.a.index.error_count() > 0, "{src}");
    }
    let l = labels(&bookings_with(
        "require not room.closed else RoomClosed",
        "require body.guests <= §\n    require x",
    ));
    has(&l, &["body", "room", "user"]);
}

#[test]
fn methods_are_offered_again_after_an_endpoint_block_has_closed() {
    let l = labels(
        "model A { x: int }\nservice S at \"/s\" {\n  post \"/\" f(body: A) -> 201 A {\n    require true\n  }\n  §\n}\n",
    );
    has(&l, &["get", "post"]);
    let l = labels(
        "model A { x: int }\nservice S at \"/s\" {\n  post \"/\" f(body: A) -> 201 A {\n    require true else E { x: 1 }\n  }\n  §\n}\n",
    );
    has(&l, &["get", "post"]);
}

#[test]
fn version_and_idempotent_are_still_offered() {
    let l = labels("model A {\n  x: int @§\n}\n");
    has(&l, &["version"]);
    let l = labels("service S at \"/s\" {\n  @§\n}\n");
    has(&l, &["idempotent"]);
}

// ---- the outline -----------------------------------------------------------------------------

#[test]
fn the_outline_lists_rules_statements_policies_the_principal_and_examples() {
    let f = fx(BOOKINGS);
    let syms = outline::document_symbols(&f.a, f.file);
    let new_booking = syms.iter().find(|s| s.name == "NewBooking").unwrap();
    let kids = new_booking.children.as_ref().unwrap();
    let names: Vec<&str> = kids.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "roomId",
            "guests",
            "start",
            "end",
            "endAfterStart",
            "atMost30Days"
        ]
    );
    let rule = kids.iter().find(|k| k.name == "endAfterStart").unwrap();
    assert_eq!(rule.kind, SymbolKind::BOOLEAN);
    assert_eq!(rule.detail.as_deref(), Some("end > start"));

    let svc = syms.iter().find(|s| s.name == "Bookings").unwrap();
    let create = svc
        .children
        .as_ref()
        .unwrap()
        .iter()
        .find(|e| e.name == "createBooking")
        .unwrap();
    let stmts = create.children.as_ref().unwrap();
    let names: Vec<&str> = stmts.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "room",
            "allow MayBook",
            "require",
            "require",
            "notInPast",
            "keepsRoom",
            "keepsGuests",
            "belongsToCaller",
            "startsPending"
        ]
    );
    assert_eq!(stmts[0].kind, SymbolKind::VARIABLE);
    assert_eq!(stmts[1].kind, SymbolKind::KEY);
    assert_eq!(stmts[4].kind, SymbolKind::BOOLEAN);
    let pay = svc
        .children
        .as_ref()
        .unwrap()
        .iter()
        .find(|e| e.name == "payBooking")
        .unwrap();
    assert!(
        pay.children
            .as_ref()
            .unwrap()
            .iter()
            .any(|k| k.name == "transition booking.status")
    );

    let policy = syms.iter().find(|s| s.name == "ManageBooking").unwrap();
    assert_eq!(policy.kind, SymbolKind::FUNCTION);
    assert_eq!(policy.children.as_ref().map(Vec::len), Some(3));
    let principal = syms.iter().find(|s| s.name == "principal").unwrap();
    assert_eq!(principal.detail.as_deref(), Some("User"));
    let example = syms
        .iter()
        .find(|s| s.name == "a valid booking is accepted")
        .unwrap();
    assert_eq!(example.kind, SymbolKind::STRING);
    let starts: Vec<(u32, u32)> = syms
        .iter()
        .map(|s| (s.range.start.line, s.range.start.character))
        .collect();
    let mut sorted = starts.clone();
    sorted.sort_unstable();
    assert_eq!(starts, sorted, "symbols are in source order");
}

#[test]
fn the_outline_of_a_file_with_an_unnamed_example_has_no_empty_names() {
    let f = fx(&format!(
        "{MINI}example \"\" {{\n  look(id: \"x\") -> ok\n}}\n"
    ));
    let syms = outline::document_symbols(&f.a, f.file);
    assert!(syms.iter().all(|s| !s.name.is_empty()));
}

// ---- semantic tokens -------------------------------------------------------------------------

fn absolute(a: &Analysis, file: FileId) -> Vec<(u32, u32, u32, u32, u32)> {
    let mut out = Vec::new();
    let (mut line, mut col) = (0, 0);
    for t in outline::semantic_tokens(a, file) {
        line += t.delta_line;
        col = if t.delta_line == 0 {
            col + t.delta_start
        } else {
            t.delta_start
        };
        out.push((line, col, t.length, t.token_type, t.token_modifiers_bitset));
    }
    out
}

#[test]
fn rule_words_and_builtins_get_token_types_and_the_old_legend_is_kept() {
    let legend = outline::legend();
    assert_eq!(legend.token_types.len(), 10);
    assert_eq!(legend.token_types[0], lsp_types::SemanticTokenType::TYPE);
    assert_eq!(
        legend.token_types[8],
        lsp_types::SemanticTokenType::VARIABLE
    );
    assert_eq!(legend.token_types[9], lsp_types::SemanticTokenType::KEYWORD);
    assert_eq!(legend.token_modifiers.len(), 2);

    let f = fx(BOOKINGS);
    let toks = absolute(&f.a, f.file);
    let line = f.line_of("    require not room.closed");
    assert!(
        toks.iter()
            .any(|t| t.0 == line && t.1 == 4 && t.2 == 7 && t.3 == 9),
        "`require` is a keyword"
    );
    let line = f.line_of("rule atMost30Days");
    let col = u32::try_from(
        f.text
            .lines()
            .nth(line as usize)
            .unwrap()
            .find("days(")
            .unwrap(),
    )
    .unwrap();
    assert!(
        toks.iter()
            .any(|t| t.0 == line && t.1 == col && t.3 == 4 && t.4 & 2 == 2),
        "`days` is a built-in function"
    );
    let col = u32::try_from(
        f.text
            .lines()
            .nth(line as usize)
            .unwrap()
            .find("end")
            .unwrap(),
    )
    .unwrap();
    assert!(
        toks.iter().any(|t| t.0 == line && t.1 == col && t.3 == 3),
        "a field is a property"
    );
    let mut prev_end = (0, 0);
    for t in &toks {
        assert!((t.0, t.1) >= prev_end, "tokens overlap at {t:?}");
        prev_end = (t.0, t.1 + t.2);
    }
}

// ---- a file that does not compile ------------------------------------------------------------

#[test]
fn hover_and_definition_work_on_the_statements_around_a_broken_one() {
    let src = bookings_with(
        "require notInPast: body.start >= now",
        "require notInPast: body.start >= )",
    );
    let f = fx(&src);
    assert!(f.a.index.ir.is_none());
    assert!(
        f.a.index
            .diagnostics
            .iter()
            .any(|d| d.code.starts_with("E000"))
    );
    let md = f.hover("require not room.closed else RoomClosed", "room");
    assert!(md.contains("fact room: Room"), "{md}");
    let md = f.hover("ensure keepsRoom: result.roomId == body.roomId", "roomId");
    assert!(md.contains("roomId: uuid"), "{md}");
    assert_eq!(
        f.def_line("ensure keepsRoom: result.roomId", "roomId"),
        f.line_of("roomId: uuid")
    );
}
