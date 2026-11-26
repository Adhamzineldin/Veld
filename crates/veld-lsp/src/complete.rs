//! Completion: keywords, contextual methods, types with auto-import, annotations with
//! argument snippets, enum variants and `use` paths.

use std::collections::BTreeMap;

use lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat, Position, Range, TextEdit};
use veld_compiler::FileId;
use veld_compiler::index::{DeclEntry, Index};
use veld_compiler::language::{self, ArgShape, Target};
use veld_compiler::semantic::DKind;

use crate::context::{self, Block, BodyKind, Context, Tok};
use crate::workspace::Analysis;

/// Which declarations a type position accepts.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypePos {
    /// Fields, parameters: models, enums, unions and the built-ins.
    Value,
    /// Union arms: declarations only.
    Arm,
    /// After `->` or `|`: errors are allowed too.
    Response,
    /// After `extends`.
    Parent,
    /// After `carries`, `on` and `given event`: events.
    Event,
    /// After `else` and `->` in an example: errors.
    Error,
}

#[must_use]
pub fn complete(a: &Analysis, file: FileId, offset: usize) -> Vec<CompletionItem> {
    let Some(lines) = a.lines(file) else {
        return Vec::new();
    };
    let text = lines.text();
    let prefix = text.get(..offset.min(text.len())).unwrap_or("");
    let ctx = context::scan(prefix);
    if ctx.inert {
        return Vec::new();
    }
    let cx = Cx {
        a,
        file,
        ctx: &ctx,
        prefix,
    };
    cx.items()
}

pub(crate) struct Cx<'a> {
    pub(crate) a: &'a Analysis,
    pub(crate) file: FileId,
    pub(crate) ctx: &'a Context,
    pub(crate) prefix: &'a str,
}

pub(crate) fn item(label: &str, kind: CompletionItemKind, detail: Option<&str>) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: detail.map(str::to_string),
        ..CompletionItem::default()
    }
}

