//! Message catalogs (`spec/platform.md` section 22).
//!
//! A member may have a directory of catalogs, one `<tag>.toml` per language, each a flat table
//! from a message key (`veld_ir::message_keys`) to a text. [`compile_with_catalogs`] compiles the
//! contracts as [`crate::compile_prefixed`] does, reads the catalogs, reports `E1301`, `W1301` and
//! `W1302`, and puts the catalogs in the IR (`Package.messages`). A project without catalogs does
//! not call it differently: with an empty [`Catalogs`] the result is the one of `compile_prefixed`.

use std::collections::{BTreeMap, BTreeSet};

use veld_ir::{Ir, PackageId, PackageKind, message_keys};

use crate::diagnostics::{Diagnostic, FileId, Span, has_errors};
use crate::{AccessRules, CompileInput, CompileOutput, compile_prefixed};

/// How many keys `W1302` lists before it says "and N more".
const LISTED: usize = 3;

/// One catalog file as read from disk: its file name and its text.
#[derive(Debug, Clone)]
pub struct CatalogFile {
    /// `ar.toml`.
    pub name: String,
    pub text: String,
}

/// The catalogs of one member.
#[derive(Debug, Clone)]
pub struct MemberCatalogs {
    /// The directory, relative to the member, with `/` separators (`messages`).
    pub dir: String,
    pub files: Vec<CatalogFile>,
}

/// The catalogs of a workspace, by package name.
pub type Catalogs = BTreeMap<String, MemberCatalogs>;

