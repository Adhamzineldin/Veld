//! Hand-built IR for the contracts of `tests/rules/core/*.veld`, one function per file.

use veld_ir::{Builtin, Field, Ir, Literal, Quantifier, Type};

use super::kit::*;

fn all_of(names: &[&str], t: &Type) -> Vec<Field> {
    names.iter().map(|n| field(n, t.clone())).collect()
}
fn pi(n: &str) -> veld_ir::Expr {
    p(n, int())
}
fn pl(n: &str) -> veld_ir::Expr {
    p(n, long())
}
fn pd(n: &str) -> veld_ir::Expr {
    p(n, dec())
}
fn pf(n: &str) -> veld_ir::Expr {
    p(n, float())
}
fn ps(n: &str) -> veld_ir::Expr {
    p(n, string())
}
fn pb(n: &str) -> veld_ir::Expr {
    p(n, boolean())
}
fn pt(n: &str) -> veld_ir::Expr {
    p(n, datetime())
}
fn pdate(n: &str) -> veld_ir::Expr {
    p(n, date())
}
fn pclock(n: &str) -> veld_ir::Expr {
    p(n, time())
}
fn pu(n: &str) -> veld_ir::Expr {
    p(n, uuid())
}

pub fn decimal() -> Ir {
    let mut k = Kit::new();
    k.model(
        "DecEq",
        all_of(&["a", "b"], &dec()),
        vec![rule("same", eq(pd("a"), pd("b")))],
    );
    let sum = eq(add(pd("a"), pd("b")), pd("c"));
    let ordered = lt(sub(pd("a"), pd("b")), pd("c"));
    k.model(
        "DecMath",
        all_of(&["a", "b", "c"], &dec()),
        vec![rule("sum", sum), rule("ordered", ordered)],
    );
    let fields = vec![
        field("price", dec()),
        field("qty", int()),
        field("total", dec()),
    ];
    let exact = eq(mul(pd("price"), widen(pi("qty"), dec())), pd("total"));
    let positive = gt(pd("total"), dc("0"));
    k.model(
        "DecMul",
        fields,
        vec![rule("exact", exact), rule("positive", positive)],
    );
    let finite = lt(mul(pf("a"), pf("b")), fl(1.0e308));
    let ratio = ge(div(pf("a"), pf("b")), fl(0.0));
    k.model(
        "FloatMath",
        all_of(&["a", "b"], &float()),
        vec![rule("finite", finite), rule("ratio", ratio)],
    );
    k.ir()
}

pub fn numbers() -> Ir {
    let mut k = Kit::new();
    let ab = || all_of(&["a", "b"], &int());
    k.model(
        "AddInt",
        ab(),
        vec![rule("small", lt(add(pi("a"), pi("b")), i(100)))],
    );
    k.model(
        "MulInt",
        ab(),
        vec![rule("nonNegative", ge(mul(pi("a"), pi("b")), i(0)))],
    );
    let half = eq(div(pi("a"), pi("b")), i(-3));
    let rest = eq(rem(pi("a"), pi("b")), i(-1));
    k.model("DivInt", ab(), vec![rule("half", half), rule("rest", rest)]);
    let exact = eq(add(pl("a"), lng(1)), pl("b"));
    let bounded = gt(mul(pl("a"), lng(2)), lng(0));
    k.model(
        "LongMath",
        all_of(&["a", "b"], &long()),
        vec![rule("exact", exact), rule("bounded", bounded)],
    );
    let fields = vec![
        field("n", int()),
        field("big", long()),
        field("price", dec()),
        field("ratio", float()),
    ];
    let to_long = lt(widen(pi("n"), long()), pl("big"));
    let to_dec = le(mul(widen(pi("n"), dec()), pd("price")), dc("100"));
    let to_float = lt(mul(widen(pi("n"), float()), pf("ratio")), fl(1.5));
    k.model(
        "Widen",
        fields,
        vec![
            rule("intToLong", to_long),
            rule("intToDecimal", to_dec),
            rule("intToFloat", to_float),
        ],
    );
    let safe = and(ne(pi("b"), i(0)), gt(div(pi("a"), pi("b")), i(1)));
    let alt = or(eq(pi("b"), i(0)), eq(rem(pi("a"), pi("b")), i(0)));
    k.model("Guarded", ab(), vec![rule("safe", safe), rule("alt", alt)]);
    k.ir()
}

