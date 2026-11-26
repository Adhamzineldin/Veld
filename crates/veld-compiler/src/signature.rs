//! Identity signatures, exactly `spec/ir.md` sections 5.1 and 5.2.
//!
//! This is the only module that computes signatures. Shapes are serialised as
//! RFC 8785 canonical JSON; they contain only strings, bools, arrays and
//! objects, so no number formatting is needed.

mod endpoint;
mod event;
mod policy;
mod rules;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

pub use endpoint::{
    EndpointRef, StatementKind, TransitionRef, endpoint_rule_bytes, endpoint_rule_id,
    fill_rules_digests, rules_digest, transition_rule_bytes, transition_rule_id,
    version_rule_bytes, version_rule_id,
};
pub use event::{event_rules_digest, fill_event_digests};
pub use policy::{policy_bytes, policy_id};
pub use rules::{derived_bytes, derived_id, derived_ids, rule_id, rule_ids};
use sha2::{Digest, Sha256};
use veld_ir::{Decl, DeclKind, Method, Package, PathSegment, Type};

/// The JSON subset used by signature shapes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Json {
    Str(String),
    Bool(bool),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

fn cmp_utf16(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

fn escape(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// RFC 8785 canonical serialisation: no whitespace, object keys sorted by
/// UTF-16 code units.
#[must_use]
pub fn canonical(j: &Json) -> String {
    let mut out = String::new();
    write_json(j, &mut out);
    out
}

fn write_json(j: &Json, out: &mut String) {
    match j {
        Json::Str(s) => escape(s, out),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json(item, out);
            }
            out.push(']');
        }
        Json::Obj(members) => {
            let mut sorted: Vec<&(String, Json)> = members.iter().collect();
            sorted.sort_by(|a, b| cmp_utf16(&a.0, &b.0));
            out.push('{');
            for (i, (k, v)) in sorted.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                escape(k, out);
                out.push(':');
                write_json(v, out);
            }
            out.push('}');
        }
    }
}

