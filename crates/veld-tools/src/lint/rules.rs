//! The rules. Each takes the IR and appends [`Raw`] findings; levels are applied afterwards.

use std::collections::{BTreeMap, BTreeSet};

use veld_compiler::codes::LINT_RULES;
use veld_ir::{Decl, DeclId, DeclKind, Ir, Literal, Origin, PackageKind, Service, ServiceId};

use super::naming::{is_camel, is_pascal, is_variant_name, to_camel, to_pascal};
use super::{LintIssue, LintLevel, is_linted};

pub(super) struct Raw {
    pub rule: &'static str,
    pub subject: String,
    pub message: String,
    pub origin: Origin,
}

impl Raw {
    pub fn into_issue(self, level: LintLevel) -> LintIssue {
        let code = LINT_RULES
            .iter()
            .find(|(_, n)| *n == self.rule)
            .map_or("L????", |(c, _)| *c);
        LintIssue {
            code,
            rule: self.rule,
            level,
            subject: self.subject,
            message: self.message,
            origin: self.origin,
        }
    }
}

fn push(out: &mut Vec<Raw>, rule: &'static str, subject: String, message: String, o: &Origin) {
    out.push(Raw {
        rule,
        subject,
        message,
        origin: o.clone(),
    });
}

pub(super) fn run(ir: &Ir, out: &mut Vec<Raw>) {
    let user: Vec<&Decl> = ir
        .decls
        .iter()
        .filter(|d| is_linted(ir.package(d.package).kind))
        .collect();
    let services: Vec<&Service> = ir
        .services
        .iter()
        .filter(|s| is_linted(ir.package(s.package).kind))
        .collect();
    unused(ir, &user, out);
    empty_service(&services, out);
    empty_model(&user, out);
    empty_constants(&user, out);
    missing_description(&services, out);
    deprecations(&user, &services, out);
    naming(&user, &services, out);
    file_naming(ir, &user, &services, out);
}

/// Declarations used by an endpoint, a constant, an `extends` or another used declaration.
fn used_decls(ir: &Ir) -> BTreeSet<DeclId> {
    let all: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    let mut seen: BTreeSet<DeclId> = ir.reachable_decls(&all).into_iter().collect();
    let mut stack: Vec<DeclId> = Vec::new();
    for d in &ir.decls {
        if let DeclKind::Model(m) = &d.kind {
            stack.extend(m.parent);
        }
        if let DeclKind::Const(c) = &d.kind {
            c.ty.collect_decls(&mut stack);
        }
    }
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        let d = ir.decl(id);
        for f in d.fields() {
            f.ty.collect_decls(&mut stack);
        }
        match &d.kind {
            DeclKind::Model(m) => stack.extend(m.parent),
            DeclKind::Union(u) => stack.extend(u.arms.iter().map(|a| a.decl)),
            DeclKind::Error(_) | DeclKind::Enum(_) | DeclKind::Const(_) => {}
        }
    }
    seen
}

fn kind_word(d: &Decl) -> &'static str {
    match d.kind {
        DeclKind::Model(_) => "model",
        DeclKind::Error(_) => "error",
        DeclKind::Enum(_) => "enum",
        DeclKind::Union(_) => "union",
        DeclKind::Const(_) => "const",
    }
}

fn unused(ir: &Ir, user: &[&Decl], out: &mut Vec<Raw>) {
    let used = used_decls(ir);
    for d in user {
        let candidate = matches!(
            d.kind,
            DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Enum(_) | DeclKind::Union(_)
        );
        if candidate && ir.package(d.package).kind == PackageKind::Service && !used.contains(&d.id)
        {
            push(
                out,
                "unused-model",
                d.name.clone(),
                format!(
                    "{} `{}` is never used by an endpoint, a model that is used, a constant or `extends`",
                    kind_word(d),
                    d.name
                ),
                &d.origin,
            );
        }
    }
}