pub fn logic() -> Ir {
    let mut k = Kit::new();
    let mut fields = all_of(&["a", "b", "c"], &boolean());
    fields.push(field("n", int()));
    let rules = vec![
        rule("chain", implies(pb("a"), implies(pb("b"), pb("c")))),
        rule("mixed", or(pb("a"), and(pb("b"), pb("c")))),
        rule("negated", eq(not(pb("a")), pb("b"))),
        rule(
            "minus",
            or(lt(neg(pi("n")), i(0)), eq(sub(pi("n"), i(-1)), i(0))),
        ),
        rule("tight", lt(sub(pi("n"), i(1)), i(5))),
        rule(
            "grouped",
            or(
                and(or(pb("a"), pb("b")), pb("c")),
                eq(add(mul(pi("n"), i(2)), i(1)), i(7)),
            ),
        ),
    ];
    k.model("Logic", fields, rules);
    k.ir()
}

pub fn strings() -> Ir {
    let mut k = Kit::new();
    let short = le(len(ps("s")), i(3));
    let no_admin = not(text_fn(Builtin::StartsWith, ps("s"), s("admin")));
    k.model(
        "Text",
        vec![field("s", string())],
        vec![rule("short", short), rule("noAdmin", no_admin)],
    );
    let rules = vec![
        rule("same", eq(ps("a"), ps("b"))),
        rule(
            "suffix",
            text_fn(Builtin::EndsWith, ps("a"), s("\u{1F600}")),
        ),
        rule("inner", text_fn(Builtin::Contains, ps("b"), s("\u{E9}"))),
    ];
    k.model("Strs", all_of(&["a", "b"], &string()), rules);
    let shape = text_fn(Builtin::Matches, ps("code"), s(r"^[A-Z]{3}-\d{4}$"));
    let has_digit = text_fn(Builtin::Matches, ps("code"), s(r"\d"));
    k.model(
        "Pattern",
        vec![field("code", string())],
        vec![rule("shape", shape), rule("hasDigit", has_digit)],
    );
    let fields = vec![
        field("data", bytes()),
        field("tags", list_of(string())),
        field("limits", map_of(int())),
    ];
    let rules = vec![
        rule("smallData", le(len(p("data", bytes())), i(3))),
        rule("fewTags", lt(len(p("tags", list_of(string()))), i(3))),
        rule("hasMax", within(s("max"), p("limits", map_of(int())))),
        rule(
            "gift",
            or(
                within(s("gift"), p("tags", list_of(string()))),
                eq(len(p("tags", list_of(string()))), i(0)),
            ),
        ),
    ];
    k.model("Sizes", fields, rules);
    k.ir()
}

pub fn time_ir() -> Ir {
    let mut k = Kit::new();
    let end_after = gt(pt("end"), pt("start"));
    let span = bin_t(veld_ir::Op::Sub, pt("end"), pt("start"), DUR);
    let max_stay = le(span, dur(Builtin::Days, i(30)));
    k.model(
        "Stay",
        all_of(&["start", "end"], &datetime()),
        vec![rule("endAfterStart", end_after), rule("maxStay", max_stay)],
    );
    let shifted = eq(add_t(pt("a"), dur(Builtin::Hours, i(2))), pt("b"));
    let back = eq(add_sub(pt("b"), dur(Builtin::Minutes, i(120))), pt("a"));
    k.model(
        "Instants",
        all_of(&["a", "b"], &datetime()),
        vec![
            rule("same", eq(pt("a"), pt("b"))),
            rule("shifted", shifted),
            rule("back", back),
        ],
    );
    let gap = bin_t(veld_ir::Op::Sub, pdate("last"), pdate("first"), DUR);
    k.model(
        "Dates",
        all_of(&["first", "last"], &date()),
        vec![
            rule("ordered", le(pdate("first"), pdate("last"))),
            rule("week", eq(gap, dur(Builtin::Days, i(7)))),
        ],
    );
    k.model(
        "Clock",
        all_of(&["opens", "closes"], &time()),
        vec![rule("ordered", lt(pclock("opens"), pclock("closes")))],
    );
    let fits = gt(dur(Builtin::Days, pl("n")), dur(Builtin::Hours, i(1)));
    let plus = bin_t(
        veld_ir::Op::Add,
        dur(Builtin::Seconds, pl("n")),
        dur(Builtin::Ms, i(500)),
        DUR,
    );
    let minus = bin_t(veld_ir::Op::Sub, plus, dur(Builtin::Ms, i(500)), DUR);
    let sums = eq(minus, dur(Builtin::Seconds, pl("n")));
    k.model(
        "Dur",
        vec![field("n", long())],
        vec![rule("fits", fits), rule("sums", sums)],
    );
    k.ir()
}

