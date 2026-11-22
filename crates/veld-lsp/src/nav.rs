//! Go to definition, find references and rename.

use std::collections::BTreeMap;

use lsp_types::{Location, Range, TextEdit, Uri, WorkspaceEdit};
use veld_compiler::index::Index;
use veld_compiler::language;
use veld_compiler::{FileId, Span};

use crate::query::{self, Hit, Symbol};
use crate::workspace::Analysis;

/// An LSP location for a span, when its file lives on disk.
#[must_use]
pub fn location(a: &Analysis, span: Span) -> Option<Location> {
    let meta = a.meta(span.file)?;
    let uri = meta.uri.clone()?;
    Some(Location::new(
        uri,
        meta.lines.range(span.start as usize, span.end as usize),
    ))
}

#[must_use]
pub fn definition(a: &Analysis, file: FileId, offset: u32) -> Option<Location> {
    let hit = query::symbol_at(&a.index, file, offset)?;
    match &hit.symbol {
        Symbol::Package(name) => {
            let first =
                a.index.files.iter().find(|f| {
                    f.package == *name && f.package_kind != veld_ir::PackageKind::Prelude
                })?;
            location(a, Span::new(first.id, 0, 0))
        }
        s @ (Symbol::Decl(_)
        | Symbol::Field { .. }
        | Symbol::Variant { .. }
        | Symbol::Arm { .. }
        | Symbol::Service(_)
        | Symbol::Endpoint { .. }
        | Symbol::Param { .. }
        | Symbol::Module(_)
        | Symbol::Annotation(_)) => location(a, query::definition_span(&a.index, s)?),
    }
}

#[must_use]
pub fn references(a: &Analysis, file: FileId, offset: u32, with_def: bool) -> Vec<Location> {
    let Some(hit) = query::symbol_at(&a.index, file, offset) else {
        return Vec::new();
    };
    query::occurrences(&a.index, &hit.symbol, with_def)
        .into_iter()
        .filter_map(|s| location(a, s))
        .collect()
}

/// Why a rename was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenameError {
    #[error("nothing renamable here")]
    NotRenamable,
    #[error("`{0}` is a reserved word or not a valid identifier")]
    BadName(String),
    #[error("`{0}` is a built-in type name")]
    BuiltIn(String),
    #[error("`{0}` is declared in the prelude and cannot be renamed")]
    Prelude(String),
    #[error("`{0}` already exists here")]
    Taken(String),
}

/// The identifier a rename would change, if the cursor is on a renamable symbol.
pub fn prepare_rename(
    a: &Analysis,
    file: FileId,
    offset: u32,
) -> Result<(Range, String), RenameError> {
    let hit = renamable(a, file, offset)?;
    let current = text_of(&a.index, hit.span).to_string();
    let lines = a.lines(hit.span.file).ok_or(RenameError::NotRenamable)?;
    Ok((
        lines.range(hit.span.start as usize, hit.span.end as usize),
        current,
    ))
}

fn text_of(ix: &Index, span: Span) -> &str {
    ix.sources
        .text(span.file)
        .and_then(|t| t.get(span.start as usize..span.end as usize))
        .unwrap_or("")
}

fn renamable(a: &Analysis, file: FileId, offset: u32) -> Result<Hit, RenameError> {
    let ix = &a.index;
    let hit = query::symbol_at(ix, file, offset).ok_or(RenameError::NotRenamable)?;
    match &hit.symbol {
        Symbol::Decl(d) => {
            let decl = ix.decls.get(*d).ok_or(RenameError::NotRenamable)?;
            if decl.is_prelude() {
                return Err(RenameError::Prelude(decl.name.clone()));
            }
        }
        Symbol::Field { .. }
        | Symbol::Variant { .. }
        | Symbol::Arm { .. }
        | Symbol::Service(_)
        | Symbol::Endpoint { .. }
        | Symbol::Param { .. } => {}
        Symbol::Module(_) | Symbol::Package(_) | Symbol::Annotation(_) => {
            return Err(RenameError::NotRenamable);
        }
    }
    let name = text_of(ix, hit.span);
    if matches!(hit.symbol, Symbol::Decl(_)) && name != decl_name(ix, &hit.symbol) {
        // An alias of an import: renaming it would not rename the declaration.
        return Err(RenameError::NotRenamable);
    }
    Ok(hit)
}

