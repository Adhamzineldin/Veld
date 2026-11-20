//! Identifier casing and reserved-word escaping.
//!
//! Generators derive target identifiers from source names here. Wire names are
//! never passed through this module.

/// Target languages that have a reserved-word list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ts,
    Java,
}

/// Splits an identifier into lowercase words.
///
/// Boundaries are non-alphanumeric characters, a lower-to-upper transition and
/// the end of an acronym (`XMLHttp` is `xml`, `http`). Digits stay with the
/// preceding letters.
#[must_use]
pub fn words(input: &str) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            flush(&mut cur, &mut out);
            continue;
        }
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1).copied();
        let boundary = c.is_uppercase()
            && prev.is_some_and(|p| {
                p.is_lowercase() || p.is_ascii_digit() || (p.is_uppercase() && next.is_some_and(char::is_lowercase))
            });
        if boundary {
            flush(&mut cur, &mut out);
        }
        cur.extend(c.to_lowercase());
    }
    flush(&mut cur, &mut out);
    out
}

fn flush(cur: &mut String, out: &mut Vec<String>) {
    if !cur.is_empty() {
        out.push(std::mem::take(cur));
    }
}

fn capitalize(word: &str) -> String {
    let mut it = word.chars();
    match it.next() {
        Some(f) => f.to_uppercase().chain(it).collect(),
        None => String::new(),
    }
}

/// `firstName`
#[must_use]
pub fn camel(input: &str) -> String {
    let w = words(input);
    let mut out = String::new();
    for (i, word) in w.iter().enumerate() {
        if i == 0 {
            out.push_str(word);
        } else {
            out.push_str(&capitalize(word));
        }
    }
    out
}

/// `FirstName`
#[must_use]
pub fn pascal(input: &str) -> String {
    words(input).iter().map(|w| capitalize(w)).collect()
}

/// `first_name`
#[must_use]
pub fn snake(input: &str) -> String {
    words(input).join("_")
}

/// `FIRST_NAME`
#[must_use]
pub fn screaming_snake(input: &str) -> String {
    snake(input).to_uppercase()
}

/// `first-name`
#[must_use]
pub fn kebab(input: &str) -> String {
    words(input).join("-")
}

const TS_RESERVED: &[&str] = &[
    "arguments", "await", "break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete",
    "do", "else", "enum", "eval", "export", "extends", "false", "finally", "for", "function", "if", "implements",
    "import", "in", "instanceof", "interface", "let", "new", "null", "package", "private", "protected", "public",
    "return", "static", "super", "switch", "this", "throw", "true", "try", "typeof", "var", "void", "while", "with",
    "yield",
];

/// Names that would shadow something generated TypeScript relies on when used as a type name.
const TS_TYPE_HAZARDS: &[&str] = &[
    "Array", "BigInt", "Boolean", "Date", "Error", "Function", "Map", "Number", "Object", "Partial", "Promise",
    "Readonly", "Record", "Set", "String", "Symbol", "VeldError", "VeldHttpError", "VeldTransportError", "any",
    "bigint", "boolean", "canonical", "never", "number", "object", "same", "string", "symbol", "undefined",
    "unknown", "veld",
];

const JAVA_RESERVED: &[&str] = &[
    "_", "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class", "const", "continue",
    "default", "do", "double", "else", "enum", "extends", "false", "final", "finally", "float", "for", "goto", "if",
    "implements", "import", "instanceof", "int", "interface", "long", "native", "new", "null", "package", "permits",
    "private", "protected", "public", "record", "return", "sealed", "short", "static", "strictfp", "super", "switch",
    "synchronized", "this", "throw", "throws", "transient", "true", "try", "var", "void", "volatile", "while",
    "yield",
];

fn reserved(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Ts => TS_RESERVED,
        Lang::Java => JAVA_RESERVED,
    }
}

/// Appends `_` when `ident` is a reserved word of `lang`; also guards a leading digit.
#[must_use]
pub fn escape(lang: Lang, ident: &str) -> String {
    let mut out = ident.to_string();
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    if reserved(lang).contains(&out.as_str()) {
        out.push('_');
    }
    out
}

/// Like [`escape`], for names that become type, class or companion identifiers.
#[must_use]
pub fn escape_type(lang: Lang, name: &str) -> String {
    let out = escape(lang, name);
    if lang == Lang::Ts && TS_TYPE_HAZARDS.contains(&out.as_str()) {
        return format!("{out}_");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_words() {
        assert_eq!(words("firstName"), ["first", "name"]);
        assert_eq!(words("XMLHttpRequest"), ["xml", "http", "request"]);
        assert_eq!(words("user_id2Name"), ["user", "id2", "name"]);
        assert_eq!(words("--a-b--"), ["a", "b"]);
        assert!(words("").is_empty());
    }

    #[test]
    fn casings() {
        assert_eq!(camel("order_item"), "orderItem");
        assert_eq!(pascal("order_item"), "OrderItem");
        assert_eq!(pascal("getOrder"), "GetOrder");
        assert_eq!(snake("OrderItem"), "order_item");
        assert_eq!(screaming_snake("orderItem"), "ORDER_ITEM");
        assert_eq!(kebab("OrderItems"), "order-items");
        assert_eq!(camel("HTTPServer"), "httpServer");
    }

    #[test]
    fn escapes_reserved_words() {
        assert_eq!(escape(Lang::Ts, "delete"), "delete_");
        assert_eq!(escape(Lang::Ts, "order"), "order");
        assert_eq!(escape(Lang::Java, "class"), "class_");
        assert_eq!(escape(Lang::Java, "delete"), "delete");
        assert_eq!(escape(Lang::Ts, "1st"), "_1st");
        assert_eq!(escape_type(Lang::Ts, "Record"), "Record_");
        assert_eq!(escape_type(Lang::Ts, "Order"), "Order");
    }
}