/// First 16 lowercase hex characters of the SHA-256 of `bytes`.
#[must_use]
pub fn hex16(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .take(8)
        .fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

fn obj(members: Vec<(&str, Json)>) -> Json {
    Json::Obj(
        members
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

fn text(s: &str) -> Json {
    Json::Str(s.to_string())
}

fn tshape(t: &Type, decls: &[Decl]) -> Json {
    match t {
        Type::Scalar(s) => text(s.name()),
        Type::List(i) => obj(vec![("list", tshape(i, decls))]),
        Type::Map(i) => obj(vec![("map", tshape(i, decls))]),
        Type::Page(i) => obj(vec![("page", tshape(i, decls))]),
        Type::Decl(d) => obj(vec![("ref", text(&decls[d.index()].qualified))]),
    }
}

fn local_shape(d: &Decl, decls: &[Decl]) -> Json {
    match &d.kind {
        DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Event(_) => {
            let mut fields: Vec<&veld_ir::Field> = d.fields().iter().collect();
            fields.sort_by(|a, b| cmp_utf16(&a.wire, &b.wire));
            let f = fields
                .into_iter()
                .map(|f| {
                    obj(vec![
                        ("w", text(&f.wire)),
                        ("t", tshape(&f.ty, decls)),
                        ("o", Json::Bool(f.optional)),
                    ])
                })
                .collect();
            let k = match &d.kind {
                DeclKind::Error(_) => "error",
                DeclKind::Event(_) => "event",
                DeclKind::Model(_)
                | DeclKind::Enum(_)
                | DeclKind::Union(_)
                | DeclKind::Const(_) => "model",
            };
            obj(vec![("k", text(k)), ("f", Json::Arr(f))])
        }
        DeclKind::Enum(e) => {
            let mut wires: Vec<&str> = e.variants.iter().map(|v| v.wire.as_str()).collect();
            wires.sort_by(|a, b| cmp_utf16(a, b));
            obj(vec![
                ("k", text("enum")),
                ("v", Json::Arr(wires.into_iter().map(text).collect())),
            ])
        }
        DeclKind::Union(u) => {
            let arms = u
                .arms
                .iter()
                .map(|a| (a.label.clone(), text(&decls[a.decl.index()].qualified)))
                .collect();
            obj(vec![
                ("k", text("union")),
                ("d", text(&u.discriminator)),
                ("a", Json::Obj(arms)),
            ])
        }
        DeclKind::Const(c) => obj(vec![("k", text("const")), ("t", tshape(&c.ty, decls))]),
    }
}

/// Declarations directly referenced by the shape of `d`.
fn shape_refs(d: &Decl) -> Vec<usize> {
    let mut ids = Vec::new();
    match &d.kind {
        DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Event(_) => {
            d.fields().iter().for_each(|f| f.ty.collect_decls(&mut ids))
        }
        DeclKind::Union(u) => ids.extend(u.arms.iter().map(|a| a.decl)),
        DeclKind::Const(c) => c.ty.collect_decls(&mut ids),
        DeclKind::Enum(_) => {}
    }
    ids.into_iter().map(veld_ir::DeclId::index).collect()
}

/// The `shape_bytes` of declaration `root` (`spec/ir.md` 5.1).
#[must_use]
pub fn shape_bytes(root: usize, decls: &[Decl]) -> String {
    let mut seen = BTreeSet::new();
    let mut stack = vec![root];
    let mut shapes: BTreeMap<String, Json> = BTreeMap::new();
    while let Some(i) = stack.pop() {
        if !seen.insert(i) {
            continue;
        }
        shapes.insert(decls[i].qualified.clone(), local_shape(&decls[i], decls));
        stack.extend(shape_refs(&decls[i]));
    }
    let members = shapes.into_iter().collect();
    canonical(&obj(vec![
        ("root", text(&decls[root].qualified)),
        ("decls", Json::Obj(members)),
    ]))
}

/// `veld:<package>/<Name>@<hex16>` for every declaration, by index.
#[must_use]
pub fn decl_sigs(decls: &[Decl], packages: &[Package]) -> Vec<String> {
    (0..decls.len())
        .map(|i| {
            let d = &decls[i];
            let pkg = &packages[d.package.index()].name;
            format!(
                "veld:{pkg}/{}@{}",
                d.name,
                hex16(shape_bytes(i, decls).as_bytes())
            )
        })
        .collect()
}

/// `veld:<package>/<Service>.<endpoint>@<hex16>` (`spec/ir.md` 5.2).
#[must_use]
pub fn op_sig(package: &str, service: &str, endpoint: &str, method: Method, path: &str) -> String {
    let erased: String = veld_ir::split_path(path)
        .into_iter()
        .map(|s| match s {
            PathSegment::Literal(l) => l.to_string(),
            PathSegment::Param(_) => "{}".to_string(),
        })
        .collect();
    let bytes = canonical(&obj(vec![
        ("m", text(method.as_str())),
        ("p", Json::Str(erased)),
    ]));
    format!(
        "veld:{package}/{service}.{endpoint}@{}",
        hex16(bytes.as_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_sorts_keys_and_escapes() {
        let j = obj(vec![
            ("b", text("x\"\\\n\u{1}")),
            ("a", Json::Arr(vec![Json::Bool(true), Json::Bool(false)])),
        ]);
        assert_eq!(canonical(&j), r#"{"a":[true,false],"b":"x\"\\\n\u0001"}"#);
    }

    #[test]
    fn keys_sort_by_utf16_code_units_not_code_points() {
        // U+1F600 is the surrogate pair D83D DE00; U+FF5E is the single unit FF5E.
        // Code point order puts FF5E first; UTF-16 order puts the emoji first.
        let j = obj(vec![
            ("\u{ff5e}", Json::Bool(true)),
            ("\u{1f600}", Json::Bool(false)),
        ]);
        assert_eq!(canonical(&j), "{\"\u{1f600}\":false,\"\u{ff5e}\":true}");
    }

    #[test]
    fn non_ascii_text_is_not_escaped() {
        assert_eq!(canonical(&text("é\u{7f}")), "\"é\u{7f}\"");
    }

    #[test]
    fn hex16_is_sixteen_lowercase_hex_chars() {
        // sha256("abc") = ba7816bf8f01cfea414140de5dae2223...
        assert_eq!(hex16(b"abc"), "ba7816bf8f01cfea");
    }

    #[test]
    fn op_sig_erases_parameter_names() {
        let a = op_sig("orders", "Orders", "getOrder", Method::Get, "/orders/{id}");
        let b = op_sig(
            "orders",
            "Orders",
            "getOrder",
            Method::Get,
            "/orders/{other}",
        );
        assert_eq!(a, b);
        assert!(a.starts_with("veld:orders/Orders.getOrder@"));
        assert_eq!(a.len(), "veld:orders/Orders.getOrder@".len() + 16);
        assert_ne!(
            a,
            op_sig("orders", "Orders", "getOrder", Method::Post, "/orders/{id}")
        );
    }

    #[test]
    fn op_bytes_match_the_spec_form() {
        let expected = hex16(br#"{"m":"GET","p":"/a/{}"}"#);
        assert!(op_sig("p", "S", "e", Method::Get, "/a/{x}").ends_with(&expected));
    }
}
