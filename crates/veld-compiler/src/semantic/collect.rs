//! Pass 1 (module graph and declaration collection) and consumes validation.

use std::collections::BTreeMap;

use super::{DKind, DeclInfo, ModuleInfo, ParsedPackage, PkgInfo, Program, Resolved, Scope};
use crate::diagnostics::{Diagnostic, Span};
use crate::language;
use crate::syntax::ast::DeclKind;
use veld_ir::PackageKind;

/// Derives the dotted module path from a package-relative file path.
pub fn module_path(rel: &str) -> Result<String, String> {
    let Some(stem) = rel.strip_suffix(".veld") else {
        return Err(format!("`{rel}` does not end in `.veld`"));
    };
    let mut parts = Vec::new();
    for seg in stem.split('/') {
        if !language::is_valid_ident(seg) {
            return Err(format!(
                "path segment `{seg}` of `{rel}` is not a valid identifier"
            ));
        }
        parts.push(seg);
    }
    Ok(parts.join("."))
}

pub fn collect<'a>(packages: &'a [ParsedPackage], diags: &mut Vec<Diagnostic>) -> Program<'a> {
    let mut prog = Program {
        packages: Vec::new(),
        modules: Vec::new(),
        decls: Vec::new(),
        services: Vec::new(),
        scopes: Vec::new(),
        prelude: BTreeMap::new(),
        prelude_pkg: usize::MAX,
        module_index: BTreeMap::new(),
    };
    let mut names: BTreeMap<(usize, String), Span> = BTreeMap::new();
    for (pi, pkg) in packages.iter().enumerate() {
        register_package(&mut prog, pi, pkg, diags);
        for f in &pkg.files {
            match module_path(&f.rel_path) {
                Ok(path) => {
                    let mi = prog.modules.len();
                    prog.module_index.insert((pi, path.clone()), mi);
                    prog.modules.push(ModuleInfo {
                        pkg: pi,
                        path,
                        file: f.file,
                        rel_path: &f.rel_path,
                        ast: &f.ast,
                    });
                    prog.scopes.push(Scope::default());
                    for decl in &f.ast.decls {
                        add_decl(&mut prog, &mut names, mi, decl, diags);
                    }
                }
                Err(msg) => diags.push(
                    Diagnostic::error("E0101", msg, Span::new(f.file, 0, 0)).with_help(
                        "module paths come from file paths; use letters, digits and `_`",
                    ),
                ),
            }
        }
    }
    for (i, d) in prog.decls.iter().enumerate() {
        if d.pkg == prog.prelude_pkg {
            prog.prelude.insert(d.name.clone(), i);
        }
    }
    prog
}

fn register_package(
    prog: &mut Program<'_>,
    pi: usize,
    pkg: &ParsedPackage,
    diags: &mut Vec<Diagnostic>,
) {
    if pkg.kind == PackageKind::Prelude {
        prog.prelude_pkg = pi;
    } else if pkg.name == language::PRELUDE_PACKAGE {
        diags.push(Diagnostic::error(
            "E0701",
            format!("package name `{}` is reserved for the prelude", pkg.name),
            Span::none(),
        ));
    }
    if pkg.kind != PackageKind::Prelude && prog.packages.iter().any(|p| p.name == pkg.name) {
        diags.push(Diagnostic::error(
            "E0701",
            format!("duplicate package `{}`", pkg.name),
            Span::none(),
        ));
    }
    prog.packages.push(PkgInfo {
        name: pkg.name.clone(),
        kind: pkg.kind,
        base_url: pkg.base_url.clone(),
        prefix: pkg.prefix.clone(),
    });
}

fn add_decl<'a>(
    prog: &mut Program<'a>,
    names: &mut BTreeMap<(usize, String), Span>,
    mi: usize,
    decl: &'a crate::syntax::ast::Decl,
    diags: &mut Vec<Diagnostic>,
) {
    let pkg = prog.modules[mi].pkg;
    let id = decl.name();
    if language::SCALARS.contains(&id.name.as_str())
        || language::GENERICS.contains(&id.name.as_str())
    {
        diags.push(Diagnostic::error(
            "E0201",
            format!(
                "`{}` is a built-in type name and cannot be declared",
                id.name
            ),
            id.span,
        ));
        return;
    }
    if let Some(first) = names.get(&(pkg, id.name.clone())) {
        let pname = &prog.packages[pkg].name;
        diags.push(
            Diagnostic::error(
                "E0201",
                format!("`{}` is already declared in package `{pname}`", id.name),
                id.span,
            )
            .with_label(*first, "first declared here")
            .with_help("declaration names are unique across all modules of a package"),
        );
        return;
    }
    names.insert((pkg, id.name.clone()), id.span);
    let kind = match &decl.kind {
        DeclKind::Model(_) => DKind::Model,
        DeclKind::Error(_) => DKind::Error,
        DeclKind::Enum(_) => DKind::Enum,
        DeclKind::Union(_) => DKind::Union,
        DeclKind::Const(_) => DKind::Const,
        DeclKind::Service(_) => return,
    };
    let qualified = format!(
        "{}:{}.{}",
        prog.packages[pkg].name, prog.modules[mi].path, id.name
    );
    prog.decls.push(DeclInfo {
        pkg,
        module: mi,
        name: id.name.clone(),
        qualified,
        kind,
        ast: decl,
        name_span: id.span,
        res: Resolved::default(),
    });
}

/// `E0702` for unknown services, `W0701` for self-consumption.
pub fn check_consumes(
    prog: &Program<'_>,
    consumes: &BTreeMap<String, Vec<String>>,
    diags: &mut Vec<Diagnostic>,
) {
    let known = |n: &str| {
        prog.packages
            .iter()
            .any(|p| p.name == n && p.kind != PackageKind::Prelude)
    };
    for (svc, list) in consumes {
        if !known(svc) {
            diags.push(Diagnostic::error(
                "E0702",
                format!("`consumes` is declared for unknown service `{svc}`"),
                Span::none(),
            ));
            continue;
        }
        for target in list {
            if target == svc {
                diags.push(Diagnostic::warning(
                    "W0701",
                    format!("service `{svc}` consumes itself"),
                    Span::none(),
                ));
            } else if !known(target) {
                diags.push(
                    Diagnostic::error(
                        "E0702",
                        format!("service `{svc}` consumes unknown service `{target}`"),
                        Span::none(),
                    )
                    .did_you_mean(
                        target,
                        prog.packages
                            .iter()
                            .filter(|p| p.kind != PackageKind::Prelude)
                            .map(|p| p.name.as_str()),
                        Some(known_list(prog)),
                    ),
                );
            }
        }
    }
}

fn known_list(prog: &Program<'_>) -> String {
    let names: Vec<&str> = prog
        .packages
        .iter()
        .filter(|p| p.kind != PackageKind::Prelude)
        .map(|p| p.name.as_str())
        .collect();
    format!("known services: {}", names.join(", "))
}

#[cfg(test)]
mod tests {
    use super::module_path;

    #[test]
    fn module_paths_come_from_files() {
        assert_eq!(module_path("users.veld").as_deref(), Ok("users"));
        assert_eq!(
            module_path("billing/invoices.veld").as_deref(),
            Ok("billing.invoices")
        );
    }

    #[test]
    fn invalid_segments_are_rejected() {
        assert!(module_path("my-file.veld").is_err());
        assert!(module_path("a.b.veld").is_err());
        assert!(module_path("model.veld").is_err());
        assert!(module_path("x.txt").is_err());
        assert!(module_path("1a.veld").is_err());
    }
}
