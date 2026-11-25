//! Pass 6: services and endpoints (`spec/language.md` section 6, E0501-E0512).

use std::collections::BTreeMap;

use super::annotations::{self, Anns};
use super::{DKind, Program, RBlock, REndpoint, RParam, ServiceInfo, rules};
use crate::diagnostics::{Diagnostic, Span};
use crate::language::{self, Target};
use crate::syntax::ast::{self, DeclKind, HttpMethod, doc_text};
use veld_ir::{Auth, Binding, Method, Type};

type RouteKey = (usize, Method, String);

pub fn analyze(prog: &mut Program<'_>, diags: &mut Vec<Diagnostic>) {
    let mut services = Vec::new();
    let mut routes: BTreeMap<RouteKey, (Span, String)> = BTreeMap::new();
    for mi in 0..prog.modules.len() {
        let ast = prog.modules[mi].ast;
        for decl in &ast.decls {
            if let DeclKind::Service(s) = &decl.kind {
                services.push(analyze_service(prog, mi, decl, s, &mut routes, diags));
            }
        }
    }
    prog.services = services;
}

struct Ctx<'p, 'a> {
    prog: &'p Program<'a>,
    module: usize,
}

fn analyze_service(
    prog: &Program<'_>,
    module: usize,
    decl: &ast::Decl,
    s: &ast::ServiceDecl,
    routes: &mut BTreeMap<RouteKey, (Span, String)>,
    diags: &mut Vec<Diagnostic>,
) -> ServiceInfo {
    let anns = annotations::parse(&decl.annotations, Target::Service, None, diags);
    let prefix = service_prefix(
        &prog.packages[prog.modules[module].pkg].prefix,
        &s.prefix,
        diags,
    );
    let auth = anns.auth.clone().unwrap_or_else(Auth::none);
    let middleware = anns.middleware.clone().unwrap_or_default();
    let ctx = Ctx { prog, module };
    let pkg = prog.modules[module].pkg;
    let mut endpoints: Vec<REndpoint> = Vec::new();
    for e in &s.endpoints {
        if let Some(prev) = endpoints.iter().find(|p| p.name == e.name.name) {
            diags.push(
                Diagnostic::error(
                    "E0509",
                    format!(
                        "endpoint `{}` is declared twice in service `{}`",
                        e.name.name, s.name.name
                    ),
                    e.name.span,
                )
                .with_label(prev.span, "first declared here"),
            );
            continue;
        }
        if let Some(ep) = ctx.endpoint(e, &prefix, &auth, &middleware, diags) {
            check_route(prog, pkg, &s.name.name, &ep, routes, diags);
            endpoints.push(ep);
        }
    }
    let info = ServiceInfo {
        pkg,
        module,
        name: s.name.name.clone(),
        prefix,
        docs: doc_text(&decl.docs),
        auth,
        middleware,
        endpoints,
        span: s.name.span,
    };
    rules::check_service_facts(&info, diags);
    info
}

fn service_prefix(base: &str, p: &ast::StrLit, diags: &mut Vec<Diagnostic>) -> String {
    if !p.value.starts_with('/') {
        diags.push(
            Diagnostic::error(
                "E0501",
                format!("service prefix `{}` must start with `/`", p.value),
                p.span,
            )
            .with_help(format!("write \"/{}\"", p.value)),
        );
        return "/".to_string();
    }
    normalize_path(&format!("{base}{}", p.value))
}

/// Collapses repeated slashes and drops a trailing slash (except on the root).
pub(super) fn normalize_path(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.chars() {
        if c == '/' && out.ends_with('/') {
            continue;
        }
        out.push(c);
    }
    while out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    if out.is_empty() { "/".to_string() } else { out }
}

/// Parameter names of a `{name}` template, or a description of what is malformed.
pub(super) fn template_params(path: &str) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    let mut rest = path;
    while let Some(i) = rest.find(['{', '}']) {
        if rest[i..].starts_with('}') {
            return Err("unmatched `}` in path".to_string());
        }
        let after = &rest[i + 1..];
        let Some(end) = after.find('}') else {
            return Err("unclosed `{` in path".to_string());
        };
        let name = &after[..end];
        if !language::is_valid_ident(name) {
            return Err(format!("`{{{name}}}` is not a valid path parameter name"));
        }
        if names.iter().any(|n| n == name) {
            return Err(format!("path parameter `{{{name}}}` appears twice"));
        }
        names.push(name.to_string());
        rest = &after[end + 1..];
    }
    Ok(names)
}

