//! Errors of a live run.

use veld_rules::ExampleError;

/// Why an example could not be run against a server.
#[derive(Debug, thiserror::Error)]
pub enum LiveError {
    /// The server could not be reached or the connection broke.
    #[error("cannot reach {url}: {message}")]
    Connect { url: String, message: String },
    /// The server answered the probe, but not as a test handler (`spec/rules.md` section 12.4).
    #[error("{url} is not a Veld test handler: {reason}")]
    NotTestHandler { url: String, reason: String },
    /// The example calls an endpoint a request cannot be built for.
    #[error("example \"{example}\": cannot run it live: {reason}")]
    Unsupported { example: String, reason: String },
    /// The example cannot be filled in.
    #[error(transparent)]
    Example(#[from] ExampleError),
}
