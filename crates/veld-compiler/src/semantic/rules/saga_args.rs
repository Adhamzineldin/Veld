//! The calls of a saga and their arguments (`spec/platform.md` section 17, `E1201` and `E1202`).

use std::collections::BTreeMap;

use super::depth;
use super::saga::{Cx, RArg, RCall, RObjectField, RValue};
use super::types::of;
use crate::diagnostics::{Diagnostic, Span};
use crate::semantic::{DKind, REndpoint, RField};
use crate::syntax::ast::{self, SagaValueDecl};
use veld_ir::{Method, Type};

/// Containers above the value of an argument in the IR: the document, `services`, the service,
/// `endpoints`, the endpoint, `saga`, `steps`, the step, the call, `args` and the argument.
const SAGA_PREFIX_DEPTH: usize = 11;

/// What each level of `{ .. }` adds: `{"object": [{"value": ..}]}`.
const OBJECT_LEVEL_DEPTH: usize = 3;

impl<'p> Cx<'p, '_, '_> {
    /// Checks one call: its service and endpoint, that the endpoint is idempotent, and its
    /// arguments.
    pub fn call(&mut self, call: &ast::SagaCallDecl, later: &[&str]) -> Option<RCall> {
        let (service, callee) = self.resolve(call)?;
        if callee.idempotent.is_none() {
            self.diags.push(
                Diagnostic::error(
                    "E1202",
                    format!(
                        "`{}.{}` is not `@idempotent`, and a saga repeats every call",
                        call.service.name, call.endpoint.name
                    ),
                    call.endpoint.span,
                )
                .with_help("annotate the endpoint with `@idempotent`: the saga sends the key `<saga id>.<step>`"),
            );
        }
        let args = self.args(call, callee, later)?;
        Some(RCall {
            service,
            endpoint: callee.name.clone(),
            args,
        })
    }

    /// The service and the endpoint a call names.
    fn resolve(&mut self, call: &ast::SagaCallDecl) -> Option<(usize, &'p REndpoint)> {
        let prog = self.prog;
        let own = &prog.packages[self.pkg].name;
        let consumed: &[String] = self.consumes.get(own).map_or(&[], Vec::as_slice);
        let reachable = |i: usize| {
            let p = prog.services[i].pkg;
            p == self.pkg
                || consumed
                    .iter()
                    .any(|c| c == "*" || *c == prog.packages[p].name)
        };
        let named: Vec<usize> = (0..prog.services.len())
            .filter(|&i| prog.services[i].name == call.service.name)
            .collect();
        let usable: Vec<usize> = named.iter().copied().filter(|&i| reachable(i)).collect();
        let name = &call.service.name;
        let index = match (usable.as_slice(), named.first()) {
            ([one], _) => *one,
            ([], None) => {
                let all: Vec<&str> = (0..prog.services.len())
                    .filter(|&i| reachable(i))
                    .map(|i| prog.services[i].name.as_str())
                    .collect();
                self.diags.push(
                    Diagnostic::error("E1201", format!("no service `{name}`"), call.service.span)
                        .did_you_mean(name, all, None),
                );
                return None;
            }
            ([], Some(&other)) => {
                let pkg = &prog.packages[prog.services[other].pkg].name;
                self.diags.push(
                    Diagnostic::error(
                        "E1201",
                        format!("service `{name}` of `{pkg}` is not consumed by `{own}`"),
                        call.service.span,
                    )
                    .with_help(format!(
                        "add `{pkg}` to `consumes` in the `veld.toml` of `{own}`"
                    )),
                );
                return None;
            }
            _ => {
                self.err(
                    "E1201",
                    format!("`{name}` names a service of more than one consumed package"),
                    call.service.span,
                );
                return None;
            }
        };
        let svc = &prog.services[index];
        let callee = self.endpoint_of(svc, call)?;
        Some((index, callee))
    }