fn erase_params(path: &str) -> String {
    let mut out = String::new();
    let mut rest = path;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        out.push_str("{}");
        rest = rest[i..].find('}').map_or("", |e| &rest[i + e + 1..]);
    }
    out.push_str(rest);
    out
}

fn check_route(
    prog: &Program<'_>,
    pkg: usize,
    svc: &str,
    ep: &REndpoint,
    routes: &mut BTreeMap<RouteKey, (Span, String)>,
    diags: &mut Vec<Diagnostic>,
) {
    let key = (pkg, ep.method, erase_params(&ep.path));
    let here = format!("{svc}.{}", ep.name);
    if let Some((prev_span, prev_name)) = routes.get(&key) {
        diags.push(
            Diagnostic::error(
                "E0510",
                format!(
                    "{} {} collides with {prev_name} in package `{}`",
                    ep.method.as_str(),
                    ep.path,
                    prog.packages[pkg].name
                ),
                ep.span,
            )
            .with_label(*prev_span, "same method and path declared here")
            .with_note(
                "paths are compared with parameter names erased: `/a/{x}` and `/a/{y}` collide",
            ),
        );
        return;
    }
    routes.insert(key, (ep.span, here));
}

fn to_method(m: HttpMethod) -> Method {
    match m {
        HttpMethod::Get => Method::Get,
        HttpMethod::Post => Method::Post,
        HttpMethod::Put => Method::Put,
        HttpMethod::Patch => Method::Patch,
        HttpMethod::Delete => Method::Delete,
        HttpMethod::Head => Method::Head,
        HttpMethod::Options => Method::Options,
        HttpMethod::Ws => Method::Ws,
    }
}

impl Ctx<'_, '_> {
    fn endpoint(
        &self,
        e: &ast::Endpoint,
        prefix: &str,
        svc_auth: &Auth,
        svc_mw: &[String],
        diags: &mut Vec<Diagnostic>,
    ) -> Option<REndpoint> {
        let anns = annotations::parse(&e.annotations, Target::Endpoint, None, diags);
        let method = to_method(e.method);
        let template = self.path_template(e, diags)?;
        let path = normalize_path(&format!("{prefix}{}", e.path.value));
        let params = self.params(e, method, &template, diags);
        let mut ep = REndpoint {
            name: e.name.name.clone(),
            method,
            path,
            docs: doc_text(&e.docs),
            deprecated: anns.deprecated.clone(),
            auth: anns.auth.clone().unwrap_or_else(|| svc_auth.clone()),
            middleware: merge_middleware(svc_mw, anns.middleware.as_deref()),
            params,
            success_status: 204,
            success_ty: None,
            success_stream: false,
            errors: Vec::new(),
            rules: RBlock::default(),
            span: e.name.span,
        };
        self.responses(e, method, &mut ep, diags);
        self.implicit_errors(&mut ep, e);
        if e.block.is_some() {
            ep.rules = rules::check_block(self.prog, self.module, e, &ep, diags);
        }
        Some(ep)
    }

    fn path_template(&self, e: &ast::Endpoint, diags: &mut Vec<Diagnostic>) -> Option<Vec<String>> {
        let p = &e.path;
        if !p.value.starts_with('/') {
            diags.push(
                Diagnostic::error(
                    "E0501",
                    format!("endpoint path `{}` must start with `/`", p.value),
                    p.span,
                )
                .with_help(format!("write \"/{}\"", p.value)),
            );
            return None;
        }
        match template_params(&p.value) {
            Ok(v) => Some(v),
            Err(msg) => {
                diags.push(Diagnostic::error("E0501", msg, p.span));
                None
            }
        }
    }

    // ---- parameters -------------------------------------------------------------

