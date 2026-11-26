//! Attaches the comment side-table to syntax items.
//!
//! The parser keeps ordinary comments outside the tree. Here every comment is
//! given exactly one home, found by position:
//! - on the same line as the end of the previous sibling: trailing comment;
//! - on the header line of a block, before its first item: header comment;
//! - otherwise on its own line before the next sibling: leading comment;
//! - after the last sibling of a block: tail comment.
//!
//! A comment inside a leaf item (a field, an endpoint, a `use`) cannot keep its
//! position, so it becomes a trailing comment of that item. Nothing is dropped.

use std::collections::BTreeMap;

use veld_compiler::Comment;
use veld_compiler::syntax::ast::{self, DeclKind, Doc};

/// Key of the root (the file itself).
pub(super) const ROOT: u32 = u32::MAX;

/// A syntax item with the byte range it occupies, docs included.
#[derive(Debug)]
pub(super) struct Node {
    /// Start of the first doc comment, annotation or keyword.
    pub start: u32,
    /// Start of the item proper (keyword, field name), after docs.
    pub kw: u32,
    /// Start of the header line proper (the declaration name).
    pub anchor: u32,
    pub end: u32,
    pub braced: bool,
    pub kids: Vec<Node>,
}

/// A leading or tail comment and whether a blank line follows it.
#[derive(Debug, Clone)]
pub(super) struct Lead {
    pub comment: Comment,
    pub gap_after: bool,
}

#[derive(Debug, Default)]
pub(super) struct Attached {
    pub leading: Vec<Lead>,
    pub trailing: Vec<Comment>,
    pub header: Vec<Comment>,
    pub tail: Vec<Lead>,
}

#[derive(Debug, Default)]
pub(super) struct Layout {
    map: BTreeMap<u32, Attached>,
}

impl Layout {
    pub fn get(&self, key: u32) -> Option<&Attached> {
        self.map.get(&key)
    }

    pub fn build(src: &str, file: &ast::File, comments: &[Comment]) -> Layout {
        let mut kids: Vec<Node> = file.uses.iter().map(use_node).collect();
        kids.extend(file.decls.iter().map(decl_node));
        kids.extend(file.examples.iter().map(example_node));
        kids.extend(file.policies.iter().map(policy_node));
        kids.extend(file.principals.iter().map(principal_node));
        kids.sort_by_key(|n| n.start);
        let mut b = Builder {
            src,
            comments,
            lay: Layout::default(),
        };
        for i in 0..comments.len() {
            b.place(
                &kids,
                &Parent {
                    key: ROOT,
                    anchor: None,
                    end: u32::MAX,
                },
                i,
            );
        }
        b.lay
    }
}

struct Parent {
    key: u32,
    /// Where the header line starts; `None` for the root.
    anchor: Option<u32>,
    end: u32,
}

struct Builder<'a> {
    src: &'a str,
    comments: &'a [Comment],
    lay: Layout,
}

impl Builder<'_> {
    fn newline_between(&self, a: u32, b: u32) -> bool {
        self.src
            .get(a as usize..(b as usize).max(a as usize))
            .is_some_and(|s| s.contains('\n'))
    }

    fn entry(&mut self, key: u32) -> &mut Attached {
        self.lay.map.entry(key).or_default()
    }

    fn gap_after(&self, i: usize, limit: u32) -> bool {
        let c = &self.comments[i];
        let next = self
            .comments
            .get(i + 1)
            .map_or(limit, |n| n.span.start.min(limit));
        self.src
            .get(c.span.end as usize..(next as usize).max(c.span.end as usize))
            .is_some_and(|s| s.matches('\n').count() >= 2)
    }

    fn place(&mut self, nodes: &[Node], parent: &Parent, i: usize) {
        let c = self.comments[i].clone();
        let (cs, ce) = (c.span.start, c.span.end);
        if let Some(k) = nodes.iter().find(|k| k.start <= cs && ce <= k.end) {
            if cs < k.kw {
                let gap = self.gap_after(i, k.kw);
                self.entry(k.start).leading.push(Lead {
                    comment: c,
                    gap_after: gap,
                });
            } else if k.braced {
                let p = Parent {
                    key: k.start,
                    anchor: Some(k.anchor),
                    end: k.end,
                };
                self.place(&k.kids, &p, i);
            } else {
                self.entry(k.start).trailing.push(c);
            }
            return;
        }
        self.place_among(nodes, parent, i);
    }

    fn place_among(&mut self, nodes: &[Node], parent: &Parent, i: usize) {
        let c = self.comments[i].clone();
        let pos = nodes.partition_point(|k| k.start < c.span.start);
        if pos > 0 {
            let prev = &nodes[pos - 1];
            if !self.newline_between(prev.end, c.span.start) {
                self.entry(prev.start).trailing.push(c);
                return;
            }
        } else if let Some(a) = parent.anchor
            && !self.newline_between(a, c.span.start)
        {
            self.entry(parent.key).header.push(c);
            return;
        }
        match nodes.get(pos) {
            Some(next) => {
                let gap = self.gap_after(i, next.start);
                self.entry(next.start).leading.push(Lead {
                    comment: c,
                    gap_after: gap,
                });
            }
            None => {
                let gap = self.gap_after(i, parent.end);
                self.entry(parent.key).tail.push(Lead {
                    comment: c,
                    gap_after: gap,
                });
            }
        }
    }
}

