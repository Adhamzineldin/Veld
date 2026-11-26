//! The broker adapters of events phase E2 (`spec/events.md` section 10.7,
//! `spec/targets/rust.md` section 13): with the target option `messaging`, one generated module
//! per broker, `src/adapters/kafka.rs` and `src/adapters/rabbitmq.rs`. Each is the only module
//! written against its library, and the generated `Cargo.toml` names the library only when the
//! adapter is chosen.

use serde::Deserialize;
use veld_codegen::{GenDiagnostic, Role};

use crate::tree::Tree;

/// The value of the option `messaging`: one broker, or a list of them.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Messaging {
    One(String),
    Many(Vec<String>),
}

impl Messaging {
    fn names(&self) -> Vec<&str> {
        match self {
            Messaging::One(name) => vec![name.as_str()],
            Messaging::Many(names) => names.iter().map(String::as_str).collect(),
        }
    }
}

/// A broker with an adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Broker {
    Kafka,
    RabbitMq,
}

impl Broker {
    fn parse(name: &str) -> Option<Broker> {
        match name {
            "kafka" => Some(Broker::Kafka),
            "rabbitmq" => Some(Broker::RabbitMq),
            _ => None,
        }
    }
}

fn e0802(message: String) -> Vec<GenDiagnostic> {
    vec![GenDiagnostic::new(
        "E0802",
        format!("invalid options for generator `rust`: {message}"),
    )]
}

/// The adapters the option asks for, once each, in a fixed order.
///
/// # Errors
/// `E0802` for a value that names no broker, and for the option on a client and on a target none of
/// whose services publishes or subscribes.
pub fn choose(
    messaging: Option<&Messaging>,
    role: Role,
    tree: &Tree<'_>,
) -> Result<Vec<Broker>, Vec<GenDiagnostic>> {
    let Some(messaging) = messaging else {
        return Ok(Vec::new());
    };
    let mut chosen = Vec::new();
    for name in messaging.names() {
        let broker = Broker::parse(name).ok_or_else(|| {
            e0802(format!(
                "option `messaging` = `{name}` is not a broker with an adapter (`kafka`, `rabbitmq`)"
            ))
        })?;
        chosen.push(broker);
    }
    chosen.sort_unstable();
    chosen.dedup();
    if !chosen.is_empty() && (role != Role::Server || !tree.uses_events()) {
        return Err(e0802(
            "option `messaging` needs a server with a service that publishes or subscribes".to_string(),
        ));
    }
    Ok(chosen)
}
