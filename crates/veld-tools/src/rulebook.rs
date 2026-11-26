//! The rule book: the rules of a workspace as Markdown, for people who do not read contracts
//! (`spec/rules.md` section 19.2, `veld export rules`).
//!
//! A pure function of the IR, deterministic: packages in name order, everything inside a package
//! in source order. Expressions are printed by the printer the OpenAPI export and `veld diff`
//! use, so one rule reads the same in all three.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use veld_ir::{
    Decl, DeclKind, Endpoint, EndpointRules, Example, ExampleOutcome, Ir, Origin, PackageId,
    Policy, PolicyEffect, Rule, Service, ServiceId, Type,
};

use crate::diff::expr_text::expr_text;

/// The sentence of a workspace without rules.
pub const NO_RULES: &str = "This workspace has no rules.";

/// The rule book of the packages of `services`, as Markdown. Only those services' endpoints are
/// listed.
#[must_use]
pub fn rule_book(ir: &Ir, services: &[ServiceId]) -> String {
    let packages: BTreeSet<(String, PackageId)> = services
        .iter()
        .map(|s| ir.service(*s).package)
        .map(|p| (ir.package(p).name.clone(), p))
        .collect();
    let sections: Vec<String> = packages
        .iter()
        .filter_map(|(name, id)| package_section(ir, name, *id, services))
        .collect();
    let mut out = String::from("# Rule book\n");
    if sections.is_empty() {
        let _ = write!(out, "\n{NO_RULES}\n");
        return out;
    }
    for s in sections {
        out.push('\n');
        out.push_str(&s);
    }
    out
}

/// The section of one package; `None` when it has nothing to say.
fn package_section(ir: &Ir, name: &str, id: PackageId, services: &[ServiceId]) -> Option<String> {
    let parts = [
        ("Declarations", declarations(ir, id)),
        ("State machines", machines(ir, id)),
        ("Policies", policies(ir, id)),
        ("Endpoints", endpoints(ir, id, services)),
    ];
    if parts.iter().all(|(_, body)| body.is_empty()) {
        return None;
    }
    let mut out = format!("## Package `{name}`\n");
    for (title, body) in parts.iter().filter(|(_, b)| !b.is_empty()) {
        let _ = write!(out, "\n### {title}\n\n{}\n", body.trim_end());
    }
    let examples = examples(ir, id);
    if !examples.is_empty() {
        let _ = write!(out, "\n### Examples\n\n{}\n", examples.trim_end());
    }
    Some(out)
}

fn by_origin<T>(items: &mut [(&T, &Origin)]) {
    items.sort_by(|a, b| key(a.1).cmp(&key(b.1)));
}

fn key(o: &Origin) -> (&str, u32, u32) {
    (&o.file, o.line, o.col)
}

/// Doc comment lines, indented so that they continue a list item.
fn indented(doc: Option<&str>, by: &str) -> String {
    doc.map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| {
            d.lines()
                .map(|l| format!("{by}{}\n", l.trim_end()))
                .collect()
        })
        .unwrap_or_default()
}

fn paragraph(doc: Option<&str>) -> String {
    let text = indented(doc, "");
    if text.is_empty() {
        text
    } else {
        format!("{text}\n")
    }
}

// ---- 1. declarations with invariants ----------------------------------------------------

fn declarations(ir: &Ir, package: PackageId) -> String {
    let mut decls: Vec<(&Decl, &Origin)> = ir
        .decls
        .iter()
        .filter(|d| d.package == package)
        .filter(|d| d.rules().iter().any(|r| r.inherited_from.is_none()))
        .map(|d| (d, &d.origin))
        .collect();
    by_origin(&mut decls);
    let mut out = String::new();
    for (d, _) in decls {
        let kind = match &d.kind {
            DeclKind::Error(_) => "error",
            DeclKind::Event(_) => "event",
            DeclKind::Model(_) | DeclKind::Enum(_) | DeclKind::Union(_) | DeclKind::Const(_) => {
                "model"
            }
        };
        let _ = write!(
            out,
            "#### {} ({kind}){}\n\n",
            d.name,
            deprecated_mark(d.deprecated.as_deref())
        );
        out.push_str(&paragraph(d.docs.as_deref()));
        for r in d.rules().iter().filter(|r| r.inherited_from.is_none()) {
            out.push_str(&invariant_line(r));
        }
        out.push('\n');
    }
    out
}

