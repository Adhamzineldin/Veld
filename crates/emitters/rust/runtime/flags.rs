//! Flags (`spec/platform.md` section 11): the provider the hooks name, a memory provider for tests,
//! and the context of a reading.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Mutex;

use super::error::Error;
// @if server
use super::server::Principal;
// @end

/// What a provider is told about the request that asks: the principal (none without one), the
/// caller's tenant text (none without one, tenancy is not generated yet) and the whole operation
/// signature of the endpoint (the signature of the event for an event).
#[derive(Debug, Clone)]
pub struct FlagContext {
    // @if server
    pub principal: Option<Principal>,
    // @end
    // @if !server
    // A crate without a server core (a service that only publishes or subscribes) has no
    // `principal`: no caller is ever known.
    // @end
    pub tenant: Option<String>,
    pub operation: String,
}
// @if event_flags

impl FlagContext {
    /// The context of an event's flags: no principal, no tenant, the signature of the event.
    #[must_use]
    pub fn of_event(operation: &str) -> Self {
        FlagContext {
            // @if server
            principal: None,
            // @end
            tenant: None,
            operation: operation.to_string(),
        }
    }
}
// @end

/// Answers whether a flag is on. The server asks each flag an endpoint can reach once per request,
/// after authentication, and uses the answer until the response is sent. An `Err` is a failing
/// provider: a statement that restricts applies and one that grants does not
/// (`spec/platform.md` section 11), and the error goes to `on_error`.
pub trait FlagProvider: Send + Sync + 'static {
    fn is_on<'a>(&'a self, name: &'a str, context: &'a FlagContext) -> impl Future<Output = Result<bool, Error>> + Send + 'a;
}

/// A provider over a set of names that are on, for tests. A flag it was not told about is off.
#[derive(Debug, Default)]
pub struct MemoryFlagProvider {
    flags: Mutex<BTreeMap<String, bool>>,
}

impl MemoryFlagProvider {
    /// A provider with the flags `on` turned on.
    #[must_use]
    pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(on: I) -> Self {
        MemoryFlagProvider { flags: Mutex::new(on.into_iter().map(|n| (n.into(), true)).collect()) }
    }

    /// Turns a flag on or off.
    pub fn set(&self, name: &str, on: bool) {
        if let Ok(mut flags) = self.flags.lock() {
            flags.insert(name.to_string(), on);
        }
    }
}

impl FlagProvider for MemoryFlagProvider {
    fn is_on<'a>(&'a self, name: &'a str, _context: &'a FlagContext) -> impl Future<Output = Result<bool, Error>> + Send + 'a {
        let on = self.flags.lock().map(|f| f.get(name).copied().unwrap_or(false)).unwrap_or(false);
        async move { Ok(on) }
    }
}
