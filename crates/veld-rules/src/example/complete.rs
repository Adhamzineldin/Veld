//! Filling in the values a partial wire value leaves out, for a tool that has a value with holes
//! in it (`veld test --from-violation`, `spec/rules.md` section 17.3): the rules of section 12.2,
//! applied to a value that was not written as an example.

use std::collections::BTreeSet;

use serde_json::Value as Json;
use veld_ir::{Constraints, Ir, Type};

use super::ExampleError;
use super::fill::Filler;

/// Completes wire values one after the other. The counters of section 12.2 (uuids, datetimes,
/// dates) run through everything it completes, like through one example.
pub struct Completer<'a> {
    filler: Filler<'a>,
}

impl<'a> Completer<'a> {
    /// A completer whose clock is `clock_ms`, milliseconds since the epoch.
    #[must_use]
    pub fn new(ir: &'a Ir, clock_ms: i64) -> Self {
        Completer {
            filler: Filler::new(ir, "capture", clock_ms),
        }
    }

    /// `given`, a value of type `ty` in wire form, with everything it leaves out filled in.
    /// Returns the value and the paths of the values that were filled in, each starting with
    /// `root` (for example `params.body.guest.phone`).
    ///
    /// # Errors
    /// [`ExampleError::CannotFill`] when a value cannot be filled in (a string with `@pattern`
    /// and no `@example`, for one).
    pub fn complete(
        &mut self,
        ty: &Type,
        given: &Json,
        root: &str,
    ) -> Result<(Json, BTreeSet<String>), ExampleError> {
        self.filler.begin();
        let value = self
            .filler
            .fill(ty, &Constraints::default(), Some(given), root)?;
        Ok((value, self.filler.filled.clone()))
    }
}

impl std::fmt::Debug for Completer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Completer").finish_non_exhaustive()
    }
}
