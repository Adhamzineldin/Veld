//! Hover: docs, kind, signature and resolved type.

use veld_compiler::FileId;
use veld_compiler::Span;
use veld_compiler::index::{DeclEntry, EndpointEntry, Index};
use veld_compiler::language;
use veld_compiler::syntax::ast::{self, DeclKind, doc_text};

use crate::query::{self, Hit, Symbol, decl_fields, service_ast};
use crate::workspace::Analysis;

/// Markdown and the range it applies to.
#[derive(Debug)]
pub struct HoverText {
    pub markdown: String,
    pub span: Span,
}

#[must_use]
pub fn hover(a: &Analysis, file: FileId, offset: u32) -> Option<HoverText> {
    let ix = &a.index;
    if let Some(h) = query::symbol_at(ix, file, offset) {
        let markdown = describe(a, &h)?;
        return Some(HoverText {
            markdown,
            span: h.span,
        });
    }
    crate::hover_events::channel_hover(a, file, offset).or_else(|| scalar_at(ix, file, offset))
}

/// Hover for built-in type names (`string`, `Page`, ...), found by scanning the word.
fn scalar_at(ix: &Index, file: FileId, offset: u32) -> Option<HoverText> {
    let text = ix.sources.text(file)?;
    let at = offset as usize;
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let start = text
        .get(..at)?
        .rfind(|c: char| !is_word(c))
        .map_or(0, |i| i + 1);
    let end = at
        + text
            .get(at..)?
            .find(|c: char| !is_word(c))
            .unwrap_or(text.len() - at);
    let word = text.get(start..end)?;
    let span = Span::new(file, start, end);
    let md = if language::SCALARS.contains(&word) {
        format!("```veld\n{word}\n```\nBuilt-in scalar type.")
    } else if language::GENERICS.contains(&word) {
        format!("```veld\n{word}<T>\n```\nBuilt-in generic type.")
    } else {
        return None;
    };
    Some(HoverText { markdown: md, span })
}

fn src(ix: &Index, span: Span) -> &str {
    ix.sources
        .text(span.file)
        .and_then(|t| t.get(span.start as usize..span.end as usize))
        .unwrap_or("")
}

fn describe(a: &Analysis, h: &Hit) -> Option<String> {
    let ix = &a.index;
    match &h.symbol {
        Symbol::Decl(d) => Some(decl_hover(a, ix.decls.get(*d)?)),
        Symbol::Field { decl, field } => field_hover(ix, *decl, *field),
        Symbol::Variant { decl, variant } => variant_hover(ix, *decl, *variant),
        Symbol::Arm { decl, arm } => {
            let d = ix.decls.get(*decl)?;
            let (label, target) = d.arms.get(*arm)?;
            let t = ix.decls.get(*target)?;
            Some(format!(
                "```veld\n{label}: {}\n```\nArm of union `{}`.",
                t.name, d.name
            ))
        }
        Symbol::Service(s) => service_hover(ix, *s),
        Symbol::Endpoint { service, endpoint } => endpoint_hover(a, *service, *endpoint),
        Symbol::Param {
            service,
            endpoint,
            param,
        } => param_hover(ix, *service, *endpoint, *param),
        Symbol::Module(f) => {
            let file = ix.file(*f)?;
            Some(format!(
                "```veld\nmodule {}:{}\n```",
                file.package, file.module
            ))
        }
        Symbol::Package(p) => Some(format!("```veld\npackage {p}\n```")),
        Symbol::Annotation(n) => {
            let spec = language::annotation(n)?;
            let targets: Vec<String> = spec
                .targets
                .iter()
                .map(|t| format!("{t:?}").to_lowercase())
                .collect();
            Some(format!(
                "```veld\n{}\n```\nAnnotation, allowed on: {}.",
                spec.usage,
                targets.join(", ")
            ))
        }
    }
}

