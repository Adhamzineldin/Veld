//! Text rendering of the small syntax pieces: literals, types, annotations.

use std::fmt::Write as _;

use veld_compiler::syntax::ast::{
    Annotation, Arg, ArgValue, LitKind, Literal, TypeExpr, TypeExprKind,
};

/// Renders a string literal with the escapes the lexer understands.
pub(crate) fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn float(f: f64) -> String {
    let s = format!("{f}");
    if s.contains('.') { s } else { format!("{s}.0") }
}

pub(super) fn literal(l: &Literal) -> String {
    match &l.kind {
        LitKind::Str(s) => quote(s),
        LitKind::Int(n) => n.to_string(),
        LitKind::Float(f) => float(*f),
        LitKind::Bool(b) => b.to_string(),
        LitKind::Null => "null".to_string(),
        LitKind::Variant(v) => format!(".{v}"),
        LitKind::EmptyList => "[]".to_string(),
    }
}

pub(super) fn type_expr(t: &TypeExpr) -> String {
    match &t.kind {
        TypeExprKind::Named { name, args } => {
            if args.is_empty() {
                name.name.clone()
            } else {
                let inner: Vec<String> = args.iter().map(type_expr).collect();
                format!("{}<{}>", name.name, inner.join(", "))
            }
        }
        TypeExprKind::List(inner) => format!("{}[]", type_expr(inner)),
    }
}

fn arg(a: &Arg) -> String {
    let value = match &a.value {
        ArgValue::Literal(l) => literal(l),
        ArgValue::Ident(i) => i.name.clone(),
        ArgValue::Call { name, args } => format!("{}({})", name.name, args_list(args)),
    };
    match &a.name {
        Some(n) => format!("{}: {value}", n.name),
        None => value,
    }
}

fn args_list(args: &[Arg]) -> String {
    args.iter().map(arg).collect::<Vec<_>>().join(", ")
}

pub(super) fn annotation(a: &Annotation) -> String {
    match &a.args {
        Some(args) => format!("@{}({})", a.name.name, args_list(args)),
        None => format!("@{}", a.name.name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_escapes_specials() {
        assert_eq!(quote("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
        assert_eq!(quote("\u{1}"), "\"\\u{1}\"");
    }

    #[test]
    fn floats_always_have_a_dot() {
        assert_eq!(float(1.0), "1.0");
        assert_eq!(float(0.25), "0.25");
        assert_eq!(float(-3.0), "-3.0");
    }
}
