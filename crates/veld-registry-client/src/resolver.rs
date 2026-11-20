//! Resolving `[dependencies]` ranges to pinned versions.

use std::collections::BTreeMap;

use semver::{Version, VersionReq};
use veld_project::packages::{LockedPackage, Lockfile, split_name};

use crate::error::{RegistryError, Result};
use crate::http::{Source, VersionInfo};

/// Parses a dependency range. `*`, `latest` and the empty string mean any version.
pub fn parse_range(range: &str) -> Result<VersionReq> {
    let r = range.trim();
    let r = if r.is_empty() || r == "latest" {
        "*"
    } else {
        r
    };
    VersionReq::parse(r)
        .map_err(|e| RegistryError::Resolve(format!("`{range}` is not a valid version range: {e}")))
}

/// The highest non-deprecated version matching `range`.
pub fn pick<'a>(name: &str, range: &str, versions: &'a [VersionInfo]) -> Result<&'a VersionInfo> {
    let req = parse_range(range)?;
    versions
        .iter()
        .filter(|v| v.deprecated.as_deref().is_none_or(str::is_empty))
        .filter_map(|v| Version::parse(&v.version).ok().map(|p| (p, v)))
        .filter(|(p, _)| req.matches(p))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, v)| v)
        .ok_or_else(|| {
            let have: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
            RegistryError::Resolve(format!(
                "no published version of {name} matches `{range}` (available: {})",
                if have.is_empty() {
                    "none".to_string()
                } else {
                    have.join(", ")
                }
            ))
        })
}

/// Resolves every dependency against `source` and returns the lock for `registry`.
pub fn resolve(
    dependencies: &BTreeMap<String, String>,
    registry: &str,
    source: &dyn Source,
) -> Result<Lockfile> {
    let mut packages = Vec::new();
    for (name, range) in dependencies {
        let (org, pkg) = split_name(name).ok_or_else(|| {
            RegistryError::Resolve(format!("`{name}` is not a valid package name (@org/name)"))
        })?;
        let versions = source.versions(org, pkg)?;
        let chosen = pick(name, range, &versions)?;
        packages.push(LockedPackage {
            name: name.clone(),
            version: chosen.version.clone(),
            sha256: chosen.sha256.clone(),
            registry: registry.to_string(),
        });
    }
    Ok(Lockfile { packages })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(BTreeMap<String, Vec<VersionInfo>>);

    impl Source for Fake {
        fn versions(&self, org: &str, name: &str) -> Result<Vec<VersionInfo>> {
            self.0
                .get(&format!("@{org}/{name}"))
                .cloned()
                .ok_or_else(|| RegistryError::Http {
                    status: 404,
                    message: "not found".into(),
                })
        }
        fn download(&self, _: &str, _: &str, _: &str) -> Result<Vec<u8>> {
            Err(RegistryError::Other("unused".into()))
        }
    }

    fn v(version: &str, deprecated: bool) -> VersionInfo {
        VersionInfo {
            version: version.into(),
            sha256: format!("sum-{version}"),
            deprecated: deprecated.then(|| "old".to_string()),
        }
    }

    fn fake() -> Fake {
        Fake(BTreeMap::from([(
            "@acme/billing".to_string(),
            vec![
                v("1.0.0", false),
                v("1.2.0", false),
                v("1.10.0", false),
                v("1.11.0", true),
                v("2.0.0", false),
                v("2.1.0-beta.1", false),
            ],
        )]))
    }

    fn deps(range: &str) -> BTreeMap<String, String> {
        BTreeMap::from([("@acme/billing".to_string(), range.to_string())])
    }

    #[test]
    fn picks_the_highest_matching_version() {
        let lock = resolve(&deps("^1.2"), "https://r", &fake()).unwrap();
        assert_eq!(
            lock.packages[0].version, "1.10.0",
            "semver, not string, order"
        );
        assert_eq!(lock.packages[0].sha256, "sum-1.10.0");
        assert_eq!(lock.packages[0].registry, "https://r");
        assert_eq!(
            resolve(&deps("*"), "r", &fake()).unwrap().packages[0].version,
            "2.0.0"
        );
        assert_eq!(
            resolve(&deps("1.0.0"), "r", &fake()).unwrap().packages[0].version,
            "1.10.0"
        );
        assert_eq!(
            resolve(&deps("=1.0.0"), "r", &fake()).unwrap().packages[0].version,
            "1.0.0"
        );
        assert_eq!(
            resolve(&deps("~1.2"), "r", &fake()).unwrap().packages[0].version,
            "1.2.0"
        );
        assert_eq!(
            resolve(&deps(">=1, <2"), "r", &fake()).unwrap().packages[0].version,
            "1.10.0"
        );
    }

    #[test]
    fn skips_deprecated_and_prerelease_versions() {
        let lock = resolve(&deps("^1"), "r", &fake()).unwrap();
        assert_eq!(lock.packages[0].version, "1.10.0");
        let pre = resolve(&deps("^2.1.0-beta.0"), "r", &fake()).unwrap();
        assert_eq!(pre.packages[0].version, "2.1.0-beta.1");
    }

    #[test]
    fn explains_failures() {
        let err = resolve(&deps("^3"), "r", &fake()).unwrap_err().to_string();
        assert!(
            err.contains("no published version") && err.contains("1.0.0"),
            "{err}"
        );
        assert!(resolve(&deps("garbage!"), "r", &fake()).is_err());
        let unknown = BTreeMap::from([("@acme/nope".to_string(), "*".to_string())]);
        assert!(matches!(
            resolve(&unknown, "r", &fake()),
            Err(RegistryError::Http { status: 404, .. })
        ));
        let bad = BTreeMap::from([("acme".to_string(), "*".to_string())]);
        assert!(resolve(&bad, "r", &fake()).is_err());
    }

    #[test]
    fn resolution_is_deterministic_and_sorted() {
        let mut map = BTreeMap::from([("@acme/billing".to_string(), "^1".to_string())]);
        map.insert("@acme/aaa".to_string(), "*".to_string());
        let mut f = fake();
        f.0.insert("@acme/aaa".into(), vec![v("0.1.0", false)]);
        let a = resolve(&map, "r", &f).unwrap();
        let b = resolve(&map, "r", &f).unwrap();
        assert_eq!(a.to_toml(), b.to_toml());
        assert_eq!(a.packages[0].name, "@acme/aaa");
    }
}