pub(crate) fn snippet(
    label: &str,
    kind: CompletionItemKind,
    detail: &str,
    body: &str,
) -> CompletionItem {
    CompletionItem {
        insert_text: Some(body.to_string()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..item(label, kind, Some(detail))
    }
}

impl Cx<'_> {
    pub(crate) fn ix(&self) -> &Index {
        &self.a.index
    }

    fn prev(&self) -> Option<&Tok> {
        self.ctx.stmt.last()
    }

    fn items(&self) -> Vec<CompletionItem> {
        if self.prev() == Some(&Tok::Punct('@')) {
            return self.annotations();
        }
        if let Some(found) = self.event_position() {
            return found;
        }
        if let Some(found) = self.rules_position() {
            return found;
        }
        if self.ctx.at_top_level && self.ctx.use_stmt.is_some() {
            return self.use_paths();
        }
        match &self.ctx.block {
            None => self.top_level(),
            Some(Block::Import) => self.import_names(),
            Some(Block::AnnotationArgs(name)) => annotation_args(name),
            Some(Block::Params) => self.params(),
            Some(Block::Body(kind)) => self.body(*kind),
            Some(Block::Endpoint | Block::Nested) => Vec::new(),
        }
    }

    // ---- positions --------------------------------------------------------------

    fn top_level(&self) -> Vec<CompletionItem> {
        match self.ctx.stmt.as_slice() {
            [Tok::Ident(k), Tok::Ident(_)] if k == "model" => {
                vec![item("extends", CompletionItemKind::KEYWORD, None)]
            }
            [Tok::Ident(k), Tok::Ident(_)] if k == "service" => {
                vec![item("at", CompletionItemKind::KEYWORD, None)]
            }
            [Tok::Ident(k), Tok::Ident(_), Tok::Ident(e)] if k == "model" && e == "extends" => {
                self.types(TypePos::Parent)
            }
            [Tok::Ident(k), ..] if k == "const" => self.const_items(),
            [] => decl_keywords(),
            _ => Vec::new(),
        }
    }

    fn const_items(&self) -> Vec<CompletionItem> {
        if self.prev() == Some(&Tok::Punct(':')) {
            self.types(TypePos::Value)
        } else {
            Vec::new()
        }
    }

    fn body(&self, kind: BodyKind) -> Vec<CompletionItem> {
        let st = self.ctx.stmt.as_slice();
        match kind {
            BodyKind::Model | BodyKind::Error => self.field_position(),
            BodyKind::Enum => Vec::new(),
            BodyKind::Union => {
                if self.prev() == Some(&Tok::Punct(':')) {
                    self.types(TypePos::Arm)
                } else {
                    Vec::new()
                }
            }
            BodyKind::Service => {
                if st.contains(&Tok::Arrow) {
                    self.response_position()
                } else if st.is_empty() {
                    method_words()
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn field_position(&self) -> Vec<CompletionItem> {
        let st = self.ctx.stmt.as_slice();
        if let Some(eq) = st.iter().position(|t| *t == Tok::Punct('=')) {
            return self.default_value(&st[..eq], &st[eq + 1..]);
        }
        match self.prev() {
            Some(Tok::Punct(':' | '<' | ',')) if st.contains(&Tok::Punct(':')) => {
                self.types(TypePos::Value)
            }
            _ => Vec::new(),
        }
    }

    fn params(&self) -> Vec<CompletionItem> {
        match self.prev() {
            Some(Tok::Punct(':' | '<' | ',')) if self.ctx.stmt.contains(&Tok::Punct(':')) => {
                let mut v = self.types(TypePos::Value);
                if self.prev() == Some(&Tok::Punct(':')) {
                    v.push(item("stream", CompletionItemKind::KEYWORD, None));
                }
                v
            }
            _ => Vec::new(),
        }
    }

    fn response_position(&self) -> Vec<CompletionItem> {
        match self.prev() {
            Some(Tok::Arrow | Tok::Punct('|' | '<' | ',')) => {
                let mut v = self.types(TypePos::Response);
                if self.prev() == Some(&Tok::Arrow) {
                    v.push(item("stream", CompletionItemKind::KEYWORD, None));
                }
                v
            }
            Some(Tok::Num) => self.types(TypePos::Response),
            _ => Vec::new(),
        }
    }

    // ---- defaults: `= .` ----------------------------------------------------------

    fn default_value(&self, before: &[Tok], after: &[Tok]) -> Vec<CompletionItem> {
        let ty = match before {
            [.., Tok::Punct(':'), Tok::Ident(t)] => Some(t.as_str()),
            _ => None,
        };
        let variants = ty.and_then(|t| self.enum_variants(t)).unwrap_or_default();
        match after {
            [Tok::Punct('.')] => variants
                .iter()
                .map(|v| item(v, CompletionItemKind::ENUM_MEMBER, ty))
                .collect(),
            [] => {
                let mut out: Vec<CompletionItem> = variants
                    .iter()
                    .map(|v| {
                        let mut i = item(&format!(".{v}"), CompletionItemKind::ENUM_MEMBER, ty);
                        i.filter_text = Some(v.clone());
                        i
                    })
                    .collect();
                for lit in ["true", "false", "null"] {
                    out.push(item(lit, CompletionItemKind::KEYWORD, None));
                }
                out
            }
            _ => Vec::new(),
        }
    }

    fn enum_variants(&self, name: &str) -> Option<Vec<String>> {
        let d = self.visible().get(name).copied()?;
        let d = self.ix().decls.get(d)?;
        (d.kind == DKind::Enum).then(|| d.variants.clone())
    }

    // ---- scope ----------------------------------------------------------------------

    /// Names visible in this file: its own declarations, imports and the prelude.
    pub(crate) fn visible(&self) -> BTreeMap<String, usize> {
        crate::scope::visible_in(self.ix(), self.file)
    }

    pub(crate) fn types(&self, pos: TypePos) -> Vec<CompletionItem> {
        let mut out = Vec::new();
        if matches!(pos, TypePos::Value | TypePos::Response) {
            for s in language::SCALARS {
                out.push(item(s, CompletionItemKind::TYPE_PARAMETER, Some("scalar")));
            }
            out.push(snippet(
                "Map",
                CompletionItemKind::TYPE_PARAMETER,
                "Map<string, T>",
                "Map<string, $1>",
            ));
            out.push(snippet(
                "Page",
                CompletionItemKind::TYPE_PARAMETER,
                "Page<T>",
                "Page<$1>",
            ));
        }
        let visible = self.visible();
        let imports = self.import_context();
        for (i, d) in self.ix().decls.iter().enumerate() {
            if !accepts(pos, d.kind) {
                continue;
            }
            match visible.get(&d.name) {
                Some(&v) if v == i => out.push(decl_item(d, None)),
                Some(_) => {}
                None if !d.is_prelude() => {
                    out.push(decl_item(
                        d,
                        Some(imports.edit_for(self.ix(), self.file, d)),
                    ));
                }
                None => {}
            }
        }
        // Aliased imports are visible under their local name.
        for (local, &i) in &visible {
            let d = &self.ix().decls[i];
            if *local != d.name && accepts(pos, d.kind) {
                out.push(decl_item_named(local, d, None));
            }
        }
        out
    }

    // ---- annotations ------------------------------------------------------------------

    fn annotations(&self) -> Vec<CompletionItem> {
        let want: &[Target] = match &self.ctx.block {
            None => language::any_decl_targets(),
            Some(Block::Body(BodyKind::Model | BodyKind::Error)) => &[Target::Field, Target::Rule],
            Some(Block::Body(BodyKind::Enum)) => &[Target::Variant],
            Some(Block::Body(BodyKind::Service)) => {
                &[Target::Endpoint, Target::Require, Target::Ensure]
            }
            Some(Block::Endpoint) => &[Target::Require, Target::Ensure],
            Some(Block::Params) => &[Target::Param],
            Some(Block::Body(BodyKind::Union))
            | Some(Block::Nested)
            | Some(Block::Import)
            | Some(Block::AnnotationArgs(_)) => {
                return Vec::new();
            }
        };
        // Before a top-level declaration an annotation for endpoints only (`@idempotent`) is no use.
        let top_level = self.ctx.block.is_none();
        language::ANNOTATIONS
            .iter()
            .filter(|a| !(top_level && a.targets == [Target::Endpoint]))
            .filter(|a| a.targets.iter().any(|t| want.contains(t)))
            .map(|a| {
                snippet(
                    a.name,
                    CompletionItemKind::FUNCTION,
                    a.usage,
                    &annotation_snippet(a.name, a.args),
                )
            })
            .collect()
    }

    // ---- `use` ------------------------------------------------------------------------

    fn use_paths(&self) -> Vec<CompletionItem> {
        let Some(stmt) = &self.ctx.use_stmt else {
            return Vec::new();
        };
        let ix = self.ix();
        let own = ix
            .file(self.file)
            .map(|f| f.package.clone())
            .unwrap_or_default();
        let (pkg, segments) = parse_use_path(stmt);
        let mut out = Vec::new();
        if pkg.is_none() && segments.is_empty() {
            let mut seen = std::collections::BTreeSet::new();
            for f in &ix.files {
                if f.package != own
                    && f.package_kind != veld_ir::PackageKind::Prelude
                    && seen.insert(f.package.clone())
                {
                    out.push(snippet(
                        &f.package,
                        CompletionItemKind::MODULE,
                        "package",
                        &format!("{}:", f.package),
                    ));
                }
            }
        }
        let target = pkg.unwrap_or(own);
        let mut modules: Vec<&str> = ix
            .files
            .iter()
            .filter(|f| f.package == target && !f.module.is_empty())
            .map(|f| f.module.as_str())
            .collect();
        modules.sort_unstable();
        let prefix = segments.join(".");
        let mut seen = std::collections::BTreeSet::new();
        for m in modules {
            let rest = if prefix.is_empty() {
                m
            } else {
                match m.strip_prefix(&format!("{prefix}.")) {
                    Some(r) => r,
                    None => continue,
                }
            };
            let seg = rest.split('.').next().unwrap_or(rest);
            if !seen.insert(seg.to_string()) {
                continue;
            }
            if rest == seg {
                out.push(snippet(
                    seg,
                    CompletionItemKind::MODULE,
                    "module",
                    &format!("{seg}.{{$0}}"),
                ));
            } else {
                out.push(item(seg, CompletionItemKind::MODULE, Some("module")));
            }
        }
        out
    }

    fn import_names(&self) -> Vec<CompletionItem> {
        let Some(stmt) = &self.ctx.use_stmt else {
            return Vec::new();
        };
        if matches!(self.prev(), Some(Tok::Ident(_))) {
            return vec![item("as", CompletionItemKind::KEYWORD, None)];
        }
        let ix = self.ix();
        let own = ix
            .file(self.file)
            .map(|f| f.package.clone())
            .unwrap_or_default();
        let (pkg, segments) = parse_use_path(stmt);
        let (pkg, module) = (pkg.unwrap_or(own), segments.join("."));
        ix.decls
            .iter()
            .filter(|d| d.package == pkg && d.module == module)
            .map(|d| decl_item(d, None))
            .collect()
    }
}

/// (package, module segments) of a `use` statement's tokens.
fn parse_use_path(stmt: &[Tok]) -> (Option<String>, Vec<String>) {
    let mut pkg = None;
    let mut segs = Vec::new();
    let mut cur: Option<String> = None;
    for t in stmt.iter().skip(1) {
        match t {
            Tok::Ident(w) => cur = Some(w.clone()),
            Tok::Punct(':') => pkg = cur.take(),
            Tok::Punct('.') => segs.extend(cur.take()),
            Tok::Punct(_) | Tok::Arrow | Tok::Str | Tok::Num => {}
        }
    }
    segs.extend(cur);
    // `use pkg:` leaves the package in `pkg`; a lone word before any `.` is a module segment.
    (pkg, segs)
}

fn accepts(pos: TypePos, kind: DKind) -> bool {
    match pos {
        TypePos::Value | TypePos::Arm => matches!(kind, DKind::Model | DKind::Enum | DKind::Union),
        TypePos::Response => !matches!(kind, DKind::Const | DKind::Event),
        TypePos::Parent => kind == DKind::Model,
        TypePos::Event => kind == DKind::Event,
        TypePos::Error => kind == DKind::Error,
    }
}

pub(crate) fn decl_item(d: &DeclEntry, edit: Option<TextEdit>) -> CompletionItem {
    decl_item_named(&d.name, d, edit)
}

fn decl_item_named(label: &str, d: &DeclEntry, edit: Option<TextEdit>) -> CompletionItem {
    let kind = match d.kind {
        DKind::Enum => CompletionItemKind::ENUM,
        DKind::Const => CompletionItemKind::CONSTANT,
        DKind::Event => CompletionItemKind::EVENT,
        DKind::Model | DKind::Error | DKind::Union => CompletionItemKind::CLASS,
    };
    let mut i = item(label, kind, Some(&d.qualified));
    i.documentation = d.docs.clone().map(lsp_types::Documentation::String);
    if let Some(e) = edit {
        i.additional_text_edits = Some(vec![e]);
        i.sort_text = Some(format!("1{label}"));
    } else {
        i.sort_text = Some(format!("0{label}"));
    }
    i
}

fn decl_keywords() -> Vec<CompletionItem> {
    let k = CompletionItemKind::KEYWORD;
    let mut out = vec![
        item("use", k, Some("import declarations")),
        snippet(
            "event",
            k,
            "event Name { ... }",
            "event ${1:Name} {\n  $0\n}",
        ),
        snippet(
            "channel",
            k,
            "channel Name { carries ... }",
            "channel ${1:Name} {\n  carries $0\n}",
        ),
        snippet(
            "model",
            k,
            "model Name { ... }",
            "model ${1:Name} {\n  $0\n}",
        ),
        snippet("enum", k, "enum Name { ... }", "enum ${1:Name} {\n  $0\n}"),
        snippet(
            "error",
            k,
            "error Name 400 { ... }",
            "error ${1:Name} ${2:400} {\n  $0\n}",
        ),
        snippet(
            "union",
            k,
            "union Name { ... }",
            "union ${1:Name} {\n  $0\n}",
        ),
        snippet(
            "service",
            k,
            "service Name at \"/path\" { ... }",
            "service ${1:Name} at \"/${2:path}\" {\n  $0\n}",
        ),
        snippet(
            "const",
            k,
            "const NAME: type = value",
            "const ${1:NAME}: ${2:string} = $0",
        ),
    ];
    out.extend(
        ["policy", "principal", "example"]
            .iter()
            .filter_map(|w| crate::words::word(w))
            .map(crate::complete_rules::word_item),
    );
    out
}

fn method_words() -> Vec<CompletionItem> {
    language::METHOD_WORDS
        .iter()
        .map(|m| {
            snippet(
                m,
                CompletionItemKind::KEYWORD,
                "endpoint",
                &format!("{m} \"${{1:/}}\" ${{2:name}}($3) -> $0"),
            )
        })
        .collect()
}

fn annotation_snippet(name: &str, shape: ArgShape) -> String {
    match shape {
        ArgShape::None => name.to_string(),
        ArgShape::OneString => format!("{name}(\"$1\")"),
        ArgShape::OneNumber => format!("{name}(${{1:n}})"),
        ArgShape::OneLiteral => format!("{name}(${{1:value}})"),
        ArgShape::Auth => format!("{name}(${{1|bearer,basic,none|}})"),
        ArgShape::Idents => format!("{name}(${{1:Name}})"),

        ArgShape::Length => format!("{name}(min: ${{1:n}}, max: ${{2:n}})"),
        ArgShape::Format => format!("{name}(${{1|email,uri,hostname,ipv4,ipv6|}})"),
        ArgShape::Rollout => format!("{name}(audit)"),
        // The arguments are optional: `@idempotent` alone is the usual form.
        ArgShape::Idempotent => name.to_string(),
        ArgShape::Retry => format!("{name}(max: ${{1:3}}, backoff: ${{2|exponential,fixed|}})"),
        ArgShape::Timeout => format!("{name}(seconds(${{1:2}}))"),
        ArgShape::Slo => format!("{name}(p99: ms(${{1:300}}), errors: ${{2:0.1}})"),
        ArgShape::Retain => {
            format!("{name}(days(${{1:30}}), since: ${{2:field}}, then: ${{3|delete,anonymize|}})")
        }
    }
}

fn annotation_args(name: &str) -> Vec<CompletionItem> {
    let words: &[&str] = match language::annotation(name).map(|a| a.args) {
        Some(ArgShape::Auth) => &["bearer", "basic", "none"],
        Some(ArgShape::Format) => &["email", "uri", "hostname", "ipv4", "ipv6"],
        Some(ArgShape::Length) => &["min", "max"],
        Some(ArgShape::Rollout) => &["audit"],
        Some(ArgShape::Idempotent) => &["header", "required"],
        _ => &[],
    };
    let mut out: Vec<CompletionItem> = words
        .iter()
        .map(|w| item(w, CompletionItemKind::ENUM_MEMBER, None))
        .collect();
    if name == "auth" {
        out.push(snippet(
            "apiKey",
            CompletionItemKind::FUNCTION,
            "apiKey(header: \"X\")",
            "apiKey(header: \"${1:X-Api-Key}\")",
        ));
    }
    out
}

// ---- auto-import --------------------------------------------------------------------------

pub(crate) struct ImportContext;

impl Cx<'_> {
    pub(crate) fn import_context(&self) -> ImportContext {
        ImportContext
    }
}

impl ImportContext {
    /// The edit that makes `d` visible in `file`: extend an existing `use` of the same module,
    /// or add a new one after the last `use`.
    pub(crate) fn edit_for(&self, ix: &Index, file: FileId, d: &DeclEntry) -> TextEdit {
        let own = ix.file(file).map(|f| f.package.as_str()).unwrap_or("");
        let Some(f) = ix.file(file) else {
            return TextEdit::new(Range::default(), String::new());
        };
        let text = ix.sources.text(file).unwrap_or("");
        let lines = crate::text::LineIndex::new(text);
        let at = |off: u32| Range::new(lines.position(off as usize), lines.position(off as usize));
        let same_module = f.ast.uses.iter().find(|u| {
            let pkg = u.package.as_ref().map_or(own, |p| p.name.as_str());
            pkg == d.package && u.module_path() == d.module && !u.imports.is_empty()
        });
        if let Some(u) = same_module
            && let Some(last) = u.imports.last()
        {
            return TextEdit::new(at(last.span.end), format!(", {}", d.name));
        }
        let pkg = if d.package == own {
            String::new()
        } else {
            format!("{}:", d.package)
        };
        let line = format!("use {pkg}{}.{{{}}}", d.module, d.name);
        match f.ast.uses.last() {
            Some(u) => TextEdit::new(at(u.span.end), format!("\n{line}")),
            None => TextEdit::new(
                Range::new(Position::new(0, 0), Position::new(0, 0)),
                format!("{line}\n\n"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;
    use std::fs;

    const TOML: &str = "[workspace]\nname = \"p\"\nmembers = [\"iam\", \"app\"]\n";
    const IAM_TOML: &str = "[service]\ncontracts = \".\"\n";
    const APP_TOML: &str = "[service]\ncontracts = \".\"\nconsumes = [\"iam\"]\n";
    const USERS: &str = "/// A user.\nmodel User { id: uuid }\nmodel Team { id: uuid }\nenum Role { admin, guest }\n";

    fn run(app: &str, cursor_marker: &str) -> (Vec<CompletionItem>, String) {
        let tmp = tempfile::tempdir().unwrap();
        for (p, t) in [
            ("veld.toml", TOML),
            ("iam/veld.toml", IAM_TOML),
            ("app/veld.toml", APP_TOML),
            ("iam/users.veld", USERS),
        ] {
            let path = tmp.path().join(p);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, t).unwrap();
        }
        let text = app.replace(cursor_marker, "");
        let off = app.find(cursor_marker).unwrap();
        let path = tmp.path().join("app/app.veld");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &text).unwrap();
        let a = Workspace::new(Some(tmp.path().to_path_buf())).analyze(None);
        let f = a.file_for(&path).unwrap();
        (complete(&a, f, off), text)
    }

    fn labels(v: &[CompletionItem]) -> Vec<&str> {
        v.iter().map(|i| i.label.as_str()).collect()
    }

    #[test]
    fn top_level_offers_declaration_keywords() {
        let (v, _) = run("^", "^");
        let l = labels(&v);
        for k in ["use", "model", "enum", "error", "union", "service", "const"] {
            assert!(l.contains(&k), "{k}");
        }
        let (v, _) = run("model A ^", "^");
        assert_eq!(labels(&v), ["extends"]);
        let (v, _) = run("service S ^", "^");
        assert_eq!(labels(&v), ["at"]);
    }

    #[test]
    fn service_bodies_offer_methods_and_response_types() {
        let (v, _) = run("service S at \"/s\" {\n  ^\n}\n", "^");
        assert!(labels(&v).contains(&"get") && labels(&v).contains(&"ws"));
        let (v, _) = run(
            "model A { x: int }\nservice S at \"/s\" {\n  get \"/\" f() -> A | ^\n}\n",
            "^",
        );
        let l = labels(&v);
        assert!(l.contains(&"NotFound") && l.contains(&"A"), "{l:?}");
        let (v, _) = run("service S at \"/s\" {\n  get \"/\" f(a: ^\n}\n", "^");
        assert!(!labels(&v).contains(&"NotFound"));
        assert!(labels(&v).contains(&"stream") && labels(&v).contains(&"string"));
    }

    #[test]
    fn types_include_scalars_local_and_auto_import_names() {
        let (v, text) = run("model A {\n  x: ^\n}\n", "^");
        let l = labels(&v);
        for t in ["string", "uuid", "Map", "Page", "A", "User", "Role"] {
            assert!(l.contains(&t), "{t} in {l:?}");
        }
        assert!(!l.contains(&"NotFound"), "errors are not field types");
        let user = v.iter().find(|i| i.label == "User").unwrap();
        let edit = &user.additional_text_edits.as_ref().unwrap()[0];
        assert_eq!(edit.new_text, "use iam:users.{User}\n\n");
        assert_eq!(edit.range.start, Position::new(0, 0));
        assert_eq!(user.detail.as_deref(), Some("iam:users.User"));
        let a = v.iter().find(|i| i.label == "A").unwrap();
        assert!(a.additional_text_edits.is_none());
        assert!(text.starts_with("model A"));
    }

    #[test]
    fn auto_import_extends_an_existing_import_or_appends_after_the_last_use() {
        let (v, _) = run("use iam:users.{User}\nmodel A {\n  x: ^\n}\n", "^");
        let team = v.iter().find(|i| i.label == "Team").unwrap();
        let e = &team.additional_text_edits.as_ref().unwrap()[0];
        assert_eq!(e.new_text, ", Team");
        assert_eq!(e.range.start, Position::new(0, 19));
        let user = v.iter().find(|i| i.label == "User").unwrap();
        assert!(user.additional_text_edits.is_none(), "already imported");
        let (v, _) = run("use app2.{X}\nmodel A {\n  x: ^\n}\n", "^");
        let role = v.iter().find(|i| i.label == "Role").unwrap();
        let e = &role.additional_text_edits.as_ref().unwrap()[0];
        assert_eq!(e.new_text, "\nuse iam:users.{Role}");
        assert_eq!(e.range.start, Position::new(0, 12));
    }

    #[test]
    fn annotations_come_with_argument_snippets() {
        let (v, _) = run("model A {\n  x: int @^\n}\n", "^");
        let min = v.iter().find(|i| i.label == "min").unwrap();
        assert_eq!(min.insert_text.as_deref(), Some("min(${1:n})"));
        assert_eq!(min.insert_text_format, Some(InsertTextFormat::SNIPPET));
        assert!(labels(&v).contains(&"readonly") && !labels(&v).contains(&"auth"));
        let (v, _) = run("service S at \"/s\" {\n  @^\n}\n", "^");
        assert!(labels(&v).contains(&"auth") && !labels(&v).contains(&"min"));
        let (v, _) = run("@^\nmodel A {}\n", "^");
        assert!(labels(&v).contains(&"deprecated"));
        let (v, _) = run("model A {\n  x: string @format(^)\n}\n", "^");
        assert!(labels(&v).contains(&"email"));
        let (v, _) = run("service S at \"/s\" {\n  @auth(^)\n}\n", "^");
        assert!(labels(&v).contains(&"bearer") && labels(&v).contains(&"apiKey"));
    }

    #[test]
    fn rollout_is_offered_before_a_rule_a_require_and_an_ensure_and_not_on_a_declaration() {
        let (v, _) = run("model A {\n  x: int\n  @^\n  rule r: x > 0\n}\n", "^");
        let rollout = v.iter().find(|i| i.label == "rollout").unwrap();
        assert_eq!(rollout.insert_text.as_deref(), Some("rollout(audit)"));
        let src = "model A { x: int }\nservice S at \"/s\" {\n  post \"/\" f(body: A) -> 201 A {\n    @^\n    require true\n  }\n}\n";
        let (v, _) = run(src, "^");
        assert!(labels(&v).contains(&"rollout"), "{:?}", labels(&v));
        let (v, _) = run(
            "model A {\n  x: int\n  @rollout(^)\n  rule r: x > 0\n}\n",
            "^",
        );
        assert_eq!(labels(&v), ["audit"]);
        let (v, _) = run("@^\nmodel A {}\n", "^");
        assert!(!labels(&v).contains(&"rollout"));
    }

    #[test]
    fn version_is_offered_on_a_field_and_idempotent_on_an_endpoint_and_nowhere_else() {
        let (v, _) = run("model A {\n  x: int @^\n}\n", "^");
        let version = v.iter().find(|i| i.label == "version").unwrap();
        assert_eq!(version.insert_text.as_deref(), Some("version"));
        assert!(!labels(&v).contains(&"idempotent"), "{:?}", labels(&v));
        let (v, _) = run("service S at \"/s\" {\n  @^\n}\n", "^");
        let idem = v.iter().find(|i| i.label == "idempotent").unwrap();
        assert_eq!(idem.insert_text.as_deref(), Some("idempotent"));
        assert!(idem.detail.as_deref().unwrap_or("").contains("required"));
        assert!(!labels(&v).contains(&"version"), "{:?}", labels(&v));
        let (v, _) = run("@^\nmodel A {}\n", "^");
        assert!(!labels(&v).contains(&"version") && !labels(&v).contains(&"idempotent"));
        // The arguments of `@idempotent` are offered by name.
        let (v, _) = run("service S at \"/s\" {\n  @idempotent(^)\n}\n", "^");
        assert_eq!(labels(&v), ["header", "required"]);
    }

    #[test]
    fn enum_variants_follow_a_dot() {
        let src = "use iam:users.{Role}\nmodel A {\n  r: Role = .^\n}\n";
        let (v, _) = run(src, "^");
        assert_eq!(labels(&v), ["admin", "guest"]);
        let (v, _) = run("use iam:users.{Role}\nmodel A {\n  r: Role = ^\n}\n", "^");
        let l = labels(&v);
        assert!(l.contains(&".admin") && l.contains(&"null"), "{l:?}");
        let (v, _) = run("model A {\n  n: int = .^\n}\n", "^");
        assert!(v.is_empty());
    }

    #[test]
    fn use_paths_and_import_names() {
        let (v, _) = run("use ^", "^");
        assert!(labels(&v).contains(&"iam"));
        let (v, _) = run("use iam:^", "^");
        assert_eq!(labels(&v), ["users"]);
        assert_eq!(v[0].insert_text.as_deref(), Some("users.{$0}"));
        let (v, _) = run("use iam:users.{^}\n", "^");
        assert_eq!(labels(&v), ["User", "Team", "Role"]);
        let (v, _) = run("use iam:users.{User ^}\n", "^");
        assert_eq!(labels(&v), ["as"]);
    }

    #[test]
    fn comments_and_strings_complete_nothing() {
        let (v, _) = run("model A {\n  // hello ^\n}\n", "^");
        assert!(v.is_empty());
        let (v, _) = run("service S at \"/a^\" {\n}\n", "^");
        assert!(v.is_empty());
    }
}