fn add_t(l: veld_ir::Expr, r: veld_ir::Expr) -> veld_ir::Expr {
    bin_t(veld_ir::Op::Add, l, r, et(&datetime()))
}
fn add_sub(l: veld_ir::Expr, r: veld_ir::Expr) -> veld_ir::Expr {
    bin_t(veld_ir::Op::Sub, l, r, et(&datetime()))
}

pub fn lists() -> Ir {
    let mut k = Kit::new();
    let item = k.model(
        "Item",
        vec![
            field("sku", string()),
            field("qty", int()),
            field("price", dec()),
        ],
        vec![],
    );
    let items = || p("items", list_of(item.clone()));
    let qty = || v("i", &["qty"], int());
    let price = || v("i", &["price"], dec());
    let all_positive = quant(
        Quantifier::All,
        items(),
        "i",
        gt(qty(), i(0)),
        et(&boolean()),
    );
    let any_big = quant(
        Quantifier::Any,
        items(),
        "i",
        ge(qty(), i(10)),
        et(&boolean()),
    );
    let some_big = or(any_big, lt(len(items()), i(2)));
    let free = quant(
        Quantifier::Count,
        items(),
        "i",
        eq(price(), dc("0")),
        et(&int()),
    );
    let total = quant(
        Quantifier::Sum,
        items(),
        "i",
        mul(price(), widen(qty(), dec())),
        et(&dec()),
    );
    k.model(
        "Order",
        vec![field("items", list_of(item.clone())), field("total", dec())],
        vec![
            rule("allPositive", all_positive),
            rule("someBig", some_big),
            rule("fewFree", le(free, i(1))),
            rule("totalMatches", eq(total, pd("total"))),
        ],
    );
    let parts = || p("parts", list_of(int()));
    let capped = le(
        quant(
            Quantifier::Sum,
            parts(),
            "p",
            v("p", &[], int()),
            et(&int()),
        ),
        i(2_147_483_647),
    );
    let positive = quant(
        Quantifier::Count,
        parts(),
        "p",
        gt(v("p", &[], int()), i(0)),
        et(&int()),
    );
    let totals = k.model(
        "Totals",
        vec![field("parts", list_of(int()))],
        vec![rule("capped", capped), rule("few", lt(positive, i(3)))],
    );
    let inner = quant(
        Quantifier::All,
        v("r", &["parts"], list_of(int())),
        "p",
        ne(v("p", &[], int()), i(0)),
        et(&boolean()),
    );
    let rows = p("rows", list_of(totals.clone()));
    let no_zero = quant(Quantifier::All, rows, "r", inner, et(&boolean()));
    k.model(
        "Matrix",
        vec![field("rows", list_of(totals))],
        vec![rule("noZero", no_zero)],
    );
    let fits = or(le(pi("guests"), i(4)), eq(ps("tier"), s("vip")));
    k.model(
        "Party",
        vec![field("guests", int()), field("tier", string())],
        vec![rule("fits", fits)],
    );
    k.ir()
}

