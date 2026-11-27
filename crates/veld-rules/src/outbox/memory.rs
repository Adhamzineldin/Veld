//! The reference `MemoryOutbox` (`spec/events.md` section 11.1).

use super::relay::{Relay, relay};
use super::{OutboxEntry, Save, Step};

/// An outbox in the process: `save` appends, `pending` lists the entries that are not `done`,
/// oldest first, `done` removes one.
///
/// The ids are decimal counters that start at `"1"`; an id is the application's own and opaque,
/// and a generated `MemoryOutbox` may choose another.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryOutbox {
    entries: Vec<OutboxEntry>,
    next: u64,
}

impl MemoryOutbox {
    #[must_use]
    pub fn new() -> MemoryOutbox {
        MemoryOutbox::default()
    }

    /// Appends an entry and answers its id.
    pub fn save(&mut self, save: Save) -> String {
        self.next += 1;
        let id = self.next.to_string();
        self.entries.push(OutboxEntry {
            id: id.clone(),
            topic: save.topic,
            key: save.key,
            bytes: save.bytes,
        });
        id
    }

    /// Up to `limit` entries that are not done, oldest first.
    #[must_use]
    pub fn pending(&self, limit: usize) -> Vec<OutboxEntry> {
        self.entries.iter().take(limit).cloned().collect()
    }

    /// Marks an entry done. An id that is not pending is ignored.
    pub fn done(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id);
    }

    /// How many entries are not done.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// One run of the relay against this outbox: [`relay`] over `pending(limit)` and `script`,
    /// then `done` for every entry the run marked done. The entries stay as they are otherwise.
    pub fn run_relay(&mut self, limit: usize, script: &[Step]) -> Relay {
        let outcome = relay(&self.pending(limit), limit, script);
        for id in &outcome.done {
            self.done(id);
        }
        outcome
    }
}
