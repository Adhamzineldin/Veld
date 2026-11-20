//! Modules become services, actions become endpoints.

use veld_compiler::language::is_reserved;

use super::conv::{FileConv, comment_block, doc_lines, todo_comment};
use super::names::{header_param_name, join_prefix, lower_camel, path_params, prelude_status, v2_path};
use super::v1::ast::{Action, Header, Headers, Module, Output, TypeRef, V1File};
use crate::fmt::text::quote;

/// The auth and middleware annotations of an action.
#[derive(Debug, Clone, PartialEq, Default)]
struct Guards {
    auth: bool,
    middleware: Vec<String>,
}

impl Guards {
    fn is_empty(&self) -> bool {
        !self.auth && self.middleware.is_empty()
    }

    fn annotations(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.auth {
            out.push("@auth(bearer)".to_string());
        }
        if !self.middleware.is_empty() {
            out.push(format!("@middleware({})", self.middleware.join(", ")));
        }
        out
    }
}

impl FileConv<'_> {
    pub fn module(&mut self, m: &Module, file: &V1File) -> String {
        let prefix = join_prefix(file.prefix.as_deref(), m.prefix.as_deref());
        if m.prefix.is_none() && file.prefix.is_none() {
            self.warn(format!("module {}: no prefix; used \"/\"", m.name));
        }
        if let Some(u) = &m.base_url {
            self.warn(format!("module {}: baseUrl `{u}` was dropped; set base_url in veld.toml", m.name));
        }
        if let Some(p) = &file.prefix {
            self.warn(format!("module {}: top-level `prefix: {p}` was prepended to the module prefix", m.name));
        }
        let guards: Vec<Guards> = m.actions.iter().map(|a| self.guards(a)).collect();
        let hoist = guards.first().filter(|g| !g.is_empty() && guards.iter().all(|x| x == *g)).cloned();
        let mut lines = comment_block(&m.comments);
        lines.extend(doc_lines(&m.description));
        if let Some(h) = &hoist {
            lines.extend(h.annotations());
        }
        lines.push(format!("service {} at {} {{", m.name, quote(&prefix)));
        for (a, g) in m.actions.iter().zip(&guards) {
            let own = if hoist.is_some() { Guards::default() } else { g.clone() };
            for l in self.endpoint(a, &own) {
                lines.push(format!("  {l}"));
            }
        }
        lines.push("}".to_string());
        lines.join("\n")
    }

    /// Maps v1 middleware: names that look like auth become `@auth(bearer)`.
    fn guards(&mut self, a: &Action) -> Guards {
        let mut g = Guards::default();
        for name in &a.middleware {
            if name.to_lowercase().contains("auth") {
                g.auth = true;
                if self.auth_noted.insert(name.clone()) {
                    self.warn(format!(
                        "middleware `{name}` looks like authentication; mapped to @auth(bearer). Use @middleware({name}) or another scheme if that is wrong"
                    ));
                }
            } else if !g.middleware.contains(name) {
                g.middleware.push(name.clone());
            }
        }
        g
    }

    fn endpoint(&mut self, a: &Action, guards: &Guards) -> Vec<String> {
        let mut lines = comment_block(&a.comments);
        lines.extend(doc_lines(&a.description));
        if let Some(d) = &a.deprecated {
            lines.push(format!("@deprecated({})", quote(d)));
        }
        lines.extend(guards.annotations());
        let method = a.method.to_lowercase();
        let path = v2_path(&a.path);
        let params = self.params(a, &method, &path, &mut lines);
        let name = self.endpoint_name(&a.name);
        let mut head = format!("{method} {} {name}({})", quote(&path), params.join(", "));
        let resp = self.responses(a);
        if !resp.is_empty() {
            head.push_str(&format!(" -> {}", resp.join(" | ")));
        }
        lines.push(head);
        lines
    }

    fn endpoint_name(&mut self, v1: &str) -> String {
        let n = lower_camel(v1);
        if is_reserved(&n) {
            self.warn(format!("action {v1}: `{n}` is a reserved word; renamed to `{n}Action`"));
            return format!("{n}Action");
        }
        n
    }

    // ---- parameters -------------------------------------------------------

    fn params(&mut self, a: &Action, method: &str, path: &str, lines: &mut Vec<String>) -> Vec<String> {
        let mut out: Vec<String> = path_params(path).into_iter().map(|p| format!("{p}: string")).collect();
        let bodyless = matches!(method, "get" | "head" | "options");
        let ws = method == "ws";
        let (body, query) = self.input_and_query(a, bodyless, lines);
        out.extend(query.map(|q| format!("query: {q}")));
        out.extend(self.header_params(a, lines));
        if ws {
            let t = a.emit.clone().or_else(|| body.clone());
            out.extend(t.map(|t| format!("body: stream {}", self.type_name(&t))));
        } else {
            out.extend(body.map(|b| format!("body: {b}")));
        }
        out
    }

    /// Resolves `input` and `query` into a body type and a query type.
    fn input_and_query(&mut self, a: &Action, bodyless: bool, lines: &mut Vec<String>) -> (Option<String>, Option<String>) {
        let input = a.input.as_ref().map(|t| self.type_ref(t, &a.name, "Input"));
        let query = a.query.as_ref().map(|t| self.type_ref(t, &a.name, "Query"));
        match (input, query, bodyless) {
            (Some(i), None, true) => {
                self.warn(format!("action {}: `input` on a body-less method became `query`", a.name));
                (None, Some(i))
            }
            (Some(i), Some(q), true) => {
                self.warn(format!("action {}: both `input` and `query` on a body-less method; `input` dropped", a.name));
                lines.push(todo_comment(&format!("input `{i}` of {} could not be mapped", a.name)));
                (None, Some(q))
            }
            (i, q, _) => (i, q),
        }
    }

    /// A named type, or a generated model for an inline `{ ... }` type.
    fn type_ref(&mut self, t: &TypeRef, action: &str, suffix: &str) -> String {
        match t {
            TypeRef::Named(n) => self.type_name(n),
            TypeRef::Inline(fields) => {
                let name = format!("{action}{suffix}");
                self.locals.insert(name.clone());
                let text = self.model_text(&name, None, &None, fields, &[]);
                self.generated.push(text);
                name
            }
        }
    }

    fn header_params(&mut self, a: &Action, lines: &mut Vec<String>) -> Vec<String> {
        match &a.headers {
            None => Vec::new(),
            Some(Headers::Inline(hs)) => hs.iter().map(|h| self.header_param(h)).collect(),
            Some(Headers::Named(m)) => {
                let Some(fields) = self.ctx.models.get(m) else {
                    self.warn(format!("action {}: headers model `{m}` not found; headers dropped", a.name));
                    lines.push(todo_comment(&format!("headers `{m}` of {} could not be expanded", a.name)));
                    return Vec::new();
                };
                let hs: Vec<Header> = fields
                    .iter()
                    .map(|f| Header {
                        name: f.name.clone(),
                        optional: f.optional,
                        ty: match &f.ty {
                            crate::migrate::v1::ast::V1Type::Named(n) => n.clone(),
                            _ => "string".to_string(),
                        },
                    })
                    .collect();
                hs.iter().map(|h| self.header_param(h)).collect()
            }
        }
    }

    fn header_param(&mut self, h: &Header) -> String {
        let ty = self.type_name(&h.ty);
        let name = header_param_name(&h.name);
        format!("@header({}) {name}{}: {ty}", quote(&h.name), if h.optional { "?" } else { "" })
    }

    // ---- responses --------------------------------------------------------

    fn responses(&mut self, a: &Action) -> Vec<String> {
        let mut items = Vec::new();
        if a.method.eq_ignore_ascii_case("ws") {
            let t = a.stream.clone().or_else(|| a.output.as_ref().and_then(named_of));
            items.extend(t.map(|t| format!("stream {}", self.type_name(&t))));
        } else {
            if a.stream.is_some() {
                self.warn(format!("action {}: `stream` is only valid on WS actions and was dropped", a.name));
            }
            if let Some(o) = &a.output {
                items.push(self.success(a, o));
            }
        }
        let mut seen = Vec::new();
        for (name, status) in &a.errors {
            if seen.contains(name) {
                continue;
            }
            seen.push(name.clone());
            items.push(self.error_ref(name, *status));
        }
        items
    }

    fn success(&mut self, a: &Action, o: &Output) -> String {
        let t = self.type_ref(&o.ty, &a.name, "Output");
        let ty = if o.array { format!("{t}[]") } else { t };
        o.status.map_or(ty.clone(), |s| format!("{s} {ty}"))
    }

    /// An error reference: prelude names stay, anything else uses its generated declaration.
    fn error_ref(&mut self, name: &str, status: Option<u16>) -> String {
        let (final_name, default) = match prelude_status(name) {
            Some(d) => (name.to_string(), d),
            None => match self.ctx.errors.get(name) {
                Some(g) => {
                    self.refs.insert(g.name.clone());
                    (g.name.clone(), g.status)
                }
                None => (name.to_string(), 400),
            },
        };
        match status {
            Some(s) if s != default => format!("{s} {final_name}"),
            _ => final_name,
        }
    }

    /// Declarations for the v1 error names this file is responsible for.
    pub fn generated_errors(&mut self, idx: usize) {
        let ctx = self.ctx;
        for (v1, g) in ctx.errors.iter().filter(|(_, g)| g.file == idx) {
            self.locals.insert(g.name.clone());
            let how = if g.inferred { "status guessed from the name; v1 errors carry none" } else { "status from the v1 contract" };
            self.warn(format!("error `{v1}` is not in the v2 prelude; declared `error {}` with status {} ({how})", g.name, g.status));
            self.generated.push(format!(
                "// generated by veld migrate from the v1 error name `{v1}`\nerror {} {} {{\n  message?: string\n}}",
                g.name, g.status
            ));
        }
    }
}

fn named_of(o: &Output) -> Option<String> {
    match &o.ty {
        TypeRef::Named(n) => Some(n.clone()),
        TypeRef::Inline(_) => None,
    }
}

