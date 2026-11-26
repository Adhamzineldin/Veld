//! A walk over a wire value of a type: through nested models, lists, maps, the `items` of a page
//! and the arm of a union, calling a [`Visitor`] on every record (a model, an error or an
//! event). Field visibility (`spec/platform.md` section 2) and the tenant check (section 3) are
//! visitors; the redaction of `spec/rules.md` section 19.3 predates it and has its own walk.
//!
//! Paths are written as the violations of `spec/rules.md` section 7 write them: `$` is the
//! root, `.name` a member (the wire name), `[2]` a list item. A value that does not have the
//! shape of its declaration is an error, not a pass: such a value cannot be known to be clean.

use serde_json::{Map, Value as Json};
use veld_ir::{Decl, DeclKind, Ir, Type};

use crate::error::InputError;

/// What happens at a record.
pub(crate) trait Visitor {
    /// The record `obj` of `decl`, at `path`. The visitor returns what is kept of it, and calls
    /// `walk.ty` for the children it keeps.
    fn record(
        &mut self,
        walk: &Walk<'_>,
        decl: &Decl,
        obj: &Map<String, Json>,
        path: &str,
    ) -> Result<Json, InputError>;
}

pub(crate) struct Walk<'a> {
    pub ir: &'a Ir,
}

fn kind_of(v: &Json) -> &'static str {
    match v {
        Json::Null => "null",
        Json::Bool(_) => "boolean",
        Json::Number(_) => "number",
        Json::String(_) => "string",
        Json::Array(_) => "array",
        Json::Object(_) => "object",
    }
}

fn mismatch(expected: &str, found: &Json, path: &str) -> InputError {
    InputError::Mismatch {
        path: path.to_owned(),
        expected: expected.to_owned(),
        found: kind_of(found).to_owned(),
    }
}

impl Walk<'_> {
    /// Walks `v`, a value of `ty`.
    pub fn ty(
        &self,
        visitor: &mut impl Visitor,
        ty: &Type,
        v: &Json,
        path: &str,
    ) -> Result<Json, InputError> {
        if v.is_null() {
            return Ok(Json::Null);
        }
        match ty {
            Type::Scalar(_) => Ok(v.clone()),
            Type::List(t) => self.items(visitor, t, v, path),
            Type::Map(t) => {
                let entries = v.as_object().ok_or_else(|| mismatch("object", v, path))?;
                let mut kept = Map::new();
                for (k, e) in entries {
                    kept.insert(k.clone(), self.ty(visitor, t, e, &format!("{path}.{k}"))?);
                }
                Ok(Json::Object(kept))
            }
            Type::Page(t) => {
                let obj = v.as_object().ok_or_else(|| mismatch("object", v, path))?;
                let mut kept = obj.clone();
                if let Some(items) = obj.get("items") {
                    let here = format!("{path}.items");
                    kept.insert("items".to_owned(), self.items(visitor, t, items, &here)?);
                }
                Ok(Json::Object(kept))
            }
            Type::Decl(id) => {
                let decl = self
                    .ir
                    .decls
                    .get(id.index())
                    .ok_or_else(|| InputError::ir("a type refers to a missing declaration"))?;
                self.decl(visitor, decl, v, path)
            }
        }
    }

    fn items(
        &self,
        visitor: &mut impl Visitor,
        item: &Type,
        v: &Json,
        path: &str,
    ) -> Result<Json, InputError> {
        let list = v.as_array().ok_or_else(|| mismatch("array", v, path))?;
        let kept: Result<Vec<Json>, InputError> = list
            .iter()
            .enumerate()
            .map(|(i, e)| self.ty(visitor, item, e, &format!("{path}[{i}]")))
            .collect();
        Ok(Json::Array(kept?))
    }

    /// Walks `v`, a value of `decl`.
    pub fn decl(
        &self,
        visitor: &mut impl Visitor,
        decl: &Decl,
        v: &Json,
        path: &str,
    ) -> Result<Json, InputError> {
        if v.is_null() {
            return Ok(Json::Null);
        }
        match &decl.kind {
            DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Event(_) => {
                let obj = v.as_object().ok_or_else(|| mismatch("object", v, path))?;
                visitor.record(self, decl, obj, path)
            }
            DeclKind::Union(u) => {
                let obj = v.as_object().ok_or_else(|| mismatch("object", v, path))?;
                let label = obj.get(&u.discriminator).and_then(Json::as_str);
                let arm = label
                    .and_then(|l| u.arms.iter().find(|a| a.label == l))
                    .ok_or_else(|| {
                        InputError::Unsupported(format!(
                            "{path}: the value names no arm of the union `{}`",
                            decl.name
                        ))
                    })?;
                let arm = self
                    .ir
                    .decls
                    .get(arm.decl.index())
                    .ok_or_else(|| InputError::ir("an arm refers to a missing declaration"))?;
                self.decl(visitor, arm, v, path)
            }
            DeclKind::Enum(_) | DeclKind::Const(_) => Ok(v.clone()),
        }
    }
}
