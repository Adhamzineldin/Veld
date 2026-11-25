//! Where each declaration is used: in requests, in responses, or both.

use std::collections::{BTreeMap, BTreeSet};

use veld_ir::{DeclId, DeclKind, Ir, Type};

/// The roles of one declaration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Role {
    pub request: bool,
    pub response: bool,
}

impl Role {
    fn is_empty(self) -> bool {
        !self.request && !self.response
    }
}

/// Roles by qualified name, for the old and the new IR.
#[derive(Debug)]
pub(super) struct Usage {
    old: BTreeMap<String, Role>,
    new: BTreeMap<String, Role>,
}

impl Usage {
    pub fn of(old: &Ir, new: &Ir) -> Usage {
        Usage {
            old: roles(old),
            new: roles(new),
        }
    }

    /// The role in the old IR, or in the new IR when the old one never uses it.
    pub fn role(&self, qualified: &str) -> Role {
        let r = self.old.get(qualified).copied().unwrap_or_default();
        if r.is_empty() {
            self.new.get(qualified).copied().unwrap_or_default()
        } else {
            r
        }
    }
}

fn refs(ir: &Ir, id: DeclId, out: &mut Vec<DeclId>) {
    let d = ir.decl(id);
    match &d.kind {
        DeclKind::Model(m) => out.extend(m.parent),
        DeclKind::Union(u) => out.extend(u.arms.iter().map(|a| a.decl)),
        DeclKind::Const(c) => c.ty.collect_decls(out),
        DeclKind::Error(_) | DeclKind::Enum(_) | DeclKind::Event(_) => {}
    }
    for f in d.fields() {
        f.ty.collect_decls(out);
    }
}

fn closure(ir: &Ir, mut stack: Vec<DeclId>) -> BTreeSet<DeclId> {
    let mut seen = BTreeSet::new();
    while let Some(id) = stack.pop() {
        if seen.insert(id) {
            refs(ir, id, &mut stack);
        }
    }
    seen
}

fn roles(ir: &Ir) -> BTreeMap<String, Role> {
    let (mut req, mut resp) = (Vec::new(), Vec::new());
    for s in &ir.services {
        for ep in &s.endpoints {
            ep.params.iter().for_each(|p| collect(&p.ty, &mut req));
            if let Some(t) = &ep.success.ty {
                collect(t, &mut resp);
            }
            resp.extend(ep.errors.iter().map(|e| e.decl));
        }
    }
    let mut out: BTreeMap<String, Role> = BTreeMap::new();
    for id in closure(ir, req) {
        out.entry(ir.decl(id).qualified.clone())
            .or_default()
            .request = true;
    }
    for id in closure(ir, resp) {
        out.entry(ir.decl(id).qualified.clone())
            .or_default()
            .response = true;
    }
    out
}

fn collect(t: &Type, out: &mut Vec<DeclId>) {
    t.collect_decls(out);
}
