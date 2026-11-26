//! Filling in what an example leaves out (`spec/rules.md` section 12.2).
//!
//! Deterministic: the same example always gives the same values. Three counters run through the
//! whole example, in the order the values are filled (the givens as written, then the principal,
//! the arguments and `returns`; inside a model, the fields in declaration order):
//!
//! * the uuid counter: `00000000-0000-4000-8000-000000000001`, then `...0002`, and so on;
//! * the datetime counter: a left-out `datetime` is the clock plus one millisecond for each
//!   `datetime` already left out;
//! * the date counter: a left-out `date` is the date of the clock plus one day for each `date`
//!   already left out.
//!
//! A left-out `time` is the time of the clock. Values the example wrote do not advance a
//! counter, and neither do values that come from `@example` or a default.

use std::collections::BTreeSet;

use serde_json::{Map, Value as Json, json};
use veld_ir::{Constraints, Decl, DeclKind, Field, Ir, Scalar, Type};

use super::ExampleError;
use crate::check::default_json;
use crate::helpers::instant::MS_PER_DAY;
use crate::wire::{format_date, format_datetime, format_time};

/// How deep a filled-in value may nest. A union whose first arm contains the union again would
/// never end otherwise.
const MAX_DEPTH: usize = 24;
/// The most elements a list with `@length(min)` is filled with.
const MAX_ELEMENTS: u64 = 10_000;

pub(super) struct Filler<'a> {
    ir: &'a Ir,
    example: &'a str,
    clock_ms: i64,
    uuids: u64,
    datetimes: i64,
    dates: i64,
    depth: usize,
    /// The paths (rooted at `$`) of the values filled in since the last [`Filler::begin`].
    pub filled: BTreeSet<String>,
    /// The part of `filled` that is a required field with no `@example` and no default: a value
    /// the written one should have had (`Completer` reports it).
    pub missing: BTreeSet<String>,
}

impl<'a> Filler<'a> {
    pub fn new(ir: &'a Ir, example: &'a str, clock_ms: i64) -> Self {
        Filler {
            ir,
            example,
            clock_ms,
            uuids: 0,
            datetimes: 0,
            dates: 0,
            depth: 0,
            filled: BTreeSet::new(),
            missing: BTreeSet::new(),
        }
    }

    /// Starts a new root value: forgets which paths were filled in. The counters go on.
    pub fn begin(&mut self) {
        self.filled.clear();
        self.missing.clear();
    }

    fn cannot(&self, path: &str, reason: &str) -> ExampleError {
        ExampleError::CannotFill {
            example: self.example.to_owned(),
            path: path.to_owned(),
            reason: reason.to_owned(),
        }
    }

    /// Completes `given` (what the example wrote, in wire form) or, without one, makes the
    /// value of type `ty`.
    pub fn fill(
        &mut self,
        ty: &Type,
        c: &Constraints,
        given: Option<&Json>,
        path: &str,
    ) -> Result<Json, ExampleError> {
        if self.depth >= MAX_DEPTH {
            return Err(self.cannot(path, "the type nests too deeply to fill in"));
        }
        self.depth += 1;
        let out = match given {
            Some(v) => self.complete(ty, v, path),
            None => self.generate(ty, c, path),
        };
        self.depth -= 1;
        out
    }

