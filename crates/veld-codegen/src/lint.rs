//! The severity a lint rule is configured to (`[lint]` in `veld.toml`). Shared by the project
//! config (`veld-project`) and the lint tool (`veld-tools`).

use serde::Deserialize;

/// What happens when a lint rule finds something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintLevel {
    /// The rule does not run.
    Off,
    /// Findings are reported and do not fail the command (the default).
    Warn,
    /// Findings are reported and make `veld check` exit 1.
    Error,
}

impl LintLevel {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            LintLevel::Off => "off",
            LintLevel::Warn => "warn",
            LintLevel::Error => "error",
        }
    }
}