fn empty_service(services: &[&Service], out: &mut Vec<Raw>) {
    for s in services {
        if s.endpoints.is_empty() {
            push(
                out,
                "empty-service",
                s.name.clone(),
                format!("service `{}` has no endpoints", s.name),
                &s.origin,
            );
        }
    }
}

fn empty_model(user: &[&Decl], out: &mut Vec<Raw>) {
    for d in user {
        if let DeclKind::Model(m) = &d.kind
            && m.fields.is_empty()
        {
            push(
                out,
                "empty-model",
                d.name.clone(),
                format!("model `{}` has no fields", d.name),
                &d.origin,
            );
        }
    }
}

/// v1 flagged a constants group with no fields. A v2 constant is one value, so the equivalent
/// is a constant that holds nothing: an empty string or an empty list.
fn empty_constants(user: &[&Decl], out: &mut Vec<Raw>) {
    for d in user {
        let DeclKind::Const(c) = &d.kind else {
            continue;
        };
        let empty = match &c.value {
            Literal::EmptyList => true,
            Literal::String(s) => s.is_empty(),
            Literal::Int(_)
            | Literal::Float(_)
            | Literal::Bool(_)
            | Literal::Null
            | Literal::EnumVariant { .. } => false,
        };
        if empty {
            push(
                out,
                "empty-constants",
                d.name.clone(),
                format!("constant `{}` has an empty value", d.name),
                &d.origin,
            );
        }
    }
}

fn missing_description(services: &[&Service], out: &mut Vec<Raw>) {
    for s in services {
        if s.docs.is_none() {
            push(
                out,
                "missing-description",
                s.name.clone(),
                format!(
                    "service `{}` has no doc comment: add `///` for richer docs and OpenAPI output",
                    s.name
                ),
                &s.origin,
            );
        }
        for e in &s.endpoints {
            if e.docs.is_none() {
                push(
                    out,
                    "missing-description",
                    format!("{}.{}", s.name, e.name),
                    format!("endpoint `{}` has no doc comment", e.name),
                    &e.origin,
                );
            }
        }
    }
}

fn deprecations(user: &[&Decl], services: &[&Service], out: &mut Vec<Raw>) {
    for s in services {
        for e in &s.endpoints {
            if let Some(msg) = &e.deprecated {
                push(
                    out,
                    "deprecated-endpoint",
                    format!("{}.{}", s.name, e.name),
                    format!("endpoint `{}` is deprecated: {msg}", e.name),
                    &e.origin,
                );
            }
        }
    }
    for d in user {
        for f in d.fields().iter().filter(|f| f.inherited_from.is_none()) {
            if let Some(msg) = &f.deprecated {
                push(
                    out,
                    "deprecated-field",
                    format!("{}.{}", d.name, f.name),
                    format!(
                        "field `{}` in {} {} is deprecated: {msg}",
                        f.name,
                        kind_word(d),
                        d.name
                    ),
                    &f.origin,
                );
            }
        }
        if let Some(msg) = &d.deprecated {
            push(
                out,
                "deprecated-declaration",
                d.name.clone(),
                format!("{} `{}` is deprecated: {msg}", kind_word(d), d.name),
                &d.origin,
            );
        }
        if let DeclKind::Enum(e) = &d.kind {
            for v in &e.variants {
                if let Some(msg) = &v.deprecated {
                    push(
                        out,
                        "deprecated-declaration",
                        format!("{}.{}", d.name, v.name),
                        format!(
                            "variant `{}` of enum {} is deprecated: {msg}",
                            v.name, d.name
                        ),
                        &d.origin,
                    );
                }
            }
        }
    }
}