fn header(ix: &Index, d: &DeclEntry) -> String {
    let Some(ast) = ix.decl_ast(d) else {
        return d.name.clone();
    };
    match &ast.kind {
        DeclKind::Model(m) => match &m.extends {
            Some(e) => format!("model {} extends {}", d.name, src(ix, e.span)),
            None => format!("model {}", d.name),
        },
        DeclKind::Error(e) => match e.status {
            Some((s, _)) => format!("error {} {s}", d.name),
            None => format!("error {}", d.name),
        },
        DeclKind::Enum(_) => format!("enum {}", d.name),
        DeclKind::Union(_) => format!("union {}", d.name),
        DeclKind::Const(c) => format!("const {}: {}", d.name, src(ix, c.ty.span)),
        DeclKind::Service(_) => format!("service {}", d.name),
        DeclKind::Event(_) => format!("event {}", d.name),
        DeclKind::Channel(_) => format!("channel {}", d.name),
    }
}

fn kind_word(d: &DeclEntry) -> &'static str {
    use veld_compiler::semantic::DKind;
    match d.kind {
        DKind::Model => "model",
        DKind::Error => "error",
        DKind::Enum => "enum",
        DKind::Union => "union",
        DKind::Const => "const",
        DKind::Event => "event",
    }
}

fn decl_hover(a: &Analysis, d: &DeclEntry) -> String {
    let ix = &a.index;
    let mut out = format!("```veld\n{}\n```\n", header(ix, d));
    if let Some(docs) = &d.docs {
        out.push_str(&format!("{docs}\n\n"));
    }
    out.push_str(&format!(
        "- kind: {}\n- declared in: `{}`\n",
        kind_word(d),
        d.qualified
    ));
    if let Some(dep) = &d.deprecated {
        out.push_str(&format!("- deprecated: {dep}\n"));
    }
    if !d.fields.is_empty() {
        let fields: Vec<String> = d
            .fields
            .iter()
            .map(|f| format!("{}{}: {}", f.name, if f.optional { "?" } else { "" }, f.ty))
            .collect();
        out.push_str(&format!("- fields: {}\n", fields.join(", ")));
    }
    if !d.variants.is_empty() {
        out.push_str(&format!("- variants: {}\n", d.variants.join(", ")));
    }
    if let Some(sig) = a.sigs.decls.get(&d.qualified) {
        let stale = if a.sigs_stale {
            " (last successful build)"
        } else {
            ""
        };
        out.push_str(&format!("- sig: `{sig}`{stale}\n"));
    }
    out
}

fn field_hover(ix: &Index, decl: usize, field: usize) -> Option<String> {
    let d = ix.decls.get(decl)?;
    let f = decl_fields(ix, d).get(field)?;
    let resolved = d.fields.iter().find(|r| r.name == f.name.name);
    let ty = resolved.map_or_else(|| src(ix, f.ty.span).to_string(), |r| r.ty.clone());
    let opt = if f.optional { "?" } else { "" };
    let mut out = format!("```veld\n{}{opt}: {ty}\n```\n", f.name.name);
    if let Some(docs) = doc_text(&f.docs) {
        out.push_str(&format!("{docs}\n\n"));
    }
    out.push_str(&format!("- field of `{}`\n", d.name));
    for an in &f.annotations {
        out.push_str(&format!("- `{}`\n", src(ix, an.span)));
    }
    Some(out)
}

fn variant_hover(ix: &Index, decl: usize, variant: usize) -> Option<String> {
    let d = ix.decls.get(decl)?;
    let DeclKind::Enum(e) = &ix.decl_ast(d)?.kind else {
        return None;
    };
    let v = e.variants.get(variant)?;
    let wire = v.wire.as_ref().map_or(v.name.name.clone(), |w| w.0.clone());
    let mut out = format!("```veld\n{}.{}\n```\n", d.name, v.name.name);
    if let Some(docs) = doc_text(&v.docs) {
        out.push_str(&format!("{docs}\n\n"));
    }
    out.push_str(&format!("- wire value: `{wire}`\n"));
    Some(out)
}