pub fn nested() -> Ir {
    let mut k = Kit::new();
    let base = k.model(
        "Base",
        all_of(&["low", "high"], &int()),
        vec![rule("ordered", le(pi("low"), pi("high")))],
    );
    let range = k.child(
        "Range",
        &base,
        vec![field("step", int())],
        vec![rule("stepFits", le(pi("step"), sub(pi("high"), pi("low"))))],
    );
    let fields = vec![
        field("rows", list_of(range.clone())),
        field("named", map_of(base.clone())),
        opt(field("main", range.clone())),
        field("page", page_of(base.clone())),
    ];
    let items = pp(&[("page", false), ("items", false)], list_of(base.clone()));
    let size = pp(&[("page", false), ("size", false)], int());
    k.model(
        "Grid",
        fields,
        vec![
            rule("hasRows", gt(len(p("rows", list_of(range))), i(0))),
            rule("pageFull", le(len(items), size)),
        ],
    );
    let card = k.model(
        "Card",
        vec![field("last4", string())],
        vec![rule("four", eq(len(ps("last4")), i(4)))],
    );
    let bank = k.model("Bank", vec![field("iban", string())], vec![]);
    let payment = k.union("Payment", "kind", &[("card", &card), ("bank", &bank)]);
    k.model(
        "Checkout",
        vec![
            field("payment", payment.clone()),
            opt(field("backup", payment)),
        ],
        vec![],
    );
    let fields = vec![
        ro(field("id", uuid())),
        field("title", string()),
        dflt(field("copies", int()), Literal::Int(1)),
        ro(field("createdBy", string())),
    ];
    k.model(
        "Draft",
        fields,
        vec![
            rule("titled", gt(len(ps("title")), i(0))),
            rule("hasCopies", ge(pi("copies"), i(1))),
            rule("ownerNamed", gt(len(ps("createdBy")), i(0))),
        ],
    );
    let fields = vec![
        field("start", date()),
        field("end", date()),
        field("guests", int()),
        opt(field("note", string())),
    ];
    k.model(
        "Form",
        fields,
        vec![
            rule("ordered", le(pdate("start"), pdate("end"))),
            rule("some", ge(pi("guests"), i(1))),
            rule(
                "noteShort",
                implies(has(&[("note", true)]), le(len(po("note", string())), i(3))),
            ),
        ],
    );
    k.ir()
}

pub fn optional() -> Ir {
    let mut k = Kit::new();
    let status = k.enum_(
        "Status",
        &[
            ("pending", "pending"),
            ("paid", "paid"),
            ("checkedIn", "CHECKED_IN"),
            ("cancelled", "cancelled"),
        ],
    );
    let fields = vec![
        field("id", uuid()),
        field("owner", uuid()),
        field("status", status.clone()),
        opt(field("previous", status.clone())),
        opt(field("paidAt", datetime())),
    ];
    let paid_set = list(
        vec![
            variant(&status, "paid", "paid"),
            variant(&status, "checkedIn", "CHECKED_IN"),
        ],
        &status,
    );
    let st = || p("status", status.clone());
    let rules = vec![
        rule("own", ne(pu("id"), pu("owner"))),
        rule(
            "paidHasTime",
            implies(within(st(), paid_set), has(&[("paidAt", true)])),
        ),
        rule(
            "notCancelled",
            ne(st(), variant(&status, "cancelled", "cancelled")),
        ),
        rule(
            "moved",
            implies(
                has(&[("previous", true)]),
                ne(po("previous", status.clone()), st()),
            ),
        ),
    ];
    k.model("Ticket", fields, rules);
    let address = k.model(
        "Address",
        vec![field("city", string()), opt(field("zip", string()))],
        vec![],
    );
    let fields = vec![
        field("name", string()),
        opt(field("nick", string())),
        opt(field("age", int())),
        opt(field("address", address)),
    ];
    let zip = || pp(&[("address", true), ("zip", true)], string());
    let rules = vec![
        rule(
            "nickShort",
            implies(has(&[("nick", true)]), le(len(po("nick", string())), i(5))),
        ),
        rule(
            "adult",
            or(not(has(&[("age", true)])), ge(po("age", int()), i(18))),
        ),
        rule(
            "zipped",
            or(
                and(
                    has(&[("address", true), ("zip", true)]),
                    eq(len(zip()), i(5)),
                ),
                not(has(&[("address", true)])),
            ),
        ),
        rule(
            "both",
            or(
                and(
                    and(has(&[("nick", true)]), has(&[("age", true)])),
                    lt(len(po("nick", string())), po("age", int())),
                ),
                not(has(&[("nick", true)])),
            ),
        ),
    ];
    k.model("Profile", fields, rules);
    k.ir()
}
