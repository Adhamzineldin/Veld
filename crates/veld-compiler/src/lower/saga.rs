//! Lowering of a checked saga (`spec/platform.md` sections 17 and 18).

use std::collections::BTreeMap;

use super::Lowerer;
use crate::semantic::{RArg, RCall, RObjectField, RSaga, RStep, RValue, ServiceInfo};
use veld_ir::{Saga, SagaArg, SagaCall, SagaField, SagaStep, SagaValue, ServiceId};

impl Lowerer<'_, '_> {
    /// The saga of an endpoint of service `s`.
    pub(super) fn saga(&self, saga: &RSaga, s: &ServiceInfo) -> Saga {
        let ids = self.service_ids();
        let mut passes: Vec<_> = saga.passes.iter().map(|d| self.decl_id(*d)).collect();
        passes.sort();
        Saga {
            steps: saga
                .steps
                .iter()
                .map(|st| self.saga_step(st, s, &ids))
                .collect(),
            timeout_ms: saga.timeout_ms,
            passes,
        }
    }

    /// The IR id of each service of the program, by its index in the program.
    fn service_ids(&self) -> BTreeMap<usize, ServiceId> {
        self.service_order()
            .into_iter()
            .enumerate()
            .map(|(new, old)| (old, ServiceId(u32::try_from(new).unwrap_or(u32::MAX))))
            .collect()
    }

    fn saga_step(&self, st: &RStep, s: &ServiceInfo, ids: &BTreeMap<usize, ServiceId>) -> SagaStep {
        SagaStep {
            name: st.name.clone(),
            call: self.saga_call(&st.call, ids),
            compensate: st.compensate.as_ref().map(|c| self.saga_call(c, ids)),
            origin: self.origin(st.span, s.module),
        }
    }

    fn saga_call(&self, c: &RCall, ids: &BTreeMap<usize, ServiceId>) -> SagaCall {
        SagaCall {
            service: ids.get(&c.service).copied().unwrap_or(ServiceId(u32::MAX)),
            endpoint: c.endpoint.clone(),
            args: c.args.iter().map(|a| self.saga_arg(a)).collect(),
        }
    }

    fn saga_arg(&self, a: &RArg) -> SagaArg {
        SagaArg {
            name: a.name.clone(),
            value: self.saga_value(&a.value),
        }
    }

    fn saga_value(&self, v: &RValue) -> SagaValue {
        match v {
            RValue::Expr(e) => SagaValue::Expr(self.remap_expr(e)),
            RValue::Object(fields) => SagaValue::Object {
                object: fields.iter().map(|f| self.saga_field(f)).collect(),
            },
        }
    }

    fn saga_field(&self, f: &RObjectField) -> SagaField {
        SagaField {
            name: f.name.clone(),
            wire: f.wire.clone(),
            value: self.saga_value(&f.value),
        }
    }
}
