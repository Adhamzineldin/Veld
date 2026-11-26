//! The reference for field visibility (`spec/platform.md` section 2).
//!
//! [`apply_visibility_type`] and [`apply_visibility_decl`] take a wire value, the caller and the
//! clock, and return the value with every `@visible` field the caller may not see left out, at
//! any depth: nested models, lists, maps, the `items` of a page and the arm of a union. They
//! also return the wire paths of what was dropped.
//!
//! * **Fail closed.** A field is dropped when its expression is false, when it cannot be
//!   evaluated (`spec/rules.md` section 3.7, or a read the value does not allow), and when the
//!   request has no caller of the principal type the field was checked against: no caller at all
//!   (`@auth(none)`), or a service whose principal is another model ([`Caller::of`]).
//! * **The expression sees the value as it is received.** Every `@visible` of a record is
//!   evaluated against the whole record before any of its fields is dropped, so one field's
//!   verdict never changes another's.
//! * **A field the value does not have** is not evaluated and not listed. A field with the value
//!   `null` is dropped and listed, as in the redaction.
//! * A value that does not have the shape of its declaration is an [`InputError`], not a pass.
//!
//! **Paths** are those of the redaction: `$` is the root, `.name` a member, `[2]` a list item,
//! sorted by their bytes without duplicates. An audit event writes them without the `$`
//! ([`unrooted`]).

use std::collections::BTreeSet;

use serde_json::{Map, Value as Json};
use veld_ir::{Decl, DeclId, Ir, Type};

use crate::error::InputError;
use crate::eval::{Env, eval};
use crate::value::Value;
use crate::walk::{Visitor, Walk};

/// Who the response is for, as far as visibility is concerned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caller<'a> {
    /// The model of the principal of the service and the caller's wire value. `None` is a
    /// request with no caller: an endpoint with `@auth(none)`.
    pub principal: Option<(DeclId, &'a Json)>,
}

impl<'a> Caller<'a> {
    /// A request with no caller.
    #[must_use]
    pub fn none() -> Caller<'a> {
        Caller { principal: None }
    }

    /// A caller whose value is `value`, of the principal model `model` of the service.
    #[must_use]
    pub fn of(model: DeclId, value: &'a Json) -> Caller<'a> {
        Caller {
            principal: Some((model, value)),
        }
    }
}

/// A value with the fields the caller may not see left out, and where they were.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub value: Json,
    /// The wire paths of the fields that were dropped, sorted, without duplicates.
    pub hidden: Vec<String>,
}

/// Applies visibility to `value`, a wire value of `decl`, rooted at `$`.
///
/// # Errors
/// [`InputError`] when the value does not have the shape of its declaration.
pub fn apply_visibility_decl(
    ir: &Ir,
    decl: &Decl,
    value: &Json,
    caller: Caller<'_>,
    now: i64,
) -> Result<Shown, InputError> {
    run(ir, caller, now, |w, v| w.decl(v, decl, value, "$"))
}

/// Applies visibility to `value`, a wire value of `ty` (a response, a list of models, a page, a
/// stream item), rooted at `$`.
///
/// # Errors
/// As [`apply_visibility_decl`].
pub fn apply_visibility_type(
    ir: &Ir,
    ty: &Type,
    value: &Json,
    caller: Caller<'_>,
    now: i64,
) -> Result<Shown, InputError> {
    run(ir, caller, now, |w, v| w.ty(v, ty, value, "$"))
}

/// Like [`apply_visibility_type`] with the paths starting at `root` instead of `$`.
///
/// # Errors
/// As [`apply_visibility_decl`].
pub fn apply_visibility_type_at(
    ir: &Ir,
    ty: &Type,
    value: &Json,
    caller: Caller<'_>,
    now: i64,
    root: &str,
) -> Result<Shown, InputError> {
    run(ir, caller, now, |w, v| w.ty(v, ty, value, root))
}

/// A path of this module without its root: `$.guestPhone` is `guestPhone`, `$[0].phone` is
/// `[0].phone`, and `$` alone is the empty text. This is the form of the `hidden` list of an
/// audit event.
#[must_use]
pub fn unrooted(path: &str) -> String {
    let rest = path.strip_prefix('$').unwrap_or(path);
    rest.strip_prefix('.').unwrap_or(rest).to_owned()
}

fn run(
    ir: &Ir,
    caller: Caller<'_>,
    now: i64,
    walk: impl FnOnce(&Walk<'_>, &mut Hide<'_>) -> Result<Json, InputError>,
) -> Result<Shown, InputError> {
    let mut hide = Hide {
        ir,
        caller,
        now,
        hidden: BTreeSet::new(),
    };
    let value = walk(&Walk { ir }, &mut hide)?;
    Ok(Shown {
        value,
        hidden: hide.hidden.into_iter().collect(),
    })
}

struct Hide<'a> {
    ir: &'a Ir,
    caller: Caller<'a>,
    now: i64,
    hidden: BTreeSet<String>,
}

impl Hide<'_> {
    /// True when the caller may see the field, whose expression is `expr` and was checked against
    /// the principal model `model`.
    fn allows(&self, field: &veld_ir::Field, obj: &Map<String, Json>) -> bool {
        let (Some(expr), Some(model)) = (&field.visible, field.visible_principal) else {
            return true;
        };
        let Some((have, value)) = self.caller.principal else {
            return false;
        };
        if have != model {
            return false;
        }
        let this = Json::Object(obj.clone());
        let mut env = Env::for_value(self.ir, &this);
        env.principal = Some(value.clone());
        env.now = Some(self.now);
        matches!(eval(&env, expr), Ok(Value::Bool(true)))
    }
}

impl Visitor for Hide<'_> {
    fn record(
        &mut self,
        walk: &Walk<'_>,
        decl: &Decl,
        obj: &Map<String, Json>,
        path: &str,
    ) -> Result<Json, InputError> {
        let mut kept = obj.clone();
        for f in decl.fields() {
            let Some(child) = obj.get(&f.wire) else {
                continue;
            };
            let here = format!("{path}.{}", f.wire);
            if f.visible.is_some() && !self.allows(f, obj) {
                kept.remove(&f.wire);
                self.hidden.insert(here);
            } else {
                kept.insert(f.wire.clone(), walk.ty(self, &f.ty, child, &here)?);
            }
        }
        Ok(Json::Object(kept))
    }
}
