//! Hover on the words of the rules language (`spec/rules.md`): rules, statements, names inside
//! expressions, builtins, policies, states and examples.

use veld_compiler::FileId;
use veld_compiler::index::Index;
use veld_compiler::syntax::ast::{self, DeclKind, doc_text};

use crate::hover::{
    HoverText, decl_hover, endpoint_hover, field_hover, param_hover, src, variant_hover,
};
use crate::query::service_ast;
use crate::rfind::{self, Found, What};
use crate::scope::{Local, LocalKind};
use crate::words;
use crate::workspace::{Analysis, policy_key, rule_key, statement_key};

/// The hover of the word under `offset`, when it is one of the rules language.
#[must_use]
pub fn hover(a: &Analysis, file: FileId, offset: u32) -> Option<HoverText> {
    let found = rfind::find(a, file, offset)?;
    let markdown = describe(a, &found)?;
    Some(HoverText {
        markdown,
        span: found.span,
    })
}

/// The markdown for one word.
#[must_use]
pub fn describe(a: &Analysis, f: &Found) -> Option<String> {
    let ix = &a.index;
    match &f.what {
        What::Rule { decl, rule } => rule_hover(a, *decl, *rule),
        What::Require {
            service,
            endpoint,
            stmt,
        }
        | What::Ensure {
            service,
            endpoint,
            stmt,
        } => statement_hover(a, *service, *endpoint, *stmt),
        What::Local(l) => local_hover(a, l),
        What::Member { owner, index } => field_hover(ix, *owner, *index),
        What::PageMember(n) => Some(format!(
            "```veld\n{n}\n```\nA member of `Page<T>`, which has `items`, `total`, `page` and `size`."
        )),
        What::Variant { decl, variant } => variant_hover(ix, *decl, *variant),
        What::Const(d) | What::Decl(d) => Some(decl_hover(a, ix.decls.get(*d)?)),
        What::Builtin(w) | What::Keyword(w) => Some(words::describe(w)),
        What::Policy { file, index } => policy_hover(a, *file, *index),
        What::Principal { file } => principal_hover(ix, *file),
        What::State { decl, variant } => state_hover(ix, *decl, *variant),
        What::Example { file, index } => example_hover(a, *file, *index),
        What::Endpoint { service, endpoint } => endpoint_hover(a, *service, *endpoint),
        What::Param {
            service,
            endpoint,
            index,
        } => param_hover(ix, *service, *endpoint, *index),
        What::Lambda(l) => Some(format!(
            "```veld\n{}: {}\n```\nThe variable of the lambda: it stands for each element of the list in turn.",
            l.name,
            l.val.show(ix)
        )),
    }
}

/// `(last successful build)` when the identity comes from an earlier compilation.
fn stale(a: &Analysis) -> &'static str {
    if a.sigs_stale {
        " (last successful build)"
    } else {
        ""
    }
}

fn is_audited(anns: &[ast::Annotation]) -> bool {
    anns.iter().any(|an| an.name.name == "rollout")
}

fn rollout_line(audited: bool) -> &'static str {
    if audited {
        "- audited (`@rollout(audit)`): evaluated and reported, it never rejects\n"
    } else {
        "- enforced\n"
    }
}

fn id_line(a: &Analysis, key: &str) -> String {
    match a.sigs.rule_ids.get(key) {
        Some(id) => format!("- id: `{id}`{}\n", stale(a)),
        None => "- id: known once the contract compiles\n".to_owned(),
    }
}

fn message_line(message: Option<&ast::StrLit>, default: &str) -> String {
    match message {
        Some(m) => format!("- message: \"{}\"\n", m.value),
        None => format!("- message: none, the default is \"{default}\"\n"),
    }
}

fn rules_of(ix: &Index, decl: usize) -> &[ast::Rule] {
    match ix
        .decls
        .get(decl)
        .and_then(|d| ix.decl_ast(d))
        .map(|a| &a.kind)
    {
        Some(DeclKind::Model(m)) => &m.rules,
        Some(DeclKind::Error(e)) => &e.rules,
        Some(DeclKind::Event(e)) => &e.rules,
        _ => &[],
    }
}

fn rule_hover(a: &Analysis, decl: usize, rule: usize) -> Option<String> {
    let ix = &a.index;
    let d = ix.decls.get(decl)?;
    let r = rules_of(ix, decl).get(rule)?;
    let mut out = format!("```veld\n{}\n```\n", src(ix, r.span).trim_end());
    if let Some(docs) = doc_text(&r.docs) {
        out.push_str(&format!("{docs}\n\n"));
    }
    out.push_str(&format!("- invariant of `{}`\n", d.name));
    out.push_str(&message_line(
        r.message.as_ref(),
        &format!("violates rule {}", r.name.name),
    ));
    out.push_str(rollout_line(is_audited(&r.annotations)));
    out.push_str(&id_line(a, &rule_key(&d.qualified, &r.name.name)));
    Some(out)
}

