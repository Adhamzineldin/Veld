//! The build profile of one target: language version, framework and framework version.
//!
//! A [`Profile`] is resolved **once**, here, from the target's options and its `framework`.
//! Generators ask it feature questions (`profile.java_at_least(21)`) and never compare raw
//! version strings. Everything that can be wrong with the combination is reported centrally:
//! an unknown language version is `E0802`; an unknown framework, a framework version outside the
//! adapter's list, or a language below the adapter's `min_language` is `E0803`.

use std::fmt;
use std::str::FromStr;

use serde_json::Value;

use crate::fileset::FileSet;
use crate::generator::{GenDiagnostic, Role};

/// The option key of the framework version (`[server.options] framework-version = "3"`).
pub const FRAMEWORK_VERSION_KEY: &str = "framework-version";

/// A language version as `major.minor`: `17`, `1.9`, or an ECMAScript edition (`es2022`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LanguageVersion {
    major: u32,
    minor: u32,
}

/// A language version that does not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionParseError(String);

impl fmt::Display for VersionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is not a version like `17`, `1.9` or `es2022`",
            self.0
        )
    }
}

impl std::error::Error for VersionParseError {}

impl LanguageVersion {
    #[must_use]
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// An ECMAScript edition: `es(2022)` is `es2022`.
    #[must_use]
    pub const fn es(year: u32) -> Self {
        Self::new(year, 0)
    }

    #[must_use]
    pub const fn major(self) -> u32 {
        self.major
    }

    #[must_use]
    pub const fn minor(self) -> u32 {
        self.minor
    }
}

impl FromStr for LanguageVersion {
    type Err = VersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || VersionParseError(s.to_string());
        let digits = s.strip_prefix("es").unwrap_or(s);
        let (major, minor) = match digits.split_once('.') {
            Some((a, b)) => (a, Some(b)),
            None => (digits, None),
        };
        let number = |t: &str| {
            if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            t.parse::<u32>().map_err(|_| bad())
        };
        Ok(Self::new(number(major)?, minor.map_or(Ok(0), number)?))
    }
}

impl fmt::Display for LanguageVersion {
    /// ECMAScript editions (a major of 1000 or more) print as `es2022`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.major >= 1000 {
            write!(f, "es{}", self.major)
        } else if self.minor == 0 {
            write!(f, "{}", self.major)
        } else {
            write!(f, "{}.{}", self.major, self.minor)
        }
    }
}

/// The versions of a language a generator supports. The first one is the default (the oldest).
/// The option that selects it is named after the language `id` (`java = "21"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageInfo {
    pub id: String,
    pub versions: Vec<String>,
}

/// One framework a generator supports for a role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameworkInfo {
    pub id: String,
    /// Supported framework versions, the first is the default; empty when it is unversioned.
    pub versions: Vec<String>,
    /// The oldest language version the adapter's output needs.
    pub min_language: Option<LanguageVersion>,
}

/// Emits the files of one server framework from the emitter's own server model `M`.
///
/// Each emitter keeps ONE registry slice of adapters. The framework lists reported to the core
/// (`Generator::frameworks`) and the central `E0803` check are derived from that slice.
pub trait FrameworkAdapter<M> {
    fn id(&self) -> &'static str;
    /// Supported framework versions (`["3"]` for Spring Boot); empty when unversioned.
    fn framework_versions(&self) -> &'static [&'static str];
    fn min_language(&self) -> LanguageVersion;
    /// # Errors
    /// Generation diagnostics, for example `E0805` for a bad output path.
    fn files(&self, model: &M, profile: &Profile) -> Result<FileSet, Vec<GenDiagnostic>>;
}

/// The core's view of one adapter.
#[must_use]
pub fn framework_info<M>(adapter: &dyn FrameworkAdapter<M>) -> FrameworkInfo {
    FrameworkInfo {
        id: adapter.id().to_string(),
        versions: adapter
            .framework_versions()
            .iter()
            .map(|v| (*v).to_string())
            .collect(),
        min_language: Some(adapter.min_language()),
    }
}

