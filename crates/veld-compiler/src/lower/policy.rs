//! Lowering of policies, the principal and `allow` (`spec/rules.md` sections 11.1 to 11.5).

use super::Lowerer;
use crate::semantic::{PolicyInfo, RAllow, RClause, ServiceInfo};
use crate::signature;
use crate::syntax::ast::doc_text;
use veld_ir::{
    Decl, EndpointAllow, Policy, PolicyClause, PolicyEffect, PolicyParam, ServicePrincipal,
};

fn effect(c: &RClause) -> PolicyEffect {
    if c.forbid {
        PolicyEffect::Forbid
    } else {
        PolicyEffect::Permit
    }
}

impl Lowerer<'_, '_> {
    /// Indices into `prog.policies`, in IR order: by qualified name.
    pub(super) fn policy_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.prog.policies.len())
            .filter(|&i| self.prog.policies[i].res.is_some())
            .collect();
        order.sort_by(|&a, &b| {
            self.prog.policies[a]
                .qualified
                .cmp(&self.prog.policies[b].qualified)
        });
        order
    }

    /// Every policy of the workspace, sorted by qualified name.
    pub(super) fn policies(&self, decls: &[Decl]) -> Vec<Policy> {
        self.policy_order()
            .into_iter()
            .filter_map(|i| self.policy(&self.prog.policies[i], decls))
            .collect()
    }

    fn policy(&self, p: &PolicyInfo<'_>, decls: &[Decl]) -> Option<Policy> {
        let res = p.res.as_ref()?;
        let pkg = &self.prog.packages[p.pkg].name;
        let clauses: Vec<PolicyClause> = res
            .clauses
            .iter()
            .map(|c| PolicyClause {
                effect: effect(c),
                expr: self.remap_expr(&c.expr),
                message: c.message.clone(),
                origin: self.origin(c.span, p.module),
            })
            .collect();
        let shapes: Vec<(PolicyEffect, &veld_ir::Expr)> =
            clauses.iter().map(|c| (c.effect, &c.expr)).collect();
        Some(Policy {
            name: p.name.clone(),
            package: self.pkg_id(p.pkg),
            module: self.prog.modules[p.module].path.clone(),
            qualified: p.qualified.clone(),
            id: signature::policy_id(decls, pkg, &p.name, &p.qualified, &shapes),
            params: res
                .params
                .iter()
                .map(|x| PolicyParam {
                    name: x.name.clone(),
                    ty: self.remap(&x.ty),
                })
                .collect(),
            clauses,
            docs: doc_text(&p.ast.docs),
            origin: self.origin(p.ast.name.span, p.module),
        })
    }

    /// The principal of the package of `s`, on each of its services.
    pub(super) fn service_principal(&self, s: &ServiceInfo) -> Option<ServicePrincipal> {
        let p = self.prog.principals.get(&s.pkg)?;
        Some(ServicePrincipal {
            decl: self.decl_id(p.decl),
            name: p.name.clone(),
        })
    }

    /// The `allow` lines of an endpoint, in source order.
    pub(super) fn allows(&self, allows: &[RAllow], module: usize) -> Vec<EndpointAllow> {
        allows
            .iter()
            .filter_map(|a| {
                Some(EndpointAllow {
                    policy: self.policy_map.get(a.policy).copied().flatten()?,
                    args: a.args.iter().map(|x| self.remap_expr(x)).collect(),
                    reads: a.reads.clone(),
                    origin: self.origin(a.span, module),
                })
            })
            .collect()
    }
}