fn statement_hover(a: &Analysis, service: usize, endpoint: usize, stmt: usize) -> Option<String> {
    let ix = &a.index;
    let sd = service_ast(ix, service)?;
    let ep = sd.endpoints.get(endpoint)?;
    let block = ep.block.as_ref()?;
    let svc = ix.services.get(service)?;
    let s = block.stmts.get(stmt)?;
    let (kind, span, docs, message, anns, default) = match s {
        ast::Stmt::Require(r) => (
            "require",
            r.span,
            &r.docs,
            r.message.as_ref(),
            &r.annotations,
            r.name
                .as_ref()
                .map_or("precondition failed".to_owned(), |n| {
                    format!("violates rule {}", n.name)
                }),
        ),
        ast::Stmt::Ensure(e) => (
            "ensure",
            e.span,
            &e.docs,
            e.message.as_ref(),
            &e.annotations,
            e.name
                .as_ref()
                .map_or("postcondition failed".to_owned(), |n| {
                    format!("violates rule {}", n.name)
                }),
        ),
        ast::Stmt::Fact(_) | ast::Stmt::Transition(_) | ast::Stmt::Allow(_) => return None,
    };
    let mut out = format!("```veld\n{}\n```\n", src(ix, span).trim_end());
    if let Some(d) = doc_text(docs) {
        out.push_str(&format!("{d}\n\n"));
    }
    let meaning = if kind == "require" {
        "precondition of `{}`: the first one that fails ends the request"
    } else {
        "postcondition of `{}`: checked on the success value of the handler"
    };
    out.push_str(&format!("- {}\n", meaning.replace("{}", &ep.name.name)));
    if let ast::Stmt::Require(r) = s
        && let Some(o) = &r.otherwise
    {
        out.push_str(&format!("- fails with: `{}`\n", src(ix, o.error.span)));
    } else if kind == "require" {
        out.push_str("- fails with: `422 ValidationError`\n");
    }
    out.push_str(&message_line(message, &default));
    out.push_str(rollout_line(is_audited(anns)));
    let same_kind = |x: &&ast::Stmt| match x {
        ast::Stmt::Require(_) => kind == "require",
        ast::Stmt::Ensure(_) => kind == "ensure",
        ast::Stmt::Fact(_) | ast::Stmt::Transition(_) | ast::Stmt::Allow(_) => false,
    };
    let nth = block
        .stmts
        .get(..stmt)
        .map_or(0, |b| b.iter().filter(same_kind).count());
    out.push_str(&id_line(
        a,
        &statement_key(&svc.name, &ep.name.name, kind, nth),
    ));
    Some(out)
}

fn local_hover(a: &Analysis, l: &Local) -> Option<String> {
    let ix = &a.index;
    let ty = l.val.show(ix).replace('?', "");
    let opt = if l.val.optional { "?" } else { "" };
    match &l.kind {
        LocalKind::Field { owner, index } => {
            let mut md = field_hover(ix, *owner, *index)?;
            if l.val.optional {
                md.push_str("- optional: guard it with `has(..)` before reading it\n");
            }
            Some(md)
        }
        LocalKind::Param {
            service,
            endpoint,
            index,
        } => param_hover(ix, *service, *endpoint, *index),
        LocalKind::Fact {
            service,
            endpoint,
            stmt,
        } => fact_hover(a, l, (*service, *endpoint, *stmt)),
        LocalKind::Principal { file } => {
            let mut md = format!(
                "```veld\n{}: {ty}\n```\nThe caller: what the auth hook returned for the request. A trusted value, like a fact.\n",
                l.name
            );
            if let Some(f) = ix.file(*file) {
                md.push_str(&format!(
                    "- declared by `principal` in package `{}`\n",
                    f.package
                ));
            }
            Some(md)
        }
        LocalKind::PolicyParam { file, policy, .. } => {
            let p = ix.file(*file)?.ast.policies.get(*policy)?;
            Some(format!(
                "```veld\n{}{opt}: {ty}\n```\n- parameter of policy `{}`\n",
                l.name, p.name.name
            ))
        }
        LocalKind::Now => words::word("now").map(words::describe),
        LocalKind::Result => Some(format!(
            "```veld\nresult: {ty}\n```\nThe success value of the endpoint, in an `ensure`.\n"
        )),
    }
}

fn fact_hover(a: &Analysis, l: &Local, at: (usize, usize, usize)) -> Option<String> {
    let ix = &a.index;
    let sd = service_ast(ix, at.0)?;
    let ep = sd.endpoints.get(at.1)?;
    let ast::Stmt::Fact(f) = ep.block.as_ref()?.stmts.get(at.2)? else {
        return None;
    };
    let mut out = format!("```veld\n{}\n```\n", src(ix, f.span).trim_end());
    out.push_str(&format!(
        "- fact of `{}`: the application loads it once, before any rule runs\n",
        ep.name.name
    ));
    if l.val.optional {
        out.push_str("- may be absent: guard it with `has(..)`\n");
    } else {
        out.push_str("- if the loader finds nothing the answer is `404 NotFound`\n");
    }
    Some(out)
}