/// The core's view of an emitter's whole registry slice, in registry order.
#[must_use]
pub fn framework_infos<M>(adapters: &[&dyn FrameworkAdapter<M>]) -> Vec<FrameworkInfo> {
    adapters.iter().map(|a| framework_info(*a)).collect()
}

/// The resolved language and framework of one target.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Profile {
    language: Option<(String, LanguageVersion)>,
    framework: Option<String>,
    framework_version: Option<String>,
}

/// What a generator offers, as the profile resolution needs it.
#[derive(Debug, Clone, Copy)]
pub struct Offer<'a> {
    pub generator: &'a str,
    pub role: Role,
    pub language: Option<&'a LanguageInfo>,
    pub frameworks: &'a [FrameworkInfo],
}

fn e0802(offer: &Offer<'_>, what: String) -> GenDiagnostic {
    GenDiagnostic::new(
        "E0802",
        format!(
            "invalid options for generator `{}`: {what}",
            offer.generator
        ),
    )
}

fn e0803(offer: &Offer<'_>, what: String) -> GenDiagnostic {
    GenDiagnostic::new(
        "E0803",
        format!(
            "{what} (generator `{}`, role `{}`)",
            offer.generator,
            offer.role.name()
        ),
    )
}

fn option_text<'v>(
    offer: &Offer<'_>,
    options: &'v Value,
    key: &str,
) -> Result<Option<&'v str>, GenDiagnostic> {
    match options.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => {
            Err(e0802(offer, format!("`{key}` must be a string")))
        }
    }
}

impl Profile {
    /// A profile with no language and no framework, for generators and tests that need none.
    #[must_use]
    pub fn unversioned() -> Self {
        Self::default()
    }

    /// Resolves the profile from the target's `framework` and options.
    ///
    /// The language option is named after the language id (`java = "21"`) and `framework-version`
    /// holds the framework version. Both are optional: the language defaults to the oldest
    /// supported version, the framework version to the adapter's first.
    ///
    /// # Errors
    /// `E0802` for an unknown language version, `E0803` for an unknown framework or framework
    /// version, or a language below the adapter's `min_language`. The message lists valid values.
    pub fn resolve(
        offer: &Offer<'_>,
        framework: Option<&str>,
        options: &Value,
    ) -> Result<Self, GenDiagnostic> {
        let language = resolve_language(offer, options)?;
        let adapter =
            match framework {
                None => None,
                Some(id) => Some(offer.frameworks.iter().find(|f| f.id == id).ok_or_else(
                    || {
                        e0803(
                            offer,
                            format!("unknown framework `{id}`{}", expected(offer)),
                        )
                    },
                )?),
            };
        let framework_version = resolve_framework_version(offer, adapter, options)?;
        if let (Some(a), Some((id, version))) = (adapter, &language)
            && let Some(min) = a.min_language
            && *version < min
        {
            return Err(e0803(
                offer,
                format!(
                    "framework `{}` needs {id} {min} or newer, but the target uses {id} {version}",
                    a.id
                ),
            ));
        }
        Ok(Self {
            language,
            framework: framework.map(str::to_string),
            framework_version,
        })
    }

    /// The language id (`java`), when the generator has a language.
    #[must_use]
    pub fn language_id(&self) -> Option<&str> {
        self.language.as_ref().map(|(id, _)| id.as_str())
    }

    #[must_use]
    pub fn language_version(&self) -> Option<LanguageVersion> {
        self.language.as_ref().map(|(_, v)| *v)
    }

    #[must_use]
    pub fn framework(&self) -> Option<&str> {
        self.framework.as_deref()
    }

    #[must_use]
    pub fn framework_version(&self) -> Option<&str> {
        self.framework_version.as_deref()
    }

    /// Whether the target's language is `id` at `version` or newer.
    #[must_use]
    pub fn language_at_least(&self, id: &str, version: LanguageVersion) -> bool {
        self.language
            .as_ref()
            .is_some_and(|(l, v)| l == id && *v >= version)
    }