fn service_hover(ix: &Index, s: usize) -> Option<String> {
    let entry = ix.services.get(s)?;
    let ast = service_ast(ix, s)?;
    let mut out = match &ast.prefix {
        Some(p) => format!("```veld\nservice {} at \"{}\"\n```\n", entry.name, p.value),
        None => format!("```veld\nservice {}\n```\n", entry.name),
    };
    if let Some(d) = &entry.docs {
        out.push_str(&format!("{d}\n\n"));
    }
    out.push_str(&format!(
        "- package: `{}`\n- endpoints: {}\n",
        entry.package,
        ast.endpoints.len()
    ));
    Some(out)
}

fn status_list(ix: &Index, ep: &EndpointEntry) -> String {
    let errs: Vec<String> = ep
        .errors
        .iter()
        .map(|(s, d)| format!("{s} {}", ix.decls.get(*d).map_or("?", |d| d.name.as_str())))
        .collect();
    errs.join(", ")
}

fn endpoint_hover(a: &Analysis, service: usize, endpoint: usize) -> Option<String> {
    let ix = &a.index;
    let sd = service_ast(ix, service)?;
    let ast_ep = sd.endpoints.get(endpoint)?;
    let svc = ix.services.get(service)?;
    let line = src(
        ix,
        Span {
            start: ast_ep.method_span.start,
            ..ast_ep.span
        },
    );
    let mut out = format!("```veld\n{line}\n```\n");
    if let Some(docs) = doc_text(&ast_ep.docs) {
        out.push_str(&format!("{docs}\n\n"));
    }
    match svc.endpoints.iter().find(|e| e.name == ast_ep.name.name) {
        Some(ep) => {
            out.push_str(&format!("- `{} {}`\n", ep.method.as_str(), ep.path));
            let ty = ep.success_ty.as_deref().unwrap_or("no body");
            let stream = if ep.success_stream { " (stream)" } else { "" };
            out.push_str(&format!("- success: {} {ty}{stream}\n", ep.success_status));
            out.push_str(&format!("- errors: {}\n", status_list(ix, ep)));
            if let Some(sig) = a.sigs.ops.get(&(svc.name.clone(), ep.name.clone())) {
                let stale = if a.sigs_stale {
                    " (last successful build)"
                } else {
                    ""
                };
                out.push_str(&format!("- sig: `{sig}`{stale}\n"));
            }
        }
        None => out.push_str(&format!(
            "- `{} {}{}`\n",
            ast_ep.method_span_text(ix),
            sd.prefix.as_ref().map_or("", |p| p.value.as_str()),
            ast_ep.path.value
        )),
    }
    Some(out)
}

trait MethodText {
    fn method_span_text(&self, ix: &Index) -> String;
}

impl MethodText for ast::Endpoint {
    fn method_span_text(&self, ix: &Index) -> String {
        src(ix, self.method_span).to_uppercase()
    }
}