fn clause_line(ix: &Index, c: &ast::PolicyClause) -> String {
    let word = if c.forbid { "forbid" } else { "permit" };
    let msg = c
        .message
        .as_ref()
        .map_or(String::new(), |m| format!("\n  message \"{}\"", m.value));
    format!("{word} when {}{msg}", src(ix, c.expr.span))
}

fn policy_hover(a: &Analysis, file: FileId, index: usize) -> Option<String> {
    let ix = &a.index;
    let f = ix.file(file)?;
    let p = f.ast.policies.get(index)?;
    let params: Vec<String> = p
        .params
        .iter()
        .map(|x| format!("{}: {}", x.name.name, src(ix, x.ty.span)))
        .collect();
    let mut out = format!("```veld\npolicy {}({})\n", p.name.name, params.join(", "));
    for c in &p.clauses {
        out.push_str(&format!("  {}\n", clause_line(ix, c)));
    }
    out.push_str("```\n");
    if let Some(docs) = doc_text(&p.docs) {
        out.push_str(&format!("{docs}\n\n"));
    }
    out.push_str("- permits when some `permit` is true and no `forbid` is\n");
    out.push_str(&id_line(a, &policy_key(&f.package, &p.name.name)));
    Some(out)
}

fn principal_hover(ix: &Index, file: FileId) -> Option<String> {
    let (_, p) = crate::scope::principal_of(ix, file)?;
    let name = p.name.as_ref().map_or("user", |n| n.name.as_str());
    let mut out = format!("```veld\n{}\n```\n", src(ix, p.span).trim_end());
    if let Some(docs) = doc_text(&p.docs) {
        out.push_str(&format!("{docs}\n\n"));
    }
    out.push_str(&format!(
        "- the caller of every endpoint that is not `@auth(none)`; in an endpoint block it is called `{name}`\n"
    ));
    Some(out)
}

/// The edges of a state machine, from its `flow` lines.
fn edges(e: &ast::EnumDecl) -> Vec<(&str, &str)> {
    e.flows
        .iter()
        .flat_map(|f| f.states.windows(2))
        .filter_map(|w| Some((w.first()?.name.as_str(), w.get(1)?.name.as_str())))
        .collect()
}

fn state_hover(ix: &Index, decl: usize, variant: usize) -> Option<String> {
    let d = ix.decls.get(decl)?;
    let DeclKind::Enum(e) = &ix.decl_ast(d)?.kind else {
        return None;
    };
    let name = &e.variants.get(variant)?.name.name;
    let all = edges(e);
    let mut out_of: Vec<&str> = all.iter().filter(|x| x.0 == name).map(|x| x.1).collect();
    let mut into: Vec<&str> = all.iter().filter(|x| x.1 == name).map(|x| x.0).collect();
    out_of.dedup();
    into.dedup();
    let is_final = e
        .finals
        .iter()
        .any(|f| f.states.iter().any(|s| s.name == *name));
    let list = |v: &[&str]| {
        if v.is_empty() {
            "none".to_owned()
        } else {
            v.join(", ")
        }
    };
    let mut md = format!(
        "```veld\n{}.{name}\n```\nA state of `{}`.\n\n",
        d.name, d.name
    );
    md.push_str(&format!("- moves out to: {}\n", list(&out_of)));
    md.push_str(&format!("- reached from: {}\n", list(&into)));
    md.push_str(if is_final {
        "- final: it has no way out\n"
    } else if out_of.is_empty() {
        "- not final, but no flow leaves it: it is a stuck state\n"
    } else {
        "- not final\n"
    });
    Some(md)
}

fn example_hover(a: &Analysis, file: FileId, index: usize) -> Option<String> {
    let ix = &a.index;
    let x = ix.file(file)?.ast.examples.get(index)?;
    let mut out = format!("```veld\nexample \"{}\"\n```\n", x.name.value);
    if let Some(docs) = doc_text(&x.docs) {
        out.push_str(&format!("{docs}\n\n"));
    }
    match x.body.as_call() {
        Some(c) => {
            let callee = match &c.service {
                Some(s) => format!("{}.{}", s.name, c.endpoint.name),
                None => c.endpoint.name.clone(),
            };
            out.push_str(&format!("- calls `{callee}`\n"));
            let outcome = src(ix, c.outcome.span).trim_end();
            out.push_str(&format!("- expects: `{outcome}`\n"));
            let givens: Vec<&str> = x.givens.iter().map(|g| g.name.name.as_str()).collect();
            if !givens.is_empty() {
                out.push_str(&format!("- given: {}\n", givens.join(", ")));
            }
            if let Some(at) = &x.at {
                out.push_str(&format!("- clock: `{}`\n", at.time.value));
            }
        }
        None => out.push_str("- an example of one delivery of an event\n"),
    }
    out.push_str("- documentation that runs: `veld test` checks it\n");
    Some(out)
}