    /// The target language is Java `major` or newer.
    #[must_use]
    pub fn java_at_least(&self, major: u32) -> bool {
        self.language_at_least("java", LanguageVersion::new(major, 0))
    }

    /// The target language is TypeScript compiled for the ECMAScript edition `year` or newer.
    #[must_use]
    pub fn ecmascript_at_least(&self, year: u32) -> bool {
        self.language_at_least("typescript", LanguageVersion::es(year))
    }

    /// The framework version is `major` or newer (false when unversioned or not numeric).
    #[must_use]
    pub fn framework_at_least(&self, major: u32) -> bool {
        self.framework_version
            .as_deref()
            .and_then(|v| v.parse::<LanguageVersion>().ok())
            .is_some_and(|v| v >= LanguageVersion::new(major, 0))
    }
}

fn expected(offer: &Offer<'_>) -> String {
    if offer.frameworks.is_empty() {
        format!("; the `{}` role takes no framework", offer.role.name())
    } else {
        let ids: Vec<&str> = offer.frameworks.iter().map(|f| f.id.as_str()).collect();
        format!("; expected one of: {}", ids.join(", "))
    }
}

fn resolve_language(
    offer: &Offer<'_>,
    options: &Value,
) -> Result<Option<(String, LanguageVersion)>, GenDiagnostic> {
    let Some(lang) = offer.language else {
        return Ok(None);
    };
    let chosen = match option_text(offer, options, &lang.id)? {
        Some(v) => v.to_string(),
        None => match lang.versions.first() {
            Some(v) => v.clone(),
            None => return Ok(None),
        },
    };
    if !lang.versions.contains(&chosen) {
        return Err(e0802(
            offer,
            format!(
                "`{} = \"{chosen}\"` is not a supported {} version; expected one of: {}",
                lang.id,
                lang.id,
                lang.versions.join(", ")
            ),
        ));
    }
    let version = chosen
        .parse::<LanguageVersion>()
        .map_err(|e| e0802(offer, format!("language version of `{}`: {e}", lang.id)))?;
    Ok(Some((lang.id.clone(), version)))
}

fn resolve_framework_version(
    offer: &Offer<'_>,
    adapter: Option<&FrameworkInfo>,
    options: &Value,
) -> Result<Option<String>, GenDiagnostic> {
    let asked = option_text(offer, options, FRAMEWORK_VERSION_KEY)?;
    let Some(adapter) = adapter else {
        return match asked {
            None => Ok(None),
            Some(v) => Err(e0803(
                offer,
                format!("`{FRAMEWORK_VERSION_KEY} = \"{v}\"` needs a `framework`"),
            )),
        };
    };
    match (asked, adapter.versions.first()) {
        (None, first) => Ok(first.cloned()),
        (Some(v), _) if adapter.versions.iter().any(|x| x == v) => Ok(Some(v.to_string())),
        (Some(v), None) => Err(e0803(
            offer,
            format!(
                "framework `{}` has no versions, so `{FRAMEWORK_VERSION_KEY} = \"{v}\"` is not allowed",
                adapter.id
            ),
        )),
        (Some(v), Some(_)) => Err(e0803(
            offer,
            format!(
                "unknown version `{v}` of framework `{}`; expected one of: {}",
                adapter.id,
                adapter.versions.join(", ")
            ),
        )),
    }
}