fn param_hover(ix: &Index, service: usize, endpoint: usize, param: usize) -> Option<String> {
    let sd = service_ast(ix, service)?;
    let ep = sd.endpoints.get(endpoint)?;
    let p = ep.params.get(param)?;
    let svc = ix.services.get(service)?;
    let resolved = svc
        .endpoints
        .iter()
        .find(|e| e.name == ep.name.name)
        .and_then(|e| e.params.iter().find(|(n, _)| *n == p.name.name))
        .map(|(_, t)| t.clone());
    let ty = resolved.unwrap_or_else(|| src(ix, p.ty.span).to_string());
    let opt = if p.optional { "?" } else { "" };
    Some(format!(
        "```veld\n{}{opt}: {ty}\n```\n- parameter of `{}`\n",
        p.name.name, ep.name.name
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;
    use std::fs;

    fn analysis(files: &[(&str, &str)]) -> (tempfile::TempDir, Analysis) {
        let tmp = tempfile::tempdir().unwrap();
        for (p, t) in files {
            let path = tmp.path().join(p);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, t).unwrap();
        }
        let a = Workspace::new(Some(tmp.path().to_path_buf())).analyze(None);
        (tmp, a)
    }

    const TOML: &str = "[project]\nname = \"p\"\n[services.s]\ncontracts = \"c\"\n";

    fn hover_at(
        a: &Analysis,
        tmp: &std::path::Path,
        rel: &str,
        needle: &str,
        add: usize,
    ) -> String {
        let f = a.file_for(&tmp.join(rel)).unwrap();
        let text = a.index.sources.text(f).unwrap();
        let off = text.find(needle).unwrap() + add;
        hover(a, f, u32::try_from(off).unwrap()).unwrap().markdown
    }

    #[test]
    fn declarations_show_docs_kind_sig_and_fields() {
        let src =
            "/// A user.\nmodel User {\n  id: uuid\n  tags?: string[]\n}\nenum Role { a, b }\n";
        let (tmp, a) = analysis(&[("veld.toml", TOML), ("c/m.veld", src)]);
        let md = hover_at(&a, tmp.path(), "c/m.veld", "User", 1);
        assert!(md.contains("model User"), "{md}");
        assert!(md.contains("A user."));
        assert!(md.contains("- kind: model"));
        assert!(md.contains("sig: `"), "{md}");
        assert!(md.contains("tags?: string[]"));
        assert!(hover_at(&a, tmp.path(), "c/m.veld", "Role", 1).contains("variants: a, b"));
    }

    #[test]
    fn fields_show_the_resolved_type() {
        let src = "model A { b: B[] }\nmodel B { x: int }\n";
        let (tmp, a) = analysis(&[("veld.toml", TOML), ("c/m.veld", src)]);
        let md = hover_at(&a, tmp.path(), "c/m.veld", "b:", 0);
        assert!(md.contains("b: B[]"), "{md}");
    }

    #[test]
    fn endpoints_show_method_path_statuses_and_errors() {
        let src = "model O { id: uuid }\n/// Orders.\nservice Orders at \"/orders\" {\n  /// Fetch.\n  get \"/{id}\" getOrder(id: uuid) -> O | NotFound\n}\n";
        let (tmp, a) = analysis(&[("veld.toml", TOML), ("c/m.veld", src)]);
        let md = hover_at(&a, tmp.path(), "c/m.veld", "getOrder", 1);
        assert!(md.contains("`GET /orders/{id}`"), "{md}");
        assert!(md.contains("success: 200 O"), "{md}");
        assert!(md.contains("404 NotFound"));
        assert!(md.contains("Fetch."));
        assert!(md.contains("sig: `"));
    }

    #[test]
    fn scalars_and_annotations_have_hover_text() {
        let src = "model A {\n  x: int @min(1)\n}\n";
        let (tmp, a) = analysis(&[("veld.toml", TOML), ("c/m.veld", src)]);
        assert!(hover_at(&a, tmp.path(), "c/m.veld", "int", 1).contains("Built-in scalar"));
        assert!(hover_at(&a, tmp.path(), "c/m.veld", "min", 1).contains("@min(n)"));
    }

    #[test]
    fn rollout_has_hover_text_on_a_rule_a_require_and_an_ensure() {
        let src = "model A {\n  x: int\n  @rollout(audit)\n  rule r: x > 0\n}\nservice S at \"/s\" {\n  post \"/\" f(body: A) -> 201 A {\n    @rollout(audit)\n    require true\n    @rollout(audit)\n    ensure true\n  }\n}\n";
        let (tmp, a) = analysis(&[("veld.toml", TOML), ("c/m.veld", src)]);
        for needle in [
            "rollout(audit)\n  rule",
            "rollout(audit)\n    require",
            "rollout(audit)\n    ensure",
        ] {
            let md = hover_at(&a, tmp.path(), "c/m.veld", needle, 1);
            assert!(md.contains("@rollout(audit)"), "{md}");
            assert!(md.contains("rule, require, ensure"), "{md}");
        }
    }
}
