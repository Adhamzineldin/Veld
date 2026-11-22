//! Case checks of `spec/language.md` section 8.

fn has_separator(s: &str) -> bool {
    s.contains('_') || s.contains('-')
}

/// `PascalCase`: starts with an upper-case letter, no separators.
pub fn is_pascal(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && !has_separator(s)
}

/// `camelCase`: starts with a lower-case letter, no separators.
pub fn is_camel(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_lowercase()) && !has_separator(s)
}

/// `snake_case`: lower-case letters, digits and single underscores inside.
pub fn is_snake(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && !s.ends_with('_')
        && !s.contains("__")
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Enum variants may be `camelCase` or `snake_case`.
pub fn is_variant_name(s: &str) -> bool {
    is_camel(s) || is_snake(s)
}

/// A short fix suggestion for the message.
pub fn to_pascal(s: &str) -> String {
    let mut out = String::new();
    for part in s.split(['_', '-']).filter(|p| !p.is_empty()) {
        let mut cs = part.chars();
        if let Some(c) = cs.next() {
            out.extend(c.to_uppercase());
            out.push_str(cs.as_str());
        }
    }
    out
}

pub fn to_camel(s: &str) -> String {
    let p = to_pascal(s);
    let mut cs = p.chars();
    match cs.next() {
        Some(c) => c.to_lowercase().chain(cs).collect(),
        None => p,
    }
}
