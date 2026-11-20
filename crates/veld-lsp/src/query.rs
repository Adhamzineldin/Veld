//! Position -> node lookups on top of the compiler's AST and resolution table.

use veld_compiler::FileId;
use veld_compiler::Span;
use veld_compiler::index::{DeclEntry, Index, RefTarget};
use veld_compiler::syntax::ast::{self, DeclKind};

/// What an identifier denotes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Symbol {
    /// A top-level declaration (index into `Index::decls`).
    Decl(usize),
    /// An own field, by index in the declaration's AST.
    Field {
        decl: usize,
        field: usize,
    },
    Variant {
        decl: usize,
        variant: usize,
    },
    /// A union arm label.
    Arm {
        decl: usize,
        arm: usize,
    },
    Service(usize),
    Endpoint {
        service: usize,
        endpoint: usize,
    },
    Param {
        service: usize,
        endpoint: usize,
        param: usize,
    },
    Module(FileId),
    /// The package name of a `use`.
    Package(String),
    /// `@name`.
    Annotation(String),
}

/// A symbol found at a cursor, with the identifier's span and whether it is the definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub symbol: Symbol,
    pub span: Span,
    pub is_definition: bool,
}

fn contains(s: Span, off: u32) -> bool {
    s.start <= off && off <= s.end
}

/// The symbol under `offset` in `file`, if any. The smallest enclosing identifier wins.
#[must_use]
pub fn symbol_at(ix: &Index, file: FileId, offset: u32) -> Option<Hit> {
    if let Some(h) = definition_at(ix, file, offset) {
        return Some(h);
    }
    let r = ix
        .refs
        .iter()
        .filter(|r| r.span.file == file && contains(r.span, offset))
        .min_by_key(|r| r.span.end - r.span.start)?;
    let symbol = match &r.target {
        RefTarget::Decl(d) => Symbol::Decl(*d),
        RefTarget::Module(f) => Symbol::Module(*f),
        RefTarget::Variant { decl, name } => {
            let variant = variant_index(ix, *decl, name)?;
            Symbol::Variant {
                decl: *decl,
                variant,
            }
        }
    };
    let mut span = r.span;
    if matches!(r.target, RefTarget::Variant { .. }) {
        span.start += 1;
    }
    Some(Hit {
        symbol,
        span,
        is_definition: false,
    })
}

fn variant_index(ix: &Index, decl: usize, name: &str) -> Option<usize> {
    let d = ix.decls.get(decl)?;
    match &ix.decl_ast(d)?.kind {
        DeclKind::Enum(e) => e.variants.iter().position(|v| v.name.name == name),
        _ => None,
    }
}

fn hit(symbol: Symbol, id: &ast::Ident) -> Hit {
    Hit {
        symbol,
        span: id.span,
        is_definition: true,
    }
}

/// Declaration names, annotations and `use` packages: identifiers that are not references.
fn definition_at(ix: &Index, file: FileId, offset: u32) -> Option<Hit> {
    let f = ix.file(file)?;
    for u in &f.ast.uses {
        if let Some(p) = &u.package
            && contains(p.span, offset)
        {
            return Some(Hit {
                symbol: Symbol::Package(p.name.clone()),
                span: p.span,
                is_definition: false,
            });
        }
    }
    for (ai, d) in f.ast.decls.iter().enumerate() {
        if !contains(d.span, offset) {
            continue;
        }
        if let Some(a) = annotation_at(&d.annotations, offset) {
            return Some(a);
        }
        let entry = ix
            .decls
            .iter()
            .position(|e| e.file == file && e.ast_index == ai && e.name == d.name().name);
        if let Some(h) = in_decl(ix, file, ai, d, entry, offset) {
            return Some(h);
        }
    }
    None
}

fn annotation_at(anns: &[ast::Annotation], offset: u32) -> Option<Hit> {
    anns.iter()
        .find(|a| contains(a.name.span, offset))
        .map(|a| Hit {
            symbol: Symbol::Annotation(a.name.name.clone()),
            span: a.name.span,
            is_definition: false,
        })
}

fn in_decl(
    ix: &Index,
    file: FileId,
    ast_index: usize,
    d: &ast::Decl,
    entry: Option<usize>,
    offset: u32,
) -> Option<Hit> {
    let name = d.name();
    if contains(name.span, offset) {
        return match (&d.kind, entry) {
            (DeclKind::Service(_), _) => {
                let s = service_index(ix, file, ast_index)?;
                Some(hit(Symbol::Service(s), name))
            }
            (_, Some(e)) => Some(hit(Symbol::Decl(e), name)),
            _ => None,
        };
    }
    let decl = entry;
    match &d.kind {
        DeclKind::Model(m) => fields_at(&m.fields, decl?, offset),
        DeclKind::Error(e) => fields_at(&e.fields, decl?, offset),
        DeclKind::Enum(e) => {
            for (i, v) in e.variants.iter().enumerate() {
                if let Some(a) = annotation_at(&v.annotations, offset) {
                    return Some(a);
                }
                if contains(v.name.span, offset) {
                    return Some(hit(
                        Symbol::Variant {
                            decl: decl?,
                            variant: i,
                        },
                        &v.name,
                    ));
                }
            }
            None
        }
        DeclKind::Union(u) => {
            let i = u.arms.iter().position(|a| contains(a.label.span, offset))?;
            Some(hit(
                Symbol::Arm {
                    decl: decl?,
                    arm: i,
                },
                &u.arms[i].label,
            ))
        }
        DeclKind::Service(s) => {
            let si = service_index(ix, file, ast_index)?;
            endpoint_at(s, si, offset)
        }
        DeclKind::Const(_) => None,
    }
}