fn naming(user: &[&Decl], services: &[&Service], out: &mut Vec<Raw>) {
    for d in user {
        if !matches!(d.kind, DeclKind::Const(_)) && !is_pascal(&d.name) {
            push(
                out,
                "type-name-case",
                d.name.clone(),
                format!(
                    "{} `{}` should be PascalCase (`{}`)",
                    kind_word(d),
                    d.name,
                    to_pascal(&d.name)
                ),
                &d.origin,
            );
        }
        for f in d.fields().iter().filter(|f| f.inherited_from.is_none()) {
            if !is_camel(&f.name) {
                push(
                    out,
                    "field-name-case",
                    format!("{}.{}", d.name, f.name),
                    format!(
                        "field `{}` should be camelCase (`{}`)",
                        f.name,
                        to_camel(&f.name)
                    ),
                    &f.origin,
                );
            }
        }
        if let DeclKind::Enum(e) = &d.kind {
            for v in e.variants.iter().filter(|v| !is_variant_name(&v.name)) {
                push(
                    out,
                    "variant-name-case",
                    format!("{}.{}", d.name, v.name),
                    format!(
                        "enum variant `{}` should be camelCase or snake_case",
                        v.name
                    ),
                    &d.origin,
                );
            }
        }
    }
    for s in services {
        if !is_pascal(&s.name) {
            push(
                out,
                "type-name-case",
                s.name.clone(),
                format!(
                    "service `{}` should be PascalCase (`{}`)",
                    s.name,
                    to_pascal(&s.name)
                ),
                &s.origin,
            );
        }
        for e in s.endpoints.iter().filter(|e| !is_camel(&e.name)) {
            push(
                out,
                "endpoint-name-case",
                format!("{}.{}", s.name, e.name),
                format!(
                    "endpoint `{}` should be camelCase (`{}`)",
                    e.name,
                    to_camel(&e.name)
                ),
                &e.origin,
            );
        }
    }
}

#[derive(Default)]
struct FileContents {
    types: Option<Origin>,
    services: Option<Origin>,
    non_enum: Option<Origin>,
}

/// v1 used `user.model.veld`; dots are not valid in a module path in v2 (`E0101`), so the same
/// convention is spelled with an underscore (`user_model.veld`).
fn file_naming(ir: &Ir, user: &[&Decl], services: &[&Service], out: &mut Vec<Raw>) {
    let mut files: BTreeMap<(u32, &str), FileContents> = BTreeMap::new();
    for d in user {
        let c = files
            .entry((d.package.0, d.origin.file.as_str()))
            .or_default();
        if matches!(
            d.kind,
            DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Union(_)
        ) {
            c.types.get_or_insert_with(|| d.origin.clone());
        }
        if !matches!(d.kind, DeclKind::Enum(_)) {
            c.non_enum.get_or_insert_with(|| d.origin.clone());
        }
    }
    for s in services {
        let c = files
            .entry((s.package.0, s.origin.file.as_str()))
            .or_default();
        c.services.get_or_insert_with(|| s.origin.clone());
    }
    for ((pkg, file), c) in files {
        let base = file.rsplit('/').next().unwrap_or(file);
        let package = &ir.packages[pkg as usize].name;
        let found = if base.ends_with("_model.veld") {
            c.services
                .as_ref()
                .map(|o| ("_model.veld", o, "service definitions"))
        } else if base.ends_with("_service.veld") {
            c.types
                .as_ref()
                .map(|o| ("_service.veld", o, "model definitions"))
        } else if base.ends_with("_module.veld") {
            c.types
                .as_ref()
                .map(|o| ("_module.veld", o, "model definitions"))
        } else if base.ends_with("_enum.veld") {
            c.non_enum
                .as_ref()
                .or(c.services.as_ref())
                .map(|o| ("_enum.veld", o, "non-enum definitions"))
        } else {
            None
        };
        if let Some((convention, origin, what)) = found {
            push(
                out,
                "file-naming-convention",
                format!("{package}:{file}"),
                format!("`{base}` uses the {convention} convention but contains {what}"),
                origin,
            );
        }
    }
}
