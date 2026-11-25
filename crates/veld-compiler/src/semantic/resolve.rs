//! Pass 2 and 3: scopes (imports, prelude) and resolution of declarations.
//!
//! Resolution is two-phase: `collect` registered every declaration of every
//! module first, so imports may form cycles and types may refer to each other.

use std::collections::BTreeMap;

use super::annotations::{self, Anns};
use super::{DKind, DeclInfo, Entry, ImportRec, Program, RArm, RField, RVariant, Resolved};
use crate::diagnostics::{Diagnostic, Span};
use crate::language::Target;
use crate::syntax::ast::{self, DeclKind, LitKind, TypeExpr, TypeExprKind, doc_text};
use veld_ir::{Literal, Scalar, Type};

type LitError = (&'static str, String);

// ---- scopes ------------------------------------------------------------------

pub fn build_scopes(prog: &mut Program<'_>, diags: &mut Vec<Diagnostic>) {
    let mut by_name: BTreeMap<(usize, String), usize> = BTreeMap::new();
    for (i, d) in prog.decls.iter().enumerate() {
        by_name.insert((d.module, d.name.clone()), i);
    }
    for mi in 0..prog.modules.len() {
        let mut scope = std::mem::take(&mut prog.scopes[mi]);
        let mut spans: BTreeMap<String, Span> = BTreeMap::new();
        for (i, d) in prog
            .decls
            .iter()
            .enumerate()
            .filter(|(_, d)| d.module == mi)
        {
            scope.names.insert(
                d.name.clone(),
                Entry {
                    decl: i,
                    import: None,
                },
            );
            spans.insert(d.name.clone(), d.name_span);
            warn_shadow(prog, &d.name, d.name_span, mi, diags);
        }
        let ast = prog.modules[mi].ast;
        for u in &ast.uses {
            import_use(prog, mi, u, &by_name, &mut scope, &mut spans, diags);
        }
        prog.scopes[mi] = scope;
    }
}

fn warn_shadow(
    prog: &Program<'_>,
    name: &str,
    span: Span,
    module: usize,
    diags: &mut Vec<Diagnostic>,
) {
    if prog.prelude.contains_key(name) && prog.modules[module].pkg != prog.prelude_pkg {
        diags.push(Diagnostic::warning(
            "W0202",
            format!("`{name}` shadows the prelude declaration of the same name"),
            span,
        ));
    }
}

fn import_use(
    prog: &Program<'_>,
    mi: usize,
    u: &ast::Use,
    by_name: &BTreeMap<(usize, String), usize>,
    scope: &mut super::Scope,
    spans: &mut BTreeMap<String, Span>,
    diags: &mut Vec<Diagnostic>,
) {
    let pkg = match &u.package {
        None => Some(prog.modules[mi].pkg),
        Some(id) => {
            let found = prog.packages.iter().position(|p| p.name == id.name);
            if found.is_none() {
                let known: Vec<&str> = prog
                    .packages
                    .iter()
                    .filter(|p| p.kind != veld_ir::PackageKind::Prelude)
                    .map(|p| p.name.as_str())
                    .collect();
                diags.push(
                    Diagnostic::error("E0203", format!("unknown package `{}`", id.name), id.span)
                        .did_you_mean(
                            &id.name,
                            known.iter().copied(),
                            Some(format!("known packages: {}", known.join(", "))),
                        ),
                );
            }
            found
        }
    };
    let poison_all = |scope: &mut super::Scope| {
        for imp in &u.imports {
            scope
                .poisoned
                .insert(imp.alias.as_ref().unwrap_or(&imp.name).name.clone());
        }
    };
    let Some(pkg) = pkg else {
        poison_all(scope);
        return;
    };
    let path = u.module_path();
    let Some(&target_mod) = prog.module_index.get(&(pkg, path.clone())) else {
        poison_all(scope);
        let span = u
            .module
            .first()
            .map_or(u.span, |f| f.span)
            .to(u.module.last().map_or(u.span, |l| l.span));
        let mods: Vec<&str> = prog
            .module_index
            .keys()
            .filter(|(p, _)| *p == pkg)
            .map(|(_, m)| m.as_str())
            .collect();
        diags.push(
            Diagnostic::error(
                "E0204",
                format!(
                    "package `{}` has no module `{path}`",
                    prog.packages[pkg].name
                ),
                span,
            )
            .did_you_mean(
                &path,
                mods.iter().copied(),
                Some(format!(
                    "modules in `{}`: {}",
                    prog.packages[pkg].name,
                    mods.join(", ")
                )),
            ),
        );
        return;
    };
    for imp in &u.imports {
        let Some(&decl) = by_name.get(&(target_mod, imp.name.name.clone())) else {
            scope
                .poisoned
                .insert(imp.alias.as_ref().unwrap_or(&imp.name).name.clone());
            diags.push(
                Diagnostic::error(
                    "E0205",
                    format!("module `{path}` has no declaration `{}`", imp.name.name),
                    imp.name.span,
                )
                .did_you_mean(
                    &imp.name.name,
                    prog.decls
                        .iter()
                        .filter(|d| d.module == target_mod)
                        .map(|d| d.name.as_str()),
                    Some(declared_in(prog, target_mod)),
                ),
            );
            continue;
        };
        let local = imp.alias.as_ref().unwrap_or(&imp.name);
        if let Some(prev) = spans.get(&local.name) {
            diags.push(
                Diagnostic::error(
                    "E0202",
                    format!(
                        "imported name `{}` collides with another name in this module",
                        local.name
                    ),
                    local.span,
                )
                .with_label(*prev, "already declared or imported here")
                .with_help("rename it with `as`: `use mod.{Name as Other}`"),
            );
            continue;
        }
        spans.insert(local.name.clone(), local.span);
        warn_shadow(prog, &local.name, local.span, mi, diags);
        scope.imports.push(ImportRec {
            local: local.name.clone(),
            span: imp.span,
            used: std::cell::Cell::new(false),
        });
        scope.names.insert(
            local.name.clone(),
            Entry {
                decl,
                import: Some(scope.imports.len() - 1),
            },
        );
    }
}

fn declared_in(prog: &Program<'_>, module: usize) -> String {
    let names: Vec<&str> = prog
        .decls
        .iter()
        .filter(|d| d.module == module)
        .map(|d| d.name.as_str())
        .collect();
    format!(
        "declared there: {}",
        if names.is_empty() {
            "(nothing)".to_string()
        } else {
            names.join(", ")
        }
    )
}

/// `W0201` for imports that no reference used.
pub fn report_unused_imports(prog: &Program<'_>, diags: &mut Vec<Diagnostic>) {
    for scope in &prog.scopes {
        for imp in scope.imports.iter().filter(|i| !i.used.get()) {
            diags.push(Diagnostic::warning(
                "W0201",
                format!("unused import `{}`", imp.local),
                imp.span,
            ));
        }
    }
}

// ---- type resolution -----------------------------------------------------------

impl Program<'_> {
    /// Human-readable type for messages.
    #[must_use]
    pub fn type_name(&self, t: &Type) -> String {
        match t {
            Type::Scalar(s) => s.name().to_string(),
            Type::List(i) => format!("{}[]", self.type_name(i)),
            Type::Map(i) => format!("Map<string, {}>", self.type_name(i)),
            Type::Page(i) => format!("Page<{}>", self.type_name(i)),
            Type::Decl(d) => self
                .decls
                .get(d.index())
                .map_or_else(|| "?".to_string(), |d| d.name.clone()),
        }
    }

    /// Every name a type position can mention here, for suggestions.
    fn visible_type_names(&self, module: usize) -> Vec<&str> {
        let mut v: Vec<&str> = self.scopes[module]
            .names
            .keys()
            .map(String::as_str)
            .collect();
        v.extend(self.prelude.keys().map(String::as_str));
        v.extend(crate::language::SCALARS);
        v.extend(crate::language::GENERICS);
        v
    }

    fn suggest_import(&self, module: usize, name: &str) -> Option<String> {
        let here = self.modules[module].pkg;
        let hit = self
            .decls
            .iter()
            .filter(|d| d.name == name && d.pkg != self.prelude_pkg)
            .min_by_key(|d| (d.pkg != here, d.qualified.clone()))?;
        Some(self.import_line(here, hit))
    }

    /// `add `use pkg:module.{Name}``, for a declaration of some other module.
    fn import_line(&self, here: usize, hit: &DeclInfo<'_>) -> String {
        let m = &self.modules[hit.module];
        let pkg = if hit.pkg == here {
            String::new()
        } else {
            format!("{}:", self.packages[hit.pkg].name)
        };
        format!("add `use {pkg}{}.{{{}}}`", m.path, hit.name)
    }

    /// A declaration of another module whose name is a near miss of `name` (`Usr` for `User`):
    /// `did you mean `User`? add `use ...``.
    fn suggest_near_import(&self, module: usize, name: &str) -> Option<String> {
        let here = self.modules[module].pkg;
        let near = veld_ir::suggest::closest(
            name,
            self.decls
                .iter()
                .filter(|d| d.pkg != self.prelude_pkg)
                .map(|d| d.name.as_str()),
        )?;
        let hit = self
            .decls
            .iter()
            .filter(|d| d.name == near && d.pkg != self.prelude_pkg)
            .min_by_key(|d| (d.pkg != here, d.qualified.clone()))?;
        Some(format!(
            "did you mean `{near}`? {}",
            self.import_line(here, hit)
        ))
    }

    /// Resolves a syntactic type. `allow_error` permits error declarations.
    pub(super) fn resolve_type(
        &self,
        module: usize,
        t: &TypeExpr,
        allow_error: bool,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<Type> {
        match &t.kind {
            TypeExprKind::List(inner) => Some(Type::List(Box::new(self.resolve_type(
                module,
                inner,
                allow_error,
                diags,
            )?))),
            TypeExprKind::Named { name, args } => {
                self.resolve_named(module, name, args, t.span, allow_error, diags)
            }
        }
    }

    fn resolve_named(
        &self,
        module: usize,
        name: &ast::Ident,
        args: &[TypeExpr],
        span: Span,
        allow_error: bool,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<Type> {
        match name.name.as_str() {
            "Map" => return self.resolve_map(module, args, span, diags),
            "Page" => {
                let [item] = args else {
                    diags.push(Diagnostic::error(
                        "E0302",
                        "`Page` takes exactly one type argument: `Page<T>`",
                        span,
                    ));
                    return None;
                };
                return Some(Type::Page(Box::new(
                    self.resolve_type(module, item, false, diags)?,
                )));
            }
            n => {
                if let Some(s) = Scalar::from_name(n) {
                    if !args.is_empty() {
                        diags.push(Diagnostic::error(
                            "E0302",
                            format!("scalar `{n}` takes no type arguments"),
                            span,
                        ));
                        return None;
                    }
                    return Some(Type::Scalar(s));
                }
            }
        }
        let Some(decl) = self.lookup(module, &name.name) else {
            if self.scopes[module].poisoned.contains(&name.name) {
                return None;
            }
            let mut d =
                Diagnostic::error("E0205", format!("unknown type `{}`", name.name), name.span);
            d = match self.suggest_import(module, &name.name) {
                Some(h) => d.with_help(h),
                None => {
                    match veld_ir::suggest::closest(&name.name, self.visible_type_names(module)) {
                        Some(s) => d.with_help(format!("did you mean `{s}`?")),
                        None => match self.suggest_near_import(module, &name.name) {
                            Some(h) => d.with_help(h),
                            None => d,
                        },
                    }
                }
            };
            diags.push(d);
            return None;
        };
        if !args.is_empty() {
            diags.push(
                Diagnostic::error("E0302", format!("`{}` is not generic", name.name), span)
                    .with_help("only `Map<string, T>` and `Page<T>` take type arguments"),
            );
            return None;
        }
        if self.decls[decl].kind == DKind::Error && !allow_error {
            diags.push(error_position(&name.name, name.span));
            return None;
        }
        Some(Type::Decl(veld_ir::DeclId(u32::try_from(decl).ok()?)))
    }

    fn resolve_map(
        &self,
        module: usize,
        args: &[TypeExpr],
        span: Span,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<Type> {
        let [key, value] = args else {
            diags.push(Diagnostic::error(
                "E0302",
                "`Map` takes two type arguments: `Map<string, T>`",
                span,
            ));
            return None;
        };
        let key_ty = self.resolve_type(module, key, false, diags)?;
        if key_ty != Type::Scalar(Scalar::String) {
            diags.push(Diagnostic::error(
                "E0301",
                format!(
                    "map keys must be `string`, found `{}`",
                    self.type_name(&key_ty)
                ),
                key.span,
            ));
            return None;
        }
        Some(Type::Map(Box::new(
            self.resolve_type(module, value, false, diags)?,
        )))
    }

    /// Enum variants of declaration `d` as (name, wire), straight from the syntax.
    fn enum_variants(&self, d: usize) -> Vec<(String, String)> {
        match &self.decls[d].ast.kind {
            DeclKind::Enum(e) => e
                .variants
                .iter()
                .map(|v| {
                    (
                        v.name.name.clone(),
                        v.wire
                            .as_ref()
                            .map_or_else(|| v.name.name.clone(), |w| w.0.clone()),
                    )
                })
                .collect(),
            DeclKind::Model(_)
            | DeclKind::Error(_)
            | DeclKind::Union(_)
            | DeclKind::Service(_)
            | DeclKind::Const(_) => Vec::new(),
        }
    }

    /// Checks that `lit` is a valid value of `ty` and converts it to an IR literal.
    pub(super) fn check_literal(&self, ty: &Type, lit: &ast::Literal) -> Result<Literal, LitError> {
        match ty {
            Type::Scalar(s) => scalar_literal(*s, &lit.kind),
            Type::List(_) if lit.kind == LitKind::EmptyList => Ok(Literal::EmptyList),
            Type::Decl(d) if self.decls[d.index()].kind == DKind::Enum => {
                self.enum_literal(d.index(), &lit.kind)
            }
            Type::List(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => Err((
                "E0303",
                format!(
                    "a literal cannot be used as a value of type `{}`",
                    self.type_name(ty)
                ),
            )),
        }
    }

    fn enum_literal(&self, d: usize, kind: &LitKind) -> Result<Literal, LitError> {
        let LitKind::Variant(name) = kind else {
            return Err((
                "E0303",
                format!(
                    "expected an enum variant like `.{}`",
                    self.enum_variants(d).first().map_or("x", |v| v.0.as_str())
                ),
            ));
        };
        match self.enum_variants(d).into_iter().find(|v| &v.0 == name) {
            Some((name, wire)) => Ok(Literal::EnumVariant { name, wire }),
            None => {
                let all: Vec<String> = self.enum_variants(d).into_iter().map(|v| v.0).collect();
                let hint = veld_ir::suggest::closest(name, all.iter().map(String::as_str))
                    .map_or_else(String::new, |s| format!("; did you mean `{s}`?"));
                Err((
                    "E0304",
                    format!(
                        "enum `{}` has no variant `{name}` (variants: {}){hint}",
                        self.decls[d].name,
                        all.join(", ")
                    ),
                ))
            }
        }
    }
}

fn error_position(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "E0405",
        format!("error `{name}` can only be used in the response position of an endpoint"),
        span,
    )
}

fn lit_kind_name(k: &LitKind) -> &'static str {
    match k {
        LitKind::Str(_) => "a string",
        LitKind::Int(_) => "an integer",
        LitKind::Float(_) => "a float",
        LitKind::Bool(_) => "a bool",
        LitKind::Null => "null",
        LitKind::Variant(_) => "an enum variant",
        LitKind::EmptyList => "an empty list",
    }
}

#[allow(clippy::cast_precision_loss)]
fn scalar_literal(s: Scalar, k: &LitKind) -> Result<Literal, LitError> {
    let mismatch = || {
        (
            "E0303",
            format!("expected a `{s}` value, found {}", lit_kind_name(k)),
        )
    };
    match (s, k) {
        (
            Scalar::String
            | Scalar::Bytes
            | Scalar::Uuid
            | Scalar::Date
            | Scalar::Datetime
            | Scalar::Time,
            LitKind::Str(v),
        ) => Ok(Literal::String(v.clone())),
        (Scalar::Int, LitKind::Int(n)) => {
            if i32::try_from(*n).is_ok() {
                Ok(Literal::Int(*n))
            } else {
                Err(("E0303", format!("`{n}` does not fit in a 32-bit `int`")))
            }
        }
        (Scalar::Long, LitKind::Int(n)) => Ok(Literal::Int(*n)),
        (Scalar::Float, LitKind::Int(n)) => Ok(Literal::Float(*n as f64)),
        (Scalar::Float, LitKind::Float(f)) => Ok(Literal::Float(*f)),
        (Scalar::Decimal, LitKind::Str(v)) => Ok(Literal::String(v.clone())),
        (Scalar::Decimal, LitKind::Int(n)) => Ok(Literal::String(n.to_string())),
        (Scalar::Decimal, LitKind::Float(f)) => Ok(Literal::String(f.to_string())),
        (Scalar::Bool, LitKind::Bool(b)) => Ok(Literal::Bool(*b)),
        (Scalar::Json, LitKind::Str(v)) => Ok(Literal::String(v.clone())),
        (Scalar::Json, LitKind::Int(n)) => Ok(Literal::Int(*n)),
        (Scalar::Json, LitKind::Float(f)) => Ok(Literal::Float(*f)),
        (Scalar::Json, LitKind::Bool(b)) => Ok(Literal::Bool(*b)),
        (Scalar::Json, LitKind::Null) => Ok(Literal::Null),
        (Scalar::Json, LitKind::EmptyList) => Ok(Literal::EmptyList),
        _ => Err(mismatch()),
    }
}

// ---- declarations -----------------------------------------------------------------

pub fn resolve_decls(prog: &mut Program<'_>, diags: &mut Vec<Diagnostic>) {
    let results: Vec<Resolved> = (0..prog.decls.len())
        .map(|i| resolve_one(prog, i, diags))
        .collect();
    for (d, r) in prog.decls.iter_mut().zip(results) {
        d.res = r;
    }
}

fn resolve_one(prog: &Program<'_>, i: usize, diags: &mut Vec<Diagnostic>) -> Resolved {
    let d = &prog.decls[i];
    let module = d.module;
    let mut res = Resolved::default();
    let target = match d.kind {
        DKind::Model => Target::Model,
        DKind::Error => Target::Error,
        DKind::Enum => Target::Enum,
        DKind::Union => Target::Union,
        DKind::Const => Target::Const,
    };
    let anns = annotations::parse(&d.ast.annotations, target, None, diags);
    res.deprecated = anns.deprecated.clone();
    match &d.ast.kind {
        DeclKind::Model(m) => {
            res.parent = m
                .extends
                .as_ref()
                .and_then(|e| resolve_parent(prog, module, e, diags));
            res.own_fields = resolve_fields(prog, module, &m.fields, diags);
        }
        DeclKind::Error(e) => {
            res.default_status = error_status(e, diags);
            res.own_fields = resolve_fields(prog, module, &e.fields, diags);
        }
        DeclKind::Enum(e) => {
            res.variants = resolve_variants(e, diags);
            res.machine = super::machine::resolve_machine(e, &res.variants, diags);
        }
        DeclKind::Union(u) => {
            res.discriminator = anns
                .discriminator
                .clone()
                .unwrap_or_else(|| "kind".to_string());
            res.arms = resolve_arms(prog, module, u, diags);
        }
        DeclKind::Const(c) => resolve_const(prog, module, c, &mut res, diags),
        DeclKind::Service(_) => {}
    }
    res
}

fn resolve_parent(
    prog: &Program<'_>,
    module: usize,
    e: &TypeExpr,
    diags: &mut Vec<Diagnostic>,
) -> Option<usize> {
    let Type::Decl(id) = prog.resolve_type(module, e, true, diags)? else {
        diags.push(Diagnostic::error(
            "E0408",
            "`extends` needs a model name",
            e.span,
        ));
        return None;
    };
    let parent = id.index();
    if prog.decls[parent].kind == DKind::Model {
        Some(parent)
    } else {
        let kind = if prog.decls[parent].kind == DKind::Error {
            "an error"
        } else {
            "not a model"
        };
        diags.push(Diagnostic::error(
            "E0408",
            format!(
                "`{}` is {kind}; `extends` takes exactly one model",
                prog.decls[parent].name
            ),
            e.span,
        ));
        None
    }
}

fn error_status(e: &ast::ErrorDecl, diags: &mut Vec<Diagnostic>) -> Option<u16> {
    let (n, span) = e.status?;
    match u16::try_from(n) {
        Ok(s) if (400..=599).contains(&s) => Some(s),
        _ => {
            diags.push(Diagnostic::error(
                "E0406",
                format!(
                    "error `{}` has default status {n}, which is not 4xx or 5xx",
                    e.name.name
                ),
                span,
            ));
            None
        }
    }
}

fn resolve_fields(
    prog: &Program<'_>,
    module: usize,
    fields: &[ast::Field],
    diags: &mut Vec<Diagnostic>,
) -> Vec<RField> {
    let mut out: Vec<RField> = Vec::new();
    for f in fields {
        let Some(ty) = prog.resolve_type(module, &f.ty, false, diags) else {
            continue;
        };
        let a = annotations::parse(&f.annotations, Target::Field, Some(&ty), diags);
        let field = build_field(prog, f, ty, &a, diags);
        if let Some(prev) = out
            .iter()
            .find(|p| p.name == field.name || p.wire == field.wire)
        {
            let what = if prev.name == field.name {
                "field name"
            } else {
                "wire name"
            };
            diags.push(
                Diagnostic::error(
                    "E0410",
                    format!(
                        "duplicate {what} `{}`",
                        if prev.name == field.name {
                            &field.name
                        } else {
                            &field.wire
                        }
                    ),
                    field.span,
                )
                .with_label(prev.span, "first used here"),
            );
            continue;
        }
        out.push(field);
    }
    out
}

fn build_field(
    prog: &Program<'_>,
    f: &ast::Field,
    ty: Type,
    a: &Anns<'_>,
    diags: &mut Vec<Diagnostic>,
) -> RField {
    let default = f
        .default
        .as_ref()
        .and_then(|lit| convert(prog, &ty, lit, diags));
    if f.optional && f.default.is_some() {
        diags.push(
            Diagnostic::error(
                "E0303",
                format!("optional field `{}` cannot have a default", f.name.name),
                f.span,
            )
            .with_help(
                "drop the `?`: a field with a default is required in output and optional in input",
            ),
        );
    }
    let example = a.example.and_then(|lit| convert(prog, &ty, lit, diags));
    RField {
        name: f.name.name.clone(),
        wire: a.wire.clone().unwrap_or_else(|| f.name.name.clone()),
        ty,
        optional: f.optional,
        default,
        readonly: a.readonly,
        inherited_from: None,
        constraints: a.constraints.clone(),
        example,
        docs: doc_text(&f.docs),
        deprecated: a.deprecated.clone(),
        span: f.name.span,
    }
}

fn convert(
    prog: &Program<'_>,
    ty: &Type,
    lit: &ast::Literal,
    diags: &mut Vec<Diagnostic>,
) -> Option<Literal> {
    match prog.check_literal(ty, lit) {
        Ok(l) => Some(l),
        Err((code, msg)) => {
            diags.push(Diagnostic::error(code, msg, lit.span));
            None
        }
    }
}

fn resolve_variants(e: &ast::EnumDecl, diags: &mut Vec<Diagnostic>) -> Vec<RVariant> {
    let mut out: Vec<RVariant> = Vec::new();
    for v in &e.variants {
        let a = annotations::parse(&v.annotations, Target::Variant, None, diags);
        let wire = v
            .wire
            .as_ref()
            .map_or_else(|| v.name.name.clone(), |w| w.0.clone());
        if let Some(prev) = out.iter().find(|p| p.name == v.name.name) {
            diags.push(
                Diagnostic::error(
                    "E0410",
                    format!("duplicate variant `{}`", v.name.name),
                    v.name.span,
                )
                .with_label(prev.span, "first declared here"),
            );
            continue;
        }
        if let Some(prev) = out.iter().find(|p| p.wire == wire) {
            diags.push(
                Diagnostic::error(
                    "E0403",
                    format!("enum wire value `{wire}` is used by more than one variant"),
                    v.span,
                )
                .with_label(prev.span, "first used here"),
            );
            continue;
        }
        out.push(RVariant {
            name: v.name.name.clone(),
            wire,
            docs: doc_text(&v.docs),
            deprecated: a.deprecated,
            span: v.name.span,
        });
    }
    out
}

fn resolve_arms(
    prog: &Program<'_>,
    module: usize,
    u: &ast::UnionDecl,
    diags: &mut Vec<Diagnostic>,
) -> Vec<RArm> {
    let mut out: Vec<RArm> = Vec::new();
    if u.arms.is_empty() {
        diags.push(Diagnostic::error(
            "E0409",
            format!("union `{}` has no arms", u.name.name),
            u.name.span,
        ));
    }
    for arm in &u.arms {
        let Some(ty) = prog.resolve_type(module, &arm.ty, true, diags) else {
            continue;
        };
        let Type::Decl(id) = ty else {
            diags.push(Diagnostic::error(
                "E0409",
                "union arms must be models",
                arm.ty.span,
            ));
            continue;
        };
        let decl = id.index();
        match prog.decls[decl].kind {
            DKind::Model => {}
            DKind::Error => {
                diags.push(error_position(&prog.decls[decl].name, arm.ty.span));
                continue;
            }
            DKind::Enum | DKind::Union | DKind::Const => {
                diags.push(Diagnostic::error(
                    "E0409",
                    format!(
                        "union arm `{}` must be a model, but `{}` is not",
                        arm.label.name, prog.decls[decl].name
                    ),
                    arm.ty.span,
                ));
                continue;
            }
        }
        if let Some(prev) = out.iter().find(|p| p.label == arm.label.name) {
            diags.push(
                Diagnostic::error(
                    "E0410",
                    format!("duplicate union arm `{}`", arm.label.name),
                    arm.label.span,
                )
                .with_label(prev.span, "first used here"),
            );
            continue;
        }
        out.push(RArm {
            label: arm.label.name.clone(),
            decl,
            span: arm.span,
        });
    }
    out
}

fn resolve_const(
    prog: &Program<'_>,
    module: usize,
    c: &ast::ConstDecl,
    res: &mut Resolved,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(ty) = prog.resolve_type(module, &c.ty, false, diags) else {
        return;
    };
    res.const_value = convert(prog, &ty, &c.value, diags);
    res.const_ty = Some(ty);
}
