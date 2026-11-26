//! The broker adapters of events phase E2 (`spec/events.md` section 10.7, `spec/targets/typescript.md`
//! section 5f): with the target option `messaging`, one generated file per broker, `server/kafka.ts`
//! and `server/rabbitmq.ts`. Each is the only file written against its library; the generated
//! `package.json` names the library as a peer dependency only when the adapter is chosen.

use serde::Deserialize;
use veld_codegen::{FileSet, GenDiagnostic};
use veld_emit_typescript::tree::Tree;
use veld_emit_typescript::util::add_file;

const KAFKA: &str = include_str!("../runtime/brokers/kafka.ts");
const RABBITMQ: &str = include_str!("../runtime/brokers/rabbitmq.ts");

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

    /// Root-relative module path of the adapter, without extension.
    fn module(self) -> &'static str {
        match self {
            Broker::Kafka => "server/kafka",
            Broker::RabbitMq => "server/rabbitmq",
        }
    }

    fn source(self) -> &'static str {
        match self {
            Broker::Kafka => KAFKA,
            Broker::RabbitMq => RABBITMQ,
        }
    }

    /// The library the adapter is written against, and the range the generated manifest asks for.
    fn peer(self) -> (&'static str, &'static str) {
        match self {
            Broker::Kafka => ("kafkajs", "^2.2.4"),
            Broker::RabbitMq => ("amqplib", "^0.10.0 || ^2.0.0"),
        }
    }
}

fn e0802(message: String) -> Vec<GenDiagnostic> {
    vec![GenDiagnostic::new("E0802", message)]
}

/// The adapters the option asks for, once each, in a fixed order.
///
/// # Errors
/// `E0802` for a value that names no broker, and for the option on a target none of whose
/// services publishes or subscribes.
pub fn choose(
    messaging: Option<&Messaging>,
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
    if !chosen.is_empty() && !tree.uses_events() {
        return Err(e0802(
            "option `messaging` needs a service that publishes or subscribes: none of the services of this target does"
                .to_owned(),
        ));
    }
    Ok(chosen)
}

/// Writes the adapter files and returns their module paths for the barrel.
pub fn emit(files: &mut FileSet, brokers: &[Broker]) -> Result<Vec<String>, Vec<GenDiagnostic>> {
    let mut modules = Vec::new();
    for broker in brokers {
        let module = broker.module();
        add_file(files, &format!("{module}.ts"), broker.source().to_owned())?;
        modules.push(module.to_owned());
    }
    Ok(modules)
}

/// The peer dependencies of the chosen adapters.
#[must_use]
pub fn peers(brokers: &[Broker]) -> Vec<(&'static str, &'static str)> {
    brokers.iter().map(|b| b.peer()).collect()
}