/// True when `tag` is a language tag in the form a catalog file uses: lower case, a primary
/// language of two or three letters, then any number of subtags of one to eight letters or digits,
/// joined by `-` (`ar`, `fr`, `pt-br`, `zh-hant-tw`).
#[must_use]
pub fn is_language_tag(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let primary_ok = parts
        .next()
        .is_some_and(|p| (2..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_lowercase()));
    primary_ok
        && parts.all(|s| {
            (1..=8).contains(&s.len())
                && s.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// Why a catalog could not be read: a message and the byte range it is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogError {
    pub message: String,
    pub at: std::ops::Range<usize>,
}

/// Reads a catalog: a flat TOML table whose values are all strings.
///
/// # Errors
/// [`CatalogError`] for text that is not TOML, a value that is not a string (a number, a list or
/// a table, so also a `[section]`).
pub fn parse_catalog(text: &str) -> Result<BTreeMap<String, String>, CatalogError> {
    let table: toml::Table = toml::from_str(text).map_err(|e| CatalogError {
        message: e.message().to_owned(),
        at: e.span().unwrap_or(0..0),
    })?;
    let mut out = BTreeMap::new();
    for (key, value) in table {
        match value {
            toml::Value::String(s) => {
                out.insert(key, s);
            }
            other => {
                let at = key_range(text, &key);
                return Err(CatalogError {
                    message: format!(
                        "`{key}` is {}, not a string: a catalog is a flat table of strings",
                        article_type(&other)
                    ),
                    at,
                });
            }
        }
    }
    Ok(out)
}

fn article_type(v: &toml::Value) -> &'static str {
    match v {
        toml::Value::String(_) => "a string",
        toml::Value::Integer(_) => "an integer",
        toml::Value::Float(_) => "a float",
        toml::Value::Boolean(_) => "a boolean",
        toml::Value::Datetime(_) => "a date or time",
        toml::Value::Array(_) => "a list",
        toml::Value::Table(_) => "a table",
    }
}

/// Where `key` is written in `text`: the quoted form first, then the bare one.
fn key_range(text: &str, key: &str) -> std::ops::Range<usize> {
    for pattern in [format!("\"{key}\""), format!("'{key}'"), key.to_owned()] {
        if let Some(at) = text.find(&pattern) {
            return at..at + pattern.len();
        }
    }
    0..0
}

/// Compiles like [`compile_prefixed`], then reads `catalogs`.
///
/// A catalog of a package that is not a service of `input` is ignored. A catalog that cannot be
/// read is `E1301` and the compilation has no IR. The IR keeps the catalogs that could be read.
#[must_use]
pub fn compile_with_catalogs(
    input: &CompileInput,
    rules: Option<&AccessRules>,
    prefixes: &BTreeMap<String, String>,
    catalogs: &Catalogs,
) -> CompileOutput {
    let mut out = compile_prefixed(input, rules, prefixes);
    if catalogs.is_empty() {
        return out;
    }
    let mut read: BTreeMap<&str, Vec<Read>> = BTreeMap::new();
    for (package, member) in catalogs {
        let is_service = input
            .packages
            .iter()
            .any(|p| &p.name == package && p.kind == PackageKind::Service);
        if is_service {
            let files = read_member(package, member, &mut out);
            read.insert(package.as_str(), files);
        }
    }
    if let Some(ir) = out.ir.as_mut() {
        attach(ir, catalogs, &read, &mut out.diagnostics);
    }
    out.diagnostics
        .sort_by_key(|d| (d.primary.file, d.primary.start, d.code));
    if has_errors(&out.diagnostics) {
        out.ir = None;
    }
    out
}

/// A catalog file that parsed: where it is, its language and its entries.
struct Read {
    file: FileId,
    text: String,
    tag: String,
    entries: BTreeMap<String, String>,
}

fn read_member(package: &str, member: &MemberCatalogs, out: &mut CompileOutput) -> Vec<Read> {
    let mut files: Vec<&CatalogFile> = member.files.iter().collect();
    files.sort_by(|a, b| a.name.cmp(&b.name));
    let mut read = Vec::new();
    for f in files {
        let Some(tag) = f.name.strip_suffix(".toml") else {
            continue;
        };
        let path = format!("{package}/{}/{}", member.dir, f.name);
        let file = out.sources.add(path, f.text.clone());
        if !is_language_tag(tag) {
            out.diagnostics.push(
                Diagnostic::error(
                    "E1301",
                    format!("`{}` is not the name of a catalog: `{tag}` is not a language tag", f.name),
                    Span::new(file, 0, 0),
                )
                .with_help("name the file after a lower case language tag: `ar.toml`, `pt-br.toml`"),
            );
            continue;
        }
        match parse_catalog(&f.text) {
            Ok(entries) => read.push(Read {
                file,
                text: f.text.clone(),
                tag: tag.to_owned(),
                entries,
            }),
            Err(e) => out.diagnostics.push(Diagnostic::error(
                "E1301",
                e.message,
                Span::new(file, e.at.start, e.at.end),
            )),
        }
    }
    read
}

fn attach(
    ir: &mut Ir,
    catalogs: &Catalogs,
    read: &BTreeMap<&str, Vec<Read>>,
    diags: &mut Vec<Diagnostic>,
) {
    for (package, files) in read {
        let Some(index) = ir.packages.iter().position(|p| p.name == *package) else {
            continue;
        };
        let id = PackageId(u32::try_from(index).unwrap_or(u32::MAX));
        let valid = valid_keys(ir, id);
        for r in files {
            check_catalog(r, &valid, diags);
        }
        let Some(p) = ir.packages.get_mut(index) else {
            continue;
        };
        p.messages = files
            .iter()
            .filter(|r| !r.entries.is_empty())
            .map(|r| (r.tag.clone(), r.entries.clone()))
            .collect();
        p.messages_dir = (!p.messages.is_empty())
            .then(|| catalogs.get(*package).map(|m| m.dir.clone()))
            .flatten();
    }
}

/// The translatable keys of the member, with the contract's text, and the fixed keys.
struct Valid {
    rules: message_keys::CatalogKeys,
    all: BTreeSet<String>,
}

fn valid_keys(ir: &Ir, package: PackageId) -> Valid {
    let rules = ir.catalog_keys(package);
    let mut all: BTreeSet<String> = rules.keys().cloned().collect();
    all.extend(message_keys::FIXED.iter().map(|k| (*k).to_owned()));
    Valid { rules, all }
}

fn check_catalog(r: &Read, valid: &Valid, diags: &mut Vec<Diagnostic>) {
    for key in r.entries.keys().filter(|k| !valid.all.contains(*k)) {
        let at = key_range(&r.text, key);
        diags.push(
            Diagnostic::warning(
                "W1301",
                format!("catalog `{}` has the key `{key}`, which no rule of the member has", r.tag),
                Span::new(r.file, at.start, at.end),
            )
            .did_you_mean(key, valid.all.iter().map(String::as_str), None),
        );
    }
    let missing: Vec<&String> = valid
        .rules
        .iter()
        .filter(|(key, text)| text.is_some() && !r.entries.contains_key(*key))
        .map(|(key, _)| key)
        .collect();
    if missing.is_empty() {
        return;
    }
    let first: Vec<String> = missing
        .iter()
        .take(LISTED)
        .map(|k| format!("`{k}`"))
        .collect();
    let more = missing.len().saturating_sub(LISTED);
    let tail = if more > 0 {
        format!(" and {more} more")
    } else {
        String::new()
    };
    diags.push(Diagnostic::warning(
        "W1302",
        format!(
            "catalog `{}` lacks {} of the member's rule messages: {}{tail}",
            r.tag,
            missing.len(),
            first.join(", ")
        ),
        Span::new(r.file, 0, 0),
    ));
}
