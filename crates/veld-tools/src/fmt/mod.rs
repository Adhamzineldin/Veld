//! The canonical formatter: `format(source)` prints from the syntax tree plus
//! the comment side-table, so every comment survives and the output is a fixed
//! point of the formatter.

mod expr;
mod layout;
mod print;
mod saga;
mod stmt;
pub(crate) mod text;

use veld_compiler::{Diagnostic, FileId, parse_file};

/// Maximum line width before enums wrap and endpoint responses move to
/// continuation lines.
pub(crate) const WIDTH: usize = 100;

/// Formats one `.veld` source file.
///
/// # Errors
/// Returns the diagnostics when the file has syntax errors, or a doc comment
/// that is attached to nothing (printing from the tree would drop it).
pub fn format(source: &str) -> Result<String, Vec<Diagnostic>> {
    let (file, comments, diags) = parse_file(source, FileId(0));
    if diags.iter().any(|d| d.is_error() || d.code == "W0001") {
        return Err(diags);
    }
    let lay = layout::Layout::build(source, &file, &comments);
    Ok(print::print_file(&file, &lay))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod rule_tests;

#[cfg(test)]
mod delivery_tests;
#[cfg(test)]
mod endpoint_tests;

#[cfg(test)]
mod policy_tests;

#[cfg(test)]
mod event_tests;

#[cfg(test)]
mod derived_tests;
#[cfg(test)]
mod flags_tests;
#[cfg(test)]
mod rollout_tests;
#[cfg(test)]
mod saga_tests;
#[cfg(test)]
mod timed_tests;
#[cfg(test)]
mod version_tests;
