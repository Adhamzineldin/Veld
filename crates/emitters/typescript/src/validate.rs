//! Validation and wire-mapping descriptions.
//!
//! Instead of emitting hand-rolled validators per type, the generator emits a
//! declarative `Schema` literal per declaration; `runtime/veld.ts` interprets
//! it for validation, wire-name mapping, defaults and canonical bytes. The
//! same schema therefore behaves identically in every generated package.

use veld_codegen::CodeWriter;
use veld_codegen::canonical::es_number;
use veld_ir::{Constraints, Field, StringFormat, Type};

use crate::tree::FileCtx;
use crate::util::{literal, ts_str};

fn format_name(f: StringFormat) -> &'static str {
    match f {
        StringFormat::Email => "email",
        StringFormat::Uri => "uri",
        StringFormat::Hostname => "hostname",
        StringFormat::Ipv4 => "ipv4",
        StringFormat::Ipv6 => "ipv6",
    }
}

fn bounds(c: &Constraints) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(v) = c.min {
        out.push(format!("min: {}", es_number(v)));
    }
    if let Some(v) = c.max {
        out.push(format!("max: {}", es_number(v)));
    }
    if let Some(v) = c.min_len {
        out.push(format!("minLen: {v}"));
    }
    if let Some(v) = c.max_len {
        out.push(format!("maxLen: {v}"));
    }
    if let Some(p) = &c.pattern {
        out.push(format!("pattern: {}", ts_str(p)));
    }
    if let Some(f) = c.format {
        out.push(format!("format: {}", ts_str(format_name(f))));
    }
    out
}

/// A single-line `Schema` literal for `ty`, with `c` applied to the outermost node.
pub fn schema_expr(fc: &mut FileCtx<'_, '_>, ty: &Type, c: &Constraints) -> String {
    let mut parts = match ty {
        Type::Scalar(s) => vec![format!("k: '{}'", s.name())],
        Type::List(t) => vec![
            "k: 'list'".to_string(),
            format!("of: {}", schema_expr(fc, t, &Constraints::default())),
        ],
        Type::Map(t) => vec![
            "k: 'map'".to_string(),
            format!("of: {}", schema_expr(fc, t, &Constraints::default())),
        ],
        Type::Page(t) => vec![
            "k: 'page'".to_string(),
            format!("of: {}", schema_expr(fc, t, &Constraints::default())),
        ],
        Type::Decl(id) => {
            let name = fc.value_ref(*id);
            vec!["k: 'ref'".to_string(), format!("to: () => {name}.schema")]
        }
    };
    parts.extend(bounds(c));
    format!("{{ {} }}", parts.join(", "))
}

fn field_expr(fc: &mut FileCtx<'_, '_>, f: &Field) -> String {
    let mut parts = vec![format!("n: {}", ts_str(&f.name))];
    if f.wire != f.name {
        parts.push(format!("w: {}", ts_str(&f.wire)));
    }
    if f.derived.is_some() {
        // A derived field is computed, never supplied: no constraint is checked, it is never
        // required of a value, and the runtime drops what a request carries for it
        // (`spec/rules.md` section 17.2).
        let t = schema_expr(fc, &f.ty, &Constraints::default());
        parts.push(format!("t: {t}"));
        parts.push("o: true".to_string());
        parts.push("dv: true".to_string());
        return format!("{{ {} }},", parts.join(", "));
    }
    parts.push(format!("t: {}", schema_expr(fc, &f.ty, &f.constraints)));
    if f.optional {
        parts.push("o: true".to_string());
    }
    if let Some(d) = &f.default {
        parts.push(format!("d: {}", literal(d)));
    }
    if f.readonly {
        parts.push("ro: true".to_string());
    }
    format!("{{ {} }},", parts.join(", "))
}

/// Writes `k: 'object', fields: [...]` (the body of an object schema) at the writer's indentation.
pub fn write_object_schema(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, fields: &[Field]) {
    w.line("k: 'object',");
    if fields.is_empty() {
        w.line("fields: [],");
        return;
    }
    w.open_with("fields:", " [");
    for f in fields {
        let line = field_expr(fc, f);
        w.line(line);
    }
    w.close_with("],");
}
