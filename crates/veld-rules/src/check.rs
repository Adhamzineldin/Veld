//! Checks a whole value against the invariants of its declaration, recursively
//! (`spec/rules.md` sections 5.2, 5.4 and 7).

use serde::{Serialize, Serializer};
use serde_json::{Map, Value as Json};
use veld_ir::{Decl, DeclKind, Field, Ir, Literal, Rule, Scalar, Type, Union};

use crate::error::{Failure, InputError};
use crate::eval::{Env, eval};
use crate::helpers::num::EvalError;
use crate::value::Value;

/// How a value is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Options {
    /// The value is a request body: rules that read a `@readonly` field are skipped and
    /// `@readonly` fields may be absent (section 5.2).
    pub request: bool,
    /// The partial check of section 5.4: a rule that would read an absent required field is
    /// skipped, and required fields may be absent.
    pub partial: bool,
}

/// One violated rule on one value (section 7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Violation {
    /// The rule's name; not part of the wire object.
    #[serde(skip)]
    pub rule_name: String,
    /// The rule id; it is the `rule` member on the wire.
    #[serde(rename = "rule")]
    pub rule_id: String,
    /// The path of the value that carries the rule (`$`, `$.rows[1]`, `$.named.k`).
    pub path: String,
    /// The wire names of the fields the rule reads.
    pub fields: Vec<String>,
    pub message: String,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_error"
    )]
    pub error: Option<EvalError>,
}

fn serialize_error<S: Serializer>(e: &Option<EvalError>, s: S) -> Result<S::Ok, S::Error> {
    match e {
        Some(e) => s.serialize_str(e.as_str()),
        None => s.serialize_none(),
    }
}

/// What checking one value found: the violations of the rules that reject, and those of the audited
/// rules (`@rollout(audit)`, section 17.1), which never reject and are only reported.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Checked {
    /// Violations of enforced rules.
    pub violations: Vec<Violation>,
    /// Violations (and evaluation errors) of audited rules, in the order of evaluation.
    pub audited: Vec<Violation>,
}

/// Checks `value` (wire form) against `decl` and everything it contains. Defaults are applied
/// first; every rule of every nested value is evaluated; the result lists every violation.
///
/// # Errors
/// [`InputError`] when the value does not have the shape of its declaration.
pub fn check_value(
    ir: &Ir,
    decl: &Decl,
    value: &Json,
    options: Options,
) -> Result<Vec<Violation>, InputError> {
    check_decl_full(ir, decl, value, options).map(|(_, violations)| violations)
}

/// Like [`check_value`], and also returns the violations of the audited rules, which are not
/// among the violations that reject.
///
/// # Errors
/// [`InputError`] when the value does not have the shape of its declaration.
pub fn check_value_audited(
    ir: &Ir,
    decl: &Decl,
    value: &Json,
    options: Options,
) -> Result<Checked, InputError> {
    check_decl_audited(ir, decl, value, options).map(|(_, found)| found)
}

/// The value with the defaults of its declaration applied, as the rules see it. The value must
/// have the shape of its declaration.
///
/// # Errors
/// [`InputError`] when the value does not have the shape of its declaration.
pub fn with_defaults(
    ir: &Ir,
    decl: &Decl,
    value: &Json,
    options: Options,
) -> Result<Json, InputError> {
    Cx { ir, opts: options }.normalize_decl(decl, value, "$")
}

/// Like [`check_value`], and also returns the value with its defaults applied.
pub(crate) fn check_decl_full(
    ir: &Ir,
    decl: &Decl,
    value: &Json,
    options: Options,
) -> Result<(Json, Vec<Violation>), InputError> {
    check_decl_audited(ir, decl, value, options).map(|(full, found)| (full, found.violations))
}

/// Like [`check_decl_full`], with the audited violations as well.
pub(crate) fn check_decl_audited(
    ir: &Ir,
    decl: &Decl,
    value: &Json,
    options: Options,
) -> Result<(Json, Checked), InputError> {
    let cx = Cx { ir, opts: options };
    let full = cx.normalize_decl(decl, value, "$")?;
    let mut out = Checked::default();
    cx.walk_decl(decl, &full, "$", &mut out)?;
    Ok((full, out))
}

/// Checks a value of any type (a parameter or a result: a model, a list of models, a page, a
/// scalar). Returns the value with its defaults applied and every violation, with paths
/// rooted at `$`.
pub(crate) fn check_type(
    ir: &Ir,
    ty: &Type,
    value: &Json,
    options: Options,
) -> Result<(Json, Vec<Violation>), InputError> {
    check_type_audited(ir, ty, value, options).map(|(full, found)| (full, found.violations))
}

/// Like [`check_type`], with the audited violations as well.
pub(crate) fn check_type_audited(
    ir: &Ir,
    ty: &Type,
    value: &Json,
    options: Options,
) -> Result<(Json, Checked), InputError> {
    let cx = Cx { ir, opts: options };
    let full = cx.normalize_type(ty, value, "$")?;
    let mut out = Checked::default();
    cx.walk_type(ty, &full, "$", &mut out)?;
    Ok((full, out))
}