fn first_start(kw: u32, docs: &[Doc], anns: &[ast::Annotation]) -> u32 {
    let d = docs.first().map_or(kw, |d| d.span.start);
    let a = anns.iter().map(|a| a.span.start).min().unwrap_or(kw);
    kw.min(d).min(a)
}

fn use_node(u: &ast::Use) -> Node {
    Node {
        start: u.span.start,
        kw: u.span.start,
        anchor: u.span.start,
        end: u.span.end,
        braced: false,
        kids: vec![],
    }
}

fn leaf(start: u32, kw: u32, end: u32) -> Node {
    Node {
        start,
        kw,
        anchor: kw,
        end,
        braced: false,
        kids: vec![],
    }
}

fn decl_kids(d: &ast::Decl) -> (bool, Vec<Node>) {
    match &d.kind {
        DeclKind::Model(m) => (true, member_nodes(&m.fields, &m.rules)),
        DeclKind::Error(e) => (true, member_nodes(&e.fields, &e.rules)),
        DeclKind::Enum(e) => (true, enum_nodes(e)),
        DeclKind::Union(u) => (
            true,
            u.arms
                .iter()
                .map(|a| leaf(a.span.start, a.span.start, a.span.end))
                .collect(),
        ),
        DeclKind::Service(s) => (true, service_nodes(s)),
        DeclKind::Event(e) => (true, member_nodes(&e.fields, &e.rules)),
        DeclKind::Channel(c) => (true, channel_nodes(c)),
        DeclKind::Const(_) => (false, vec![]),
    }
}

/// Endpoints, `publishes` lines and `subscribes` blocks of a service, in source order (the
/// printer regroups them).
fn service_nodes(s: &ast::ServiceDecl) -> Vec<Node> {
    let mut nodes: Vec<Node> = s.endpoints.iter().map(endpoint_node).collect();
    nodes.extend(
        s.publishes
            .iter()
            .map(|p| leaf(p.span.start, p.span.start, p.span.end)),
    );
    nodes.extend(s.subscribes.iter().map(subscribes_node));
    nodes.sort_by_key(|n| n.start);
    nodes
}

/// A subscription is a braced node whose children are its `on` lines.
fn subscribes_node(s: &ast::Subscribes) -> Node {
    Node {
        start: s.span.start,
        kw: s.span.start,
        anchor: s.span.start,
        end: s.span.end,
        braced: true,
        kids: s
            .handlers
            .iter()
            .map(|h| leaf(on_key(h), h.span.start, h.span.end))
            .collect(),
    }
}

pub(super) fn on_key(h: &ast::OnLine) -> u32 {
    first_start(h.span.start, &h.docs, &[])
}

/// The `carries` and `key` lines of a channel, in source order.
fn channel_nodes(c: &ast::ChannelDecl) -> Vec<Node> {
    let mut nodes: Vec<Node> = c
        .carries
        .iter()
        .map(|x| leaf(x.span.start, x.span.start, x.span.end))
        .collect();
    nodes.extend(
        c.keys
            .iter()
            .map(|k| leaf(k.span.start, k.span.start, k.span.end)),
    );
    nodes.sort_by_key(|n| n.start);
    nodes
}

/// Variants, flows and finals of an enum body, in source order (the printer regroups them).
fn enum_nodes(e: &ast::EnumDecl) -> Vec<Node> {
    let mut nodes: Vec<Node> = e
        .variants
        .iter()
        .map(|v| {
            leaf(
                first_start(v.span.start, &v.docs, &v.annotations),
                v.span.start,
                v.span.end,
            )
        })
        .collect();
    nodes.extend(
        e.flows
            .iter()
            .map(|f| leaf(f.span.start, f.span.start, f.span.end)),
    );
    nodes.extend(
        e.finals
            .iter()
            .map(|f| leaf(f.span.start, f.span.start, f.span.end)),
    );
    nodes.sort_by_key(|n| n.start);
    nodes
}

/// An endpoint is a leaf, or a braced node holding the statements of its block. The header
/// comment of the block is the one on the line of its `{`.
fn endpoint_node(e: &ast::Endpoint) -> Node {
    let start = first_start(e.span.start, &e.docs, &e.annotations);
    let Some(block) = &e.block else {
        return leaf(start, e.span.start, e.span.end);
    };
    Node {
        start,
        kw: e.span.start,
        anchor: block.span.start,
        end: e.span.end,
        braced: true,
        kids: block
            .stmts
            .iter()
            .map(|s| leaf(s.start(), stmt_kw(s), stmt_end(s)))
            .collect(),
    }
}