    fn params(
        &self,
        e: &ast::Endpoint,
        method: Method,
        template: &[String],
        diags: &mut Vec<Diagnostic>,
    ) -> Vec<RParam> {
        let mut out: Vec<RParam> = Vec::new();
        for p in &e.params {
            if out.iter().any(|q| q.name == p.name.name) {
                diags.push(Diagnostic::error(
                    "E0512",
                    format!("parameter `{}` is declared twice", p.name.name),
                    p.name.span,
                ));
                continue;
            }
            let Some(ty) = self.prog.resolve_type(self.module, &p.ty, false, diags) else {
                continue;
            };
            let anns = annotations::parse(&p.annotations, Target::Param, Some(&ty), diags);
            if let Some(rp) = self.bind(p, ty, &anns, method, template, diags) {
                out.push(rp);
            }
        }
        for name in template {
            if !e.params.iter().any(|p| &p.name.name == name) {
                diags.push(
                    Diagnostic::error(
                        "E0506",
                        format!("path parameter `{{{name}}}` has no matching parameter"),
                        e.path.span,
                    )
                    .with_help(format!("add `{name}: <type>` to the parameter list")),
                );
            }
        }
        out
    }

    fn bind(
        &self,
        p: &ast::Param,
        ty: Type,
        anns: &Anns<'_>,
        method: Method,
        template: &[String],
        diags: &mut Vec<Diagnostic>,
    ) -> Option<RParam> {
        let name = p.name.name.as_str();
        let (binding, header) = if template.iter().any(|t| t == name) {
            self.check_path(p, &ty, diags)?;
            (Binding::Path, None)
        } else if name == "body" {
            if !method.allows_body() {
                diags.push(Diagnostic::error(
                    "E0503",
                    format!("`body` is not allowed on `{}` endpoints", method.lower()),
                    p.name.span,
                ));
                return None;
            }
            (Binding::Body, None)
        } else if name == "query" {
            self.check_query_object(p, &ty, diags)?;
            (Binding::QueryObject, None)
        } else if anns.query.is_some() {
            self.check_single_query(p, &ty, diags)?;
            (Binding::Query, None)
        } else if let Some((h, _)) = &anns.header {
            self.check_header(p, &ty, diags)?;
            (Binding::Header, Some(h.clone()))
        } else {
            diags.push(unbound(p));
            return None;
        };
        self.check_conflicts(p, binding, anns, diags)?;
        self.check_stream(p, binding, method, diags)?;
        Some(RParam {
            name: p.name.name.clone(),
            binding,
            header,
            ty,
            optional: p.optional,
            stream: p.stream,
            constraints: anns.constraints.clone(),
        })
    }

    fn check_conflicts(
        &self,
        p: &ast::Param,
        binding: Binding,
        anns: &Anns<'_>,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<()> {
        let conflict =
            (anns.query.is_some() && binding != Binding::Query && binding != Binding::QueryObject)
                || (anns.header.is_some() && binding != Binding::Header);
        if conflict {
            let by = match binding {
                Binding::Path => "its name appears in the path",
                Binding::Body => "it is named `body`",
                Binding::Query | Binding::QueryObject | Binding::Header => "it is named `query`",
            };
            diags.push(Diagnostic::error(
                "E0602",
                format!(
                    "binding annotation on `{}` conflicts with its binding: {by}",
                    p.name.name
                ),
                p.name.span,
            ));
            return None;
        }
        Some(())
    }

