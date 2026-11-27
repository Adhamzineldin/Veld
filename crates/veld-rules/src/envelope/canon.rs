//! The canonical scalar formats of `spec/ir.md` section 5.3 for the `data` of a message: a `uuid`
//! in lowercase, a `datetime` in UTC with `Z` and at most milliseconds. Other scalars are written
//! as they are given: a decimal or a long is already text on the wire, a `bytes` value is already
//! padded base64.

use serde_json::Value as Json;
use veld_ir::{DeclKind, Ir, Scalar, Type};

use crate::helpers::instant::parse_datetime;
use crate::wire::format_datetime;

/// Rewrites the scalars of `value` (an instance of `ty`) in place.
pub(crate) fn scalars(ir: &Ir, ty: &Type, value: &mut Json) {
    match ty {
        Type::Scalar(Scalar::Uuid) => {
            if let Json::String(s) = value {
                *s = s.to_lowercase();
            }
        }
        Type::Scalar(Scalar::Datetime) => {
            if let Some(ms) = value.as_str().and_then(parse_datetime) {
                *value = Json::String(format_datetime(ms));
            }
        }
        Type::Scalar(_) => {}
        Type::List(t) => {
            for item in value.as_array_mut().into_iter().flatten() {
                scalars(ir, t, item);
            }
        }
        Type::Map(t) => {
            for item in value
                .as_object_mut()
                .into_iter()
                .flat_map(|m| m.values_mut())
            {
                scalars(ir, t, item);
            }
        }
        Type::Page(t) => {
            if let Some(items) = value.get_mut("items") {
                scalars(ir, &Type::List(t.clone()), items);
            }
        }
        Type::Decl(id) => decl(ir, *id, value),
    }
}

fn decl(ir: &Ir, id: veld_ir::DeclId, value: &mut Json) {
    let Some(d) = ir.decls.get(id.index()) else {
        return;
    };
    match &d.kind {
        DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Event(_) => {
            for f in d.fields() {
                if let Some(child) = value.get_mut(&f.wire) {
                    scalars(ir, &f.ty, child);
                }
            }
        }
        DeclKind::Union(u) => {
            let label = value.get(&u.discriminator).and_then(Json::as_str);
            let arm = u.arms.iter().find(|a| Some(a.label.as_str()) == label);
            if let Some(arm) = arm {
                decl(ir, arm.decl, value);
            }
        }
        DeclKind::Enum(_) | DeclKind::Const(_) => {}
    }
}