fn fields_at(fields: &[ast::Field], decl: usize, offset: u32) -> Option<Hit> {
    for (i, f) in fields.iter().enumerate() {
        if let Some(a) = annotation_at(&f.annotations, offset) {
            return Some(a);
        }
        if contains(f.name.span, offset) {
            return Some(hit(Symbol::Field { decl, field: i }, &f.name));
        }
    }
    None
}

fn endpoint_at(s: &ast::ServiceDecl, service: usize, offset: u32) -> Option<Hit> {
    for (ei, ep) in s.endpoints.iter().enumerate() {
        if let Some(a) = annotation_at(&ep.annotations, offset) {
            return Some(a);
        }
        if contains(ep.name.span, offset) {
            return Some(hit(
                Symbol::Endpoint {
                    service,
                    endpoint: ei,
                },
                &ep.name,
            ));
        }
        for (pi, p) in ep.params.iter().enumerate() {
            if let Some(a) = annotation_at(&p.annotations, offset) {
                return Some(a);
            }
            if contains(p.name.span, offset) {
                return Some(hit(
                    Symbol::Param {
                        service,
                        endpoint: ei,
                        param: pi,
                    },
                    &p.name,
                ));
            }
        }
    }
    None
}

fn service_index(ix: &Index, file: FileId, ast_index: usize) -> Option<usize> {
    ix.services
        .iter()
        .position(|s| s.file == file && s.ast_index == ast_index)
}

/// Every span that denotes `sym`: use sites, and the definition when `with_def` is set.
#[must_use]
pub fn occurrences(ix: &Index, sym: &Symbol, with_def: bool) -> Vec<Span> {
    let mut out: Vec<Span> = match sym {
        Symbol::Decl(d) => refs_to(ix, |t| matches!(t, RefTarget::Decl(x) if x == d), 0),
        Symbol::Module(f) => refs_to(ix, |t| matches!(t, RefTarget::Module(x) if x == f), 0),
        Symbol::Variant { decl, variant } => {
            let name = variant_name(ix, *decl, *variant).unwrap_or_default();
            refs_to(
                ix,
                |t| matches!(t, RefTarget::Variant { decl: d, name: n } if d == decl && *n == name),
                1,
            )
        }
        _ => Vec::new(),
    };
    if with_def && let Some(s) = definition_span(ix, sym) {
        out.push(s);
    }
    out.sort_by_key(|s| (s.file, s.start));
    out.dedup();
    out
}

fn refs_to(ix: &Index, pred: impl Fn(&RefTarget) -> bool, skip: u32) -> Vec<Span> {
    ix.refs
        .iter()
        .filter(|r| pred(&r.target))
        .map(|r| Span {
            start: r.span.start + skip,
            ..r.span
        })
        .collect()
}

fn variant_name(ix: &Index, decl: usize, variant: usize) -> Option<String> {
    ix.decls.get(decl)?.variants.get(variant).cloned()
}

/// The span of the identifier that defines `sym`.
#[must_use]
pub fn definition_span(ix: &Index, sym: &Symbol) -> Option<Span> {
    match sym {
        Symbol::Decl(d) => Some(ix.decls.get(*d)?.name_span),
        Symbol::Field { decl, field } => decl_fields(ix, ix.decls.get(*decl)?)
            .get(*field)
            .map(|f| f.name.span),
        Symbol::Variant { decl, variant } => {
            let d = ix.decls.get(*decl)?;
            match &ix.decl_ast(d)?.kind {
                DeclKind::Enum(e) => e.variants.get(*variant).map(|v| v.name.span),
                _ => None,
            }
        }
        Symbol::Arm { decl, arm } => {
            let d = ix.decls.get(*decl)?;
            match &ix.decl_ast(d)?.kind {
                DeclKind::Union(u) => u.arms.get(*arm).map(|a| a.label.span),
                _ => None,
            }
        }
        Symbol::Service(s) => Some(service_ast(ix, *s)?.name.span),
        Symbol::Endpoint { service, endpoint } => service_ast(ix, *service)?
            .endpoints
            .get(*endpoint)
            .map(|e| e.name.span),
        Symbol::Param {
            service,
            endpoint,
            param,
        } => service_ast(ix, *service)?
            .endpoints
            .get(*endpoint)?
            .params
            .get(*param)
            .map(|p| p.name.span),
        Symbol::Module(f) => Some(Span::new(*f, 0, 0)),
        Symbol::Package(_) | Symbol::Annotation(_) => None,
    }
}