    fn endpoint_of(
        &mut self,
        svc: &'p crate::semantic::ServiceInfo,
        call: &ast::SagaCallDecl,
    ) -> Option<&'p REndpoint> {
        let name = &call.endpoint.name;
        let Some(callee) = svc.endpoints.iter().find(|e| &e.name == name) else {
            self.diags.push(
                Diagnostic::error(
                    "E1201",
                    format!("service `{}` has no endpoint `{name}`", svc.name),
                    call.endpoint.span,
                )
                .did_you_mean(
                    name,
                    svc.endpoints.iter().map(|e| e.name.as_str()),
                    None,
                ),
            );
            return None;
        };
        let streams = callee.success_stream
            || callee.method == Method::Ws
            || callee.params.iter().any(|p| p.stream);
        if streams {
            self.err(
                "E1201",
                format!("`{}.{name}` streams, and a step needs one answer", svc.name),
                call.endpoint.span,
            );
            return None;
        }
        Some(callee)
    }

    /// The arguments, in the order of the parameters of the endpoint.
    fn args(
        &mut self,
        call: &ast::SagaCallDecl,
        callee: &REndpoint,
        later: &[&str],
    ) -> Option<Vec<RArg>> {
        let mut ok = true;
        let mut given: BTreeMap<&str, RValue> = BTreeMap::new();
        for a in &call.args {
            let name = a.name.name.as_str();
            let Some(p) = callee.params.iter().find(|p| p.name == name) else {
                self.diags.push(
                    Diagnostic::error(
                        "E1201",
                        format!("`{}` has no parameter `{name}`", callee.name),
                        a.name.span,
                    )
                    .did_you_mean(
                        name,
                        callee.params.iter().map(|p| p.name.as_str()),
                        None,
                    ),
                );
                ok = false;
                continue;
            };
            if given.contains_key(name) {
                self.err(
                    "E1201",
                    format!("argument `{name}` is given twice"),
                    a.name.span,
                );
                ok = false;
            } else if let Some(v) = self.value(&a.value, &p.ty, name, later, 0) {
                given.insert(name, v);
            } else {
                ok = false;
            }
        }
        for p in callee.params.iter().filter(|p| !p.optional) {
            if !given.contains_key(p.name.as_str()) {
                self.err(
                    "E1201",
                    format!("`{}` needs the argument `{}`", callee.name, p.name),
                    call.span,
                );
                ok = false;
            }
        }
        ok.then(|| {
            callee
                .params
                .iter()
                .filter_map(|p| {
                    given.remove(p.name.as_str()).map(|value| RArg {
                        name: p.name.clone(),
                        value,
                    })
                })
                .collect()
        })
    }

    /// One value, expected to be of type `want`; `level` objects deep.
    fn value(
        &mut self,
        v: &SagaValueDecl,
        want: &Type,
        what: &str,
        later: &[&str],
        level: usize,
    ) -> Option<RValue> {
        match v {
            SagaValueDecl::Expr(e) => self.expr_value(e, want, what, later, level),
            SagaValueDecl::Object { fields, span } => {
                self.object_value(fields, *span, want, what, later, level)
            }
        }
    }

    fn expr_value(
        &mut self,
        e: &ast::Expr,
        want: &Type,
        what: &str,
        later: &[&str],
        level: usize,
    ) -> Option<RValue> {
        if self.reads_later(e, later) {
            return None;
        }
        let want_t = of(want.clone());
        let typed = self.with_checker(|c| c.expr(e, Some(&want_t)))?;
        let typed = self.with_checker(|c| {
            c.fit_code("E1201", typed, &want_t, e.span, |found, expected| {
                format!("`{what}` is `{found}`, the parameter takes `{expected}`")
            })
        })?;
        let budget = depth::SERDE_JSON_MAX_DEPTH
            - depth::PLUGIN_ENVELOPE_DEPTH
            - SAGA_PREFIX_DEPTH
            - OBJECT_LEVEL_DEPTH * level;
        if depth::json_depth(&typed) > budget {
            self.err(
                "E0005",
                "this expression nests too deeply to be written as IR JSON",
                e.span,
            );
            return None;
        }
        Some(RValue::Expr(typed))
    }

    /// `{ field: value, .. }` for a parameter or field whose type is a model.
    fn object_value(
        &mut self,
        fields: &[ast::SagaFieldDecl],
        span: Span,
        want: &Type,
        what: &str,
        later: &[&str],
        level: usize,
    ) -> Option<RValue> {
        let prog = self.prog;
        let model = match want {
            Type::Decl(d) if prog.decls[d.index()].kind == DKind::Model => &prog.decls[d.index()],
            Type::Decl(_) | Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) => {
                self.err(
                    "E1201",
                    format!(
                        "`{what}` is a `{}`: `{{ .. }}` builds a model",
                        prog.type_name(want)
                    ),
                    span,
                );
                return None;
            }
        };
        let mut ok = true;
        let mut given: BTreeMap<&str, RValue> = BTreeMap::new();
        for f in fields {
            let name = f.name.name.as_str();
            let Some(field) = model.res.fields.iter().find(|x| x.name == name) else {
                self.diags.push(
                    Diagnostic::error(
                        "E1201",
                        format!("`{}` has no field `{name}`", model.name),
                        f.name.span,
                    )
                    .did_you_mean(
                        name,
                        model.res.fields.iter().map(|x| x.name.as_str()),
                        None,
                    ),
                );
                ok = false;
                continue;
            };
            ok &= self.object_field(f, field, &mut given, later, level);
        }
        ok &= self.missing_fields(&model.res.fields, &model.name, &given, span);
        ok.then(|| {
            RValue::Object(
                model
                    .res
                    .fields
                    .iter()
                    .filter_map(|x| {
                        given.remove(x.name.as_str()).map(|value| RObjectField {
                            name: x.name.clone(),
                            wire: x.wire.clone(),
                            value,
                        })
                    })
                    .collect(),
            )
        })
    }

    fn object_field<'f>(
        &mut self,
        f: &ast::SagaFieldDecl,
        field: &'f RField,
        given: &mut BTreeMap<&'f str, RValue>,
        later: &[&str],
        level: usize,
    ) -> bool {
        if field.readonly || field.tenant {
            self.err(
                "E1201",
                format!(
                    "field `{}` is `@{}`: the server sets it",
                    field.name,
                    if field.readonly { "readonly" } else { "tenant" }
                ),
                f.name.span,
            );
            return false;
        }
        if given.contains_key(field.name.as_str()) {
            self.err(
                "E1201",
                format!("field `{}` is given twice", field.name),
                f.name.span,
            );
            return false;
        }
        match self.value(&f.value, &field.ty, &field.name, later, level + 1) {
            Some(v) => {
                given.insert(field.name.as_str(), v);
                true
            }
            None => false,
        }
    }

    fn missing_fields(
        &mut self,
        fields: &[RField],
        model: &str,
        given: &BTreeMap<&str, RValue>,
        span: Span,
    ) -> bool {
        let mut ok = true;
        for f in fields
            .iter()
            .filter(|f| !f.optional && f.default.is_none() && !f.readonly && !f.tenant)
        {
            if !given.contains_key(f.name.as_str()) {
                self.err(
                    "E1201",
                    format!("`{model}` needs the field `{}`", f.name),
                    span,
                );
                ok = false;
            }
        }
        ok
    }
}