/// The options without the keys the core consumed (the language option and `framework-version`),
/// so a generator's typed `Options` never sees them.
#[must_use]
pub fn strip_profile_keys(options: &Value, language: Option<&LanguageInfo>) -> Value {
    let Value::Object(map) = options else {
        return options.clone();
    };
    let mut rest = map.clone();
    rest.remove(FRAMEWORK_VERSION_KEY);
    if let Some(l) = language {
        rest.remove(&l.id);
    }
    Value::Object(rest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn java() -> LanguageInfo {
        LanguageInfo {
            id: "java".into(),
            versions: vec!["17".into(), "21".into()],
        }
    }

    fn frameworks() -> Vec<FrameworkInfo> {
        vec![
            FrameworkInfo {
                id: "jdk".into(),
                versions: vec![],
                min_language: Some(LanguageVersion::new(17, 0)),
            },
            FrameworkInfo {
                id: "spring".into(),
                versions: vec!["3".into()],
                min_language: Some(LanguageVersion::new(21, 0)),
            },
        ]
    }

    fn resolve(framework: Option<&str>, options: &Value) -> Result<Profile, GenDiagnostic> {
        let (lang, fw) = (java(), frameworks());
        let offer = Offer {
            generator: "java",
            role: Role::Server,
            language: Some(&lang),
            frameworks: &fw,
        };
        Profile::resolve(&offer, framework, options)
    }

    #[test]
    fn versions_parse_display_and_order() {
        let v = |s: &str| s.parse::<LanguageVersion>().unwrap();
        assert!(v("17") < v("21") && v("1.9") < v("1.10") && v("es2020") < v("es2022"));
        assert_eq!(v("es2022").to_string(), "es2022");
        assert_eq!(v("1.9").to_string(), "1.9");
        assert_eq!(v("17").to_string(), "17");
        for bad in ["", "x", "1.", ".1", "1.x", "es", "-1"] {
            assert!(bad.parse::<LanguageVersion>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn defaults_are_the_oldest_language_and_first_framework_version() {
        let p = resolve(Some("spring"), &json!({ "java": "21" })).unwrap();
        assert!(p.java_at_least(21) && p.java_at_least(17) && !p.java_at_least(22));
        assert_eq!(p.framework_version(), Some("3"));
        assert!(p.framework_at_least(3) && !p.framework_at_least(4));
        let p = resolve(None, &Value::Null).unwrap();
        assert!(p.java_at_least(17) && !p.java_at_least(21));
        assert_eq!(p.framework(), None);
    }

    #[test]
    fn unknown_language_version_is_e0802_and_lists_the_valid_ones() {
        let err = resolve(None, &json!({ "java": "19" })).unwrap_err();
        assert_eq!(err.code, "E0802");
        assert!(err.message.contains("17, 21"), "{}", err.message);
        assert_eq!(
            resolve(None, &json!({ "java": 17 })).unwrap_err().code,
            "E0802"
        );
    }

    #[test]
    fn framework_version_is_checked_against_the_adapter() {
        let err = resolve(
            Some("spring"),
            &json!({ "framework-version": "2", "java": "21" }),
        )
        .unwrap_err();
        assert_eq!(err.code, "E0803");
        assert!(
            err.message.contains("expected one of: 3"),
            "{}",
            err.message
        );
        let err = resolve(Some("jdk"), &json!({ "framework-version": "1" })).unwrap_err();
        assert_eq!(err.code, "E0803");
        assert!(err.message.contains("no versions"), "{}", err.message);
        let err = resolve(None, &json!({ "framework-version": "3" })).unwrap_err();
        assert_eq!(err.code, "E0803");
        let err = resolve(Some("koa"), &Value::Null).unwrap_err();
        assert!(err.message.contains("jdk, spring"), "{}", err.message);
    }

    #[test]
    fn language_below_the_adapters_minimum_is_e0803() {
        // `spring` needs 21 in this test registry; the default language is 17.
        let err = resolve(Some("spring"), &Value::Null).unwrap_err();
        assert_eq!(err.code, "E0803");
        assert!(err.message.contains("java 21 or newer"), "{}", err.message);
    }

    #[test]
    fn profile_keys_are_stripped_from_the_options() {
        let o = json!({ "java": "21", "framework-version": "3", "package": "a.b" });
        assert_eq!(
            strip_profile_keys(&o, Some(&java())),
            json!({ "package": "a.b" })
        );
        assert_eq!(strip_profile_keys(&Value::Null, None), Value::Null);
    }
}