fn decl_name<'a>(ix: &'a Index, s: &Symbol) -> &'a str {
    match s {
        Symbol::Decl(d) => ix.decls.get(*d).map_or("", |d| d.name.as_str()),
        Symbol::Field { .. }
        | Symbol::Variant { .. }
        | Symbol::Arm { .. }
        | Symbol::Service(_)
        | Symbol::Endpoint { .. }
        | Symbol::Param { .. }
        | Symbol::Module(_)
        | Symbol::Package(_)
        | Symbol::Annotation(_) => "",
    }
}

/// Renames the symbol under the cursor everywhere in the workspace.
pub fn rename(
    a: &Analysis,
    file: FileId,
    offset: u32,
    new: &str,
) -> Result<WorkspaceEdit, RenameError> {
    let hit = renamable(a, file, offset)?;
    let ix = &a.index;
    if language::SCALARS.contains(&new) || language::GENERICS.contains(&new) {
        return Err(RenameError::BuiltIn(new.to_string()));
    }
    if !language::is_valid_ident(new) {
        return Err(RenameError::BadName(new.to_string()));
    }
    check_collision(ix, &hit.symbol, new)?;
    let old = text_of(ix, hit.span).to_string();
    let mut spans = query::occurrences(ix, &hit.symbol, true);
    // Uses under an import alias spell another name; leave them alone.
    spans.retain(|s| text_of(ix, *s) == old);
    let mut changes: BTreeMap<Uri, Vec<TextEdit>> = BTreeMap::new();
    for s in spans {
        let (Some(meta), Some(uri)) = (a.meta(s.file), a.uri(s.file)) else {
            continue;
        };
        let range = meta.lines.range(s.start as usize, s.end as usize);
        changes
            .entry(uri.clone())
            .or_default()
            .push(TextEdit::new(range, new.to_string()));
    }
    Ok(WorkspaceEdit {
        changes: Some(changes.into_iter().collect()),
        ..WorkspaceEdit::default()
    })
}