fn invariant_line(r: &Rule) -> String {
    let title = r.message.as_deref().unwrap_or(&r.name);
    let marks = format!(
        "{}{}",
        audited_mark(r.is_audited()),
        deprecated_mark(r.deprecated.as_deref())
    );
    let mut line = format!("- {title}: `{}`{marks}\n", expr_text(&r.expr));
    line.push_str(&indented(r.docs.as_deref(), "  "));
    line
}

fn audited_mark(audited: bool) -> &'static str {
    if audited {
        " (audited, not enforced)"
    } else {
        ""
    }
}

/// ` (deprecated)` or ` (deprecated: reason)`.
fn deprecated_mark(reason: Option<&str>) -> String {
    match reason.map(str::trim) {
        None => String::new(),
        Some("") => " (deprecated)".to_owned(),
        Some(why) => format!(" (deprecated: {why})"),
    }
}

// ---- 2. state machines --------------------------------------------------------------------

fn machines(ir: &Ir, package: PackageId) -> String {
    let mut found: Vec<(&Decl, &Origin)> = ir
        .decls
        .iter()
        .filter(|d| d.package == package)
        .filter(|d| matches!(&d.kind, DeclKind::Enum(e) if e.is_state_machine()))
        .map(|d| (d, &d.origin))
        .collect();
    by_origin(&mut found);
    let mut out = String::new();
    for (d, _) in found {
        let DeclKind::Enum(e) = &d.kind else { continue };
        let _ = write!(
            out,
            "#### {}{}\n\n",
            d.name,
            deprecated_mark(d.deprecated.as_deref())
        );
        out.push_str(&paragraph(d.docs.as_deref()));
        out.push_str("| From | To |\n|---|---|\n");
        for f in &e.flows {
            let _ = writeln!(out, "| {} | {} |", f.from, f.to);
        }
        if !e.finals.is_empty() {
            let _ = write!(out, "\nFinal states: {}\n", e.finals.join(", "));
        }
        out.push('\n');
    }
    out
}

// ---- 3. policies --------------------------------------------------------------------------

fn policies(ir: &Ir, package: PackageId) -> String {
    let mut found: Vec<(&Policy, &Origin)> = ir
        .policies
        .iter()
        .filter(|p| p.package == package)
        .map(|p| (p, &p.origin))
        .collect();
    by_origin(&mut found);
    let mut out = String::new();
    for (p, _) in found {
        let params: Vec<&str> = p.params.iter().map(|x| x.name.as_str()).collect();
        let _ = write!(out, "#### {}({})\n\n", p.name, params.join(", "));
        out.push_str(&paragraph(p.docs.as_deref()));
        for c in &p.clauses {
            let word = match c.effect {
                PolicyEffect::Permit => "permit",
                PolicyEffect::Forbid => "forbid",
            };
            let message = c
                .message
                .as_deref()
                .map_or(String::new(), |m| format!(": {m}"));
            let _ = writeln!(out, "- {word} when `{}`{message}", expr_text(&c.expr));
        }
        out.push('\n');
    }
    out
}

// ---- 4. endpoints with rules --------------------------------------------------------------

fn endpoints(ir: &Ir, package: PackageId, services: &[ServiceId]) -> String {
    let mut found: Vec<(&Service, &Endpoint)> = services
        .iter()
        .map(|s| ir.service(*s))
        .filter(|s| s.package == package)
        .flat_map(|s| s.endpoints.iter().map(move |e| (s, e)))
        .filter(|(_, e)| e.rules.has_statements())
        .collect();
    found.sort_by(|a, b| key(&a.1.origin).cmp(&key(&b.1.origin)));
    let mut out = String::new();
    for (s, e) in found {
        let _ = write!(
            out,
            "#### {} {} ({}.{}){}\n\n",
            e.method.as_str(),
            e.path,
            s.name,
            e.name,
            deprecated_mark(e.deprecated.as_deref())
        );
        out.push_str(&paragraph(e.docs.as_deref()));
        out.push_str(&statements(ir, &e.rules));
        out.push('\n');
    }
    out
}

