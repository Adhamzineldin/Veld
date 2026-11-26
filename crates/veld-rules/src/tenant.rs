//! The reference for tenancy (`spec/platform.md` section 3).
//!
//! * [`caller_tenant`] reads the tenant of a caller from its principal.
//! * [`refused`] walks a wire value of a type, at any depth (nested models, lists, maps, the
//!   `items` of a page, the arm of a union), and reports every tenant-scoped value that is not
//!   the caller's: the answer is then `500 Internal`, with these as what `onRuleViolation` is
//!   told, and the value is never sent. The same function judges a fact that was loaded: a fact
//!   with such a value is **not found**.
//!
//! A tenant-scoped value is the value of a model with a `@tenant` field. It belongs to the
//! caller when both have a tenant and the two are equal as values of the field's type (a uuid in
//! any case, a `long` by its number). A caller with no tenant, and a value with none, belong to
//! nobody.

use serde_json::{Map, Value as Json};
use veld_ir::{Decl, DeclId, Ir, Type};

use crate::check::Violation;
use crate::error::InputError;
use crate::ops;
use crate::value::Value;
use crate::walk::{Visitor, Walk};

/// The tenant of the caller: the value of the `@tenant` field of the principal's model. `None`
/// when the model has none, the principal does not have the member, or it is `null`.
#[must_use]
pub fn caller_tenant(ir: &Ir, model: DeclId, principal: &Json) -> Option<Json> {
    let field = ir.decls.get(model.index())?.tenant_field()?;
    principal.get(&field.wire).filter(|v| !v.is_null()).cloned()
}

/// True when the tenant `have` of a value of the field `field` is the caller's tenant `caller`.
fn same(ir: &Ir, field: &veld_ir::Field, have: &Json, caller: &Json) -> bool {
    let Ok(a) = Value::from_json(ir, &field.ty, have) else {
        return false;
    };
    let Ok(b) = Value::from_json(ir, &field.ty, caller) else {
        return false;
    };
    ops::equals(&a, &b).unwrap_or(false)
}

/// Every tenant-scoped value of `value`, a wire value of `ty`, that is not the caller's, in the
/// order of the walk. `caller` is the caller's tenant ([`caller_tenant`]); `root` starts the
/// paths of the violations (`result`, `error`, `fact booking`).
///
/// A violation has the rule name `tenant`, the id of the `@tenant` field, the path of the
/// value, the wire name of the field in `fields` and the message `violates rule tenant`.
///
/// # Errors
/// [`InputError`] when the value does not have the shape of its type.
pub fn refused(
    ir: &Ir,
    ty: &Type,
    value: &Json,
    caller: Option<&Json>,
    root: &str,
) -> Result<Vec<Violation>, InputError> {
    let mut visit = Foreign {
        ir,
        caller,
        root,
        found: Vec::new(),
    };
    Walk { ir }.ty(&mut visit, ty, value, "$")?;
    Ok(visit.found)
}

struct Foreign<'a> {
    ir: &'a Ir,
    caller: Option<&'a Json>,
    root: &'a str,
    found: Vec<Violation>,
}

impl Visitor for Foreign<'_> {
    fn record(
        &mut self,
        walk: &Walk<'_>,
        decl: &Decl,
        obj: &Map<String, Json>,
        path: &str,
    ) -> Result<Json, InputError> {
        if let Some(field) = decl.tenant_field() {
            let mine = match (obj.get(&field.wire).filter(|v| !v.is_null()), self.caller) {
                (Some(have), Some(caller)) => same(self.ir, field, have, caller),
                (_, _) => false,
            };
            if !mine {
                self.found.push(Violation {
                    rule_name: "tenant".to_owned(),
                    rule_id: field.tenant_id.clone().unwrap_or_default(),
                    path: format!("{}{}", self.root, path.strip_prefix('$').unwrap_or(path)),
                    fields: vec![field.wire.clone()],
                    message: "violates rule tenant".to_owned(),
                    error: None,
                });
            }
        }
        for f in decl.fields() {
            if let Some(child) = obj.get(&f.wire) {
                walk.ty(self, &f.ty, child, &format!("{path}.{}", f.wire))?;
            }
        }
        Ok(Json::Null)
    }
}
