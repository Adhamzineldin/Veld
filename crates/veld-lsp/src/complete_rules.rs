//! Completion for the rules language (`spec/rules.md`): expressions, the words of statements,
//! errors after `else`, policies after `allow`, states, and the words of examples.
//!
//! The line being typed is rarely valid, so the position comes from `scan::site` (the text
//! before the cursor) and only the enclosing declaration, endpoint or policy is read from the
//! syntax tree. A statement that did not parse is simply not in the tree.

use lsp_types::{CompletionItem, CompletionItemKind, Documentation};
use veld_compiler::Span;
use veld_compiler::index::Index;
use veld_compiler::semantic::DKind;
use veld_compiler::syntax::ast::DeclKind;

use crate::complete::{Cx, TypePos, item, snippet};
use crate::query::service_ast;
use crate::rfind::endpoint_called;
use crate::scan::{Frame, K, Site, Tk, site};
use crate::scope::{
    self, Lambda, LocalKind, Phase, Scope, Ty, Val, endpoint_scope, member, members, policy_scope,
    rule_scope, visible_in,
};
use crate::words::{self, Class, Word};

/// A completion item for a word of the table.
pub(crate) fn word_item(w: &Word) -> CompletionItem {
    let kind = match w.class {
        Class::Function => CompletionItemKind::FUNCTION,
        Class::Keyword | Class::Operator => CompletionItemKind::KEYWORD,
        Class::Name => CompletionItemKind::VARIABLE,
    };
    let mut i = snippet(w.word, kind, w.signature, w.snippet);
    i.documentation = Some(Documentation::String(w.meaning.to_owned()));
    i
}

fn words_items(names: &[&str]) -> Vec<CompletionItem> {
    words::statement_words(names)
        .into_iter()
        .map(word_item)
        .collect()
}

/// Words that cannot end an operand.
fn is_operator_word(t: &Tk) -> bool {
    t.is_word()
        && matches!(
            t.text.as_str(),
            "not"
                | "in"
                | "implies"
                | "else"
                | "message"
                | "when"
                | "require"
                | "ensure"
                | "permit"
                | "forbid"
                | "fact"
                | "allow"
                | "transition"
                | "rule"
        )
}

/// True when the token can end a value: a name, a literal or a closing bracket.
fn ends_operand(t: &Tk) -> bool {
    match t.k {
        K::Word => !is_operator_word(t),
        K::Num | K::Str => true,
        K::Punct => t.is(")") || t.is("]"),
    }
}

/// `a.b.c` read backwards from the token before `end`.
fn path_before(toks: &[Tk], end: usize) -> Option<Vec<String>> {
    let mut segs = Vec::new();
    let mut i = end;
    loop {
        let t = toks.get(i.checked_sub(1)?)?;
        if !t.is_word() || is_operator_word(t) {
            return None;
        }
        segs.push(t.text.clone());
        let before = i.checked_sub(2).and_then(|p| toks.get(p));
        match before {
            Some(d) if d.is(".") && i >= 3 => i -= 2,
            Some(_) | None => break,
        }
    }
    segs.reverse();
    Some(segs)
}

/// The value of the path `segs`, read in `scope`.
fn resolve_segs(ix: &Index, scope: &Scope, lams: &[Lambda], segs: &[String]) -> Val {
    let Some((first, rest)) = segs.split_first() else {
        return Val::unknown();
    };
    let mut val = scope::lookup(ix, scope, lams, first).unwrap_or_else(Val::unknown);
    for s in rest {
        val = member(ix, &val.ty, s).map_or_else(Val::unknown, |m| m.val);
    }
    val
}

/// The tokens that make up a path and nothing else.
fn whole_path(toks: &[Tk]) -> Option<Vec<String>> {
    let last = toks.len();
    let segs = path_before(toks, last)?;
    (segs.len() * 2 - 1 == last).then_some(segs)
}

