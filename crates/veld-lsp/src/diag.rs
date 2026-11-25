//! Compiler diagnostics as LSP diagnostics, grouped per file.

use std::collections::BTreeMap;

use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, DiagnosticTag, Location,
    NumberOrString, Position, Range, Uri,
};
use veld_compiler::{FileId, Severity};

use crate::uri::path_to_uri;
use crate::workspace::Analysis;

const SOURCE: &str = "veld";

/// Diagnostics for every file of the project, keyed by URI. Files without problems map to an
/// empty list, so that a client clears what was fixed.
#[must_use]
pub fn all(a: &Analysis) -> BTreeMap<Uri, Vec<Diagnostic>> {
    let mut out: BTreeMap<Uri, Vec<Diagnostic>> = BTreeMap::new();
    for meta in &a.files {
        if let Some(uri) = &meta.uri {
            out.entry(uri.clone()).or_default();
        }
    }
    let config_uri = a.config_path.as_deref().and_then(path_to_uri);
    if let Some(uri) = &config_uri {
        let list = out.entry(uri.clone()).or_default();
        if let Some(msg) = &a.config_error {
            list.push(plain(msg, DiagnosticSeverity::ERROR, Some("E0700")));
        }
    }
    for d in &a.index.diagnostics {
        let converted = convert(a, d);
        let target = if d.primary.file == FileId::NONE {
            config_uri.clone()
        } else {
            a.uri(d.primary.file).cloned()
        };
        if let (Some(uri), Some(diag)) = (target, converted) {
            out.entry(uri).or_default().push(diag);
        }
    }
    out
}

fn plain(msg: &str, severity: DiagnosticSeverity, code: Option<&str>) -> Diagnostic {
    Diagnostic {
        range: Range::new(Position::new(0, 0), Position::new(0, 0)),
        severity: Some(severity),
        code: code.map(|c| NumberOrString::String(c.to_string())),
        source: Some(SOURCE.to_string()),
        message: msg.to_string(),
        ..Diagnostic::default()
    }
}

fn convert(a: &Analysis, d: &veld_compiler::Diagnostic) -> Option<Diagnostic> {
    let range = match a.meta(d.primary.file) {
        Some(m) => m
            .lines
            .range(d.primary.start as usize, d.primary.end as usize),
        None if d.primary.file == FileId::NONE => Range::default(),
        None => return None,
    };
    let mut message = d.message.clone();
    for n in &d.notes {
        message.push_str(&format!("\nnote: {n}"));
    }
    if let Some(h) = &d.help {
        message.push_str(&format!("\nhelp: {h}"));
    }
    let related: Vec<DiagnosticRelatedInformation> = d
        .labels
        .iter()
        .filter_map(|(span, text)| {
            let meta = a.meta(span.file)?;
            Some(DiagnosticRelatedInformation {
                location: Location::new(
                    meta.uri.clone()?,
                    meta.lines.range(span.start as usize, span.end as usize),
                ),
                message: text.clone(),
            })
        })
        .collect();
    Some(Diagnostic {
        range,
        severity: Some(match d.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
        }),
        code: Some(NumberOrString::String(d.code.to_string())),
        source: Some(SOURCE.to_string()),
        message,
        related_information: (!related.is_empty()).then_some(related),
        tags: (d.code == "W0201").then(|| vec![DiagnosticTag::UNNECESSARY]),
        ..Diagnostic::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;
    use std::fs;

    #[test]
    fn groups_by_file_with_codes_labels_and_clears() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        fs::write(d.join("veld.toml"), "[service]\nname = \"p\"\n").unwrap();
        fs::create_dir_all(d.join("contracts")).unwrap();
        fs::write(
            d.join("contracts/a.veld"),
            "model A { x: int }\nmodel A { y: int }\n",
        )
        .unwrap();
        fs::write(d.join("contracts/b.veld"), "model B { x: int }\n").unwrap();
        let mut ws = Workspace::new(Some(d.to_path_buf()));
        let a = ws.analyze(None);
        let all = all(&a);
        let (with, without): (Vec<_>, Vec<_>) = all.values().partition(|v| !v.is_empty());
        assert_eq!(with.len(), 1);
        assert!(!without.is_empty());
        let diag = &with[0][0];
        assert_eq!(diag.code, Some(NumberOrString::String("E0201".into())));
        assert_eq!(diag.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diag.related_information.as_ref().unwrap().len(), 1);
        assert_eq!(diag.range.start.line, 1);
        ws.open(&d.join("contracts/a.veld"), "model A { x: int }\n".into());
        let fixed = super::all(&ws.analyze(None));
        assert!(fixed.values().all(Vec::is_empty));
    }

    #[test]
    fn config_errors_land_on_veld_toml() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("veld.toml"), "[project\n").unwrap();
        let a = Workspace::new(Some(tmp.path().to_path_buf())).analyze(None);
        let all = all(&a);
        let toml = all
            .iter()
            .find(|(u, _)| u.as_str().ends_with("veld.toml"))
            .unwrap();
        assert_eq!(toml.1.len(), 1);
    }
}