/// Own fields of a model or error declaration.
#[must_use]
pub fn decl_fields<'a>(ix: &'a Index, d: &DeclEntry) -> &'a [ast::Field] {
    match ix.decl_ast(d).map(|a| &a.kind) {
        Some(DeclKind::Model(m)) => &m.fields,
        Some(DeclKind::Error(e)) => &e.fields,
        _ => &[],
    }
}

/// The syntax node of a service entry.
#[must_use]
pub fn service_ast(ix: &Index, service: usize) -> Option<&ast::ServiceDecl> {
    let s = ix.services.get(service)?;
    match &ix.file(s.file)?.ast.decls.get(s.ast_index)?.kind {
        DeclKind::Service(sd) => Some(sd),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::build;

    const USERS: &str = "model User {\n  id: uuid\n  name: string\n}\nenum Role { admin, guest }\n";
    const APP: &str = "use iam:users.{User, Role}\nmodel Order {\n  owner: User\n  role: Role = .admin\n}\nservice Orders at \"/o\" {\n  get \"/{id}\" get(id: uuid) -> Order | NotFound\n}\n";

    fn offset_of(ix: &Index, file: FileId, needle: &str, nth: usize) -> u32 {
        let text = ix.sources.text(file).unwrap();
        let (i, _) = text.match_indices(needle).nth(nth).unwrap();
        u32::try_from(i).unwrap() + 1
    }

    #[test]
    fn finds_references_definitions_and_variants() {
        let ix = build(&[("iam", "users.veld", USERS), ("app", "app.veld", APP)]);
        let app = ix.module_file("app", "app").unwrap();
        let users = ix.module_file("iam", "users").unwrap();
        let user = ix.find_decl("iam", "User").unwrap();

        let h = symbol_at(&ix, app, offset_of(&ix, app, "owner: User", 0) + 8).unwrap();
        assert_eq!(h.symbol, Symbol::Decl(user));
        assert!(!h.is_definition);
        let h = symbol_at(&ix, app, offset_of(&ix, app, "{User,", 0) + 1).unwrap();
        assert_eq!(h.symbol, Symbol::Decl(user));

        let h = symbol_at(&ix, users, offset_of(&ix, users, "User {", 0)).unwrap();
        assert!(h.is_definition);
        let uses = occurrences(&ix, &h.symbol, false);
        assert_eq!(uses.len(), 2);
        assert_eq!(occurrences(&ix, &h.symbol, true).len(), 3);

        let h = symbol_at(&ix, app, offset_of(&ix, app, ".admin", 0) + 1).unwrap();
        assert!(matches!(h.symbol, Symbol::Variant { variant: 0, .. }));
        let text = ix.sources.text(app).unwrap();
        assert_eq!(&text[h.span.start as usize..h.span.end as usize], "admin");
        assert_eq!(occurrences(&ix, &h.symbol, true).len(), 2);

        let h = symbol_at(&ix, app, offset_of(&ix, app, "iam:", 0)).unwrap();
        assert_eq!(h.symbol, Symbol::Package("iam".into()));
        let h = symbol_at(&ix, app, offset_of(&ix, app, "users.{", 0)).unwrap();
        assert!(matches!(h.symbol, Symbol::Module(_)));
    }

    #[test]
    fn finds_fields_endpoints_params_and_errors() {
        let ix = build(&[("iam", "users.veld", USERS), ("app", "app.veld", APP)]);
        let app = ix.module_file("app", "app").unwrap();
        let h = symbol_at(&ix, app, offset_of(&ix, app, "owner", 0)).unwrap();
        assert!(matches!(h.symbol, Symbol::Field { field: 0, .. }) && h.is_definition);
        let h = symbol_at(&ix, app, offset_of(&ix, app, "get(", 0)).unwrap();
        assert!(matches!(h.symbol, Symbol::Endpoint { .. }));
        let h = symbol_at(&ix, app, offset_of(&ix, app, "id: uuid", 0)).unwrap();
        assert!(matches!(h.symbol, Symbol::Param { .. }));
        let h = symbol_at(&ix, app, offset_of(&ix, app, "NotFound", 0)).unwrap();
        let Symbol::Decl(d) = h.symbol else { panic!() };
        assert_eq!(ix.decls[d].name, "NotFound");
        assert!(symbol_at(&ix, app, offset_of(&ix, app, "get \"", 0)).is_none());
    }

    #[test]
    fn annotations_are_symbols() {
        let src = "model A {\n  x: int @min(1)\n}\n";
        let ix = build(&[("app", "app.veld", src)]);
        let f = ix.module_file("app", "app").unwrap();
        let h = symbol_at(&ix, f, offset_of(&ix, f, "@min", 0) + 1).unwrap();
        assert_eq!(h.symbol, Symbol::Annotation("min".into()));
    }
}
