//! Lint rules over the IR (run by `veld check`).
//!
//! Lint finds contracts that are legal but probably not what the author meant. It never blocks
//! generation by itself: every rule defaults to `warn`, and `[lint]` in `veld.toml` sets each
//! rule to `off`, `warn` or `error`. Rule names and `L####` codes are the table
//! [`veld_compiler::codes::LINT_RULES`].
//!
//! Only packages of the project (services and libraries) are linted. The prelude and registry
//! packages are not the author's to fix. `unused-model` skips libraries, whose declarations are
//! there to be imported.

#[cfg(test)]
mod auth_tests;
mod config;
#[cfg(test)]
mod event_tests;
mod naming;
mod pii;
#[cfg(test)]
mod pii_tests;
mod rules;
#[cfg(test)]
mod tests;

use veld_ir::{Ir, Origin, PackageKind};

pub use config::{LintConfig, UnknownRule};
pub use veld_codegen::LintLevel;

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintIssue {
    /// `L####`.
    pub code: &'static str,
    /// The rule name, as used in `[lint]`.
    pub rule: &'static str,
    /// `Warn` or `Error` (never `Off`).
    pub level: LintLevel,
    /// Human-readable location, for example `Orders.getOrder` or `Order.total`.
    pub subject: String,
    pub message: String,
    pub origin: Origin,
}

/// Runs every enabled rule over the IR. Errors come first, then by file, line, column and code,
/// so the order is deterministic.
#[must_use]
pub fn lint(ir: &Ir, config: &LintConfig) -> Vec<LintIssue> {
    let mut raw = Vec::new();
    rules::run(ir, &mut raw);
    let mut issues: Vec<LintIssue> = raw
        .into_iter()
        .filter_map(|r| {
            let level = config.level(r.rule);
            (level != LintLevel::Off).then(|| r.into_issue(level))
        })
        .collect();
    issues.sort_by(|a, b| {
        (
            b.level,
            &a.origin.file,
            a.origin.line,
            a.origin.col,
            a.code,
            &a.subject,
        )
            .cmp(&(
                a.level,
                &b.origin.file,
                b.origin.line,
                b.origin.col,
                b.code,
                &b.subject,
            ))
    });
    issues
}

/// True when any issue has level `Error`.
#[must_use]
pub fn has_errors(issues: &[LintIssue]) -> bool {
    issues.iter().any(|i| i.level == LintLevel::Error)
}

/// Rule name and code pairs, in code order, for help text and `[lint]` completion.
#[must_use]
pub fn rule_names() -> Vec<(&'static str, &'static str)> {
    veld_compiler::codes::LINT_RULES
        .iter()
        .map(|(code, name)| (*name, *code))
        .collect()
}

/// Whether the package's declarations are the project's own (lint applies).
pub(crate) fn is_linted(kind: PackageKind) -> bool {
    matches!(kind, PackageKind::Service | PackageKind::Library)
}

/// [`is_linted`] for the tools that report on the project's own packages only.
pub(crate) fn is_linted_package(kind: PackageKind) -> bool {
    is_linted(kind)
}
