//! Feature-gated regions in embedded runtime files.
//!
//! A runtime file is real source in the target language, embedded with `include_str!` and
//! copied into the output. Some of its code is only needed when the contract uses an optional
//! feature (rules, for example). That code sits between two marker lines:
//!
//! ```text
//! // #if rules
//! ...lines kept only when the feature is on...
//! // #endif
//! ```
//!
//! [`gate_regions`] removes the marker lines and keeps or drops each region. The text outside
//! regions is copied byte for byte, so the output for a contract that does not use the feature
//! is exactly the file without its regions. That is what keeps "no rules, same bytes"
//! (`spec/rules.md` section 1) true while the runtime grows.
//!
//! Regions do not nest. Markers are whole lines: optional leading whitespace, `//`, one space,
//! then `#if <name>` or `#endif`.

/// A marker that does not pair up.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GateError {
    #[error("line {line}: `#if {name}` inside the region opened on line {opened}")]
    Nested {
        line: usize,
        name: String,
        opened: usize,
    },
    #[error("line {line}: `#endif` without `#if`")]
    StrayEnd { line: usize },
    #[error("line {opened}: `#if {name}` is never closed")]
    Unclosed { opened: usize, name: String },
}

enum Marker<'a> {
    If(&'a str),
    End,
}

fn marker(line: &str) -> Option<Marker<'_>> {
    let text = line.trim_end_matches(['\r', '\n']).trim();
    let rest = text.strip_prefix("// #")?;
    if rest == "endif" {
        return Some(Marker::End);
    }
    let name = rest.strip_prefix("if ")?.trim();
    if name.is_empty() || name.contains(char::is_whitespace) {
        return None;
    }
    Some(Marker::If(name))
}

/// Returns `source` without its marker lines, keeping a region only when `enabled(name)` is true.
///
/// # Errors
/// The markers do not pair up.
pub fn gate_regions(source: &str, enabled: &dyn Fn(&str) -> bool) -> Result<String, GateError> {
    let mut out = String::with_capacity(source.len());
    // The open region: its name, the line it opened on, and whether its lines are kept.
    let mut open: Option<(&str, usize, bool)> = None;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        let number = index + 1;
        match (marker(line), open) {
            (Some(Marker::If(name)), None) => open = Some((name, number, enabled(name))),
            (Some(Marker::If(name)), Some((_, opened, _))) => {
                return Err(GateError::Nested {
                    line: number,
                    name: name.to_owned(),
                    opened,
                });
            }
            (Some(Marker::End), Some(_)) => open = None,
            (Some(Marker::End), None) => return Err(GateError::StrayEnd { line: number }),
            (None, Some((_, _, false))) => {}
            (None, Some((_, _, true)) | None) => out.push_str(line),
        }
    }
    match open {
        Some((name, opened, _)) => Err(GateError::Unclosed {
            opened,
            name: name.to_owned(),
        }),
        None => Ok(out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str =
        "a\n  // #if rules\n  r1\n  r2\n  // #endif\nb\n// #if events\ne\n// #endif\nc";

    #[test]
    fn a_disabled_region_and_its_markers_disappear() {
        assert_eq!(gate_regions(SOURCE, &|_| false), Ok("a\nb\nc".to_owned()));
    }

    #[test]
    fn an_enabled_region_keeps_its_lines_without_the_markers() {
        let rules_only = gate_regions(SOURCE, &|name| name == "rules");
        assert_eq!(rules_only, Ok("a\n  r1\n  r2\nb\nc".to_owned()));
        let both = gate_regions(SOURCE, &|_| true);
        assert_eq!(both, Ok("a\n  r1\n  r2\nb\ne\nc".to_owned()));
    }

    #[test]
    fn text_without_markers_is_returned_unchanged() {
        let text = "x\r\n// not a marker #if rules\r\n//#if rules\r\ny";
        assert_eq!(gate_regions(text, &|_| false), Ok(text.to_owned()));
    }

    #[test]
    fn crlf_lines_are_kept_as_they_are() {
        let text = "a\r\n// #if rules\r\nr\r\n// #endif\r\nb\r\n";
        assert_eq!(
            gate_regions(text, &|_| true),
            Ok("a\r\nr\r\nb\r\n".to_owned())
        );
        assert_eq!(gate_regions(text, &|_| false), Ok("a\r\nb\r\n".to_owned()));
    }

    #[test]
    fn markers_that_do_not_pair_up_are_errors() {
        assert_eq!(
            gate_regions("// #if a\n// #if b\n// #endif\n", &|_| true),
            Err(GateError::Nested {
                line: 2,
                name: "b".to_owned(),
                opened: 1
            })
        );
        assert_eq!(
            gate_regions("x\n// #endif\n", &|_| true),
            Err(GateError::StrayEnd { line: 2 })
        );
        assert_eq!(
            gate_regions("// #if a\nx\n", &|_| true),
            Err(GateError::Unclosed {
                opened: 1,
                name: "a".to_owned()
            })
        );
    }
}