fn statements(ir: &Ir, rules: &EndpointRules) -> String {
    let mut out = String::new();
    for f in &rules.facts {
        let key = f
            .key
            .as_ref()
            .map_or(String::new(), |k| format!(" = {}", expr_text(k)));
        let optional = if f.optional { "?" } else { "" };
        let _ = writeln!(
            out,
            "- Loads: `{}{optional}: {}{key}`",
            f.name,
            type_text(ir, &f.ty)
        );
    }
    for a in &rules.allows {
        let args: Vec<String> = a.args.iter().map(expr_text).collect();
        let _ = writeln!(
            out,
            "- Who may call it: `{}({})`",
            ir.policy(a.policy).name,
            args.join(", ")
        );
    }
    if let Some(t) = &rules.transition {
        let path: Vec<&str> = t.steps.iter().map(|s| s.name.as_str()).collect();
        let _ = writeln!(
            out,
            "- Move: `{}.{}`: {} -> {}",
            t.fact,
            path.join("."),
            t.from.join(", "),
            t.to
        );
    }
    if !rules.requires.is_empty() {
        out.push_str("- Requires:\n");
        for (i, r) in rules.requires.iter().enumerate() {
            let error = r
                .else_error
                .as_ref()
                .map_or("ValidationError", |e| ir.decl(e.decl).name.as_str());
            let title = title_of(r.message.as_deref(), r.name.as_deref(), "require", i);
            let marks = audited_mark(r.rollout.is_some());
            let _ = writeln!(
                out,
                "  - {title}: `{}`, answers `{error}`{marks}",
                expr_text(&r.expr)
            );
            out.push_str(&indented(r.docs.as_deref(), "    "));
        }
    }
    if !rules.ensures.is_empty() {
        out.push_str("- Promises:\n");
        for (i, e) in rules.ensures.iter().enumerate() {
            let title = title_of(e.message.as_deref(), e.name.as_deref(), "ensure", i);
            let marks = audited_mark(e.rollout.is_some());
            let _ = writeln!(out, "  - {title}: `{}`{marks}", expr_text(&e.expr));
            out.push_str(&indented(e.docs.as_deref(), "    "));
        }
    }
    out
}

/// The message, else the name, else `require #2`.
fn title_of(message: Option<&str>, name: Option<&str>, kind: &str, index: usize) -> String {
    message
        .or(name)
        .map_or_else(|| format!("{kind} #{}", index + 1), str::to_owned)
}

fn type_text(ir: &Ir, ty: &Type) -> String {
    match ty {
        Type::Scalar(s) => s.name().to_owned(),
        Type::List(t) => format!("{}[]", type_text(ir, t)),
        Type::Map(t) => format!("Map<string, {}>", type_text(ir, t)),
        Type::Page(t) => format!("Page<{}>", type_text(ir, t)),
        Type::Decl(id) => ir.decl(*id).name.clone(),
    }
}

// ---- 5. examples --------------------------------------------------------------------------

fn examples(ir: &Ir, package: PackageId) -> String {
    let mut found: Vec<(&Example, &Origin)> = ir
        .examples
        .iter()
        .filter(|e| e.package == package)
        .map(|e| (e, &e.origin))
        .collect();
    by_origin(&mut found);
    let mut out = String::new();
    for (e, _) in found {
        let _ = writeln!(out, "- {}: {}", e.name, outcome_text(ir, e));
    }
    out
}

/// `ok`, the name of the error, `rejected`, or the handlers an event is delivered to.
fn outcome_text(ir: &Ir, e: &Example) -> String {
    if let Some(expect) = &e.expect {
        return match expect {
            veld_ir::EventExpect::Rejected => "rejected".to_owned(),
            veld_ir::EventExpect::Calls(calls) => calls
                .iter()
                .map(|c| {
                    let not = if c.called { "" } else { " not" };
                    format!("{}.{}{not} called", ir.service(c.service).name, c.handler)
                })
                .collect::<Vec<_>>()
                .join(", "),
        };
    }
    match &e.outcome {
        Some(ExampleOutcome::Ok(_)) | None => "ok".to_owned(),
        Some(ExampleOutcome::Error(x)) => ir.decl(x.decl).name.clone(),
    }
}

#[cfg(test)]
mod tests;
