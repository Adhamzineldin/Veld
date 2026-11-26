//! The event envelope of `spec/events.md` section 6: the reference every generator's publisher
//! and consumer is compared with.
//!
//! * [`encode`] is what a publisher does after the checks of section 7: an event value and the
//!   inputs the publisher supplies (`id`, `source`, the time) become the canonical bytes of one
//!   CloudEvents 1.0 message.
//! * [`decode`] is steps 1 to 4 of section 8: bytes become the parts a consumer needs, or one of
//!   the failures the section names, as a typed [`ConsumeError`].
//!
//! The canonical writer (RFC 8785) is `veld_codegen::canonical`: it is the one the generators
//! and `tests/wire` already use, and `veld-codegen` depends on nothing but `veld-ir`, so this
//! crate takes the one extra edge instead of keeping a second copy of the writer.

pub(crate) mod canon;
mod consume;
mod dead_letter;
mod delivery;
mod inbox;
mod publish;

#[cfg(test)]
mod tests;

pub use consume::{ConsumeError, Received, decode};
pub use dead_letter::{
    ATTEMPTS_MEMBER, GROUP_MEMBER, ISSUES_MEMBER, Issue, REASON_MEMBER, Reason, VIOLATIONS_MEMBER,
    dead_letter_copy,
};
pub use delivery::{
    Answer, FailedRun, HandlerResult, Policy, Settlement, Timing, handled, ignored, rejected,
    wait_ms,
};
pub use inbox::MemoryInbox;
pub use publish::{Inputs, PublishError, encode, encode_unchecked, source_of};

/// The media type of a message (`spec/events.md` section 6.2).
pub const MEDIA_TYPE: &str = "application/cloudevents+json; charset=UTF-8";

/// The `datacontenttype` of every message.
pub const DATA_CONTENT_TYPE: &str = "application/json";

/// The extension attribute that carries the rules digest of the event.
pub const RULES_ATTRIBUTE: &str = "veldrules";

/// The extension attribute that carries the tenant of an event with a `@tenant` field
/// (`spec/platform.md` section 3).
pub const TENANT_ATTRIBUTE: &str = "veldtenant";

/// The declaration signature without its hash: `veld:pkg/Name`.
pub(crate) fn type_name(sig: &str) -> &str {
    sig.rsplit_once('@').map_or(sig, |(name, _)| name)
}