    fn decl(&self, ty: &veld_ir::DeclId, path: &str) -> Result<&'a Decl, ExampleError> {
        self.ir
            .decls
            .get(ty.index())
            .ok_or_else(|| self.cannot(path, "the type refers to a missing declaration"))
    }

    // ---- values the example wrote: fill in what they leave out --------------------------

    fn complete(&mut self, ty: &Type, v: &Json, path: &str) -> Result<Json, ExampleError> {
        match (ty, v) {
            (Type::List(t), Json::Array(items)) => self.complete_items(t, items, path),
            (Type::Map(t), Json::Object(entries)) => {
                let mut out = Map::new();
                for (k, e) in entries {
                    let child = format!("{path}.{k}");
                    out.insert(
                        k.clone(),
                        self.fill(t, &Constraints::default(), Some(e), &child)?,
                    );
                }
                Ok(Json::Object(out))
            }
            (Type::Page(t), Json::Object(entries)) => {
                let mut out = entries.clone();
                if let Some(Json::Array(items)) = entries.get("items") {
                    let child = format!("{path}.items");
                    out.insert("items".to_owned(), self.complete_items(t, items, &child)?);
                }
                Ok(Json::Object(out))
            }
            (Type::Decl(id), Json::Object(obj)) => {
                let decl = self.decl(id, path)?;
                match &decl.kind {
                    DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Event(_) => {
                        self.model(decl, Some(obj), path)
                    }
                    DeclKind::Union(_) => self.complete_union(decl, obj, path),
                    DeclKind::Enum(_) | DeclKind::Const(_) => Ok(v.clone()),
                }
            }
            _ => Ok(v.clone()),
        }
    }

    fn complete_items(
        &mut self,
        t: &Type,
        items: &[Json],
        path: &str,
    ) -> Result<Json, ExampleError> {
        let mut out = Vec::new();
        for (i, e) in items.iter().enumerate() {
            let child = format!("{path}[{i}]");
            out.push(self.fill(t, &Constraints::default(), Some(e), &child)?);
        }
        Ok(Json::Array(out))
    }

    fn complete_union(
        &mut self,
        decl: &Decl,
        obj: &Map<String, Json>,
        path: &str,
    ) -> Result<Json, ExampleError> {
        let DeclKind::Union(u) = &decl.kind else {
            return Ok(Json::Object(obj.clone()));
        };
        let label = obj.get(&u.discriminator).and_then(Json::as_str);
        let Some(arm) = u.arms.iter().find(|a| Some(a.label.as_str()) == label) else {
            return Ok(Json::Object(obj.clone()));
        };
        let arm_decl = self.decl(&arm.decl, path)?;
        self.model(arm_decl, Some(obj), path)
    }

    // ---- a model, field by field --------------------------------------------------------

    fn model(
        &mut self,
        decl: &Decl,
        given: Option<&Map<String, Json>>,
        path: &str,
    ) -> Result<Json, ExampleError> {
        let mut out = given.cloned().unwrap_or_default();
        for f in decl.fields() {
            if f.derived.is_some() {
                // Computed, never filled in (`spec/rules.md` sections 12.2 and 17.2).
                out.remove(&f.wire);
                continue;
            }
            let child = format!("{path}.{}", f.wire);
            if let Some(v) = out.get(&f.wire).cloned() {
                let filled = self.fill(&f.ty, &f.constraints, Some(&v), &child)?;
                out.insert(f.wire.clone(), filled);
            } else if let Some(v) = preset(f) {
                self.filled.insert(child);
                out.insert(f.wire.clone(), v);
            } else if !f.optional {
                self.filled.insert(child.clone());
                self.missing.insert(child.clone());
                let v = self.fill(&f.ty, &f.constraints, None, &child)?;
                out.insert(f.wire.clone(), v);
            }
        }
        Ok(Json::Object(out))
    }

    // ---- values the example did not write -----------------------------------------------

    fn generate(&mut self, ty: &Type, c: &Constraints, path: &str) -> Result<Json, ExampleError> {
        match ty {
            Type::Scalar(s) => self.scalar(*s, c, path),
            Type::List(t) => {
                let n = c.min_len.unwrap_or(0);
                if n > MAX_ELEMENTS {
                    return Err(self.cannot(path, "@length(min) asks for too many elements"));
                }
                let mut out = Vec::new();
                for i in 0..n {
                    let child = format!("{path}[{i}]");
                    out.push(self.fill(t, &Constraints::default(), None, &child)?);
                }
                Ok(Json::Array(out))
            }
            Type::Map(_) => Ok(json!({})),
            Type::Page(_) => Ok(json!({ "items": [], "total": "0", "page": 1, "size": 0 })),
            Type::Decl(id) => self.generate_decl(self.decl(id, path)?, path),
        }
    }

    fn generate_decl(&mut self, decl: &Decl, path: &str) -> Result<Json, ExampleError> {
        match &decl.kind {
            DeclKind::Enum(e) => match e.variants.first() {
                Some(v) => Ok(Json::String(v.wire.clone())),
                None => Err(self.cannot(path, "the enum has no variant")),
            },
            DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Event(_) => {
                self.model(decl, None, path)
            }
            DeclKind::Union(u) => {
                let arm = u
                    .arms
                    .first()
                    .ok_or_else(|| self.cannot(path, "the union has no arm"))?;
                let arm_decl = self.decl(&arm.decl, path)?;
                let mut obj = self.model(arm_decl, None, path)?;
                if let Json::Object(map) = &mut obj {
                    map.insert(u.discriminator.clone(), Json::String(arm.label.clone()));
                }
                Ok(obj)
            }
            DeclKind::Const(_) => Err(self.cannot(path, "a constant is not a value of a field")),
        }
    }

    fn scalar(&mut self, s: Scalar, c: &Constraints, path: &str) -> Result<Json, ExampleError> {
        Ok(match s {
            Scalar::String => return self.string(c, path),
            Scalar::Int => {
                let n = bounded(c, true) as i64;
                Json::from(n.clamp(i64::from(i32::MIN), i64::from(i32::MAX)))
            }
            Scalar::Long => Json::String((bounded(c, true) as i64).to_string()),
            Scalar::Float => serde_json::Number::from_f64(bounded(c, false))
                .map(Json::Number)
                .ok_or_else(|| self.cannot(path, "the bounds are not finite"))?,
            Scalar::Decimal => Json::String(format!("{}", bounded(c, false))),
            Scalar::Bool => Json::Bool(false),
            Scalar::Uuid => {
                self.uuids += 1;
                Json::String(format!("00000000-0000-4000-8000-{:012}", self.uuids))
            }
            Scalar::Datetime => {
                let at = self.clock_ms.saturating_add(self.datetimes);
                self.datetimes += 1;
                Json::String(format_datetime(at))
            }
            Scalar::Date => {
                let day = self
                    .clock_ms
                    .div_euclid(MS_PER_DAY)
                    .saturating_add(self.dates);
                self.dates += 1;
                Json::String(format_date(day))
            }
            Scalar::Time => Json::String(format_time(self.clock_ms)),
            Scalar::Bytes => Json::String(String::new()),
            Scalar::Json => json!({}),
        })
    }

    fn string(&self, c: &Constraints, path: &str) -> Result<Json, ExampleError> {
        if c.pattern.is_some() || c.format.is_some() {
            return Err(self.cannot(path, "it has @pattern or @format and no @example"));
        }
        let n = c.min_len.unwrap_or(0).max(1);
        if n > 1_000_000 {
            return Err(self.cannot(path, "@length(min) is too long to fill in"));
        }
        let n = usize::try_from(n).map_err(|_| self.cannot(path, "@length(min) is too long"))?;
        Ok(Json::String("a".repeat(n)))
    }
}

/// `@example`, then the default.
fn preset(f: &Field) -> Option<Json> {
    f.example
        .as_ref()
        .or(f.default.as_ref())
        .and_then(|lit| default_json(lit, &f.ty))
}

/// Zero, moved inside `@min` and `@max`.
fn bounded(c: &Constraints, integer: bool) -> f64 {
    let mut v = 0.0_f64;
    if let Some(min) = c.min.filter(|m| v < *m) {
        v = if integer { min.ceil() } else { min };
    }
    if let Some(max) = c.max.filter(|m| v > *m) {
        v = if integer { max.floor() } else { max };
    }
    v
}