    fn check_stream(
        &self,
        p: &ast::Param,
        binding: Binding,
        method: Method,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<()> {
        if p.stream && !(binding == Binding::Body && method == Method::Ws) {
            diags.push(Diagnostic::error(
                "E0511",
                "`stream` parameters are only allowed as `body: stream T` on `ws` endpoints",
                p.name.span,
            ));
            return None;
        }
        Some(())
    }

    fn is_simple(&self, ty: &Type) -> bool {
        match ty {
            Type::Scalar(_) => true,
            Type::Decl(d) => self.prog.decls[d.index()].kind == DKind::Enum,
            Type::List(_) | Type::Map(_) | Type::Page(_) => false,
        }
    }

    fn is_simple_or_list(&self, ty: &Type) -> bool {
        match ty {
            Type::List(inner) => self.is_simple(inner),
            Type::Scalar(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => self.is_simple(ty),
        }
    }

    fn check_path(&self, p: &ast::Param, ty: &Type, diags: &mut Vec<Diagnostic>) -> Option<()> {
        if p.optional {
            diags.push(Diagnostic::error(
                "E0502",
                format!("path parameter `{}` cannot be optional", p.name.name),
                p.name.span,
            ));
            return None;
        }
        if !self.is_simple(ty) {
            diags.push(Diagnostic::error(
                "E0502",
                format!(
                    "path parameter `{}` must be a scalar or an enum, found `{}`",
                    p.name.name,
                    self.prog.type_name(ty)
                ),
                p.ty.span,
            ));
            return None;
        }
        Some(())
    }

    fn check_query_object(
        &self,
        p: &ast::Param,
        ty: &Type,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<()> {
        let bad = |msg: String, diags: &mut Vec<Diagnostic>| {
            diags.push(Diagnostic::error("E0504", msg, p.ty.span).with_help(
                "`query` must be a model whose fields are scalars, enums or lists of them",
            ));
        };
        let Type::Decl(d) = ty else {
            bad(
                format!(
                    "`query` must be a model, found `{}`",
                    self.prog.type_name(ty)
                ),
                diags,
            );
            return None;
        };
        let decl = &self.prog.decls[d.index()];
        if decl.kind != DKind::Model {
            bad(
                format!("`query` must be a model, but `{}` is not", decl.name),
                diags,
            );
            return None;
        }
        if let Some(f) = decl
            .res
            .fields
            .iter()
            .find(|f| !self.is_simple_or_list(&f.ty))
        {
            bad(
                format!(
                    "field `{}` of `{}` has type `{}`, which cannot be a query parameter",
                    f.name,
                    decl.name,
                    self.prog.type_name(&f.ty)
                ),
                diags,
            );
            return None;
        }
        Some(())
    }

    fn check_single_query(
        &self,
        p: &ast::Param,
        ty: &Type,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<()> {
        if self.is_simple_or_list(ty) {
            return Some(());
        }
        diags.push(Diagnostic::error(
            "E0504",
            format!(
                "`@query` parameter `{}` must be a scalar, enum or list of them, found `{}`",
                p.name.name,
                self.prog.type_name(ty)
            ),
            p.ty.span,
        ));
        None
    }

    fn check_header(&self, p: &ast::Param, ty: &Type, diags: &mut Vec<Diagnostic>) -> Option<()> {
        if self.is_simple(ty) {
            return Some(());
        }
        diags.push(Diagnostic::error(
            "E0505",
            format!(
                "header parameter `{}` must be a scalar or an enum, found `{}`",
                p.name.name,
                self.prog.type_name(ty)
            ),
            p.ty.span,
        ));
        None
    }

    // ---- responses ----------------------------------------------------------------

    fn responses(
        &self,
        e: &ast::Endpoint,
        method: Method,
        ep: &mut REndpoint,
        diags: &mut Vec<Diagnostic>,
    ) {
        let Some(r) = &e.responses else { return };
        for (idx, item) in r.items.iter().enumerate() {
            let ty = item
                .ty
                .as_ref()
                .and_then(|t| self.prog.resolve_type(self.module, t, true, diags));
            if item.ty.is_some() && ty.is_none() {
                continue;
            }
            match self.error_decl(ty.as_ref()) {
                Some(decl) => self.error_item(item, decl, ep, diags),
                None if idx == 0 => self.success_item(item, ty, method, ep, diags),
                None => diags.push(
                    Diagnostic::error(
                        "E0405",
                        "only `error` declarations may follow the success response",
                        item.span,
                    )
                    .with_help("write the success response first: `-> Success | ErrorA | ErrorB`"),
                ),
            }
        }
    }

    fn error_decl(&self, ty: Option<&Type>) -> Option<usize> {
        match ty? {
            Type::Decl(d) if self.prog.decls[d.index()].kind == DKind::Error => Some(d.index()),
            Type::Decl(_) | Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) => None,
        }
    }

    fn error_item(
        &self,
        item: &ast::RespItem,
        decl: usize,
        ep: &mut REndpoint,
        diags: &mut Vec<Diagnostic>,
    ) {
        let d = &self.prog.decls[decl];
        if item.stream {
            diags.push(Diagnostic::error(
                "E0511",
                "`stream` is not allowed on an error response",
                item.span,
            ));
            return;
        }
        let status = match (item.status, d.res.default_status) {
            (Some((n, span)), _) => match u16::try_from(n) {
                Ok(s) if (400..=599).contains(&s) => s,
                _ => {
                    diags.push(Diagnostic::error(
                        "E0406",
                        format!("error status {n} is not 4xx or 5xx"),
                        span,
                    ));
                    return;
                }
            },
            (None, Some(s)) => s,
            (None, None) => {
                diags.push(
                    Diagnostic::error(
                        "E0508",
                        format!(
                            "error `{}` has no status here and no default status",
                            d.name
                        ),
                        item.span,
                    )
                    .with_help(format!(
                        "write `<status> {}` or give the error a default: `error {} 409 {{ ... }}`",
                        d.name, d.name
                    )),
                );
                return;
            }
        };
        if !ep.errors.contains(&(status, decl)) {
            ep.errors.push((status, decl));
        }
    }

    fn success_item(
        &self,
        item: &ast::RespItem,
        ty: Option<Type>,
        method: Method,
        ep: &mut REndpoint,
        diags: &mut Vec<Diagnostic>,
    ) {
        let status = match item.status {
            Some((n, span)) => match u16::try_from(n) {
                Ok(s) if (200..=299).contains(&s) => Some(s),
                _ => {
                    diags.push(
                        Diagnostic::error("E0507", format!("success status must be 2xx, found {n}"), span)
                            .with_help("error responses are listed after the success response and must be `error` declarations"),
                    );
                    None
                }
            },
            None => None,
        };
        if item.stream && !matches!(method, Method::Get | Method::Ws) {
            diags.push(Diagnostic::error("E0511", format!("`stream` responses are only allowed on `get` (SSE) and `ws` endpoints, not `{}`", method.lower()), item.span));
        }
        if item.status.is_some() && status.is_none() {
            return;
        }
        ep.success_status = status.unwrap_or(if method == Method::Post { 201 } else { 200 });
        ep.success_ty = ty;
        ep.success_stream = item.stream;
    }

    /// Adds the implicit errors every server response needs a declaration for:
    /// `ValidationError` (when the endpoint has parameters), `Unauthorized`
    /// (when auth is not `none`) and `Internal` (always), unless already listed.
    fn implicit_errors(&self, ep: &mut REndpoint, e: &ast::Endpoint) {
        // A `require` without `else` answers `ValidationError`, also when nothing is bound.
        if !ep.params.is_empty() || rules::has_bare_require(e) {
            self.add_implicit(ep, "ValidationError", 422);
        }
        // A required fact the loader cannot find answers `404 NotFound` (`spec/rules.md` 9.2).
        if rules::has_required_fact(e) {
            self.add_implicit(ep, "NotFound", 404);
        }
        if ep.auth.kind != veld_ir::AuthKind::None {
            self.add_implicit(ep, "Unauthorized", 401);
        }
        self.add_implicit(ep, "Internal", 500);
    }

    fn add_implicit(&self, ep: &mut REndpoint, name: &str, status: u16) {
        let Some(&v) = self.prog.prelude.get(name) else {
            return;
        };
        let listed = ep
            .errors
            .iter()
            .any(|(_, d)| self.prog.decls[*d].name == name);
        if !listed {
            ep.errors.push((status, v));
        }
    }
}

fn merge_middleware(svc: &[String], own: Option<&[String]>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for m in svc.iter().chain(own.unwrap_or(&[])) {
        if !out.contains(m) {
            out.push(m.clone());
        }
    }
    out
}

fn unbound(p: &ast::Param) -> Diagnostic {
    Diagnostic::error("E0505", format!("parameter `{}` has no binding", p.name.name), p.name.span).with_help(
        "a parameter is bound by (1) appearing as `{name}` in the path, (2) being named `body`, (3) being named `query` (a model), \
         (4) `@query`, or (5) `@header(\"Name\")`",
    )
}
