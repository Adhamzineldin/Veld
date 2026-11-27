//! The broker adapters of events phase E2 (`spec/events.md` section 10.7, `spec/targets/java.md`
//! section 12): with the target option `messaging`, one generated file per broker,
//! `<base>.adapters.KafkaTransport` and `<base>.adapters.RabbitMqTransport`. Each is the only file
//! written against its library. The Java target writes no build manifest, so the README names the
//! library and its coordinates, only when the adapter is chosen.

use serde::Deserialize;
use veld_codegen::{FileSet, GenDiagnostic};

use crate::runtime::{Streaming, adapter_package, substitute};
use crate::tree::{Tree, java_path};
use crate::util::add_file;

const KAFKA: &str = include_str!("../adapters/KafkaTransport.java");
const RABBITMQ: &str = include_str!("../adapters/RabbitMqTransport.java");

/// The value of the option `messaging`: one broker, or a list of them.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Messaging {
    /// `messaging = "kafka"`.
    One(String),
    /// `messaging = ["kafka", "rabbitmq"]`.
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
    /// `KafkaTransport`.
    Kafka,
    /// `RabbitMqTransport`.
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

    fn class(self) -> &'static str {
        match self {
            Broker::Kafka => "KafkaTransport",
            Broker::RabbitMq => "RabbitMqTransport",
        }
    }

    fn source(self) -> &'static str {
        match self {
            Broker::Kafka => KAFKA,
            Broker::RabbitMq => RABBITMQ,
        }
    }

    /// The Maven coordinates of the library the adapter is written against, and the version it was
    /// compiled and tested with.
    fn library(self) -> (&'static str, &'static str) {
        match self {
            Broker::Kafka => ("org.apache.kafka:kafka-clients", "4.1.2"),
            Broker::RabbitMq => ("com.rabbitmq:amqp-client", "5.27.1"),
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
                "invalid options for generator `java`: `messaging` = `{name}` is not a broker with an adapter (`kafka`, `rabbitmq`)"
            ))
        })?;
        chosen.push(broker);
    }
    chosen.sort_unstable();
    chosen.dedup();
    if !chosen.is_empty() && !tree.events {
        return Err(e0802(
            "invalid options for generator `java`: `messaging` needs a service that publishes or subscribes: none of the services of this target does"
                .to_owned(),
        ));
    }
    Ok(chosen)
}

/// Writes the adapter files below `<base>.adapters`.
///
/// # Errors
/// A diagnostic when a path is written twice.
pub fn emit(files: &mut FileSet, base: &str, brokers: &[Broker]) -> Result<(), Vec<GenDiagnostic>> {
    let package = adapter_package(base);
    for broker in brokers {
        add_file(
            files,
            &java_path(&package, broker.class()),
            substitute(broker.source(), base, Streaming::default()),
        )?;
    }
    Ok(())
}

/// The paragraph of the README that names the libraries of the chosen adapters; empty when none.
#[must_use]
pub fn readme_text(base: &str, brokers: &[Broker]) -> String {
    if brokers.is_empty() {
        return String::new();
    }
    let mut t = String::from(
        "## Message brokers\n\nThe adapters below are the only generated files that need a library, and the generator writes no build file, so add the dependency to yours:\n\n",
    );
    for b in brokers {
        let (artifact, version) = b.library();
        t.push_str(&format!(
            "- `{base}.adapters.{}` needs `{artifact}` (written and tested against {version}).\n",
            b.class()
        ));
    }
    t.push_str("\nBuild the adapter from what your application configured: `new KafkaTransport(properties)` from the Kafka client properties (addresses, TLS, credentials), `new RabbitMqTransport(connection)` from an open `com.rabbitmq.client.Connection`; close the transport before the client or the connection.\n\n");
    t
}