fn stmt_kw(s: &ast::Stmt) -> u32 {
    match s {
        ast::Stmt::Fact(f) => f.span.start,
        ast::Stmt::Require(r) => r.span.start,
        ast::Stmt::Ensure(e) => e.span.start,
        ast::Stmt::Transition(t) => t.span.start,
        ast::Stmt::Allow(a) => a.span.start,
    }
}

fn stmt_end(s: &ast::Stmt) -> u32 {
    match s {
        ast::Stmt::Fact(f) => f.span.end,
        ast::Stmt::Require(r) => r.span.end,
        ast::Stmt::Ensure(e) => e.span.end,
        ast::Stmt::Transition(t) => t.span.end,
        ast::Stmt::Allow(a) => a.span.end,
    }
}

/// A policy: a braced node whose children are its clauses.
fn policy_node(p: &ast::PolicyDecl) -> Node {
    Node {
        start: policy_key(p),
        kw: p.span.start,
        anchor: p.span.start,
        end: p.span.end,
        braced: true,
        kids: p
            .clauses
            .iter()
            .map(|c| leaf(c.span.start, c.span.start, c.span.end))
            .collect(),
    }
}

pub(super) fn policy_key(p: &ast::PolicyDecl) -> u32 {
    first_start(p.span.start, &p.docs, &[])
}

fn principal_node(p: &ast::PrincipalDecl) -> Node {
    leaf(principal_key(p), p.span.start, p.span.end)
}

pub(super) fn principal_key(p: &ast::PrincipalDecl) -> u32 {
    first_start(p.span.start, &p.docs, &[])
}

/// An example: a braced node whose children are its givens, principal, clock and call.
fn example_node(x: &ast::Example) -> Node {
    let mut kids: Vec<Node> = x
        .givens
        .iter()
        .map(|g| leaf(g.span.start, g.span.start, g.span.end))
        .collect();
    if let Some(p) = &x.principal {
        kids.push(leaf(p.span.start, p.span.start, p.span.end));
    }
    if let Some(a) = &x.at {
        kids.push(leaf(a.span.start, a.span.start, a.span.end));
    }
    match &x.body {
        ast::ExampleBody::Call(c) => {
            if let Some(i) = &c.if_match {
                kids.push(leaf(i.span.start, i.span.start, i.span.end));
            }
            if let Some(k) = &c.key {
                kids.push(leaf(k.span.start, k.span.start, k.span.end));
            }
            kids.push(leaf(c.span.start, c.span.start, c.span.end));
            kids.extend(
                c.emitted
                    .iter()
                    .map(|x| leaf(x.span.start, x.span.start, x.span.end)),
            );
        }
        ast::ExampleBody::Event(e) => {
            let given_end = e
                .channel
                .as_ref()
                .map_or(e.value.span.end, |c| c.span.end.max(e.value.span.end));
            kids.push(leaf(e.span.start, e.span.start, given_end));
            kids.extend(
                e.expects
                    .iter()
                    .map(|x| leaf(x.span.start, x.span.start, x.span.end)),
            );
        }
    }
    Node {
        start: example_key(x),
        kw: x.span.start,
        anchor: x.span.start,
        end: x.span.end,
        braced: true,
        kids,
    }
}

pub(super) fn example_key(x: &ast::Example) -> u32 {
    first_start(x.span.start, &x.docs, &[])
}

/// Fields and rules of a body, in source order (the printer regroups them, the comments stay
/// with their own item).
fn member_nodes(fields: &[ast::Field], rules: &[ast::Rule]) -> Vec<Node> {
    let mut nodes: Vec<Node> = fields.iter().map(field_node).collect();
    nodes.extend(rules.iter().map(rule_node));
    nodes.sort_by_key(|n| n.start);
    nodes
}

fn rule_node(r: &ast::Rule) -> Node {
    leaf(
        first_start(r.span.start, &r.docs, &r.annotations),
        r.span.start,
        r.span.end,
    )
}

fn field_node(f: &ast::Field) -> Node {
    leaf(
        first_start(f.span.start, &f.docs, &f.annotations),
        f.span.start,
        f.span.end,
    )
}

/// Node of a declaration. Its `kw` is the span start, which already covers
/// annotations; docs precede it.
pub(super) fn decl_node(d: &ast::Decl) -> Node {
    let (braced, kids) = decl_kids(d);
    let start = d
        .docs
        .first()
        .map_or(d.span.start, |x| x.span.start.min(d.span.start));
    Node {
        start,
        kw: d.span.start,
        anchor: d.name().span.start,
        end: d.span.end,
        braced,
        kids,
    }
}

/// Key under which a declaration's comments are stored.
pub(super) fn decl_key(d: &ast::Decl) -> u32 {
    decl_node(d).start
}

pub(super) fn field_key(f: &ast::Field) -> u32 {
    field_node(f).start
}

pub(super) fn rule_key(r: &ast::Rule) -> u32 {
    rule_node(r).start
}

pub(super) fn variant_key(v: &ast::Variant) -> u32 {
    first_start(v.span.start, &v.docs, &v.annotations)
}

pub(super) fn endpoint_key(e: &ast::Endpoint) -> u32 {
    first_start(e.span.start, &e.docs, &e.annotations)
}