impl Cx<'_> {
    /// The completion for a position of the rules language, `None` when the cursor is elsewhere.
    pub(crate) fn rules_position(&self) -> Option<Vec<CompletionItem>> {
        let s = site(self.prefix);
        if s.inert {
            return Some(Vec::new());
        }
        match s.frame().clone() {
            Frame::Decl { kw, name } if matches!(kw.as_str(), "model" | "error" | "event") => {
                self.in_record(&s, &name)
            }
            Frame::Decl { kw, name } if kw == "enum" => self.in_enum(&s, &name),
            Frame::Endpoint { service, name } => self.in_endpoint(&s, &service, &name),
            Frame::Payload { error } => self.in_payload(&s, &error),
            Frame::Policy { name } => self.in_policy(&s, &name),
            Frame::Example => self.in_example(&s),
            Frame::Object => Some(self.in_object(&s)),
            Frame::Top | Frame::Decl { .. } | Frame::Service { .. } | Frame::Other => None,
        }
    }

    // ---- expressions --------------------------------------------------------------------

    /// The lambda variables that the open calls of the statement declare.
    fn lambdas(&self, s: &Site, scope: &Scope) -> Vec<Lambda> {
        let mut out: Vec<Lambda> = Vec::new();
        let calls = s.opens.iter().filter(|o| {
            o.paren && matches!(o.call.as_deref(), Some("all" | "any" | "count" | "sum"))
        });
        for o in calls {
            let after = s.toks.get(o.at + 1..).unwrap_or(&[]);
            let comma = after.iter().position(|t| t.is(",")).unwrap_or(after.len());
            let first = after.get(..comma).unwrap_or(&[]);
            let elem = whole_path(first).map_or(Ty::Unknown, |p| {
                resolve_segs(self.ix(), scope, &out, &p).ty.elem()
            });
            for (j, t) in after.iter().enumerate() {
                if t.is_word() && after.get(j + 1).is_some_and(|n| n.is("=>")) {
                    out.push(Lambda {
                        name: t.text.clone(),
                        val: Val::new(elem.clone()),
                        span: Span::new(self.file, t.start, t.end),
                    });
                }
            }
        }
        out
    }

    /// The items for an expression whose tokens end the text before the cursor. `from` is the
    /// index of the first token of the expression; `ops` are the words that may follow it.
    fn expr_items(
        &self,
        s: &Site,
        scope: &Scope,
        from: usize,
        ops: &[&str],
    ) -> Vec<CompletionItem> {
        let lams = self.lambdas(s, scope);
        let last = if s.toks.len() > from {
            s.toks.last()
        } else {
            None
        };
        match last {
            Some(t) if t.is(".") => self.after_dot(s, scope, &lams),
            Some(t) if ends_operand(t) => operator_items(ops),
            Some(_) | None => self.operands(s, scope, &lams),
        }
    }

    fn after_dot(&self, s: &Site, scope: &Scope, lams: &[Lambda]) -> Vec<CompletionItem> {
        let ix = self.ix();
        let dot = s.toks.len().saturating_sub(1);
        let prev = dot.checked_sub(1).and_then(|p| s.toks.get(p));
        if prev.is_some_and(|p| p.is_word() && !is_operator_word(p)) {
            let val = path_before(&s.toks, dot)
                .map_or_else(Val::unknown, |p| resolve_segs(ix, scope, lams, &p));
            return members(ix, &val.ty)
                .into_iter()
                .map(|m| {
                    let mut i = item(&m.name, CompletionItemKind::FIELD, Some(&m.val.show(ix)));
                    i.sort_text = Some(format!("0{}", m.name));
                    i
                })
                .collect();
        }
        if prev.is_some_and(|p| p.is(")")) {
            return Vec::new();
        }
        self.enum_literals(s, scope, lams, dot)
    }

    /// The variants of the enum the context expects after a `.`: from the other side of `==`,
    /// `!=` or `in`, else the variants of every enum in sight.
    fn enum_literals(
        &self,
        s: &Site,
        scope: &Scope,
        lams: &[Lambda],
        dot: usize,
    ) -> Vec<CompletionItem> {
        let ix = self.ix();
        let before = s.toks.get(..dot).unwrap_or(&[]);
        let other = match before.last() {
            Some(t) if t.is("==") || t.is("!=") => path_before(before, before.len()),
            Some(t) if t.is("[") || t.is(",") => {
                let open = list_open(before);
                let word = open
                    .and_then(|o| o.checked_sub(1))
                    .and_then(|p| before.get(p));
                match (open, word) {
                    (Some(o), Some(w)) if w.is("in") => path_before(before, o.saturating_sub(1)),
                    _ => None,
                }
            }
            Some(_) | None => None,
        };
        let hint = other.map(|p| resolve_segs(ix, scope, lams, &p).ty);
        let decls: Vec<usize> = match hint.and_then(|t| t.enum_decl(ix)) {
            Some(d) => vec![d],
            None => visible_in(ix, self.file)
                .into_values()
                .filter(|&d| ix.decls.get(d).is_some_and(|e| e.kind == DKind::Enum))
                .collect(),
        };
        variant_items(ix, &decls)
    }

    fn operands(&self, s: &Site, scope: &Scope, lams: &[Lambda]) -> Vec<CompletionItem> {
        let ix = self.ix();
        let mut out = Vec::new();
        for l in &scope.locals {
            let (kind, what) = match &l.kind {
                LocalKind::Field { .. } => (CompletionItemKind::FIELD, "field"),
                LocalKind::Param { .. } | LocalKind::PolicyParam { .. } => {
                    (CompletionItemKind::VARIABLE, "parameter")
                }
                LocalKind::Fact { .. } => (CompletionItemKind::VARIABLE, "fact"),
                LocalKind::Principal { .. } => (CompletionItemKind::VARIABLE, "the caller"),
                LocalKind::Now | LocalKind::Result => (CompletionItemKind::VARIABLE, "built-in"),
            };
            let mut i = item(&l.name, kind, Some(&format!("{what}: {}", l.val.show(ix))));
            i.sort_text = Some(format!("0{}", l.name));
            out.push(i);
        }
        for l in lams {
            let mut i = item(&l.name, CompletionItemKind::VARIABLE, Some(&l.val.show(ix)));
            i.sort_text = Some(format!("0{}", l.name));
            out.push(i);
        }
        let in_ensure = s.first() == Some("ensure");
        for b in words::BUILTINS
            .iter()
            .filter(|b| b.word != "old" || in_ensure)
        {
            let mut i = word_item(b);
            i.sort_text = Some(format!("1{}", b.word));
            out.push(i);
        }
        out.extend(self.consts());
        out.extend(words_items(&["true", "false", "not"]));
        out
    }

    fn consts(&self) -> Vec<CompletionItem> {
        let ix = self.ix();
        visible_in(ix, self.file)
            .into_values()
            .filter_map(|d| ix.decls.get(d))
            .filter(|d| d.kind == DKind::Const)
            .map(|d| item(&d.name, CompletionItemKind::CONSTANT, d.const_ty.as_deref()))
            .collect()
    }

    // ---- scopes ------------------------------------------------------------------------

    fn endpoint_ref(&self, service: &str, endpoint: &str) -> Option<(usize, usize)> {
        let ix = self.ix();
        let si = ix
            .services
            .iter()
            .position(|s| s.file == self.file && s.name == service)?;
        let ei = service_ast(ix, si)?
            .endpoints
            .iter()
            .position(|e| e.name.name == endpoint)?;
        Some((si, ei))
    }

    fn endpoint_scope_at(&self, service: &str, endpoint: &str, phase: Phase) -> Scope {
        match self.endpoint_ref(service, endpoint) {
            Some((si, ei)) => endpoint_scope(self.ix(), si, ei, phase),
            None => Scope {
                file: Some(self.file),
                locals: Vec::new(),
            },
        }
    }

    // ---- invariants ---------------------------------------------------------------------

    fn in_record(&self, s: &Site, name: &str) -> Option<Vec<CompletionItem>> {
        let st = s.stmt_toks();
        if st.is_empty() {
            return s.line_start.then(|| words_items(&["rule"]));
        }
        if s.first() != Some("rule") {
            return None;
        }
        let Some(colon) = st.iter().position(|t| t.is(":")) else {
            return Some(Vec::new());
        };
        if st.iter().skip(colon).any(|t| t.is("message")) {
            return Some(Vec::new());
        }
        let ix = self.ix();
        let decl = ix
            .decls
            .iter()
            .position(|d| d.file == self.file && d.name == name);
        let scope = decl.map_or_else(
            || Scope {
                file: Some(self.file),
                locals: Vec::new(),
            },
            |d| rule_scope(ix, d),
        );
        Some(self.expr_items(s, &scope, s.stmt + colon + 1, &["message"]))
    }

    // ---- state machines -----------------------------------------------------------------

    fn in_enum(&self, s: &Site, name: &str) -> Option<Vec<CompletionItem>> {
        let st = s.stmt_toks();
        let Some(first) = st.first() else {
            return s.line_start.then(|| words_items(&["flow", "final"]));
        };
        let last = st.last()?;
        let wants_state = match first.text.as_str() {
            "flow" => last.is("flow") || last.is("->"),
            "final" => last.is("final") || last.is(","),
            _ => return None,
        };
        let ix = self.ix();
        let decl = ix
            .decls
            .iter()
            .position(|d| d.file == self.file && d.name == name);
        Some(match (wants_state, decl) {
            (true, Some(d)) => variant_items(ix, &[d]),
            _ => Vec::new(),
        })
    }

    // ---- endpoint blocks ----------------------------------------------------------------

    fn in_endpoint(&self, s: &Site, service: &str, endpoint: &str) -> Option<Vec<CompletionItem>> {
        let st = s.stmt_toks();
        let complete = st.last().is_some_and(ends_operand) && s.opens.is_empty();
        let words = ["fact", "allow", "transition", "require", "ensure"];
        let Some(first) = s.first() else {
            return Some(words_items(&words));
        };
        if s.line_start && complete {
            let mut out = words_items(&words);
            out.extend(operator_items(if first == "require" {
                &["else", "message"]
            } else {
                &["message"]
            }));
            return Some(out);
        }
        let from = s.stmt + 1;
        Some(match first {
            "fact" => self.fact_items(s, service, endpoint),
            "require" => self.require_items(s, service, endpoint, from),
            "ensure" => {
                let scope = self.endpoint_scope_at(service, endpoint, Phase::Ensure);
                self.statement_expr(s, &scope, from, &["message"])
            }
            "allow" => self.allow_items(s, service, endpoint),
            "transition" => self.transition_items(s, service, endpoint),
            _ => words_items(&words),
        })
    }

    /// The expression of a `require` or an `ensure`, after an optional `name:`.
    fn statement_expr(
        &self,
        s: &Site,
        scope: &Scope,
        from: usize,
        ops: &[&str],
    ) -> Vec<CompletionItem> {
        let named = s.toks.get(from).is_some_and(Tk::is_word)
            && s.toks.get(from + 1).is_some_and(|t| t.is(":"));
        let from = if named { from + 2 } else { from };
        let st = s.toks.get(from..).unwrap_or(&[]);
        if st.iter().any(|t| t.is("message")) {
            return Vec::new();
        }
        self.expr_items(s, scope, from, ops)
    }

    fn require_items(
        &self,
        s: &Site,
        service: &str,
        endpoint: &str,
        from: usize,
    ) -> Vec<CompletionItem> {
        let toks = s.toks.get(from..).unwrap_or(&[]);
        if let Some(at) = toks.iter().position(|t| t.is("else")) {
            let after = toks.get(at + 1..).unwrap_or(&[]);
            return match after.last() {
                None => self.error_items(service, endpoint),
                Some(t) if t.is_word() && !t.is("message") && after.len() == 1 => {
                    operator_items(&["message"])
                }
                Some(_) => Vec::new(),
            };
        }
        let scope = self.endpoint_scope_at(service, endpoint, Phase::Require);
        self.statement_expr(s, &scope, from, &["else", "message"])
    }

    fn fact_items(&self, s: &Site, service: &str, endpoint: &str) -> Vec<CompletionItem> {
        let st = s.stmt_toks();
        if let Some(eq) = st.iter().position(|t| t.is("=")) {
            let offset = st
                .first()
                .and_then(|t| u32::try_from(t.start).ok())
                .unwrap_or(0);
            let scope = self.endpoint_scope_at(service, endpoint, Phase::FactKey(offset));
            return self.expr_items(s, &scope, s.stmt + eq + 1, &[]);
        }
        match st.last() {
            Some(t) if t.is(":") => self.types(TypePos::Value),
            Some(t) if t.is("<") || t.is(",") => self.types(TypePos::Value),
            Some(t) if t.is_word() && st.iter().any(|x| x.is(":")) => {
                vec![snippet("=", CompletionItemKind::OPERATOR, "= key", "= $0")]
            }
            Some(_) | None => Vec::new(),
        }
    }

    fn allow_items(&self, s: &Site, service: &str, endpoint: &str) -> Vec<CompletionItem> {
        let policy_call = s.opens.first().filter(|o| o.paren);
        let Some(open) = policy_call else {
            return if s.stmt_toks().len() == 1 {
                self.policy_items()
            } else {
                Vec::new()
            };
        };
        let scope = self.endpoint_scope_at(service, endpoint, Phase::Require);
        let after = s.toks.get(open.at + 1..).unwrap_or(&[]);
        let from = after
            .iter()
            .rposition(|t| t.is(","))
            .map_or(open.at + 1, |c| open.at + 2 + c);
        self.expr_items(s, &scope, from, &[])
    }

    fn transition_items(&self, s: &Site, service: &str, endpoint: &str) -> Vec<CompletionItem> {
        let ix = self.ix();
        let st = s.stmt_toks();
        let scope = self.endpoint_scope_at(service, endpoint, Phase::Require);
        let Some(colon) = st.iter().position(|t| t.is(":")) else {
            // The path: a fact, then fields.
            let toks = s.toks.get(s.stmt + 1..).unwrap_or(&[]);
            return match toks.last() {
                None => self.operands_of(&scope, |l| matches!(l.kind, LocalKind::Fact { .. })),
                Some(t) if t.is(".") => self.after_dot(s, &scope, &[]),
                Some(_) => Vec::new(),
            };
        };
        let path = st.get(1..colon).unwrap_or(&[]);
        let segs: Vec<String> = path
            .iter()
            .filter(|t| t.is_word())
            .map(|t| t.text.clone())
            .collect();
        let val = resolve_segs(ix, &scope, &[], &segs);
        match (st.last(), val.ty.enum_decl(ix)) {
            (Some(t), Some(d)) if t.is(":") || t.is(",") || t.is("->") => variant_items(ix, &[d]),
            (Some(_) | None, _) => Vec::new(),
        }
    }

    fn operands_of(
        &self,
        scope: &Scope,
        keep: impl Fn(&scope::Local) -> bool,
    ) -> Vec<CompletionItem> {
        let ix = self.ix();
        scope
            .locals
            .iter()
            .filter(|l| keep(l))
            .map(|l| item(&l.name, CompletionItemKind::VARIABLE, Some(&l.val.show(ix))))
            .collect()
    }

    fn in_payload(&self, s: &Site, error: &str) -> Option<Vec<CompletionItem>> {
        let (service, endpoint) = s.endpoint()?;
        let st = s.stmt_toks();
        let tail_from = st.iter().rposition(|t| t.is(",")).map_or(0, |c| c + 1);
        let tail = st.get(tail_from..).unwrap_or(&[]);
        if let Some(colon) = tail.iter().position(|t| t.is(":")) {
            let scope = self.endpoint_scope_at(service, endpoint, Phase::Require);
            let from = s.stmt + tail_from + colon + 1;
            return Some(self.expr_items(s, &scope, from, &[]));
        }
        if !tail.is_empty() {
            return Some(Vec::new());
        }
        let ix = self.ix();
        let decl = visible_in(ix, self.file).get(error).copied()?;
        Some(
            members(ix, &Ty::Decl(decl))
                .into_iter()
                .map(|m| {
                    snippet(
                        &m.name,
                        CompletionItemKind::FIELD,
                        &m.val.show(ix),
                        &format!("{}: $0", m.name),
                    )
                })
                .collect(),
        )
    }

    // ---- policies -----------------------------------------------------------------------

    fn in_policy(&self, s: &Site, name: &str) -> Option<Vec<CompletionItem>> {
        let st = s.stmt_toks();
        let Some(first) = s.first() else {
            return Some(words_items(&["permit", "forbid"]));
        };
        if s.line_start && st.last().is_some_and(ends_operand) && s.opens.is_empty() {
            let mut out = words_items(&["permit", "forbid"]);
            out.extend(operator_items(&["message"]));
            return Some(out);
        }
        if !matches!(first, "permit" | "forbid") {
            return Some(words_items(&["permit", "forbid"]));
        }
        let Some(when) = st.iter().position(|t| t.is("when")) else {
            return Some(if st.len() == 1 {
                words_items(&["when"])
            } else {
                Vec::new()
            });
        };
        if st.iter().skip(when).any(|t| t.is("message")) {
            return Some(Vec::new());
        }
        let ix = self.ix();
        let idx = ix
            .file(self.file)
            .and_then(|f| f.ast.policies.iter().position(|p| p.name.name == name));
        let scope = idx.map_or_else(
            || Scope {
                file: Some(self.file),
                locals: Vec::new(),
            },
            |i| policy_scope(ix, self.file, i),
        );
        Some(self.expr_items(s, &scope, s.stmt + when + 1, &["message"]))
    }

    /// Every policy of the workspace, those of this package first.
    fn policy_items(&self) -> Vec<CompletionItem> {
        let ix = self.ix();
        let own = ix.file(self.file).map(|f| f.package.clone());
        let mut out = Vec::new();
        for f in &ix.files {
            for p in &f.ast.policies {
                let params: Vec<String> = p
                    .params
                    .iter()
                    .map(|x| format!("{}: {}", x.name.name, crate::hover::src(ix, x.ty.span)))
                    .collect();
                let mut i = snippet(
                    &p.name.name,
                    CompletionItemKind::FUNCTION,
                    &format!("policy ({})", params.join(", ")),
                    &format!("{}($0)", p.name.name),
                );
                let rank = if own.as_deref() == Some(f.package.as_str()) {
                    '0'
                } else {
                    '1'
                };
                i.sort_text = Some(format!("{rank}{}", p.name.name));
                out.push(i);
            }
        }
        out
    }

    // ---- errors -------------------------------------------------------------------------

    /// The errors of the endpoint first, then every other error.
    fn error_items(&self, service: &str, endpoint: &str) -> Vec<CompletionItem> {
        let declared = self.declared_errors(service, endpoint);
        let mut out = self.types(TypePos::Error);
        for i in &mut out {
            if declared.contains(&i.label) {
                i.sort_text = Some(format!("00{}", i.label));
            }
        }
        out
    }

    fn declared_errors(&self, service: &str, endpoint: &str) -> Vec<String> {
        let ix = self.ix();
        let Some((si, ei)) = self.endpoint_ref(service, endpoint) else {
            return Vec::new();
        };
        let ep = service_ast(ix, si).and_then(|s| s.endpoints.get(ei));
        ep.and_then(|e| e.responses.as_ref())
            .into_iter()
            .flat_map(|r| r.items.iter())
            .filter_map(|i| i.ty.as_ref())
            .filter_map(|t| match &t.kind {
                veld_compiler::syntax::ast::TypeExprKind::Named { name, .. } => {
                    Some(name.name.clone())
                }
                veld_compiler::syntax::ast::TypeExprKind::List(_) => None,
            })
            .collect()
    }

    // ---- examples -----------------------------------------------------------------------

    fn in_example(&self, s: &Site) -> Option<Vec<CompletionItem>> {
        let st = s.stmt_toks();
        let Some(first) = st.first() else {
            let mut out = words_items(&["given", "as", "at", "ifMatch", "key"]);
            out.extend(self.endpoint_items(None));
            return Some(out);
        };
        let last = st.last()?;
        Some(match first.text.as_str() {
            "given" => self.given_items(st, last),
            "as" | "at" | "ifMatch" | "key" => Vec::new(),
            _ => self.call_items(s, st, last),
        })
    }

    fn given_items(&self, st: &[Tk], last: &Tk) -> Vec<CompletionItem> {
        if st.len() == 1 {
            return self.fact_name_items();
        }
        if last.is("=") {
            return words_items(&["absent"]);
        }
        Vec::new()
    }

    /// Every fact name of the endpoints of this package: the example's call comes after its
    /// givens, so the endpoint is not known yet.
    fn fact_name_items(&self) -> Vec<CompletionItem> {
        let ix = self.ix();
        let mut seen: Vec<(String, String)> = Vec::new();
        for (si, svc) in ix.services.iter().enumerate() {
            let same_package = ix.file(self.file).is_some_and(|f| f.package == svc.package);
            let Some(sd) = service_ast(ix, si).filter(|_| same_package) else {
                continue;
            };
            for (ei, _) in sd.endpoints.iter().enumerate() {
                let sc = endpoint_scope(ix, si, ei, Phase::Require);
                for l in sc
                    .locals
                    .iter()
                    .filter(|l| matches!(l.kind, LocalKind::Fact { .. }))
                {
                    if !seen.iter().any(|(n, _)| *n == l.name) {
                        seen.push((l.name.clone(), l.val.show(ix)));
                    }
                }
            }
        }
        seen.into_iter()
            .map(|(n, t)| {
                item(
                    &n,
                    CompletionItemKind::VARIABLE,
                    Some(&format!("fact: {t}")),
                )
            })
            .collect()
    }

    /// The endpoints of this package, or those of `service`, as call snippets.
    fn endpoint_items(&self, service: Option<&str>) -> Vec<CompletionItem> {
        let ix = self.ix();
        let pkg = ix.file(self.file).map(|f| f.package.clone());
        let mut out = Vec::new();
        for (si, svc) in ix.services.iter().enumerate() {
            if Some(&svc.package) != pkg.as_ref() || service.is_some_and(|n| n != svc.name) {
                continue;
            }
            for e in service_ast(ix, si)
                .into_iter()
                .flat_map(|s| s.endpoints.iter())
            {
                let n = &e.name.name;
                let mut i = snippet(
                    n,
                    CompletionItemKind::METHOD,
                    &format!(
                        "{} (in {})",
                        crate::hover::src(ix, e.method_span).to_uppercase(),
                        svc.name
                    ),
                    &format!("{n}($0)"),
                );
                i.sort_text = Some(format!("2{n}"));
                out.push(i);
            }
        }
        out
    }

    fn call_items(&self, s: &Site, st: &[Tk], last: &Tk) -> Vec<CompletionItem> {
        let ix = self.ix();
        let callee = |toks: &[Tk]| -> Option<(Option<String>, String)> {
            match toks {
                [a, d, b, ..] if d.is(".") && a.is_word() && b.is_word() => {
                    Some((Some(a.text.clone()), b.text.clone()))
                }
                [a, ..] if a.is_word() => Some((None, a.text.clone())),
                _ => None,
            }
        };
        if st.len() == 2 && last.is(".") {
            return self.endpoint_items(st.first().map(|t| t.text.as_str()));
        }
        let Some((svc, name)) = callee(st) else {
            return Vec::new();
        };
        let target = endpoint_called(ix, self.file, svc.as_deref(), &name);
        if let Some(open) = s.opens.first() {
            let arg_start = last.is("(") || (s.opens.len() == 1 && last.is(","));
            if !arg_start || !open.paren {
                return Vec::new();
            }
            return target
                .map(|(si, ei)| self.param_items(si, ei))
                .unwrap_or_default();
        }
        if last.is("->") {
            let mut out = words_items(&["ok"]);
            out.extend(match target.and_then(|(si, ei)| self.names_of(si, ei)) {
                Some((svc, ep)) => self.error_items(&svc, &ep),
                None => self.types(TypePos::Error),
            });
            return out;
        }
        if last.is(")") {
            let mut out = words_items(&["returns"]);
            out.push(snippet(
                "->",
                CompletionItemKind::OPERATOR,
                "-> outcome",
                "-> $0",
            ));
            return out;
        }
        Vec::new()
    }

    fn names_of(&self, si: usize, ei: usize) -> Option<(String, String)> {
        let ix = self.ix();
        let svc = ix.services.get(si)?;
        let ep = service_ast(ix, si)?.endpoints.get(ei)?;
        Some((svc.name.clone(), ep.name.name.clone()))
    }

    fn param_items(&self, si: usize, ei: usize) -> Vec<CompletionItem> {
        let ix = self.ix();
        let sc = endpoint_scope(ix, si, ei, Phase::Require);
        sc.locals
            .iter()
            .filter(|l| matches!(l.kind, LocalKind::Param { .. }))
            .map(|l| {
                snippet(
                    &l.name,
                    CompletionItemKind::PROPERTY,
                    &l.val.show(ix),
                    &format!("{}: $0", l.name),
                )
            })
            .collect()
    }

    /// Inside a `{ }` value of an example: the variants after a `.`, otherwise nothing.
    fn in_object(&self, s: &Site) -> Vec<CompletionItem> {
        let ix = self.ix();
        let dot = s.toks.last().is_some_and(|t| t.is("."))
            && !s.toks.iter().rev().nth(1).is_some_and(|p| p.is_word());
        if !dot {
            return Vec::new();
        }
        let enums: Vec<usize> = visible_in(ix, self.file)
            .into_values()
            .filter(|&d| ix.decls.get(d).is_some_and(|e| e.kind == DKind::Enum))
            .collect();
        variant_items(ix, &enums)
    }
}

fn operator_items(ops: &[&str]) -> Vec<CompletionItem> {
    let mut names = vec!["in", "implies"];
    names.extend(ops);
    words_items(&names)
}

/// The index of the unmatched `[` before the end of `toks`.
fn list_open(toks: &[Tk]) -> Option<usize> {
    let mut depth = 0usize;
    for (i, t) in toks.iter().enumerate().rev() {
        if t.is("]") || t.is(")") {
            depth += 1;
        } else if t.is("[") || t.is("(") {
            if depth == 0 {
                return t.is("[").then_some(i);
            }
            depth -= 1;
        }
    }
    None
}

/// The variants of the enums `decls`, once for each name.
fn variant_items(ix: &Index, decls: &[usize]) -> Vec<CompletionItem> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for &d in decls {
        let Some(e) = ix.decls.get(d) else { continue };
        if !matches!(ix.decl_ast(e).map(|a| &a.kind), Some(DeclKind::Enum(_))) {
            continue;
        }
        for v in &e.variants {
            if !seen.contains(&v.as_str()) {
                seen.push(v);
                out.push(item(v, CompletionItemKind::ENUM_MEMBER, Some(&e.name)));
            }
        }
    }
    out
}
