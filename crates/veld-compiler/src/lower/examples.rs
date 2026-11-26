//! Lowering of checked examples to `veld_ir::Example` (`spec/rules.md` section 13.1).

use std::collections::BTreeMap;

use super::Lowerer;
use crate::semantic::{RExample, RKeyStore, ROutcome, RSagaAnswer, RSagaExample, RSagaExpect};
use veld_ir::endpoint_rules::{
    Example, ExampleArg, ExampleErrorOutcome, ExampleGiven, ExampleKey, ExampleOutcome, KeyStore,
};
use veld_ir::{ExampleSaga, SagaAnswer, SagaExpect, SagaGiven, ServiceId};

impl Lowerer<'_, '_> {
    /// Every example, by package and then name. The IR holds only what was written: what an
    /// example leaves out is filled in when it runs.
    pub(super) fn examples(&self) -> Vec<Example> {
        let ids: BTreeMap<usize, ServiceId> = self
            .service_order()
            .into_iter()
            .enumerate()
            .map(|(pos, old)| (old, ServiceId(u32::try_from(pos).unwrap_or(u32::MAX))))
            .collect();
        let mut out: Vec<Example> = self
            .prog
            .examples
            .iter()
            .map(|ex| {
                self.example(
                    ex,
                    ids.get(&ex.service).copied().unwrap_or(ServiceId(u32::MAX)),
                )
            })
            .collect();
        out.extend(self.event_examples(&ids));
        out.sort_by(|a, b| (a.package, &a.name).cmp(&(b.package, &b.name)));
        out
    }

    /// The saga lines of an example.
    fn example_saga(&self, s: &RSagaExample) -> ExampleSaga {
        let given = |list: &[(String, RSagaAnswer)]| -> Vec<SagaGiven> {
            list.iter()
                .map(|(step, a)| SagaGiven {
                    step: step.clone(),
                    answer: match a {
                        RSagaAnswer::Unavailable => SagaAnswer::Unavailable,
                        RSagaAnswer::Answer(o) => SagaAnswer::Answer(self.outcome(o)),
                    },
                })
                .collect()
        };
        ExampleSaga {
            steps: given(&s.steps),
            undos: given(&s.undos),
            expect: s.expect.as_ref().map(|e| match e {
                RSagaExpect::Compensated(steps) => SagaExpect::Compensated(steps.clone()),
                RSagaExpect::Stuck => SagaExpect::Stuck,
            }),
        }
    }

    fn outcome(&self, o: &ROutcome) -> ExampleOutcome {
        match o {
            ROutcome::Ok(v) => ExampleOutcome::Ok(v.clone()),
            ROutcome::Error(decl, pattern) => ExampleOutcome::Error(ExampleErrorOutcome {
                decl: self.decl_id(*decl),
                pattern: pattern.clone(),
            }),
        }
    }

    fn example(&self, ex: &RExample, service: ServiceId) -> Example {
        Example {
            name: ex.name.clone(),
            package: self.pkg_id(ex.pkg),
            service: Some(service),
            endpoint: ex.endpoint.clone(),
            givens: ex
                .givens
                .iter()
                .map(|(fact, value)| ExampleGiven {
                    fact: fact.clone(),
                    value: value.clone(),
                })
                .collect(),
            principal: ex.principal.clone(),
            now: ex.now.clone(),
            args: ex
                .args
                .iter()
                .map(|(param, value)| ExampleArg {
                    param: param.clone(),
                    value: value.clone(),
                })
                .collect(),
            returns: ex.returns.clone(),
            if_match: ex.if_match.clone(),
            key: ex.key.as_ref().map(|k| ExampleKey {
                value: k.value.clone(),
                store: match &k.store {
                    RKeyStore::New => KeyStore::New,
                    RKeyStore::Other => KeyStore::Other,
                    RKeyStore::Running => KeyStore::Running,
                    RKeyStore::Stored(o) => KeyStore::Stored(self.outcome(o)),
                },
            }),
            saga: ex.saga.as_ref().map(|s| self.example_saga(s)),
            outcome: Some(self.outcome(&ex.outcome)),
            docs: ex.docs.clone(),
            event: None,
            expect: None,
            origin: self.origin(ex.span, ex.module),
        }
    }
}
