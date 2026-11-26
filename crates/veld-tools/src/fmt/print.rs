//! Prints the syntax tree in the canonical style.

use veld_compiler::Comment;
use veld_compiler::syntax::ast::{
    Annotation, Decl, DeclKind, Doc, Endpoint, EnumDecl, Example, ExampleBody, Field, File,
    HttpMethod, Param, PolicyDecl, PrincipalDecl, RespItem, Rule, Stmt, UnionDecl, Use, Variant,
};
use veld_compiler::syntax::lexer::CommentKind;

use super::WIDTH;
use super::expr::{expr_text, rule_lines};
use super::layout::{self, Attached, Layout, Lead, ROOT};
use super::stmt;
use super::text::{annotation, literal, quote, type_expr};

mod events;

pub(super) fn print_file(file: &File, lay: &Layout) -> String {
    let mut p = Printer {
        out: String::new(),
        lay,
        skip: None,
    };
    p.file(file);
    p.out
}

struct Printer<'a> {
    out: String,
    lay: &'a Layout,
    /// Leading comments of this item already printed as the file header.
    skip: Option<(u32, usize)>,
}

fn comment_text(c: &Comment) -> String {
    match c.kind {
        CommentKind::Line => format!("//{}", c.text),
        CommentKind::Block => format!("/*{}*/", c.text),
    }
}

fn pad(n: usize) -> String {
    " ".repeat(n)
}

fn use_sort_key(u: &Use) -> (String, String) {
    (
        u.package
            .as_ref()
            .map_or_else(String::new, |p| p.name.clone()),
        u.module_path(),
    )
}

fn use_line(u: &Use) -> String {
    let mut imports: Vec<(&str, Option<&str>)> = u
        .imports
        .iter()
        .map(|i| {
            (
                i.name.name.as_str(),
                i.alias.as_ref().map(|a| a.name.as_str()),
            )
        })
        .collect();
    imports.sort_unstable();
    let names: Vec<String> = imports
        .iter()
        .map(|(n, a)| a.map_or_else(|| (*n).to_string(), |a| format!("{n} as {a}")))
        .collect();
    let pkg = u
        .package
        .as_ref()
        .map_or_else(String::new, |p| format!("{}:", p.name));
    format!("use {pkg}{}.{{{}}}", u.module_path(), names.join(", "))
}

