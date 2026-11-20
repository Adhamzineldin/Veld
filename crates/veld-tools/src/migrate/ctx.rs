//! Package-wide facts collected before any file is converted.

use std::collections::{BTreeMap, BTreeSet};

use super::names::{inferred_status, module_of, prelude_status};
use super::v1::ast::{Field, Item, V1File};

/// One input file after parsing.
#[derive(Debug)]
pub(super) struct Source {
    pub rel: String,
    pub raw: String,
    pub out_path: String,
    pub module: String,
    pub parsed: Result<V1File, String>,
}

impl Source {
    pub fn new(rel: &str, raw: &str, parsed: Result<V1File, String>) -> Source {
        let (module, out_path, _) = module_of(rel);
        Source {
            rel: rel.to_string(),
            raw: raw.to_string(),
            out_path,
            module,
            parsed,
        }
    }
}

/// A v1 error name that is not in the v2 prelude and needs a declaration.
#[derive(Debug, Clone)]
pub(super) struct GenError {
    /// Name of the generated v2 error (`Name`, or `NameError` on a clash).
    pub name: String,
    pub status: u16,
    /// True when the status came from the name, not from the contract.
    pub inferred: bool,
    /// Index of the file that declares it.
    pub file: usize,
}

#[derive(Debug, Default)]
pub(super) struct Ctx {
    /// Type name to the index of the file that declares it. First wins.
    pub decls: BTreeMap<String, usize>,
    pub enums: BTreeSet<String>,
    pub models: BTreeMap<String, Vec<Field>>,
    /// v1 error name to its generated declaration.
    pub errors: BTreeMap<String, GenError>,
    /// Constant names declared more than once in the package.
    pub dup_consts: BTreeSet<String>,
    pub warnings: Vec<String>,
}

impl Ctx {
    pub fn collect(sources: &[Source]) -> Ctx {
        let mut ctx = Ctx::default();
        for (idx, s) in sources.iter().enumerate() {
            if let Ok(f) = &s.parsed {
                ctx.collect_decls(idx, &s.rel, f);
            }
        }
        for (idx, s) in sources.iter().enumerate() {
            if let Ok(f) = &s.parsed {
                ctx.collect_errors(idx, f);
            }
        }
        ctx.collect_const_dups(sources);
        ctx
    }

    fn declare(&mut self, name: &str, idx: usize, rel: &str) {
        if self.decls.contains_key(name) {
            self.warnings.push(format!(
                "{rel}: `{name}` is declared in more than one file; v2 names are unique per package, rename one of them"
            ));
        } else {
            self.decls.insert(name.to_string(), idx);
        }
    }

    fn collect_decls(&mut self, idx: usize, rel: &str, f: &V1File) {
        for item in &f.items {
            match item {
                Item::Model(m) => {
                    self.declare(&m.name, idx, rel);
                    self.models
                        .entry(m.name.clone())
                        .or_insert_with(|| m.fields.clone());
                }
                Item::Enum(e) => {
                    self.declare(&e.name, idx, rel);
                    self.enums.insert(e.name.clone());
                }
                Item::Constants(_) | Item::Module(_) => {}
            }
        }
    }

    fn collect_errors(&mut self, idx: usize, f: &V1File) {
        for item in &f.items {
            let Item::Module(m) = item else { continue };
            for a in &m.actions {
                for (name, status) in &a.errors {
                    self.note_error(name, *status, idx);
                }
            }
        }
    }

    fn note_error(&mut self, name: &str, status: Option<u16>, idx: usize) {
        if prelude_status(name).is_some() || self.errors.contains_key(name) {
            return;
        }
        let final_name = if self.decls.contains_key(name) {
            format!("{name}Error")
        } else {
            name.to_string()
        };
        let guess = status.or_else(|| inferred_status(name));
        self.errors.insert(
            name.to_string(),
            GenError {
                name: final_name,
                status: guess.unwrap_or(400),
                inferred: status.is_none(),
                file: idx,
            },
        );
    }

    fn collect_const_dups(&mut self, sources: &[Source]) {
        let mut seen = BTreeSet::new();
        for s in sources {
            let Ok(f) = &s.parsed else { continue };
            for item in &f.items {
                let Item::Constants(g) = item else { continue };
                for c in &g.items {
                    if !seen.insert(c.name.clone()) {
                        self.dup_consts.insert(c.name.clone());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::v1;
    use super::*;

    fn source(rel: &str, src: &str) -> Source {
        Source::new(rel, src, v1::parse(src).map(|p| p.file))
    }

    #[test]
    fn collects_declarations_and_generated_errors() {
        let a = source("models/a.veld", "model A { x: int }\nenum E { a b }");
        let b = source(
            "m.veld",
            "module M { action X { method: GET path: / errors: [NotFound, Teapot:418, A] } }",
        );
        let ctx = Ctx::collect(&[a, b]);
        assert_eq!(ctx.decls.get("A"), Some(&0));
        assert!(ctx.enums.contains("E"));
        assert!(!ctx.errors.contains_key("NotFound"));
        let teapot = ctx.errors.get("Teapot");
        assert_eq!(
            teapot.map(|e| (e.name.as_str(), e.status, e.file)),
            Some(("Teapot", 418, 1))
        );
        assert_eq!(ctx.errors.get("A").map(|e| e.name.as_str()), Some("AError"));
    }

    #[test]
    fn duplicate_declarations_warn() {
        let a = source("a.veld", "model A { x: int }");
        let b = source("b.veld", "model A { y: int }");
        let ctx = Ctx::collect(&[a, b]);
        assert_eq!(ctx.warnings.len(), 1);
        assert_eq!(ctx.decls.get("A"), Some(&0));
    }
}
