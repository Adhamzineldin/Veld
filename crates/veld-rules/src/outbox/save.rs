//! What `emit` hands to `outbox.save` (`spec/events.md` section 11.1).

use serde_json::Value as Json;
use thiserror::Error;
use veld_ir::{ChannelId, DeclId, Ir, ServiceId};

use crate::envelope::{Inputs, PublishError, encode_with_key, source_of};

/// The arguments of `outbox.save(topic, key | none, bytes)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Save {
    pub topic: String,
    /// The key text (`subject`) of the message; `None` for a channel without a key.
    pub key: Option<String>,
    pub bytes: Vec<u8>,
}

/// An entry as `pending(limit)` answers it: the application's own opaque `id`, and what `save`
/// was given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxEntry {
    pub id: String,
    pub topic: String,
    pub key: Option<String>,
    pub bytes: Vec<u8>,
}

/// Why nothing is saved.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum OutboxError {
    /// The service does not publish the channel (`E1011` at compile time).
    #[error("the service does not publish the channel `{0}`")]
    NotPublished(String),
    /// Steps 1 to 5 of publishing failed: the typed `ValidationError` of the generated code.
    #[error(transparent)]
    Publish(#[from] PublishError),
}

/// Steps 1 to 5 of publishing (`spec/events.md` section 7) for the event `event` of the move,
/// written as `service`: shape, invariants, key, envelope and bytes. `id` is the id source of
/// the hooks, `time_ms` their clock. Nothing is sent to a transport.
///
/// # Errors
/// [`OutboxError`] when the service does not publish the channel, and for everything
/// [`crate::envelope::encode`] refuses; the handler's transaction should end with it.
pub fn save_of(
    ir: &Ir,
    service: ServiceId,
    channel: ChannelId,
    event: DeclId,
    value: &Json,
    id: &str,
    time_ms: i64,
) -> Result<Save, OutboxError> {
    let published = ir
        .service(service)
        .published_channels()
        .any(|c| c == channel);
    let target = ir
        .channels
        .get(channel.index())
        .ok_or(PublishError::NoSuchChannel)?;
    if !published {
        return Err(OutboxError::NotPublished(target.name.clone()));
    }
    let source = source_of(ir, service);
    let inputs = Inputs {
        id,
        source: &source,
        time_ms,
    };
    let (key, bytes) = encode_with_key(ir, channel, event, value, &inputs)?;
    Ok(Save {
        topic: target.topic.clone(),
        key,
        bytes,
    })
}
