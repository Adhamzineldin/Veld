//! Which language an answer is written in, and the text of one message in it
//! (`spec/platform.md` section 22).
//!
//! A server with message catalogs reads `Accept-Language` once per request ([`choose_language`]),
//! and writes each message of the answer through [`localize`]: the catalog's text when the chosen
//! catalog has the message key, the contract's text otherwise. `Content-Language` is set when some
//! message of the answer came from a catalog ([`content_language`]). The generators must give the
//! same results; `tests/wire/messages` holds the goldens.

use veld_ir::{Ir, PackageId};

use crate::check::Violation;
use crate::policy::PolicyVerdict;

/// The language tags of an `Accept-Language` value, lower case, best first.
///
/// An entry is dropped when it is `*` (a wildcard matches no catalog), when its quality is `0`
/// ("not acceptable") or is not a number from 0 to 1, or when its tag is not a language range
/// (one to eight letters, then subtags of one to eight letters or digits). The rest keep the
/// header's order among entries of the same quality; a higher quality comes first.
#[must_use]
pub fn accepted_languages(header: &str) -> Vec<String> {
    let mut entries: Vec<(f64, String)> = Vec::new();
    for item in header.split(',') {
        let mut parts = item.split(';');
        let tag = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
        let Some(quality) = quality(parts) else {
            continue;
        };
        if quality > 0.0 && is_range(&tag) {
            entries.push((quality, tag));
        }
    }
    entries.sort_by(|a, b| b.0.total_cmp(&a.0));
    entries.into_iter().map(|(_, tag)| tag).collect()
}

/// The `q` of the parameters of one entry: 1 when there is none, `None` when it is not a number
/// from 0 to 1.
fn quality<'a>(params: impl Iterator<Item = &'a str>) -> Option<f64> {
    for p in params {
        let p = p.trim();
        let Some((name, value)) = p.split_once('=') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("q") {
            let q: f64 = value.trim().parse().ok()?;
            return (0.0..=1.0).contains(&q).then_some(q);
        }
    }
    Some(1.0)
}

fn is_range(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let primary = parts
        .next()
        .is_some_and(|p| (1..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_lowercase()));
    primary
        && parts.all(|s| {
            (1..=8).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// The first language of `header`, in order of preference, for which `available` has a catalog.
/// A tag matches a catalog of the same tag, and then one of its primary language (`pt-br` matches
/// `pt-br`, then `pt`); `pt` does **not** match a catalog `pt-br`. `None` for no header, a header
/// with nothing usable, and a header that matches nothing: the contract's text is used.
#[must_use]
pub fn choose_language(header: Option<&str>, available: &[&str]) -> Option<String> {
    let wanted = accepted_languages(header?);
    wanted.iter().find_map(|tag| {
        let primary = tag.split('-').next().unwrap_or(tag);
        [tag.as_str(), primary]
            .into_iter()
            .find(|t| available.contains(t))
            .map(str::to_owned)
    })
}

/// One message as the answer carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Localized {
    /// The text: the catalog's, or the contract's.
    pub text: String,
    /// The `messageKey` of the violation.
    pub message_key: String,
    /// The language of the catalog the text came from; `None` when the contract's text is used.
    /// This is what `Content-Language` says.
    pub language: Option<String>,
}

impl Localized {
    /// True when the text came from a catalog.
    #[must_use]
    pub fn catalog_used(&self) -> bool {
        self.language.is_some()
    }
}

/// The text of the message `key` of the member `package` for a request with `accept_language`.
/// `default` is the contract's text, used when no catalog is chosen or the chosen one lacks `key`.
#[must_use]
pub fn localize(
    ir: &Ir,
    package: PackageId,
    accept_language: Option<&str>,
    key: &str,
    default: &str,
) -> Localized {
    let catalogs = &ir.package(package).messages;
    let available: Vec<&str> = catalogs.keys().map(String::as_str).collect();
    let chosen = choose_language(accept_language, &available);
    let found = chosen
        .as_deref()
        .and_then(|tag| catalogs.get(tag).map(|c| (tag, c)))
        .and_then(|(tag, c)| c.get(key).map(|text| (tag.to_owned(), text.clone())));
    match found {
        Some((tag, text)) => Localized {
            text,
            message_key: key.to_owned(),
            language: Some(tag),
        },
        None => Localized {
            text: default.to_owned(),
            message_key: key.to_owned(),
            language: None,
        },
    }
}

/// [`localize`] for a violation of a rule. An evaluation error is not translated: its text is
/// not the author's, but it has its `messageKey`. `None` when the rule id has no key.
#[must_use]
pub fn localize_violation(
    ir: &Ir,
    package: PackageId,
    accept_language: Option<&str>,
    violation: &Violation,
) -> Option<Localized> {
    let key = ir.violation_key(&violation.rule_id, None)?;
    if violation.error.is_some() {
        return Some(Localized {
            text: violation.message.clone(),
            message_key: key,
            language: None,
        });
    }
    Some(localize(
        ir,
        package,
        accept_language,
        &key,
        &violation.message,
    ))
}

/// [`localize`] for the denial of a policy: the key is that of the `forbid` that was true, or
/// `veld/not-permitted` when no `forbid` with a message explains it.
#[must_use]
pub fn localize_denial(
    ir: &Ir,
    package: PackageId,
    accept_language: Option<&str>,
    policy_id: &str,
    verdict: &PolicyVerdict,
) -> Option<Localized> {
    let key = ir.violation_key(policy_id, verdict.forbid)?;
    Some(localize(
        ir,
        package,
        accept_language,
        &key,
        verdict.denial_message(),
    ))
}

/// The `Content-Language` of an answer: the language of its messages that came from a catalog,
/// `None` when none did. Every message of one answer is written in the same language, so the first
/// one decides.
#[must_use]
pub fn content_language(messages: &[Localized]) -> Option<&str> {
    messages.iter().find_map(|m| m.language.as_deref())
}
