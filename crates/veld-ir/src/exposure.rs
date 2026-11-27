//! What a response can contain (`spec/rules.md` section 19.3, `spec/platform.md` section 23).
//!
//! The lints `pii-without-auth`, `secret-in-output` and `pii-without-retention` and the export of
//! `veld export retention` ask the same question: which declarations can an answer of an endpoint,
//! or an event, carry? An answer is the success value (each item of a stream), and every declared
//! error. A request body or a parameter is not output, and neither is a fact, which the
//! application loads and never sends.

use std::collections::{BTreeMap, BTreeSet};

use crate::{DeclId, Endpoint, Ir, ServiceId};

/// Where a declaration can appear on the wire as output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Carrier {
    /// The answer of an endpoint: `service`, and the index into its `endpoints`.
    Endpoint { service: ServiceId, endpoint: usize },
    /// An event: the declaration of the event.
    Event(DeclId),
}

impl Ir {
    /// Every declaration an answer of `ep` can contain: the success type (the item type of a
    /// stream) and the declared errors, with what their fields, parents and union arms reach.
    /// Sorted by id. Parameters are not part of it.
    #[must_use]
    pub fn output_decls(&self, ep: &Endpoint) -> Vec<DeclId> {
        let mut stack = Vec::new();
        if let Some(t) = &ep.success.ty {
            t.collect_decls(&mut stack);
        }
        stack.extend(ep.errors.iter().map(|e| e.decl));
        self.close_over(stack)
    }

    /// The event `id` and every declaration its fields reach, sorted by id.
    #[must_use]
    pub fn event_output_decls(&self, id: DeclId) -> Vec<DeclId> {
        self.close_over(vec![id])
    }

    /// For each declaration, the endpoints and events that can carry it as output, in service,
    /// endpoint and then declaration order. A declaration nothing carries is absent.
    #[must_use]
    pub fn exposure_index(&self) -> BTreeMap<DeclId, Vec<Carrier>> {
        let mut index: BTreeMap<DeclId, BTreeSet<Carrier>> = BTreeMap::new();
        for s in &self.services {
            for (i, ep) in s.endpoints.iter().enumerate() {
                let at = Carrier::Endpoint {
                    service: s.id,
                    endpoint: i,
                };
                for d in self.output_decls(ep) {
                    index.entry(d).or_default().insert(at);
                }
            }
        }
        for d in &self.decls {
            if d.is_event() {
                for r in self.event_output_decls(d.id) {
                    index.entry(r).or_default().insert(Carrier::Event(d.id));
                }
            }
        }
        index
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect()
    }
}
