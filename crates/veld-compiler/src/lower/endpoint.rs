//! Lowering of checked endpoint blocks (`spec/rules.md` sections 9 and 13.1).

use std::collections::BTreeMap;

use super::Lowerer;
use crate::semantic::{RElse, REndpoint, REnsure, RRequire, RTransition, ServiceInfo};
use crate::signature::{self, EndpointRef, StatementKind, TransitionRef};
use veld_ir::endpoint_rules::{
    ElseError, ElseField, EndpointEnsure, EndpointFact, EndpointRequire, EndpointRules,
    EndpointTransition, EndpointVersion, ServiceFact, TransitionEmit,
};
use veld_ir::{Decl, Type};

impl Lowerer<'_, '_> {
    /// The rules of endpoint `e`: facts in load order, `require` and `ensure` in source order.
    pub(super) fn endpoint_rules(
        &self,
        e: &REndpoint,
        s: &ServiceInfo,
        pkg: &str,
        decls: &[Decl],
    ) -> EndpointRules {
        let module = &self.prog.modules[s.module].path;
        let at = EndpointRef {
            package: pkg,
            module,
            service: &s.name,
            endpoint: &e.name,
        };
        let facts = e.rules.facts.iter().map(|f| EndpointFact {
            name: f.name.clone(),
            ty: self.remap(&f.ty),
            optional: f.optional,
            key: f.key.as_ref().map(|k| self.remap_expr(k)),
            origin: self.origin(f.span, s.module),
        });
        let requires = e
            .rules
            .requires
            .iter()
            .map(|r| self.require(r, s.module, &at, decls));
        let ensures = e
            .rules
            .ensures
            .iter()
            .map(|r| self.ensure(r, s.module, &at, decls));
        let transition = e
            .rules
            .transition
            .as_ref()
            .map(|t| self.transition(t, s.module, &at));
        EndpointRules {
            transition,
            allows: self.allows(&e.rules.allows, s.module),
            facts: facts.collect(),
            requires: requires.collect(),
            ensures: ensures.collect(),
            version: e.rules.version.as_ref().map(|v| {
                let path = [v.wire.clone()];
                EndpointVersion {
                    id: signature::version_rule_id(&at, &v.fact, &path),
                    fact: v.fact.clone(),
                    field: v.field.clone(),
                    wire: v.wire.clone(),
                    origin: self.origin(v.span, s.module),
                }
            }),
        }
    }

    fn require(
        &self,
        r: &RRequire,
        module: usize,
        at: &EndpointRef<'_>,
        decls: &[Decl],
    ) -> EndpointRequire {
        let expr = self.remap_expr(&r.expr);
        let kind = StatementKind::Require;
        EndpointRequire {
            name: r.name.clone(),
            id: signature::endpoint_rule_id(decls, at, kind, r.name.as_deref(), &expr),
            expr,
            message: r.message.clone(),
            reads: r.reads.clone(),
            else_error: r.otherwise.as_ref().map(|o| self.else_error(o)),
            docs: r.docs.clone(),
            rollout: r.rollout,
            origin: self.origin(r.span, module),
        }
    }

    fn ensure(
        &self,
        r: &REnsure,
        module: usize,
        at: &EndpointRef<'_>,
        decls: &[Decl],
    ) -> EndpointEnsure {
        let expr = self.remap_expr(&r.expr);
        let kind = StatementKind::Ensure;
        EndpointEnsure {
            name: r.name.clone(),
            id: signature::endpoint_rule_id(decls, at, kind, r.name.as_deref(), &expr),
            expr,
            message: r.message.clone(),
            reads: r.reads.clone(),
            docs: r.docs.clone(),
            rollout: r.rollout,
            origin: self.origin(r.span, module),
        }
    }

    fn transition(
        &self,
        t: &RTransition,
        module: usize,
        at: &EndpointRef<'_>,
    ) -> EndpointTransition {
        let path: Vec<String> = t.steps.iter().map(|s| s.wire.clone()).collect();
        let id = signature::transition_rule_id(
            at,
            &TransitionRef {
                fact: &t.fact,
                path: &path,
                from: &t.from_wire,
                to: &t.to_wire,
            },
        );
        EndpointTransition {
            id,
            fact: t.fact.clone(),
            steps: t.steps.clone(),
            decl: self.decl_id(t.decl),
            from: t.from.clone(),
            to: t.to.clone(),
            emits: t
                .emits
                .iter()
                .map(|e| TransitionEmit {
                    event: self.decl_id(e.event),
                    channel: self.channel_id(e.channel),
                })
                .collect(),
            origin: self.origin(t.span, module),
        }
    }

    fn else_error(&self, o: &RElse) -> ElseError {
        ElseError {
            decl: self.decl_id(o.decl),
            status: o.status,
            fields: o
                .fields
                .iter()
                .map(|f| ElseField {
                    name: f.name.clone(),
                    wire: f.wire.clone(),
                    value: self.remap_expr(&f.value),
                })
                .collect(),
        }
    }

    /// One loader method per fact name, sorted by name (`spec/rules.md` section 9.2). The checker
    /// already made every fact of one name agree.
    pub(super) fn service_facts(&self, s: &ServiceInfo) -> Vec<ServiceFact> {
        let mut by_name: BTreeMap<&str, ServiceFact> = BTreeMap::new();
        for f in s.endpoints.iter().flat_map(|e| &e.rules.facts) {
            by_name.entry(&f.name).or_insert_with(|| ServiceFact {
                name: f.name.clone(),
                ty: self.remap(&f.ty),
                key: f.key_ty.as_ref().map(|t: &Type| self.remap(t)),
                optional: f.optional,
            });
        }
        by_name.into_values().collect()
    }
}