struct Cx<'a> {
    ir: &'a Ir,
    opts: Options,
}

fn shape(expected: &str, found: &Json, path: &str) -> InputError {
    let found = match found {
        Json::Null => "null",
        Json::Bool(_) => "boolean",
        Json::Number(_) => "number",
        Json::String(_) => "string",
        Json::Array(_) => "array",
        Json::Object(_) => "object",
    };
    InputError::mismatch(expected, found).at_path(path)
}

/// A field default as wire JSON, typed by the field. `null` means no value.
pub(crate) fn default_json(lit: &Literal, ty: &Type) -> Option<Json> {
    let scalar = match ty {
        Type::Scalar(s) => Some(*s),
        Type::List(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => None,
    };
    let as_text = matches!(scalar, Some(Scalar::Long | Scalar::Decimal));
    match lit {
        Literal::Null => None,
        Literal::String(s) => Some(Json::String(s.clone())),
        Literal::Bool(b) => Some(Json::Bool(*b)),
        Literal::EnumVariant { wire, .. } => Some(Json::String(wire.clone())),
        Literal::Int(n) if as_text => Some(Json::String(n.to_string())),
        Literal::Int(n) => Some(Json::from(*n)),
        Literal::Float(x) if as_text => Some(Json::String(x.to_string())),
        Literal::Float(x) => serde_json::Number::from_f64(*x).map(Json::Number),
        Literal::EmptyList if matches!(ty, Type::Map(_)) => Some(Json::Object(Map::new())),
        Literal::EmptyList => Some(Json::Array(Vec::new())),
    }
}

impl Cx<'_> {
    fn decl_of(&self, id: veld_ir::DeclId) -> Result<&Decl, InputError> {
        self.ir
            .decls
            .get(id.index())
            .ok_or_else(|| InputError::ir("a type refers to a missing declaration"))
    }

    /// The declaration of the union arm that `value` selects with its discriminator.
    fn arm(&self, u: &Union, value: &Json, path: &str) -> Result<&Decl, InputError> {
        let obj = value
            .as_object()
            .ok_or_else(|| shape("object", value, path))?;
        let label = obj
            .get(&u.discriminator)
            .and_then(Json::as_str)
            .ok_or_else(|| InputError::MissingField {
                path: path.to_owned(),
                field: u.discriminator.clone(),
            })?;
        let arm = u.arms.iter().find(|a| a.label == label).ok_or_else(|| {
            InputError::Unsupported(format!("{path}: `{label}` is not an arm of the union"))
        })?;
        self.decl_of(arm.decl)
    }

    // ---- defaults and shape ------------------------------------------------------------

    fn normalize_decl(&self, decl: &Decl, v: &Json, path: &str) -> Result<Json, InputError> {
        match &decl.kind {
            DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Event(_) => {
                self.normalize_model(decl, v, path)
            }
            DeclKind::Union(u) => {
                let arm = self.arm(u, v, path)?;
                self.normalize_decl(arm, v, path)
            }
            DeclKind::Enum(_) | DeclKind::Const(_) => Ok(v.clone()),
        }
    }

    fn normalize_model(&self, decl: &Decl, v: &Json, path: &str) -> Result<Json, InputError> {
        let obj = v.as_object().ok_or_else(|| shape("object", v, path))?;
        let mut out = obj.clone();
        for f in decl.fields() {
            let here = format!("{path}.{}", f.wire);
            if let Some(child) = obj.get(&f.wire) {
                out.insert(f.wire.clone(), self.normalize_type(&f.ty, child, &here)?);
            } else if let Some(d) = self.absent(f, path)? {
                out.insert(f.wire.clone(), d);
            }
        }
        Ok(Json::Object(out))
    }

    /// What an absent field becomes: its default, nothing, or an error when it is required.
    fn absent(&self, f: &Field, path: &str) -> Result<Option<Json>, InputError> {
        if let Some(lit) = &f.default {
            return Ok(default_json(lit, &f.ty));
        }
        let may_lack = f.optional || self.opts.partial || (self.opts.request && f.readonly);
        if may_lack {
            Ok(None)
        } else {
            Err(InputError::MissingField {
                path: path.to_owned(),
                field: f.wire.clone(),
            })
        }
    }

    fn normalize_type(&self, ty: &Type, v: &Json, path: &str) -> Result<Json, InputError> {
        match ty {
            Type::Scalar(_) => Ok(v.clone()),
            Type::List(t) => {
                let items = v.as_array().ok_or_else(|| shape("array", v, path))?;
                let out: Result<Vec<Json>, InputError> = items
                    .iter()
                    .enumerate()
                    .map(|(i, e)| self.normalize_type(t, e, &format!("{path}[{i}]")))
                    .collect();
                Ok(Json::Array(out?))
            }
            Type::Map(t) => {
                let entries = v.as_object().ok_or_else(|| shape("object", v, path))?;
                let mut out = Map::new();
                for (k, e) in entries {
                    out.insert(
                        k.clone(),
                        self.normalize_type(t, e, &format!("{path}.{k}"))?,
                    );
                }
                Ok(Json::Object(out))
            }
            Type::Page(t) => self.normalize_page(t, v, path),
            Type::Decl(id) => self.normalize_decl(self.decl_of(*id)?, v, path),
        }
    }

    fn normalize_page(&self, item: &Type, v: &Json, path: &str) -> Result<Json, InputError> {
        let obj = v.as_object().ok_or_else(|| shape("object", v, path))?;
        let mut out = obj.clone();
        match obj.get("items") {
            Some(items) => {
                let list = Type::List(Box::new(item.clone()));
                let here = format!("{path}.items");
                out.insert(
                    "items".to_owned(),
                    self.normalize_type(&list, items, &here)?,
                );
            }
            None if self.opts.partial => {}
            None => {
                return Err(InputError::MissingField {
                    path: path.to_owned(),
                    field: "items".to_owned(),
                });
            }
        }
        Ok(Json::Object(out))
    }

    // ---- rules -------------------------------------------------------------------------

    fn walk_decl(
        &self,
        decl: &Decl,
        v: &Json,
        path: &str,
        out: &mut Checked,
    ) -> Result<(), InputError> {
        match &decl.kind {
            DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Event(_) => {
                self.check_rules(decl, v, path, out)?;
                for f in decl.fields() {
                    if let Some(child) = v.get(&f.wire) {
                        self.walk_type(&f.ty, child, &format!("{path}.{}", f.wire), out)?;
                    }
                }
                Ok(())
            }
            DeclKind::Union(u) => {
                let arm = self.arm(u, v, path)?;
                self.walk_decl(arm, v, path, out)
            }
            DeclKind::Enum(_) | DeclKind::Const(_) => Ok(()),
        }
    }

    fn walk_type(
        &self,
        ty: &Type,
        v: &Json,
        path: &str,
        out: &mut Checked,
    ) -> Result<(), InputError> {
        match ty {
            Type::Scalar(_) => Ok(()),
            Type::List(t) => self.walk_items(t, v, path, out),
            Type::Map(t) => {
                for (k, e) in v.as_object().into_iter().flatten() {
                    self.walk_type(t, e, &format!("{path}.{k}"), out)?;
                }
                Ok(())
            }
            Type::Page(t) => match v.get("items") {
                Some(items) => self.walk_items(t, items, &format!("{path}.items"), out),
                None => Ok(()),
            },
            Type::Decl(id) => self.walk_decl(self.decl_of(*id)?, v, path, out),
        }
    }

    fn walk_items(
        &self,
        item: &Type,
        v: &Json,
        path: &str,
        out: &mut Checked,
    ) -> Result<(), InputError> {
        for (i, e) in v.as_array().into_iter().flatten().enumerate() {
            self.walk_type(item, e, &format!("{path}[{i}]"), out)?;
        }
        Ok(())
    }

    /// Evaluates every rule of `decl` on the value `v` that sits at `path`.
    fn check_rules(
        &self,
        decl: &Decl,
        v: &Json,
        path: &str,
        out: &mut Checked,
    ) -> Result<(), InputError> {
        for rule in decl.rules() {
            if self.opts.request && rule.reads_readonly {
                continue;
            }
            // The partial check is the client's, and a client skips audited rules (section 17.1).
            if self.opts.partial && rule.is_audited() {
                continue;
            }
            let verdict = eval(&Env::for_value(self.ir, v), &rule.expr);
            let sink = if rule.is_audited() {
                &mut out.audited
            } else {
                &mut out.violations
            };
            match verdict {
                Ok(Value::Bool(true)) => {}
                Ok(Value::Bool(false)) => sink.push(violation(decl, rule, path, None)),
                Ok(other) => {
                    let what = format!("rule `{}` is {}, not bool", rule.name, other.kind());
                    return Err(InputError::Ir(what));
                }
                Err(Failure::Eval(e)) => sink.push(violation(decl, rule, path, Some(e))),
                Err(Failure::Input(InputError::MissingField { .. })) if self.opts.partial => {}
                Err(Failure::Input(e)) => return Err(e.at_path(path)),
            }
        }
        Ok(())
    }
}

fn violation(decl: &Decl, rule: &Rule, path: &str, error: Option<EvalError>) -> Violation {
    let message = match (error, &rule.message) {
        (Some(_), _) => format!("rule {} could not be evaluated", rule.name),
        (None, Some(m)) => m.clone(),
        (None, None) => format!("violates rule {}", rule.name),
    };
    let wire = |name: &String| {
        decl.fields()
            .iter()
            .find(|f| &f.name == name)
            .map_or_else(|| name.clone(), |f| f.wire.clone())
    };
    Violation {
        rule_name: rule.name.clone(),
        rule_id: rule.id.clone(),
        path: path.to_owned(),
        fields: rule.reads.iter().map(wire).collect(),
        message,
        error,
    }
}
