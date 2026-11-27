//! Values of an example, checked against the type they are used at (`spec/rules.md` 12.1) and
//! lowered to `ExampleValue`: a literal in wire form (`json`), a read from a given (`ref`), an
//! `object` or a `list`. Every problem is `E0952`, except `E0951` for a `given`.

use serde_json::{Number, Value as Json};

use super::{ROutcome, is_implicit_error, is_record};
use crate::diagnostics::{Diagnostic, Span};
use crate::semantic::{DKind, Program, REndpoint};
use crate::syntax::ast::{self, ValueKind};
use veld_ir::endpoint_rules::{ExampleField, ExampleRef, ExampleValue};
use veld_ir::{Scalar, Type};

/// Largest integer a `float` holds exactly.
const MAX_EXACT_FLOAT: i64 = 1 << 53;

pub(super) struct Values<'p, 'a, 'd> {
    pub prog: &'p Program<'a>,
    pub module: usize,
    pub diags: &'d mut Vec<Diagnostic>,
    /// The givens seen so far: the name and the type, or `None` for `absent`.
    pub givens: Vec<(String, Option<Type>)>,
    /// The value being checked is a pattern the outcome is compared with (after `ok` or an
    /// error): a derived field may be named there, to say what it is computed to be.
    pub pattern: bool,
}

