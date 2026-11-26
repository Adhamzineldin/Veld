//! The reference of the outbox and the relay (`spec/events.md` section 11.1), the oracle the
//! generated `emit`, `MemoryOutbox` and `run<Service>OutboxRelay` are compared with.
//!
//! * [`save_of`] is steps 1 to 5 of publishing for one emitted event: what `emit.<event>(value)`
//!   hands to `outbox.save`. It reuses the envelope encoder, so the bytes are the ones of
//!   `tests/wire/events`.
//! * [`relay`] is the relay as a pure function over the pending entries and a script of what the
//!   transport and `done` answer.
//! * [`MemoryOutbox`] is the in-process outbox of the generated tests.
//! * [`expected_events`] and [`check_written`] are what the server checks after a handler.
//!
//! Nothing here touches a database, a clock or a transport.

mod memory;
mod relay;
mod save;
mod written;

pub use memory::MemoryOutbox;
pub use relay::{DEFAULT_LIMIT, Relay, Step, relay};
pub use save::{OutboxEntry, OutboxError, Save, save_of};
pub use written::{WrittenFault, check_written, expected_events};
