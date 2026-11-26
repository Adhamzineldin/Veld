//! The reference for what a capture or an audit event may keep of a value
//! (`spec/rules.md` section 19.3).
//!
//! [`redact_decl`] and [`redact_type`] take a wire value and return it with every field marked
//! `@pii` or `@secret` left out, at any depth: nested models, lists, maps, the `items` of a page and
//! the arm of a union. They also return the wire paths of what was left out. A field that is
//! marked is removed whole, with whatever it holds, and only its own path is listed. A field the
//! value does not have is not listed. Members the declaration does not know are kept.
//!
//! **Paths** are written as the violations of section 7 write them: `$` is the root, `.name` a
//! member (the wire name), `[2]` a list item. They are sorted by their bytes and without
//! duplicates, so the same value always gives the same list.

use std::collections::BTreeSet;

use serde_json::{Map, Value as Json};
use veld_ir::{Decl, DeclKind, Ir, Type};

use crate::error::InputError;

/// A value with the marked fields left out, and where they were.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redacted {
    pub value: Json,
    /// The wire paths of the fields that were left out, sorted, without duplicates.
    pub paths: Vec<String>,
}

/// Redacts `value`, a wire value of `decl`, rooted at `$`.
///
/// # Errors
/// [`InputError`] when the value does not have the shape of its declaration: a value that cannot
/// be walked cannot be known to be clean, so nothing is returned.
pub fn redact_decl(ir: &Ir, decl: &Decl, value: &Json) -> Result<Redacted, InputError> {
    redact_decl_at(ir, decl, value, "$")
}

/// Redacts `value`, a wire value of `ty` (a parameter, a fact, a list of models, a page, a
/// scalar), rooted at `$`.
///
/// # Errors
/// As [`redact_decl`].
pub fn redact_type(ir: &Ir, ty: &Type, value: &Json) -> Result<Redacted, InputError> {
    redact_type_at(ir, ty, value, "$")
}

/// Like [`redact_decl`], with the paths starting at `root` instead of `$` (`params.body`, for
/// a value that is one part of a larger capture).
///
/// # Errors
/// As [`redact_decl`].
pub fn redact_decl_at(
    ir: &Ir,
    decl: &Decl,
    value: &Json,
    root: &str,
) -> Result<Redacted, InputError> {
    let mut paths = BTreeSet::new();
    let value = Walk { ir }.decl(decl, value, root, &mut paths)?;
    Ok(Redacted {
        value,
        paths: paths.into_iter().collect(),
    })
}

/// Like [`redact_type`], with the paths starting at `root`.
///
/// # Errors
/// As [`redact_decl`].
pub fn redact_type_at(
    ir: &Ir,
    ty: &Type,
    value: &Json,
    root: &str,
) -> Result<Redacted, InputError> {
    let mut paths = BTreeSet::new();
    let value = Walk { ir }.ty(ty, value, root, &mut paths)?;
    Ok(Redacted {
        value,
        paths: paths.into_iter().collect(),
    })
}

struct Walk<'a> {
    ir: &'a Ir,
}

fn found_kind(v: &Json) -> &'static str {
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
        found: found_kind(found).to_owned(),
    }
}

impl Walk<'_> {
    fn ty(
        &self,
        ty: &Type,
        v: &Json,
        path: &str,
        out: &mut BTreeSet<String>,
    ) -> Result<Json, InputError> {
        if v.is_null() {
            return Ok(Json::Null);
        }
        match ty {
            Type::Scalar(_) => Ok(v.clone()),
            Type::List(t) => self.items(t, v, path, out),
            Type::Map(t) => {
                let entries = v.as_object().ok_or_else(|| mismatch("object", v, path))?;
                let mut kept = Map::new();
                for (k, e) in entries {
                    kept.insert(k.clone(), self.ty(t, e, &format!("{path}.{k}"), out)?);
                }
                Ok(Json::Object(kept))
            }
            Type::Page(t) => {
                let obj = v.as_object().ok_or_else(|| mismatch("object", v, path))?;
                let mut kept = obj.clone();
                if let Some(items) = obj.get("items") {
                    let here = format!("{path}.items");
                    kept.insert("items".to_owned(), self.items(t, items, &here, out)?);
                }
                Ok(Json::Object(kept))
            }
            Type::Decl(id) => {
                let decl = self
                    .ir
                    .decls
                    .get(id.index())
                    .ok_or_else(|| InputError::ir("a type refers to a missing declaration"))?;
                self.decl(decl, v, path, out)
            }
        }
    }

    fn items(
        &self,
        item: &Type,
        v: &Json,
        path: &str,
        out: &mut BTreeSet<String>,
    ) -> Result<Json, InputError> {
        let list = v.as_array().ok_or_else(|| mismatch("array", v, path))?;
        let kept: Result<Vec<Json>, InputError> = list
            .iter()
            .enumerate()
            .map(|(i, e)| self.ty(item, e, &format!("{path}[{i}]"), out))
            .collect();
        Ok(Json::Array(kept?))
    }

    fn decl(
        &self,
        decl: &Decl,
        v: &Json,
        path: &str,
        out: &mut BTreeSet<String>,
    ) -> Result<Json, InputError> {
        if v.is_null() {
            return Ok(Json::Null);
        }
        match &decl.kind {
            DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Event(_) => {
                self.record(decl, v, path, out)
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
                self.decl(arm, v, path, out)
            }
            DeclKind::Enum(_) | DeclKind::Const(_) => Ok(v.clone()),
        }
    }

    fn record(
        &self,
        decl: &Decl,
        v: &Json,
        path: &str,
        out: &mut BTreeSet<String>,
    ) -> Result<Json, InputError> {
        let obj = v.as_object().ok_or_else(|| mismatch("object", v, path))?;
        let mut kept = obj.clone();
        for f in decl.fields() {
            let Some(child) = obj.get(&f.wire) else {
                continue;
            };
            let here = format!("{path}.{}", f.wire);
            if f.pii || f.secret {
                kept.remove(&f.wire);
                out.insert(here);
            } else {
                kept.insert(f.wire.clone(), self.ty(&f.ty, child, &here, out)?);
            }
        }
        Ok(Json::Object(kept))
    }
}