impl Values<'_, '_, '_> {
    fn err(&mut self, msg: impl Into<String>, span: Span) {
        self.diags.push(Diagnostic::error("E0952", msg, span));
    }

    fn mismatch(&mut self, found: &str, ty: &Type, span: Span) {
        let shown = self.prog.type_name(ty);
        self.err(format!("{found} does not fit the type `{shown}`"), span);
    }

    // ---- the parts of an example ----------------------------------------------------

    /// `given name = value`.
    pub fn given(
        &mut self,
        g: &ast::Given,
        ep: &REndpoint,
    ) -> Option<(String, Option<ExampleValue>)> {
        let name = &g.name.name;
        let Some(fact) = ep.rules.facts.iter().find(|f| &f.name == name) else {
            let facts: Vec<&str> = ep.rules.facts.iter().map(|f| f.name.as_str()).collect();
            let help = if facts.is_empty() {
                format!("endpoint `{}` has no facts", ep.name)
            } else {
                format!("facts of `{}`: {}", ep.name, facts.join(", "))
            };
            let diag = Diagnostic::error(
                "E0951",
                format!("`{name}` is not a fact of endpoint `{}`", ep.name),
                g.name.span,
            )
            .did_you_mean(name, facts.iter().copied(), Some(help));
            self.diags.push(diag);
            return None;
        };
        if self.givens.iter().any(|(n, _)| n == name) {
            self.diags.push(Diagnostic::error(
                "E0951",
                format!("`{name}` is given twice"),
                g.name.span,
            ));
            return None;
        }
        let value = g.value.as_ref().map(|v| self.value(v, &fact.ty));
        let known = value.is_some().then(|| fact.ty.clone());
        self.givens.push((name.clone(), known));
        match value {
            None => Some((name.clone(), None)),
            Some(v) => v.map(|v| (name.clone(), Some(v))),
        }
    }

    /// `as value`, typed by the model of the principal of the package (`spec/rules.md` 11.1).
    /// The outer `None` is an error; `Some(None)` is no `as`.
    pub fn principal(&mut self, p: Option<&ast::ExampleAs>) -> Option<Option<ExampleValue>> {
        let Some(p) = p else { return Some(None) };
        let pkg = self.prog.modules[self.module].pkg;
        let Some(principal) = self.prog.principals.get(&pkg) else {
            self.diags.push(
                Diagnostic::error(
                    "E0952",
                    "this package declares no principal, so an example cannot say who calls",
                    p.span,
                )
                .with_help("principals arrive with policies (spec/rules.md section 11)"),
            );
            return None;
        };
        let ty = Type::Decl(veld_ir::DeclId(
            u32::try_from(principal.decl).unwrap_or(u32::MAX),
        ));
        self.value(&p.value, &ty).map(Some)
    }

    /// `at "time"`. The outer `None` is an error; `Some(None)` is no clock.
    pub fn clock(&mut self, at: Option<&ast::ExampleAt>) -> Option<Option<String>> {
        let Some(at) = at else { return Some(None) };
        if is_datetime(&at.time.value) {
            return Some(Some(at.time.value.clone()));
        }
        self.err(
            "the clock of an example is a date and time such as \"2026-03-01T09:00:00Z\"",
            at.time.span,
        );
        None
    }

    /// The arguments of the call, by parameter name.
    pub fn args(
        &mut self,
        call: &ast::ExampleCall,
        ep: &REndpoint,
    ) -> Option<Vec<(String, ExampleValue)>> {
        let mut out: Vec<(String, ExampleValue)> = Vec::new();
        let mut ok = true;
        for a in &call.args {
            let name = &a.name.name;
            let Some(p) = ep.params.iter().find(|p| &p.name == name && !p.stream) else {
                let names: Vec<&str> = ep
                    .params
                    .iter()
                    .filter(|p| !p.stream)
                    .map(|p| p.name.as_str())
                    .collect();
                let diag = Diagnostic::error(
                    "E0952",
                    format!("endpoint `{}` has no parameter `{name}`", ep.name),
                    a.name.span,
                )
                .did_you_mean(
                    name,
                    names.iter().copied(),
                    Some(format!("parameters: {}", names.join(", "))),
                );
                self.diags.push(diag);
                ok = false;
                continue;
            };
            if out.iter().any(|(n, _)| n == name) {
                self.err(format!("parameter `{name}` is given twice"), a.name.span);
                ok = false;
                continue;
            }
            match self.value(&a.value, &p.ty) {
                Some(v) => out.push((name.clone(), v)),
                None => ok = false,
            }
        }
        ok.then_some(out)
    }

    /// `returns value`: what the handler returns, a value of the success type.
    pub fn returns(
        &mut self,
        call: &ast::ExampleCall,
        ep: &REndpoint,
    ) -> Option<Option<ExampleValue>> {
        let Some(v) = &call.returns else {
            return Some(None);
        };
        let ty = self.success(ep, v.span)?;
        self.value(v, &ty).map(Some)
    }

    fn success(&mut self, ep: &REndpoint, span: Span) -> Option<Type> {
        match (&ep.success_ty, ep.success_stream) {
            (Some(t), false) => Some(t.clone()),
            (Some(_) | None, _) => {
                self.err(
                    format!(
                        "endpoint `{}` has no success body to give a value for",
                        ep.name
                    ),
                    span,
                );
                None
            }
        }
    }

    pub fn outcome(&mut self, call: &ast::ExampleCall, ep: &REndpoint) -> Option<ROutcome> {
        self.outcome_of(&call.outcome, ep)
    }

    /// An outcome as written: after `->`, or after `stored` on a `key` line.
    pub fn outcome_of(&mut self, o: &ast::Outcome, ep: &REndpoint) -> Option<ROutcome> {
        let Some(te) = &o.error else {
            let value = match &o.value {
                None => None,
                Some(v) => {
                    let ty = self.success(ep, v.span)?;
                    Some(self.pattern_value(v, &ty)?)
                }
            };
            return Some(ROutcome::Ok(value));
        };
        let ty = self.prog.resolve_type(self.module, te, true, self.diags)?;
        let decl = self.answerable(&ty, te, ep)?;
        let pattern = match &o.value {
            None => None,
            Some(v) => Some(self.pattern_value(v, &ty)?),
        };
        Some(ROutcome::Error(decl, pattern))
    }

    /// The error declaration named by an outcome, if the endpoint can answer with it.
    pub(super) fn answerable(
        &mut self,
        ty: &Type,
        te: &ast::TypeExpr,
        ep: &REndpoint,
    ) -> Option<usize> {
        let decl = match ty {
            Type::Decl(d) if self.prog.decls[d.index()].kind == DKind::Error => d.index(),
            Type::Decl(_) | Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) => {
                let shown = self.prog.type_name(ty);
                self.err(format!("`{shown}` is not an error"), te.span);
                return None;
            }
        };
        if ep.errors.iter().any(|(_, d)| *d == decl) || is_implicit_error(self.prog, decl) {
            return Some(decl);
        }
        let name = self.prog.decls[decl].name.clone();
        self.diags.push(
            Diagnostic::error(
                "E0952",
                format!("endpoint `{}` cannot answer with `{name}`", ep.name),
                te.span,
            )
            .with_help("an outcome is `ok` or an error the endpoint declares or has implicitly"),
        );
        None
    }

    // ---- values -----------------------------------------------------------------------

    /// A value that an outcome is compared with: derived fields may be named in it.
    fn pattern_value(&mut self, v: &ast::Value, ty: &Type) -> Option<ExampleValue> {
        let before = std::mem::replace(&mut self.pattern, true);
        let out = self.value(v, ty);
        self.pattern = before;
        out
    }

    pub fn value(&mut self, v: &ast::Value, ty: &Type) -> Option<ExampleValue> {
        match &v.kind {
            ValueKind::Path(p) => self.reference(p, ty, v.span),
            ValueKind::EnumLit(id) => self.enum_value(id, ty, v.span),
            ValueKind::Object(fields) => self.object(fields, ty, v.span),
            ValueKind::List(items) => self.list(items, ty, v.span),
            ValueKind::Bool(_)
            | ValueKind::Int(_)
            | ValueKind::Float { .. }
            | ValueKind::Str(_) => self.literal(&v.kind, ty, v.span).map(ExampleValue::Json),
        }
    }

    fn literal(&mut self, kind: &ValueKind, ty: &Type, span: Span) -> Option<Json> {
        let Type::Scalar(s) = ty else {
            self.mismatch(describe(kind), ty, span);
            return None;
        };
        let out = scalar_json(*s, kind);
        match out {
            Ok(j) => Some(j),
            Err(None) => {
                self.mismatch(describe(kind), ty, span);
                None
            }
            Err(Some(why)) => {
                self.err(why, span);
                None
            }
        }
    }

    fn enum_value(&mut self, id: &ast::Ident, ty: &Type, span: Span) -> Option<ExampleValue> {
        let Type::Decl(d) = ty else {
            self.mismatch("an enum value", ty, span);
            return None;
        };
        let info = &self.prog.decls[d.index()];
        if info.kind != DKind::Enum {
            self.mismatch("an enum value", ty, span);
            return None;
        }
        if let Some(v) = info.res.variants.iter().find(|v| v.name == id.name) {
            return Some(ExampleValue::Json(Json::String(v.wire.clone())));
        }
        let names: Vec<&str> = info.res.variants.iter().map(|v| v.name.as_str()).collect();
        let diag = Diagnostic::error(
            "E0952",
            format!("enum `{}` has no variant `{}`", info.name, id.name),
            id.span,
        )
        .did_you_mean(
            &id.name,
            names.iter().copied(),
            Some(format!("variants: {}", names.join(", "))),
        );
        self.diags.push(diag);
        None
    }

    fn list(&mut self, items: &[ast::Value], ty: &Type, span: Span) -> Option<ExampleValue> {
        match ty {
            Type::List(elem) => {
                let mut out = Vec::new();
                let mut ok = true;
                for i in items {
                    match self.value(i, elem) {
                        Some(v) => out.push(v),
                        None => ok = false,
                    }
                }
                ok.then_some(ExampleValue::List(out))
            }
            Type::Scalar(Scalar::Json) => self
                .json_of(&ast::Value {
                    kind: ValueKind::List(items.to_vec()),
                    span,
                })
                .map(ExampleValue::Json),
            Type::Scalar(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => {
                self.mismatch("a list", ty, span);
                None
            }
        }
    }

    fn object(
        &mut self,
        fields: &[ast::ValueField],
        ty: &Type,
        span: Span,
    ) -> Option<ExampleValue> {
        match ty {
            Type::Decl(d) => match self.prog.decls[d.index()].kind {
                DKind::Model | DKind::Error | DKind::Event => self.record(d.index(), fields),
                DKind::Union => self.union(d.index(), fields, span),
                DKind::Enum | DKind::Const => {
                    self.mismatch("an object", ty, span);
                    None
                }
            },
            Type::Map(inner) => self.map(inner, fields),
            Type::Page(item) => self.page(item, fields),
            Type::Scalar(Scalar::Json) => self
                .json_of(&ast::Value {
                    kind: ValueKind::Object(fields.to_vec()),
                    span,
                })
                .map(ExampleValue::Json),
            Type::Scalar(_) | Type::List(_) => {
                self.mismatch("an object", ty, span);
                None
            }
        }
    }

    /// The fields of a model or error: each must exist and fit; none is required.
    fn record(&mut self, decl: usize, fields: &[ast::ValueField]) -> Option<ExampleValue> {
        let info = &self.prog.decls[decl];
        let mut out: Vec<ExampleField> = Vec::new();
        let mut ok = true;
        for f in fields {
            let Some(field) = info.res.fields.iter().find(|x| x.name == f.name.name) else {
                let names: Vec<&str> = info.res.fields.iter().map(|x| x.name.as_str()).collect();
                let diag = Diagnostic::error(
                    "E0952",
                    format!("`{}` has no field `{}`", info.name, f.name.name),
                    f.name.span,
                )
                .did_you_mean(
                    &f.name.name,
                    names.iter().copied(),
                    Some(format!("fields: {}", names.join(", "))),
                );
                self.diags.push(diag);
                ok = false;
                continue;
            };
            if field.derived && !self.pattern {
                let diag = Diagnostic::error(
                    "E0952",
                    format!(
                        "`{}` is a derived field: it is computed, an example cannot write it",
                        f.name.name
                    ),
                    f.name.span,
                )
                .with_help("leave it out; after `ok` or an error you may name it to say what it computes to");
                self.diags.push(diag);
                ok = false;
                continue;
            }
            if out.iter().any(|o| o.name == field.name) {
                self.err(
                    format!("field `{}` is given twice", f.name.name),
                    f.name.span,
                );
                ok = false;
                continue;
            }
            let value = if self.says_absent(&f.value) {
                self.absent_member(field, f)
            } else {
                self.value(&f.value, &field.ty)
            };
            match value {
                Some(value) => out.push(ExampleField {
                    name: field.name.clone(),
                    wire: field.wire.clone(),
                    value,
                }),
                None => ok = false,
            }
        }
        ok.then_some(ExampleValue::Object(out))
    }

    /// `field: absent` in a pattern (`spec/platform.md` section 2): the word `absent` alone,
    /// unless the example has a given of that name.
    fn says_absent(&self, v: &ast::Value) -> bool {
        matches!(&v.kind, ValueKind::Path(p) if p.len() == 1 && p[0].name == "absent")
            && self.pattern
            && !self.givens.iter().any(|(n, _)| n == "absent")
    }

    /// The value `absent` of an optional member; a required one is always there.
    fn absent_member(
        &mut self,
        field: &crate::semantic::RField,
        f: &ast::ValueField,
    ) -> Option<ExampleValue> {
        if !field.optional {
            self.err(
                format!(
                    "`{}` is not optional, so it is always there and cannot be `absent`",
                    f.name.name
                ),
                f.value.span,
            );
            return None;
        }
        Some(ExampleValue::Absent(veld_ir::rules::Always))
    }

    /// A union value names its arm with the discriminator, as it does on the wire.
    fn union(
        &mut self,
        decl: usize,
        fields: &[ast::ValueField],
        span: Span,
    ) -> Option<ExampleValue> {
        let info = &self.prog.decls[decl];
        let disc = info.res.discriminator.clone();
        let Some(tag) = fields.iter().find(|f| f.name.name == disc) else {
            self.diags.push(
                Diagnostic::error(
                    "E0952",
                    format!(
                        "a value of union `{}` needs its discriminator `{disc}`",
                        info.name
                    ),
                    span,
                )
                .with_help(format!("write `{disc}: \"<arm>\"`")),
            );
            return None;
        };
        let ValueKind::Str(label) = &tag.value.kind else {
            self.err(
                format!("`{disc}` must be the label of an arm, as a string"),
                tag.value.span,
            );
            return None;
        };
        let Some(arm) = info.res.arms.iter().find(|a| &a.label == label) else {
            let labels: Vec<&str> = info.res.arms.iter().map(|a| a.label.as_str()).collect();
            self.err(
                format!(
                    "union `{}` has no arm `{label}` (arms: {})",
                    info.name,
                    labels.join(", ")
                ),
                tag.value.span,
            );
            return None;
        };
        let rest: Vec<ast::ValueField> = fields
            .iter()
            .filter(|f| f.name.name != disc)
            .cloned()
            .collect();
        let ExampleValue::Object(mut out) = self.record(arm.decl, &rest)? else {
            return None;
        };
        out.insert(
            0,
            ExampleField {
                name: disc.clone(),
                wire: disc,
                value: ExampleValue::Json(Json::String(label.clone())),
            },
        );
        Some(ExampleValue::Object(out))
    }

    fn map(&mut self, inner: &Type, fields: &[ast::ValueField]) -> Option<ExampleValue> {
        let mut out: Vec<ExampleField> = Vec::new();
        let mut ok = true;
        for f in fields {
            if out.iter().any(|o| o.name == f.name.name) {
                self.err(format!("key `{}` is given twice", f.name.name), f.name.span);
                ok = false;
                continue;
            }
            match self.value(&f.value, inner) {
                Some(value) => out.push(ExampleField {
                    name: f.name.name.clone(),
                    wire: f.name.name.clone(),
                    value,
                }),
                None => ok = false,
            }
        }
        ok.then_some(ExampleValue::Object(out))
    }

    /// `Page<T>`: `items`, `total`, `page` and `size`.
    fn page(&mut self, item: &Type, fields: &[ast::ValueField]) -> Option<ExampleValue> {
        let mut out: Vec<ExampleField> = Vec::new();
        let mut ok = true;
        for f in fields {
            let ty = match f.name.name.as_str() {
                "items" => Type::List(Box::new(item.clone())),
                "total" => Type::Scalar(Scalar::Long),
                "page" | "size" => Type::Scalar(Scalar::Int),
                other => {
                    self.err(
                        format!(
                            "a `Page` has no field `{other}` (it has items, total, page, size)"
                        ),
                        f.name.span,
                    );
                    ok = false;
                    continue;
                }
            };
            match self.value(&f.value, &ty) {
                Some(value) => out.push(ExampleField {
                    name: f.name.name.clone(),
                    wire: f.name.name.clone(),
                    value,
                }),
                None => ok = false,
            }
        }
        ok.then_some(ExampleValue::Object(out))
    }

    /// `given.field.field`: a read from a given, by wire names.
    fn reference(&mut self, path: &[ast::Ident], ty: &Type, span: Span) -> Option<ExampleValue> {
        let root = path.first()?;
        let Some((_, given)) = self.givens.iter().find(|(n, _)| n == &root.name) else {
            let names: Vec<&str> = self.givens.iter().map(|(n, _)| n.as_str()).collect();
            let diag = Diagnostic::error(
                "E0952",
                format!("`{}` is not a given of this example", root.name),
                root.span,
            )
            .did_you_mean(
                &root.name,
                names.iter().copied(),
                Some("a value can read only the facts the example gives, above it".to_string()),
            );
            self.diags.push(diag);
            return None;
        };
        let Some(mut cur) = given.clone() else {
            self.err(
                format!("`{}` is absent, so it has no fields to read", root.name),
                root.span,
            );
            return None;
        };
        let mut wires = Vec::new();
        for id in &path[1..] {
            let (wire, next) = self.step(&cur, id)?;
            wires.push(wire);
            cur = next;
        }
        if &cur != ty && !matches!(ty, Type::Scalar(Scalar::Json)) {
            let found = format!("`{}` (a `{}`)", join(path), self.prog.type_name(&cur));
            self.mismatch(&found, ty, span);
            return None;
        }
        Some(ExampleValue::Ref(ExampleRef {
            given: root.name.clone(),
            path: wires,
        }))
    }

    fn step(&mut self, ty: &Type, id: &ast::Ident) -> Option<(String, Type)> {
        match ty {
            Type::Page(item) => {
                let next = match id.name.as_str() {
                    "items" => Type::List(item.clone()),
                    "total" => Type::Scalar(Scalar::Long),
                    "page" | "size" => Type::Scalar(Scalar::Int),
                    _ => {
                        self.err(format!("a `Page` has no field `{}`", id.name), id.span);
                        return None;
                    }
                };
                Some((id.name.clone(), next))
            }
            Type::Decl(_) if is_record(self.prog, ty).is_some() => {
                let decl = is_record(self.prog, ty)?;
                let info = &self.prog.decls[decl];
                let Some(f) = info.res.fields.iter().find(|f| f.name == id.name) else {
                    let names: Vec<&str> =
                        info.res.fields.iter().map(|f| f.name.as_str()).collect();
                    let diag = Diagnostic::error(
                        "E0952",
                        format!("`{}` has no field `{}`", info.name, id.name),
                        id.span,
                    )
                    .did_you_mean(
                        &id.name,
                        names.iter().copied(),
                        Some(format!("fields: {}", names.join(", "))),
                    );
                    self.diags.push(diag);
                    return None;
                };
                Some((f.wire.clone(), f.ty.clone()))
            }
            Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Decl(_) => {
                let shown = self.prog.type_name(ty);
                self.err(
                    format!("cannot read `.{}` of a value of type `{shown}`", id.name),
                    id.span,
                );
                None
            }
        }
    }

    /// A value written inside a `json` type: literals, objects and lists only.
    fn json_of(&mut self, v: &ast::Value) -> Option<Json> {
        match &v.kind {
            ValueKind::Bool(_)
            | ValueKind::Int(_)
            | ValueKind::Float { .. }
            | ValueKind::Str(_) => match scalar_json(Scalar::Json, &v.kind) {
                Ok(j) => Some(j),
                Err(_) => {
                    self.err("this number cannot be written as JSON", v.span);
                    None
                }
            },
            ValueKind::List(items) => items
                .iter()
                .map(|i| self.json_of(i))
                .collect::<Option<Vec<_>>>()
                .map(Json::Array),
            ValueKind::Object(fields) => {
                let mut map = serde_json::Map::new();
                for f in fields {
                    map.insert(f.name.name.clone(), self.json_of(&f.value)?);
                }
                Some(Json::Object(map))
            }
            ValueKind::Path(_) | ValueKind::EnumLit(_) => {
                self.err(
                    "a `json` value holds literals only: no reads from a given and no enum values",
                    v.span,
                );
                None
            }
        }
    }
}

