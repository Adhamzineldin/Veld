//! Plugin errors. Codes match `spec/plugin-protocol.md`.

use thiserror::Error;
use veld_codegen::GenDiagnostic;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PluginError {
    /// `E0705`: the executable's SHA-256 differs from the one pinned in `veld.toml`.
    #[error("plugin `{plugin}` executable hash mismatch (expected {expected}, found {actual})")]
    HashMismatch {
        plugin: String,
        expected: String,
        actual: String,
    },
    /// `E0706`: a handshake or protocol problem (including a different protocol major version).
    #[error("plugin `{plugin}`: {message}")]
    Protocol { plugin: String, message: String },
    /// `E0707`: the plugin returned files the host refuses.
    #[error("plugin `{plugin}` returned a file the host refuses: {message}")]
    Files { plugin: String, message: String },
    /// `E0708`: the plugin could not be started, timed out, crashed or exited non-zero.
    #[error("plugin `{plugin}`: {message}")]
    Process { plugin: String, message: String },
    /// `E0802`: the `timeout` option is not usable.
    #[error("plugin `{plugin}`: invalid options: {message}")]
    Options { plugin: String, message: String },
}

impl PluginError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::HashMismatch { .. } => "E0705",
            Self::Protocol { .. } => "E0706",
            Self::Files { .. } => "E0707",
            Self::Process { .. } => "E0708",
            Self::Options { .. } => "E0802",
        }
    }

    pub(crate) fn protocol(plugin: &str, message: impl Into<String>) -> Self {
        Self::Protocol {
            plugin: plugin.to_string(),
            message: message.into(),
        }
    }

    pub(crate) fn process(plugin: &str, message: impl Into<String>) -> Self {
        Self::Process {
            plugin: plugin.to_string(),
            message: message.into(),
        }
    }

    pub(crate) fn files(plugin: &str, message: impl Into<String>) -> Self {
        Self::Files {
            plugin: plugin.to_string(),
            message: message.into(),
        }
    }
}

impl From<PluginError> for GenDiagnostic {
    fn from(e: PluginError) -> Self {
        GenDiagnostic::new(e.code(), e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_diagnostics() {
        let e = PluginError::process("k", "timed out");
        assert_eq!(e.code(), "E0708");
        let d: GenDiagnostic = e.into();
        assert_eq!(
            (d.code, d.message.as_str()),
            ("E0708", "plugin `k`: timed out")
        );
        assert_eq!(PluginError::protocol("k", "x").code(), "E0706");
        assert_eq!(PluginError::files("k", "x").code(), "E0707");
    }
}
