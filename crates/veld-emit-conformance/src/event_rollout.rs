//! Contracts whose only audited invariant sits behind an event (`spec/rules.md` section 17.1).
//!
//! No endpoint reaches an event, so a service that publishes or subscribes to a channel carrying an
//! event with an audited invariant uses `rules_rollout` through its channels. These contracts let a
//! gate test see that. No generator is checked against them here: what a generator does with them
//! is its own decision.

use veld_ir::{Ir, ServiceId};

use crate::compile;

/// An audited invariant on the event itself, published by `Hello`.
const ON_THE_EVENT: &str = "event Greeted {
  name: string
  @rollout(audit)
  rule named: len(name) > 0
}

channel GreetingEvents {
  carries Greeted
  key name
}

service Hello at \"/hello\" {
  publishes GreetingEvents
  get \"/{name}\" greet(name: string) -> 204
}

service Audit {
  subscribes GreetingEvents as \"audit\" {
    on Greeted -> record
  }
}
";

/// An audited invariant on a model that the event reaches through a field.
const ON_A_REACHED_MODEL: &str = "model Address {
  street: string
  @rollout(audit)
  rule filled: len(street) > 0
}

event Moved {
  id: string
  to: Address
}

channel MoveEvents {
  carries Moved
  key id
}

service Registry {
  publishes MoveEvents
}

service Movers {
  subscribes MoveEvents as \"movers\" {
    on Moved -> apply
  }
}
";

/// A contract with the audited invariant on an event, and the line of the rule.
#[derive(Debug)]
pub struct EventRolloutContract {
    pub ir: Ir,
    /// The line of the first audited rule.
    pub line: u32,
    /// The services of the contract that publish or subscribe, by name.
    pub services: Vec<String>,
}

impl EventRolloutContract {
    /// The id of the service called `name`.
    #[must_use]
    pub fn service(&self, name: &str) -> Option<ServiceId> {
        self.ir
            .services
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.id)
    }
}

fn line_of(source: &str, needle: &str) -> u32 {
    source
        .lines()
        .position(|l| l.contains(needle))
        .and_then(|i| u32::try_from(i + 1).ok())
        .unwrap_or(0)
}

/// The contracts: the invariant on the event, and on a model the event reaches. Each is also
/// returned without the annotation by [`without_the_annotation`].
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn compiled_event_rollout_contracts() -> Vec<EventRolloutContract> {
    let cases = [
        (
            "compiled_event_rollout_on_event",
            ON_THE_EVENT,
            "rule named",
            ["Hello", "Audit"],
        ),
        (
            "compiled_event_rollout_reached",
            ON_A_REACHED_MODEL,
            "rule filled",
            ["Registry", "Movers"],
        ),
    ];
    cases
        .into_iter()
        .map(|(name, source, needle, services)| EventRolloutContract {
            ir: compile(name, source),
            line: line_of(source, needle),
            services: services.iter().map(ToString::to_string).collect(),
        })
        .collect()
}

/// The same contracts with `@rollout(audit)` removed, in the order of
/// [`compiled_event_rollout_contracts`].
///
/// # Panics
/// When a contract of this crate does not compile.
#[must_use]
pub fn without_the_annotation() -> Vec<Ir> {
    [
        ("compiled_event_rollout_on_event_plain", ON_THE_EVENT),
        ("compiled_event_rollout_reached_plain", ON_A_REACHED_MODEL),
    ]
    .into_iter()
    .map(|(name, source)| compile(name, &source.replace("  @rollout(audit)\n", "")))
    .collect()
}

#[cfg(test)]
mod tests {
    use veld_codegen::{Capabilities, GenRequest, Role, check_capabilities};
    use veld_ir::Feature;

    use super::*;

    fn gate(
        ir: &Ir,
        role: Role,
        services: Vec<ServiceId>,
        caps: &Capabilities,
    ) -> Vec<veld_codegen::GenDiagnostic> {
        let req = GenRequest {
            ir,
            role,
            services,
            consumes: vec![],
            framework: None,
        };
        check_capabilities(ir, &req, caps)
    }

    fn eligible(c: &EventRolloutContract) -> Vec<ServiceId> {
        let mut ids: Vec<ServiceId> = c.services.iter().filter_map(|n| c.service(n)).collect();
        ids.dedup();
        ids
    }

    #[test]
    fn a_publisher_and_a_subscriber_use_rules_rollout() {
        for c in compiled_event_rollout_contracts() {
            for id in eligible(&c) {
                let used = c.ir.features_used(&[id]);
                assert!(used.contains(&Feature::RulesRollout), "{id:?}: {used:?}");
                assert!(used.contains(&Feature::Events), "{id:?}: {used:?}");
            }
        }
    }

    #[test]
    fn without_rules_rollout_the_gate_points_at_the_audited_rule() {
        let caps = Capabilities::all_except(&[]);
        assert!(caps.supports(Feature::Events) && !caps.supports(Feature::RulesRollout));
        for c in compiled_event_rollout_contracts() {
            for id in eligible(&c) {
                let diags = gate(&c.ir, Role::Server, vec![id], &caps);
                assert_eq!(diags.len(), 1, "{diags:?}");
                assert_eq!(diags[0].code, "E0801");
                assert!(diags[0].message.contains("`rules_rollout`"), "{diags:?}");
                assert_eq!(diags[0].origin.as_ref().map(|o| o.line), Some(c.line));
            }
        }
    }

    #[test]
    fn with_both_features_the_gate_reports_nothing() {
        let caps = Capabilities::all_except(&[]).with(Feature::RulesRollout);
        for c in compiled_event_rollout_contracts() {
            for id in eligible(&c) {
                assert!(gate(&c.ir, Role::Server, vec![id], &caps).is_empty());
            }
        }
    }

    #[test]
    fn a_client_and_a_caller_are_not_concerned() {
        let caps = Capabilities::all_except(&[]);
        for c in compiled_event_rollout_contracts() {
            let all: Vec<ServiceId> = c.ir.services.iter().map(|s| s.id).collect();
            assert!(gate(&c.ir, Role::Client, all, &caps).is_empty());
        }
        // A server that only calls the publisher over HTTP generates nothing for its events.
        let c = &compiled_event_rollout_contracts()[0];
        let Some(hello) = c.service("Hello") else {
            panic!("no Hello")
        };
        let req = GenRequest {
            ir: &c.ir,
            role: Role::Server,
            services: vec![],
            consumes: vec![hello],
            framework: None,
        };
        assert!(check_capabilities(&c.ir, &req, &caps).is_empty());
    }

    #[test]
    fn the_same_contract_without_the_annotation_does_not_use_rules_rollout() {
        let caps = Capabilities::all_except(&[]);
        for (ir, c) in without_the_annotation()
            .iter()
            .zip(compiled_event_rollout_contracts())
        {
            for id in eligible(&c) {
                let used = ir.features_used(&[id]);
                assert!(!used.contains(&Feature::RulesRollout), "{used:?}");
                assert!(gate(ir, Role::Server, vec![id], &caps).is_empty());
            }
        }
    }
}