fn join(path: &[ast::Ident]) -> String {
    path.iter()
        .map(|i| i.name.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn describe(k: &ValueKind) -> &'static str {
    match k {
        ValueKind::Bool(_) => "a bool",
        ValueKind::Int(_) => "an integer",
        ValueKind::Float { .. } => "a number with a fraction",
        ValueKind::Str(_) => "a string",
        ValueKind::EnumLit(_) => "an enum value",
        ValueKind::Path(_) => "a read from a given",
        ValueKind::Object(_) => "an object",
        ValueKind::List(_) => "a list",
    }
}

fn number(f: f64) -> Option<Json> {
    Number::from_f64(f).map(Json::Number)
}

/// The wire form of a literal at a scalar type. `Err(None)` is a plain mismatch,
/// `Err(Some(why))` a more specific problem.
fn scalar_json(s: Scalar, k: &ValueKind) -> Result<Json, Option<String>> {
    let range = |what: &str| Err(Some(format!("{what} is out of range")));
    match (s, k) {
        (Scalar::Bool | Scalar::Json, ValueKind::Bool(b)) => Ok(Json::Bool(*b)),
        (Scalar::Int, ValueKind::Int(n)) => i32::try_from(*n)
            .map(|n| Json::Number(n.into()))
            .or_else(|_| range(&format!("`{n}` for an `int`"))),
        (Scalar::Json, ValueKind::Int(n)) => Ok(Json::Number((*n).into())),
        (Scalar::Long, ValueKind::Int(n)) => Ok(Json::String(n.to_string())),
        (Scalar::Long, ValueKind::Str(t)) if is_integer(t) => Ok(Json::String(t.clone())),
        (Scalar::Float, ValueKind::Int(n)) if n.abs() <= MAX_EXACT_FLOAT =>
        {
            #[allow(clippy::cast_precision_loss)]
            number(*n as f64).ok_or(None)
        }
        (Scalar::Float, ValueKind::Int(n)) => Err(Some(format!(
            "`{n}` cannot be represented exactly as a `float`"
        ))),
        (Scalar::Float | Scalar::Json, ValueKind::Float { value, .. }) => {
            number(*value).ok_or(Some("a `float` must be finite".to_string()))
        }
        (Scalar::Decimal, ValueKind::Int(n)) => Ok(Json::String(n.to_string())),
        (Scalar::Decimal, ValueKind::Float { text, .. }) if !text.contains(['e', 'E']) => {
            Ok(Json::String(text.clone()))
        }
        (Scalar::Decimal, ValueKind::Float { .. }) => Err(Some(
            "a `decimal` has no exponent: write the digits out in full".to_string(),
        )),
        (Scalar::Decimal, ValueKind::Str(t)) if is_decimal(t) => Ok(Json::String(t.clone())),
        (Scalar::String | Scalar::Json, ValueKind::Str(t)) => Ok(Json::String(t.clone())),
        (Scalar::Uuid, ValueKind::Str(t)) if is_uuid(t) => Ok(Json::String(t.to_ascii_lowercase())),
        (Scalar::Date, ValueKind::Str(t)) if is_date(t) => Ok(Json::String(t.clone())),
        (Scalar::Datetime, ValueKind::Str(t)) if is_datetime(t) => Ok(Json::String(t.clone())),
        (Scalar::Time, ValueKind::Str(t)) if is_time(t) => Ok(Json::String(t.clone())),
        (Scalar::Bytes, ValueKind::Str(t)) if is_base64(t) => Ok(Json::String(t.clone())),
        (Scalar::Uuid, ValueKind::Str(_)) => Err(Some(
            "not a UUID such as \"00000000-0000-4000-8000-000000000001\"".to_string(),
        )),
        (Scalar::Date, ValueKind::Str(_)) => {
            Err(Some("not a date such as \"2026-03-01\"".to_string()))
        }
        (Scalar::Datetime, ValueKind::Str(_)) => Err(Some(
            "not a date and time such as \"2026-03-01T09:00:00Z\"".to_string(),
        )),
        (Scalar::Time, ValueKind::Str(_)) => {
            Err(Some("not a time such as \"09:00:00\"".to_string()))
        }
        (Scalar::Bytes, ValueKind::Str(_)) => Err(Some("not padded base64".to_string())),
        (Scalar::Decimal | Scalar::Long, ValueKind::Str(_)) => {
            Err(Some("not a number written in digits".to_string()))
        }
        _ => Err(None),
    }
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn is_integer(s: &str) -> bool {
    all_digits(s.strip_prefix('-').unwrap_or(s)) && s.parse::<i64>().is_ok()
}

fn is_decimal(s: &str) -> bool {
    let d = s.strip_prefix('-').unwrap_or(s);
    match d.split_once('.') {
        Some((w, f)) => all_digits(w) && all_digits(f),
        None => all_digits(d),
    }
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

fn is_date(s: &str) -> bool {
    let p: Vec<&str> = s.split('-').collect();
    let [y, m, d] = p.as_slice() else {
        return false;
    };
    let num = |t: &str, len: usize| {
        (t.len() == len && all_digits(t))
            .then(|| t.parse::<u32>().ok())
            .flatten()
    };
    matches!(
        (num(y, 4), num(m, 2), num(d, 2)),
        (Some(_), Some(1..=12), Some(1..=31))
    )
}

/// `HH:MM:SS` with an optional fraction, and nothing after it.
fn is_time(s: &str) -> bool {
    let (clock, frac) = s.split_once('.').map_or((s, None), |(c, f)| (c, Some(f)));
    let p: Vec<&str> = clock.split(':').collect();
    let [h, m, sec] = p.as_slice() else {
        return false;
    };
    let num = |t: &str| {
        (t.len() == 2 && all_digits(t))
            .then(|| t.parse::<u32>().ok())
            .flatten()
    };
    matches!(
        (num(h), num(m), num(sec)),
        (Some(0..=23), Some(0..=59), Some(0..=60))
    ) && frac.is_none_or(all_digits)
}

/// RFC 3339: a date, `T`, a time and `Z` or an offset.
fn is_datetime(s: &str) -> bool {
    let Some((date, rest)) = s.split_once(['T', 't']) else {
        return false;
    };
    let (time, zone_ok) = if let Some(t) = rest.strip_suffix(['Z', 'z']) {
        (t, true)
    } else if let Some(i) = rest.rfind(['+', '-']) {
        let off = &rest[i + 1..];
        let ok = off.len() == 5
            && off.as_bytes()[2] == b':'
            && all_digits(&off[..2])
            && all_digits(&off[3..]);
        (&rest[..i], ok)
    } else {
        (rest, false)
    };
    zone_ok && is_date(date) && is_time(time)
}

fn is_base64(s: &str) -> bool {
    let body = s.trim_end_matches('=');
    s.len().is_multiple_of(4)
        && s.len() - body.len() <= 2
        && body
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
}
