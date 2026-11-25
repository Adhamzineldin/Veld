//! Small builders for SMT-LIB 2 text. Everything here is pure and deterministic.

/// An integer literal; a negative one is `(- n)`.
#[must_use]
pub fn int(n: i128) -> String {
    if n < 0 {
        format!("(- {})", -n)
    } else {
        n.to_string()
    }
}

/// A real literal from decimal text such as `120.00` or `-3`.
#[must_use]
pub fn real_text(text: &str) -> String {
    let (neg, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let body = if digits.contains('.') {
        digits.to_owned()
    } else {
        format!("{digits}.0")
    };
    if neg { format!("(- {body})") } else { body }
}

/// A real literal for an integer.
#[must_use]
pub fn real_int(n: i128) -> String {
    real_text(&n.to_string())
}

/// A real literal for a finite `f64` that prints as plain decimal text (`Display` never uses an
/// exponent).
#[must_use]
pub fn real_f64(x: f64) -> String {
    real_text(&format!("{x}"))
}

/// `(and a b)`, simplified when a side is a constant.
#[must_use]
pub fn and2(a: &str, b: &str) -> String {
    match (a, b) {
        ("false", _) | (_, "false") => "false".to_owned(),
        ("true", x) | (x, "true") => x.to_owned(),
        _ => format!("(and {a} {b})"),
    }
}

/// `(or a b)`, simplified when a side is a constant.
#[must_use]
pub fn or2(a: &str, b: &str) -> String {
    match (a, b) {
        ("true", _) | (_, "true") => "true".to_owned(),
        ("false", x) | (x, "false") => x.to_owned(),
        _ => format!("(or {a} {b})"),
    }
}

/// `(or a b c ...)` over any number of terms.
#[must_use]
pub fn or_all(items: &[String]) -> String {
    items.iter().fold("false".to_owned(), |acc, x| or2(&acc, x))
}

/// `(and a b c ...)` over any number of terms.
#[must_use]
pub fn and_all(items: &[String]) -> String {
    items.iter().fold("true".to_owned(), |acc, x| and2(&acc, x))
}

/// `(not a)`.
#[must_use]
pub fn not(a: &str) -> String {
    match a {
        "true" => "false".to_owned(),
        "false" => "true".to_owned(),
        _ => format!("(not {a})"),
    }
}

/// `x` is outside `[lo, hi]`.
#[must_use]
pub fn outside(x: &str, lo: i128, hi: i128) -> String {
    format!("(or (< {x} {}) (> {x} {}))", int(lo), int(hi))
}

/// `a / b` truncated toward zero, for integers; SMT `div` rounds differently for a negative
/// divisor or dividend. `a` and `b` are atoms. For `b = 0` the value is arbitrary.
#[must_use]
pub fn trunc_div(a: &str, b: &str) -> String {
    format!(
        "(ite (>= {a} 0) (ite (> {b} 0) (div {a} {b}) (- (div {a} (- {b})))) \
         (ite (> {b} 0) (- (div (- {a}) {b})) (div (- {a}) (- {b}))))"
    )
}

/// `|name|`, the quoted symbol of a name.
#[must_use]
pub fn sym(name: &str) -> String {
    format!("|{name}|")
}

/// A string literal for `(echo ...)`.
#[must_use]
pub fn echo(text: &str) -> String {
    format!("(echo \"{}\")", text.replace('"', "\"\""))
}
