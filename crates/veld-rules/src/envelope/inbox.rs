//! The reference inbox of `spec/events.md` section 10.3: a memory implementation for tests.
//!
//! `handle(source, id, run)` runs `run` and records the pair, or does nothing when the pair is
//! already recorded. When `run` fails the pair is not recorded and the failure is the handler's.

use std::collections::BTreeSet;

/// Remembers which `(source, id)` pairs were handled.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryInbox {
    seen: BTreeSet<(String, String)>,
}

impl MemoryInbox {
    #[must_use]
    pub fn new() -> MemoryInbox {
        MemoryInbox::default()
    }

    /// Runs `run` unless the pair was handled. Answers whether `run` ran.
    ///
    /// # Errors
    /// What `run` fails with; the pair is then not recorded.
    pub fn handle<E>(
        &mut self,
        source: &str,
        id: &str,
        run: impl FnOnce() -> Result<(), E>,
    ) -> Result<bool, E> {
        let key = (source.to_owned(), id.to_owned());
        if self.seen.contains(&key) {
            return Ok(false);
        }
        run()?;
        self.seen.insert(key);
        Ok(true)
    }

    /// True when the pair was handled.
    #[must_use]
    pub fn contains(&self, source: &str, id: &str) -> bool {
        self.seen.contains(&(source.to_owned(), id.to_owned()))
    }

    /// How many pairs are recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}
