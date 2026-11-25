//! A generated request as an `example` block (`spec/rules.md` section 12), so a developer can
//! paste a failing request into the contract.
//!
//! Every value is written out, including the ones a filled-in example would supply, so that
//! filling the block in gives back the request. The one exception is a `@readonly` field of a
//! request body: the generator leaves it out (it is absent in a request), and the filler adds it.

use serde_json::Value as Json;
use veld_ir::{Decl, DeclKind, Endpoint, Field, Ir, Scalar, Service, Type};
use veld_rules::{Outcome, Request, format_datetime};

/// The block for a request that is expected to end in `outcome`.
///
/// `None` when a value cannot be written in the syntax of an example (a `json` value that holds
/// `null`, or a map key that is not an identifier).
pub fn render_example(
    ir: &Ir,
    service: &Service,
    ep: &Endpoint,
    req: &Request,
    outcome: &Outcome,
    name: &str,
) -> Option<String> {
    let mut lines = vec![format!("example {} {{", quote(name)?)];
    for f in &ep.rules.facts {
        let value = match req.facts.get(&f.name) {
            Some(Some(v)) => value(ir, &f.ty, v)?,
            Some(None) | None => "absent".to_owned(),
        };
        lines.push(format!("  given {} = {value}", f.name));
    }
    lines.push(format!("  at {}", quote(&format_datetime(req.now))?));
    let mut args = Vec::new();
    for p in ep.params.iter().filter(|p| !p.stream) {
        if let Some(v) = req.params.get(&p.name) {
            args.push(format!("{}: {}", p.name, value(ir, &p.ty, v)?));
        }
    }
    let callee = if is_ambiguous(ir, &ep.name) {
        format!("{}.{}", service.name, ep.name)
    } else {
        ep.name.clone()
    };
    lines.push(format!(
        "  {callee}({}) -> {}",
        args.join(", "),
        outcome_text(ir, outcome)?
    ));
    lines.push("}".to_owned());
    Some(lines.join("\n"))
}

fn is_ambiguous(ir: &Ir, endpoint: &str) -> bool {
    let owners = ir
        .services
        .iter()
        .filter(|s| s.endpoints.iter().any(|e| e.name == endpoint))
        .count();
    owners > 1
}

fn outcome_text(ir: &Ir, outcome: &Outcome) -> Option<String> {
    match outcome {
        Outcome::Passes => Some("ok".to_owned()),
        Outcome::Error(a) => {
            let has_data = a.data.as_object().is_some_and(|o| !o.is_empty());
            if a.name == "ValidationError" || !has_data {
                return Some(a.name.clone());
            }
            Some(format!(
                "{} {}",
                a.name,
                value(ir, &Type::Decl(a.decl), &a.data)?
            ))
        }
    }
}

fn quote(text: &str) -> Option<String> {
    serde_json::to_string(text).ok()
}

/// A wire value of type `ty`, in the syntax of an example.
fn value(ir: &Ir, ty: &Type, j: &Json) -> Option<String> {
    match ty {
        Type::Scalar(s) => scalar(*s, j),
        Type::List(t) => {
            let items: Option<Vec<String>> =
                j.as_array()?.iter().map(|i| value(ir, t, i)).collect();
            Some(format!("[{}]", items?.join(", ")))
        }
        Type::Map(t) => {
            let mut parts = Vec::new();
            for (k, v) in j.as_object()? {
                parts.push(format!("{}: {}", ident(k)?, value(ir, t, v)?));
            }
            Some(braces(&parts))
        }
        Type::Page(t) => {
            let obj = j.as_object()?;
            let mut parts = Vec::new();
            for (k, v) in obj {
                let rendered = match k.as_str() {
                    "items" => value(ir, &Type::List(t.clone()), v)?,
                    "total" => scalar(Scalar::Long, v)?,
                    _ => scalar(Scalar::Int, v)?,
                };
                parts.push(format!("{k}: {rendered}"));
            }
            Some(braces(&parts))
        }
        Type::Decl(id) => decl_value(ir, ir.decls.get(id.index())?, j),
    }
}

fn decl_value(ir: &Ir, decl: &Decl, j: &Json) -> Option<String> {
    match &decl.kind {
        DeclKind::Enum(e) => {
            let wire = j.as_str()?;
            let v = e.variants.iter().find(|v| v.wire == wire)?;
            Some(format!(".{}", v.name))
        }
        DeclKind::Model(_) | DeclKind::Error(_) => Some(braces(&fields(ir, decl.fields(), j)?)),
        DeclKind::Union(u) => {
            let label = j.get(&u.discriminator)?.as_str()?;
            let arm = u.arms.iter().find(|a| a.label == label)?;
            let arm_decl = ir.decls.get(arm.decl.index())?;
            let mut parts = vec![format!("{}: {}", u.discriminator, quote(label)?)];
            parts.extend(fields(ir, arm_decl.fields(), j)?);
            Some(braces(&parts))
        }
        DeclKind::Const(_) => None,
    }
}

fn fields(ir: &Ir, decl_fields: &[Field], j: &Json) -> Option<Vec<String>> {
    let obj = j.as_object()?;
    let mut parts = Vec::new();
    for f in decl_fields {
        if let Some(v) = obj.get(&f.wire) {
            parts.push(format!("{}: {}", f.name, value(ir, &f.ty, v)?));
        }
    }
    Some(parts)
}

fn braces(parts: &[String]) -> String {
    if parts.is_empty() {
        "{}".to_owned()
    } else {
        format!("{{ {} }}", parts.join(", "))
    }
}

fn ident(key: &str) -> Option<&str> {
    let mut chars = key.chars();
    let first = chars.next()?;
    let ok = (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then_some(key)
}

fn scalar(s: Scalar, j: &Json) -> Option<String> {
    match (s, j) {
        (Scalar::Json, _) => json_text(j),
        (Scalar::Bool, Json::Bool(b)) => Some(b.to_string()),
        (Scalar::Int | Scalar::Float, Json::Number(n)) => Some(n.to_string()),
        (Scalar::Long, Json::String(t)) => Some(t.clone()),
        (_, Json::String(t)) => quote(t),
        _ => None,
    }
}

fn json_text(j: &Json) -> Option<String> {
    match j {
        Json::Null => None,
        Json::Bool(b) => Some(b.to_string()),
        Json::Number(n) => Some(n.to_string()),
        Json::String(s) => quote(s),
        Json::Array(items) => {
            let parts: Option<Vec<String>> = items.iter().map(json_text).collect();
            Some(format!("[{}]", parts?.join(", ")))
        }
        Json::Object(map) => {
            let mut parts = Vec::new();
            for (k, v) in map {
                parts.push(format!("{}: {}", ident(k)?, json_text(v)?));
            }
            Some(braces(&parts))
        }
    }
}