impl Printer<'_> {
    fn att(&self, key: u32) -> Option<&Attached> {
        self.lay.get(key)
    }

    fn blank_line(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with("\n\n") {
            self.out.push('\n');
        }
    }

    fn line(&mut self, ind: usize, text: &str) {
        self.out.push_str(&pad(ind));
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn file(&mut self, f: &File) {
        self.file_header(f);
        let mut uses: Vec<&Use> = f.uses.iter().collect();
        uses.sort_by_key(|u| use_sort_key(u));
        for u in uses {
            self.leading(u.span.start, 0);
            self.out.push_str(&use_line(u));
            self.end_line(u.span.start, 0);
        }
        let mut items: Vec<(u32, Item<'_>)> = f
            .decls
            .iter()
            .map(|d| (layout::decl_key(d), Item::Decl(d)))
            .chain(
                f.examples
                    .iter()
                    .map(|x| (layout::example_key(x), Item::Example(x))),
            )
            .chain(
                f.policies
                    .iter()
                    .map(|x| (layout::policy_key(x), Item::Policy(x))),
            )
            .chain(
                f.principals
                    .iter()
                    .map(|x| (layout::principal_key(x), Item::Principal(x))),
            )
            .collect();
        items.sort_by_key(|(k, _)| *k);
        for (_, item) in items {
            self.blank_line();
            match item {
                Item::Decl(d) => self.decl(d),
                Item::Example(x) => self.example(x),
                Item::Policy(x) => self.policy(x),
                Item::Principal(x) => self.principal(x),
            }
        }
        let tail = self.att(ROOT).map(|a| a.tail.clone()).unwrap_or_default();
        if !tail.is_empty() {
            self.blank_line();
            self.lead_list(&tail, 0);
        }
        while self.out.ends_with("\n\n") {
            self.out.pop();
        }
    }

    /// Comments at the top of the file that are followed by a blank line stay
    /// at the top: sorting `use` lines must not carry them away.
    fn file_header(&mut self, f: &File) {
        let first = f
            .uses
            .iter()
            .map(|u| u.span.start)
            .chain(f.decls.iter().map(layout::decl_key))
            .chain(f.examples.iter().map(layout::example_key))
            .chain(f.policies.iter().map(layout::policy_key))
            .chain(f.principals.iter().map(layout::principal_key))
            .min();
        let Some(key) = first else { return };
        let list = self.att(key).map(|a| a.leading.clone()).unwrap_or_default();
        let Some(last) = list.iter().rposition(|l| l.gap_after) else {
            return;
        };
        self.lead_list(&list[..=last], 0);
        self.skip = Some((key, last + 1));
    }

    /// Leading comments of the item stored under `key`.
    fn leading(&mut self, key: u32, ind: usize) {
        let mut list = self.att(key).map(|a| a.leading.clone()).unwrap_or_default();
        if let Some((k, n)) = self.skip
            && k == key
        {
            list.drain(..n);
        }
        self.lead_list(&list, ind);
    }

    fn lead_list(&mut self, list: &[Lead], ind: usize) {
        for l in list {
            self.line(ind, &comment_text(&l.comment));
            if l.gap_after {
                self.out.push('\n');
            }
        }
    }

    /// Writes ` c1 c2` after the text already on the line. Block comments come
    /// first so a line comment never swallows another comment. Extra line
    /// comments move to their own lines.
    fn inline_comments(&mut self, cs: &[Comment], ind: usize) {
        let (blocks, lines): (Vec<&Comment>, Vec<&Comment>) =
            cs.iter().partition(|c| c.kind == CommentKind::Block);
        for c in blocks {
            self.out.push(' ');
            self.out.push_str(&comment_text(c));
        }
        for (i, c) in lines.into_iter().enumerate() {
            if i == 0 {
                self.out.push(' ');
            } else {
                self.out.push('\n');
                self.out.push_str(&pad(ind));
            }
            self.out.push_str(&comment_text(c));
        }
    }

    /// Ends the current line, first adding the trailing comments of `key`.
    fn end_line(&mut self, key: u32, ind: usize) {
        let cs = self
            .att(key)
            .map(|a| a.trailing.clone())
            .unwrap_or_default();
        self.inline_comments(&cs, ind);
        self.out.push('\n');
    }

    fn docs(&mut self, docs: &[Doc], ind: usize) {
        for d in docs {
            if d.text.is_empty() {
                self.line(ind, "///");
            } else {
                self.line(ind, &format!("/// {}", d.text));
            }
        }
    }

    fn annotations_own_lines(&mut self, anns: &[Annotation], ind: usize) {
        for a in anns {
            self.line(ind, &annotation(a));
        }
    }

    // ---- declarations ---------------------------------------------------

    fn decl(&mut self, d: &Decl) {
        let key = layout::decl_key(d);
        self.leading(key, 0);
        self.docs(&d.docs, 0);
        self.annotations_own_lines(&d.annotations, 0);
        match &d.kind {
            DeclKind::Model(m) => {
                let ext = m
                    .extends
                    .as_ref()
                    .map_or_else(String::new, |e| format!(" extends {}", type_expr(e)));
                let head = format!("model {}{ext}", m.name.name);
                self.block(
                    key,
                    &head,
                    0,
                    m.fields.is_empty() && m.rules.is_empty(),
                    |p| {
                        p.members(&m.fields, &m.rules, 2);
                    },
                );
            }
            DeclKind::Error(e) => {
                let status = e.status.map_or_else(String::new, |(s, _)| format!(" {s}"));
                let head = format!("error {}{status}", e.name.name);
                self.block(
                    key,
                    &head,
                    0,
                    e.fields.is_empty() && e.rules.is_empty(),
                    |p| {
                        p.members(&e.fields, &e.rules, 2);
                    },
                );
            }
            DeclKind::Enum(e) => self.enum_decl(key, e),
            DeclKind::Union(u) => self.union_decl(key, u),
            DeclKind::Service(s) => self.service(key, s),
            DeclKind::Event(e) => {
                let head = format!("event {}", e.name.name);
                self.block(
                    key,
                    &head,
                    0,
                    e.fields.is_empty() && e.rules.is_empty(),
                    |p| {
                        p.members(&e.fields, &e.rules, 2);
                    },
                );
            }
            DeclKind::Channel(c) => self.channel(key, c),
            DeclKind::Const(c) => {
                let text = format!(
                    "const {}: {} = {}",
                    c.name.name,
                    type_expr(&c.ty),
                    literal(&c.value)
                );
                self.out.push_str(&text);
                self.end_line(key, 0);
            }
        }
    }

    /// Prints `head {`, the body and `}`; an empty block with no comments is `head {}`.
    fn block(
        &mut self,
        key: u32,
        head: &str,
        ind: usize,
        empty: bool,
        body: impl FnOnce(&mut Self),
    ) {
        let (header, tail) = self
            .att(key)
            .map(|a| (a.header.clone(), a.tail.clone()))
            .unwrap_or_default();
        self.out.push_str(&pad(ind));
        self.out.push_str(head);
        if empty && header.is_empty() && tail.is_empty() {
            self.out.push_str(" {}");
            self.end_line(key, ind);
            return;
        }
        self.out.push_str(" {");
        self.inline_comments(&header, ind + 2);
        self.out.push('\n');
        body(self);
        self.lead_list(&tail, ind + 2);
        self.out.push_str(&pad(ind));
        self.out.push('}');
        self.end_line(key, ind);
    }

    fn fields(&mut self, fields: &[Field], ind: usize) {
        for f in fields {
            let key = layout::field_key(f);
            self.leading(key, ind);
            self.docs(&f.docs, ind);
            self.out.push_str(&pad(ind));
            self.out.push_str(&field_line(f));
            self.end_line(key, ind);
        }
    }

    /// Fields first, one blank line, then the rules, each in source order (`spec/rules.md` 4).
    fn members(&mut self, fields: &[Field], rules: &[Rule], ind: usize) {
        self.fields(fields, ind);
        if !fields.is_empty() && !rules.is_empty() {
            self.out.push('\n');
        }
        for r in rules {
            let key = layout::rule_key(r);
            self.leading(key, ind);
            self.docs(&r.docs, ind);
            self.annotations_own_lines(&r.annotations, ind);
            self.out.push_str(&rule_lines(r, ind).join("\n"));
            self.end_line(key, ind);
        }
    }

    fn has_comments(&self, keys: &[u32]) -> bool {
        keys.iter().any(|k| {
            self.att(*k)
                .is_some_and(|a| !a.leading.is_empty() || !a.trailing.is_empty())
        })
    }

    fn node_quiet(&self, key: u32) -> bool {
        self.att(key)
            .is_none_or(|a| a.header.is_empty() && a.tail.is_empty())
    }

    fn enum_decl(&mut self, key: u32, e: &EnumDecl) {
        let keys: Vec<u32> = e.variants.iter().map(layout::variant_key).collect();
        let items: Vec<String> = e.variants.iter().map(variant_text).collect();
        let plain = e
            .variants
            .iter()
            .all(|v| v.docs.is_empty() && v.annotations.is_empty());
        let one = if items.is_empty() {
            format!("enum {} {{}}", e.name.name)
        } else {
            format!("enum {} {{ {} }}", e.name.name, items.join(", "))
        };
        let machine = !e.flows.is_empty() || !e.finals.is_empty();
        if plain
            && !machine
            && self.node_quiet(key)
            && !self.has_comments(&keys)
            && one.chars().count() <= WIDTH
        {
            self.out.push_str(&one);
            self.end_line(key, 0);
            return;
        }
        let head = format!("enum {}", e.name.name);
        self.block(key, &head, 0, items.is_empty() && !machine, |p| {
            for (v, k) in e.variants.iter().zip(&keys) {
                p.list_item(*k, &v.docs, &v.annotations, &variant_text(v), 2);
            }
            p.machine(e);
        });
    }

    /// The flows, then the finals of an enum, each in source order, after a blank line.
    fn machine(&mut self, e: &EnumDecl) {
        if e.flows.is_empty() && e.finals.is_empty() {
            return;
        }
        if !e.variants.is_empty() {
            self.out.push('\n');
        }
        let flows = e.flows.iter().map(|f| {
            let after = f.after.as_ref().map(|d| format!(" after {}", expr_text(d)));
            (f.span.start, "flow", " -> ", &f.states, after)
        });
        let finals = e
            .finals
            .iter()
            .map(|f| (f.span.start, "final", ", ", &f.states, None));
        for (key, word, sep, states, suffix) in flows.chain(finals) {
            let names: Vec<&str> = states.iter().map(|s| s.name.as_str()).collect();
            self.leading(key, 2);
            self.out.push_str(&format!(
                "  {word} {}{}",
                names.join(sep),
                suffix.unwrap_or_default()
            ));
            self.end_line(key, 2);
        }
    }

    fn list_item(&mut self, key: u32, docs: &[Doc], anns: &[Annotation], text: &str, ind: usize) {
        self.leading(key, ind);
        self.docs(docs, ind);
        let mut line = String::new();
        for a in anns {
            line.push_str(&annotation(a));
            line.push(' ');
        }
        line.push_str(text);
        line.push(',');
        self.out.push_str(&pad(ind));
        self.out.push_str(&line);
        self.end_line(key, ind);
    }

    fn union_decl(&mut self, key: u32, u: &UnionDecl) {
        let keys: Vec<u32> = u.arms.iter().map(|a| a.span.start).collect();
        let items: Vec<String> = u
            .arms
            .iter()
            .map(|a| format!("{}: {}", a.label.name, type_expr(&a.ty)))
            .collect();
        let one = if items.is_empty() {
            format!("union {} {{}}", u.name.name)
        } else {
            format!("union {} {{ {} }}", u.name.name, items.join(", "))
        };
        if self.node_quiet(key) && !self.has_comments(&keys) && one.chars().count() <= WIDTH {
            self.out.push_str(&one);
            self.end_line(key, 0);
            return;
        }
        let head = format!("union {}", u.name.name);
        self.block(key, &head, 0, items.is_empty(), |p| {
            for (it, k) in items.iter().zip(&keys) {
                p.list_item(*k, &[], &[], it, 2);
            }
        });
    }

    // ---- services ---------------------------------------------------------

    fn endpoint(&mut self, e: &Endpoint, lines: &[String], ind: usize) {
        let key = layout::endpoint_key(e);
        self.leading(key, ind);
        self.docs(&e.docs, ind);
        self.annotations_own_lines(&e.annotations, ind);
        let Some(block) = &e.block else {
            self.out.push_str(&lines.join("\n"));
            self.end_line(key, ind);
            return;
        };
        let joined = lines.join("\n");
        let head = joined.strip_prefix(&pad(ind)).unwrap_or(&joined);
        self.block(key, head, ind, block.stmts.is_empty(), |p| {
            p.statements(&block.stmts, ind + 2);
        });
    }

    /// Facts, `allow`, `transition`, `require`, then `ensure`, each group in source order (`spec/rules.md` 9).
    fn statements(&mut self, stmts: &[Stmt], ind: usize) {
        for group in 0..5 {
            for s in stmts.iter().filter(|s| stmt::stmt_group(s) == group) {
                let key = s.start();
                self.leading(key, ind);
                self.docs(stmt::stmt_docs(s), ind);
                self.annotations_own_lines(stmt::stmt_annotations(s), ind);
                self.out.push_str(&stmt::stmt_lines(s, ind).join("\n"));
                self.end_line(key, ind);
            }
        }
    }

    fn example(&mut self, x: &Example) {
        let key = layout::example_key(x);
        self.leading(key, 0);
        self.docs(&x.docs, 0);
        self.block(key, &stmt::example_head(x), 0, false, |p| {
            for g in &x.givens {
                p.example_line(g.span.start, &stmt::given_line(g));
            }
            if let Some(a) = &x.principal {
                p.example_line(a.span.start, &format!("as {}", stmt::value_text(&a.value)));
            }
            if let Some(a) = &x.at {
                p.example_line(a.span.start, &format!("at {}", quote(&a.time.value)));
            }
            match &x.body {
                ExampleBody::Call(c) => {
                    if let Some(i) = &c.if_match {
                        p.example_line(i.span.start, &stmt::if_match_line(i));
                    }
                    if let Some(k) = &c.key {
                        p.example_line(k.span.start, &stmt::key_line(k));
                    }
                    let key = c.span.start;
                    p.leading(key, 2);
                    p.out.push_str(&stmt::call_lines(c, 2).join("\n"));
                    p.end_line(key, 2);
                }
                ExampleBody::Event(e) => p.event_example(e),
            }
        });
    }

    fn policy(&mut self, p: &PolicyDecl) {
        let key = layout::policy_key(p);
        self.leading(key, 0);
        self.docs(&p.docs, 0);
        self.block(key, &stmt::policy_head(p), 0, p.clauses.is_empty(), |s| {
            for c in &p.clauses {
                let ck = c.span.start;
                s.leading(ck, 2);
                s.out.push_str(&stmt::clause_lines(c, 2).join("\n"));
                s.end_line(ck, 2);
            }
        });
    }

    fn principal(&mut self, p: &PrincipalDecl) {
        let key = layout::principal_key(p);
        self.leading(key, 0);
        self.docs(&p.docs, 0);
        self.out.push_str(&stmt::principal_line(p));
        self.end_line(key, 0);
    }

    fn example_line(&mut self, key: u32, text: &str) {
        self.leading(key, 2);
        self.line_no_newline(2, text);
        self.end_line(key, 2);
    }

    fn line_no_newline(&mut self, ind: usize, text: &str) {
        self.out.push_str(&pad(ind));
        self.out.push_str(text);
    }
}

enum Item<'a> {
    Decl(&'a Decl),
    Example(&'a Example),
    Policy(&'a PolicyDecl),
    Principal(&'a PrincipalDecl),
}

fn field_line(f: &Field) -> String {
    let mut s = format!(
        "{}{}: {}",
        f.name.name,
        if f.optional { "?" } else { "" },
        type_expr(&f.ty)
    );
    if let Some(d) = &f.default {
        s.push_str(" = ");
        s.push_str(&literal(d));
    }
    for a in &f.annotations {
        s.push(' ');
        s.push_str(&annotation(a));
    }
    s
}

fn variant_text(v: &Variant) -> String {
    match &v.wire {
        Some((w, _)) => format!("{} = {}", v.name.name, quote(w)),
        None => v.name.name.clone(),
    }
}

fn param_text(p: &Param) -> String {
    let mut s = String::new();
    for a in &p.annotations {
        s.push_str(&annotation(a));
        s.push(' ');
    }
    s.push_str(&p.name.name);
    if p.optional {
        s.push('?');
    }
    s.push_str(": ");
    if p.stream {
        s.push_str("stream ");
    }
    s.push_str(&type_expr(&p.ty));
    s
}

fn response_text(r: &RespItem) -> String {
    let mut parts = Vec::new();
    if let Some((s, _)) = r.status {
        parts.push(s.to_string());
    }
    if r.stream {
        parts.push("stream".to_string());
    }
    if let Some(t) = &r.ty {
        parts.push(type_expr(t));
    }
    parts.join(" ")
}

/// The lines of one endpoint, indented by `ind`. Responses wrap onto
/// continuation lines (indent + 4) when the flat form is wider than `WIDTH`.
fn endpoint_lines(e: &Endpoint, ind: usize) -> Vec<String> {
    let params: Vec<String> = e.params.iter().map(param_text).collect();
    let head = format!(
        "{}{} {} {}({})",
        pad(ind),
        method_word(e.method),
        quote(&e.path.value),
        e.name.name,
        params.join(", ")
    );
    let Some(r) = &e.responses else {
        return vec![head];
    };
    let items: Vec<String> = r.items.iter().map(response_text).collect();
    let flat = format!("{head} -> {}", items.join(" | "));
    if flat.chars().count() <= WIDTH || items.len() == 1 {
        return vec![flat];
    }
    let mut lines = vec![format!("{head} -> {}", items[0])];
    lines.extend(items[1..].iter().map(|i| format!("{}| {i}", pad(ind + 4))));
    lines
}

fn method_word(m: HttpMethod) -> &'static str {
    match m {
        HttpMethod::Get => "get",
        HttpMethod::Post => "post",
        HttpMethod::Put => "put",
        HttpMethod::Patch => "patch",
        HttpMethod::Delete => "delete",
        HttpMethod::Head => "head",
        HttpMethod::Options => "options",
        HttpMethod::Ws => "ws",
    }
}
