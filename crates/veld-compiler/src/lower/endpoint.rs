//! Lowering of checked endpoint blocks (`spec/rules.md` sections 9 and 13.1).

use std::collections::BTreeMap;

use super::Lowerer;
use crate::semantic::{RElse, REndpoint, ServiceInfo};
use crate::signature::{self, EndpointRef, StatementKind};
use veld_ir::endpoint_rules::{
    ElseError, ElseField, EndpointEnsure, EndpointFact, EndpointRequire, EndpointRules, ServiceFact,
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
        let requires = e.rules.requires.iter().map(|r| {
            let expr = self.remap_expr(&r.expr);
            EndpointRequire {
                name: r.name.clone(),
                id: signature::endpoint_rule_id(
                    decls,
                    &at,
                    StatementKind::Require,
                    r.name.as_deref(),
                    &expr,
                ),
                expr,
                message: r.message.clone(),
                reads: r.reads.clone(),
                else_error: r.otherwise.as_ref().map(|o| self.else_error(o)),
                docs: r.docs.clone(),
                origin: self.origin(r.span, s.module),
            }
        });
        let ensures = e.rules.ensures.iter().map(|r| {
            let expr = self.remap_expr(&r.expr);
            EndpointEnsure {
                name: r.name.clone(),
                id: signature::endpoint_rule_id(
                    decls,
                    &at,
                    StatementKind::Ensure,
                    r.name.as_deref(),
                    &expr,
                ),
                expr,
                message: r.message.clone(),
                reads: r.reads.clone(),
                docs: r.docs.clone(),
                origin: self.origin(r.span, s.module),
            }
        });
        EndpointRules {
            facts: facts.collect(),
            requires: requires.collect(),
            ensures: ensures.collect(),
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