fn check_collision(ix: &Index, sym: &Symbol, new: &str) -> Result<(), RenameError> {
    let taken = match sym {
        Symbol::Decl(d) => {
            let pkg = ix.decls.get(*d).map(|d| d.package.as_str()).unwrap_or("");
            ix.decls.iter().any(|x| x.package == pkg && x.name == new)
                || ix
                    .services
                    .iter()
                    .any(|s| s.package == pkg && s.name == new)
        }
        Symbol::Service(s) => {
            let pkg = ix
                .services
                .get(*s)
                .map(|s| s.package.as_str())
                .unwrap_or("");
            ix.decls.iter().any(|x| x.package == pkg && x.name == new)
                || ix
                    .services
                    .iter()
                    .any(|x| x.package == pkg && x.name == new)
        }
        Symbol::Field { decl, .. } => ix
            .decls
            .get(*decl)
            .is_some_and(|d| query::decl_fields(ix, d).iter().any(|f| f.name.name == new)),
        Symbol::Variant { decl, .. } => ix
            .decls
            .get(*decl)
            .is_some_and(|d| d.variants.iter().any(|v| v == new)),
        Symbol::Arm { decl, .. } => ix
            .decls
            .get(*decl)
            .is_some_and(|d| d.arms.iter().any(|(l, _)| l == new)),
        Symbol::Endpoint { service, .. } => query::service_ast(ix, *service)
            .is_some_and(|s| s.endpoints.iter().any(|e| e.name.name == new)),
        Symbol::Param {
            service, endpoint, ..
        } => query::service_ast(ix, *service)
            .and_then(|s| s.endpoints.get(*endpoint))
            .is_some_and(|e| e.params.iter().any(|p| p.name.name == new)),
        Symbol::Module(_) | Symbol::Package(_) | Symbol::Annotation(_) => false,
    };
    if taken {
        Err(RenameError::Taken(new.to_string()))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;
    use std::fs;

    const TOML: &str = "[project]\nname = \"p\"\n[services.iam]\ncontracts = \"iam\"\n[services.app]\ncontracts = \"app\"\nconsumes = [\"iam\"]\n";
    const USERS: &str = "model User {\n  id: uuid\n}\nenum Role { admin, guest }\n";
    const APP: &str = "use iam:users.{User as U, Role}\nmodel Order {\n  a: U\n  r: Role = .admin\n}\nmodel B { u: U }\nservice S at \"/s\" {\n  get \"/\" list() -> Order\n}\n";

    fn setup() -> (tempfile::TempDir, Analysis) {
        let tmp = tempfile::tempdir().unwrap();
        for (p, t) in [
            ("veld.toml", TOML),
            ("iam/users.veld", USERS),
            ("app/app.veld", APP),
        ] {
            let path = tmp.path().join(p);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, t).unwrap();
        }
        let a = Workspace::new(Some(tmp.path().to_path_buf())).analyze(None);
        assert_eq!(
            a.index.error_count(),
            0,
            "{}",
            a.index.sources.render_all(&a.index.diagnostics)
        );
        (tmp, a)
    }

    fn off(a: &Analysis, f: FileId, needle: &str, add: usize) -> u32 {
        u32::try_from(a.index.sources.text(f).unwrap().find(needle).unwrap() + add).unwrap()
    }

    #[test]
    fn definition_crosses_packages_and_imports() {
        let (tmp, a) = setup();
        let app = a.file_for(&tmp.path().join("app/app.veld")).unwrap();
        let loc = definition(&a, app, off(&a, app, "a: U", 3)).unwrap();
        assert!(loc.uri.as_str().ends_with("iam/users.veld"));
        assert_eq!(loc.range.start.line, 0);
        let loc = definition(&a, app, off(&a, app, "users.{", 1)).unwrap();
        assert!(loc.uri.as_str().ends_with("iam/users.veld"));
        let loc = definition(&a, app, off(&a, app, "iam:", 1)).unwrap();
        assert!(loc.uri.as_str().ends_with("iam/users.veld"));
        let loc = definition(&a, app, off(&a, app, "{User as", 2)).unwrap();
        assert_eq!(loc.range.start.line, 0);
        // An endpoint name is its own definition.
        assert!(definition(&a, app, off(&a, app, "list() -> Order", 0)).is_some());
    }

    #[test]
    fn references_span_files() {
        let (tmp, a) = setup();
        let users = a.file_for(&tmp.path().join("iam/users.veld")).unwrap();
        let refs = references(&a, users, off(&a, users, "User", 1), true);
        assert_eq!(refs.len(), 4, "{refs:?}");
    }

    #[test]
    fn rename_edits_every_file_and_respects_aliases() {
        let (tmp, a) = setup();
        let users = a.file_for(&tmp.path().join("iam/users.veld")).unwrap();
        let edit = rename(&a, users, off(&a, users, "User", 1), "Account").unwrap();
        let changes = edit.changes.unwrap();
        assert_eq!(changes.len(), 2);
        let total: usize = changes.values().map(Vec::len).sum();
        assert_eq!(
            total, 2,
            "declaration and the import name; `U` uses are untouched"
        );
        let r = rename(&a, users, off(&a, users, "admin", 1), "root").unwrap();
        assert_eq!(r.changes.unwrap().values().map(Vec::len).sum::<usize>(), 2);
        // The alias `U` itself is not renamable.
        let app = a.file_for(&tmp.path().join("app/app.veld")).unwrap();
        assert_eq!(
            prepare_rename(&a, app, off(&a, app, "a: U", 3)).unwrap_err(),
            RenameError::NotRenamable
        );
    }

    #[test]
    fn rename_refuses_bad_names_and_collisions() {
        let (tmp, a) = setup();
        let users = a.file_for(&tmp.path().join("iam/users.veld")).unwrap();
        let at = off(&a, users, "User", 1);
        assert_eq!(
            rename(&a, users, at, "model").unwrap_err(),
            RenameError::BadName("model".into())
        );
        assert_eq!(
            rename(&a, users, at, "string").unwrap_err(),
            RenameError::BuiltIn("string".into())
        );
        assert_eq!(
            rename(&a, users, at, "1x").unwrap_err(),
            RenameError::BadName("1x".into())
        );
        assert_eq!(
            rename(&a, users, at, "Role").unwrap_err(),
            RenameError::Taken("Role".into())
        );
        let app = a.file_for(&tmp.path().join("app/app.veld")).unwrap();
        let nf = off(&a, app, "list() -> Order", 0);
        assert!(rename(&a, app, nf, "x").is_ok());
        assert!(prepare_rename(&a, users, off(&a, users, "model", 1)).is_err());
    }

    #[test]
    fn fields_and_endpoints_rename_at_their_definition() {
        let (tmp, a) = setup();
        let app = a.file_for(&tmp.path().join("app/app.veld")).unwrap();
        let e = rename(&a, app, off(&a, app, "list", 1), "all").unwrap();
        assert_eq!(e.changes.unwrap().values().map(Vec::len).sum::<usize>(), 1);
        let (range, cur) = prepare_rename(&a, app, off(&a, app, "r: Role", 0)).unwrap();
        assert_eq!(cur, "r");
        assert_eq!(range.start.line, 3);
        assert!(rename(&a, app, off(&a, app, "r: Role", 0), "a").is_err());
    }
}
