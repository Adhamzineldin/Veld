//! Lowering of channels, subscriptions and event examples (`spec/events.md` section 13).

use std::collections::BTreeMap;

use super::Lowerer;
use crate::semantic::{ChannelInfo, REventExample, REventExpect, RSubscription, ServiceInfo};
use veld_ir::endpoint_rules::{EventExpect, Example, ExampleEvent, ExpectedCall};
use veld_ir::{Channel, ChannelId, ChannelKey, Handler, Publication, ServiceId, Subscription};

impl Lowerer<'_, '_> {
    /// Indices into `prog.channels`, in IR order: by qualified name.
    pub(super) fn channel_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.prog.channels.len())
            .filter(|&i| self.prog.channels[i].res.is_some())
            .collect();
        order.sort_by(|&a, &b| {
            self.prog.channels[a]
                .qualified
                .cmp(&self.prog.channels[b].qualified)
        });
        order
    }

    pub(super) fn channel_id(&self, old: usize) -> ChannelId {
        self.channel_map
            .get(old)
            .copied()
            .flatten()
            .unwrap_or(ChannelId(u32::MAX))
    }

    pub(super) fn channels(&self) -> Vec<Channel> {
        self.channel_order()
            .into_iter()
            .map(|i| self.channel(&self.prog.channels[i]))
            .collect()
    }

    fn channel(&self, c: &ChannelInfo<'_>) -> Channel {
        let res = c.res.as_ref();
        Channel {
            name: c.name.clone(),
            package: self.pkg_id(c.pkg),
            module: self.prog.modules[c.module].path.clone(),
            qualified: c.qualified.clone(),
            topic: res.map(|r| r.topic.clone()).unwrap_or_default(),
            events: res
                .map(|r| r.events.iter().map(|e| self.decl_id(*e)).collect())
                .unwrap_or_default(),
            key: res.and_then(|r| r.key.as_ref()).map(|k| ChannelKey {
                name: k.name.clone(),
                wire: k.wire.clone(),
                ty: self.remap(&k.ty),
            }),
            docs: res.and_then(|r| r.docs.clone()),
            deprecated: res.and_then(|r| r.deprecated.clone()),
            origin: self.origin(c.name_span, c.module),
        }
    }

    pub(super) fn service_publishes(&self, s: &ServiceInfo) -> Vec<Publication> {
        s.publishes
            .iter()
            .zip(&s.publish_spans)
            .map(|(c, span)| Publication {
                channel: self.channel_id(*c),
                origin: self.origin(*span, s.module),
            })
            .collect()
    }

    pub(super) fn service_subscriptions(&self, s: &ServiceInfo) -> Vec<Subscription> {
        s.subscriptions
            .iter()
            .map(|sub| self.subscription(sub, s.module))
            .collect()
    }

    fn subscription(&self, sub: &RSubscription, module: usize) -> Subscription {
        Subscription {
            channel: self.channel_id(sub.channel),
            group: sub.group.clone(),
            handlers: sub
                .handlers
                .iter()
                .map(|h| Handler {
                    event: self.decl_id(h.event),
                    name: h.name.clone(),
                    docs: h.docs.clone(),
                    origin: self.origin(h.span, module),
                })
                .collect(),
            origin: self.origin(sub.span, module),
        }
    }

    /// The event examples, lowered; `ids` maps a program service index to its `ServiceId`.
    pub(super) fn event_examples(&self, ids: &BTreeMap<usize, ServiceId>) -> Vec<Example> {
        self.prog
            .event_examples
            .iter()
            .map(|ex| self.event_example(ex, ids))
            .collect()
    }

    fn event_example(&self, ex: &REventExample, ids: &BTreeMap<usize, ServiceId>) -> Example {
        let expect = match &ex.expect {
            REventExpect::Rejected => EventExpect::Rejected,
            REventExpect::Calls(list) => EventExpect::Calls(
                list.iter()
                    .map(|(s, handler, called)| ExpectedCall {
                        service: ids.get(s).copied().unwrap_or(ServiceId(u32::MAX)),
                        handler: handler.clone(),
                        called: *called,
                    })
                    .collect(),
            ),
        };
        Example {
            name: ex.name.clone(),
            package: self.pkg_id(ex.pkg),
            service: None,
            endpoint: String::new(),
            givens: Vec::new(),
            principal: None,
            now: None,
            args: Vec::new(),
            returns: None,
            outcome: None,
            docs: ex.docs.clone(),
            event: Some(ExampleEvent {
                decl: self.decl_id(ex.event),
                channel: self.channel_id(ex.channel),
                value: ex.value.clone(),
            }),
            expect: Some(expect),
            origin: self.origin(ex.span, ex.module),
        }
    }
}
