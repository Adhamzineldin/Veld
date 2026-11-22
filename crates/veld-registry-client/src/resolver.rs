//! Resolving `[dependencies]` ranges, and the ranges of the packages they pull in, to one
//! pinned version per package.
//!
//! The resolver walks the dependency graph with backtracking. For each package it tries the
//! highest version that satisfies every range collected so far (a version the existing lock
//! already pins comes first, so adding a dependency does not upgrade unrelated packages).
//! When a later requirement rules a choice out, it backs up and tries the next version. There is
//! never more than one version of a package, so `use @org/name:...` is unambiguous. If no
//! assignment satisfies everyone, the error shows the dependency paths of the clashing ranges.
//!
//! Package dependencies come from the `dependencies` field of the registry's versions listing
//! (the server copies it from the package manifest at publish time), so resolution never
//! downloads a package. Cycles between packages are fine as long as the versions are consistent.
//! The result depends only on the inputs: names are visited in sorted order.

use std::collections::BTreeMap;

use semver::{Version, VersionReq};
use veld_project::packages::{LockedPackage, Lockfile, split_name};

use crate::error::{RegistryError, Result};
use crate::http::{Source, VersionInfo};

/// Upper bound on backtracking steps, so a pathological graph fails instead of hanging.
const MAX_STEPS: usize = 200_000;

/// The name used for the project itself in dependency paths.
const ROOT: &str = "veld.toml";

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
        .ok_or_else(|| no_match(name, range, versions))
}

fn no_match(name: &str, range: &str, versions: &[VersionInfo]) -> RegistryError {
    RegistryError::Resolve(format!(
        "no published version of {name} matches `{range}` (available: {})",
        available(versions)
    ))
}

fn available(versions: &[VersionInfo]) -> String {
    let have: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
    if have.is_empty() {
        "none".to_string()
    } else {
        have.join(", ")
    }
}

/// Resolves `dependencies` and everything they depend on against `source`; the lock holds the
/// whole graph for `registry`.
pub fn resolve(
    dependencies: &BTreeMap<String, String>,
    registry: &str,
    source: &dyn Source,
) -> Result<Lockfile> {
    resolve_preferring(dependencies, registry, source, &BTreeMap::new())
}

/// Like [`resolve`], but tries the version in `prefer` (name to version, from an existing lock)
/// first for each package, as long as it satisfies every range.
pub fn resolve_preferring(
    dependencies: &BTreeMap<String, String>,
    registry: &str,
    source: &dyn Source,
    prefer: &BTreeMap<String, String>,
) -> Result<Lockfile> {
    let mut state = State::default();
    for (name, range) in dependencies {
        if split_name(name).is_none() {
            return Err(RegistryError::Resolve(format!(
                "`{name}` is not a valid package name (@org/name)"
            )));
        }
        let req = Req::new(range, vec![ROOT.to_string()])?;
        state.reqs.entry(name.clone()).or_default().push(req);
    }
    let mut solver = Solver {
        source,
        prefer,
        known: BTreeMap::new(),
        first_conflict: None,
        steps: 0,
    };
    let Some(done) = solver.solve(state)? else {
        return Err(RegistryError::Resolve(
            solver
                .first_conflict
                .unwrap_or_else(|| "the dependencies cannot be resolved".to_string()),
        ));
    };
    let packages = done
        .chosen
        .into_iter()
        .map(|(name, c)| LockedPackage {
            name,
            version: c.info.version,
            sha256: c.info.sha256,
            registry: registry.to_string(),
            dependencies: c.info.dependencies.into_keys().collect(),
        })
        .collect();
    Ok(Lockfile { packages })
}

/// One requirement on a package, with the chain of packages that led to it.
#[derive(Clone)]
struct Req {
    req: VersionReq,
    text: String,
    /// `veld.toml`, then each `name@version` down to the requirer.
    path: Vec<String>,
}

impl Req {
    fn new(range: &str, path: Vec<String>) -> Result<Self> {
        Ok(Self {
            req: parse_range(range)?,
            text: range.trim().to_string(),
            path,
        })
    }
}

#[derive(Clone)]
struct Chosen {
    version: Version,
    info: VersionInfo,
}

#[derive(Clone, Default)]
struct State {
    reqs: BTreeMap<String, Vec<Req>>,
    chosen: BTreeMap<String, Chosen>,
}

/// The usable versions of one package, highest first.
struct Known {
    candidates: Vec<(Version, VersionInfo)>,
    listed: Vec<VersionInfo>,
}

struct Solver<'a> {
    source: &'a dyn Source,
    prefer: &'a BTreeMap<String, String>,
    known: BTreeMap<String, Known>,
    first_conflict: Option<String>,
    steps: usize,
}

impl Solver<'_> {
    fn note(&mut self, message: String) {
        self.first_conflict.get_or_insert(message);
    }

    fn load(&mut self, name: &str, via: &[String]) -> Result<()> {
        if self.known.contains_key(name) {
            return Ok(());
        }
        let (org, pkg) = split_name(name).ok_or_else(|| {
            RegistryError::Resolve(format!("`{name}` is not a valid package name"))
        })?;
        let listed = self.source.versions(org, pkg).map_err(|e| {
            if matches!(e, RegistryError::Http { status: 404, .. }) && via.len() > 1 {
                RegistryError::Resolve(format!(
                    "{name} is not in the registry (required by {})",
                    via.join(" → ")
                ))
            } else {
                e
            }
        })?;
        let mut candidates: Vec<(Version, VersionInfo)> = listed
            .iter()
            .filter(|v| v.deprecated.as_deref().is_none_or(str::is_empty))
            .filter_map(|v| Version::parse(&v.version).ok().map(|p| (p, v.clone())))
            .collect();
        candidates.sort_by(|a, b| b.0.cmp(&a.0));
        self.known
            .insert(name.to_string(), Known { candidates, listed });
        Ok(())
    }

    /// Decides the next undecided package; `Ok(None)` is a dead end (the caller backtracks).
    fn solve(&mut self, state: State) -> Result<Option<State>> {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return Err(RegistryError::Resolve(
                "the dependency graph is too complex to resolve; narrow some version ranges".into(),
            ));
        }
        let Some(name) = state
            .reqs
            .keys()
            .find(|n| !state.chosen.contains_key(*n))
            .cloned()
        else {
            return Ok(Some(state));
        };
        let reqs = state.reqs[&name].clone();
        self.load(&name, &reqs[0].path)?;
        let fits = self.fitting(&name, &reqs);
        if fits.is_empty() {
            let msg = conflict_message(&name, &reqs, &self.known[&name].listed);
            self.note(msg);
            return Ok(None);
        }
        for (version, info) in fits {
            let mut next = state.clone();
            if !self.apply(&mut next, &name, version, info) {
                continue;
            }
            if let Some(done) = self.solve(next)? {
                return Ok(Some(done));
            }
        }
        Ok(None)
    }

    /// The versions of `name` that satisfy every range, in the order to try them: the locked
    /// version first, then highest to lowest.
    fn fitting(&self, name: &str, reqs: &[Req]) -> Vec<(Version, VersionInfo)> {
        let mut fits: Vec<(Version, VersionInfo)> = self.known[name]
            .candidates
            .iter()
            .filter(|(v, _)| reqs.iter().all(|r| r.req.matches(v)))
            .cloned()
            .collect();
        let preferred = self
            .prefer
            .get(name)
            .and_then(|pv| fits.iter().position(|(v, _)| v.to_string() == *pv));
        if let Some(i) = preferred {
            let kept = fits.remove(i);
            fits.insert(0, kept);
        }
        fits
    }

    /// Selects `version` of `name` and adds its dependencies' requirements. `false` when one of
    /// them contradicts an earlier choice.
    fn apply(
        &mut self,
        state: &mut State,
        name: &str,
        version: Version,
        info: VersionInfo,
    ) -> bool {
        let mut path = state.reqs[name][0].path.clone();
        path.push(format!("{name}@{version}"));
        let deps = info.dependencies.clone();
        state
            .chosen
            .insert(name.to_string(), Chosen { version, info });
        for (dep, range) in deps {
            let req = match Req::new(&range, path.clone()) {
                Ok(r) if split_name(&dep).is_some() => r,
                _ => {
                    let who = path.last().cloned().unwrap_or_default();
                    self.note(format!(
                        "{who} declares an invalid dependency `{dep}` = `{range}`"
                    ));
                    return false;
                }
            };
            let fits = state
                .chosen
                .get(&dep)
                .is_none_or(|c| req.req.matches(&c.version));
            let list = state.reqs.entry(dep.clone()).or_default();
            list.push(req);
            if !fits {
                let listed = self.known.get(&dep).map(|k| k.listed.clone());
                let msg = conflict_message(&dep, list, &listed.unwrap_or_default());
                self.note(msg);
                return false;
            }
        }
        true
    }
}

/// The error for a package whose requirements cannot all hold, naming each path that leads to it.
fn conflict_message(name: &str, reqs: &[Req], listed: &[VersionInfo]) -> String {
    if let [only] = reqs {
        return no_match(name, &only.text, listed).to_string();
    }
    let mut out = format!("cannot resolve {name}: no single version satisfies every requirement");
    for r in reqs {
        let cycle = r.path.iter().any(|p| p.starts_with(&format!("{name}@")));
        out.push_str(&format!(
            "\n  {} → {name}@{}{}",
            r.path.join(" → "),
            r.text,
            if cycle { "  (dependency cycle)" } else { "" }
        ));
    }
    out.push_str(&format!(
        "\n  available versions: {}\nVeld installs one version of each package; change a range, or publish a compatible release.",
        available(listed)
    ));
    out
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

    fn vd(version: &str, deprecated: bool, deps: &[(&str, &str)]) -> VersionInfo {
        VersionInfo {
            version: version.into(),
            sha256: format!("sum-{version}"),
            deprecated: deprecated.then(|| "old".to_string()),
            dependencies: deps
                .iter()
                .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
                .collect(),
        }
    }

    fn v(version: &str, deprecated: bool) -> VersionInfo {
        vd(version, deprecated, &[])
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

    fn graph(entries: &[(&str, Vec<VersionInfo>)]) -> Fake {
        Fake(
            entries
                .iter()
                .map(|(n, v)| ((*n).to_string(), v.clone()))
                .collect(),
        )
    }

    fn roots(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
            .collect()
    }

    fn versions_of(lock: &Lockfile) -> Vec<(String, String)> {
        lock.packages
            .iter()
            .map(|p| (p.name.clone(), p.version.clone()))
            .collect()
    }

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
            .collect()
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
    fn prerelease_ranges_follow_semver() {
        let src = graph(&[(
            "@a/p",
            vec![
                v("1.0.0", false),
                v("1.1.0-rc.1", false),
                v("1.1.0-rc.2", false),
            ],
        )]);
        // `^1` never selects a prerelease; naming a prerelease of the same release does.
        let stable = resolve(&roots(&[("@a/p", "^1")]), "r", &src).unwrap();
        assert_eq!(stable.packages[0].version, "1.0.0");
        let pre = resolve(&roots(&[("@a/p", ">=1.1.0-rc.1")]), "r", &src).unwrap();
        assert_eq!(pre.packages[0].version, "1.1.0-rc.2");
        let exact = resolve(&roots(&[("@a/p", "=1.1.0-rc.1")]), "r", &src).unwrap();
        assert_eq!(exact.packages[0].version, "1.1.0-rc.1");
        assert!(resolve(&roots(&[("@a/p", "^1.2.0-rc.1")]), "r", &src).is_err());
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

    #[test]
    fn resolves_transitive_dependencies_and_records_them() {
        let src = graph(&[
            (
                "@acme/billing",
                vec![vd("1.0.0", false, &[("@acme/money", "^1")])],
            ),
            (
                "@acme/money",
                vec![v("1.0.0", false), v("1.4.0", false), v("2.0.0", false)],
            ),
        ]);
        let lock = resolve(&roots(&[("@acme/billing", "^1")]), "r", &src).unwrap();
        assert_eq!(
            versions_of(&lock),
            pairs(&[("@acme/billing", "1.0.0"), ("@acme/money", "1.4.0")])
        );
        assert_eq!(
            lock.get("@acme/billing").unwrap().dependencies,
            ["@acme/money"]
        );
        assert!(lock.get("@acme/money").unwrap().dependencies.is_empty());
    }

    #[test]
    fn a_diamond_picks_the_highest_version_everyone_accepts() {
        // app -> a, b; a -> shared ^1.2; b -> shared <1.6; shared has 1.0 .. 2.0.
        let src = graph(&[
            ("@x/a", vec![vd("1.0.0", false, &[("@x/shared", "^1.2")])]),
            ("@x/b", vec![vd("1.0.0", false, &[("@x/shared", "<1.6.0")])]),
            (
                "@x/shared",
                vec![
                    v("1.0.0", false),
                    v("1.2.0", false),
                    v("1.5.3", false),
                    v("1.7.0", false),
                    v("2.0.0", false),
                ],
            ),
        ]);
        let lock = resolve(&roots(&[("@x/a", "*"), ("@x/b", "*")]), "r", &src).unwrap();
        assert_eq!(lock.get("@x/shared").unwrap().version, "1.5.3");
    }

    #[test]
    fn backtracks_to_an_older_version_of_a_dependent() {
        // a 2.0 needs shared ^2, but the project pins shared ^1: a 1.0 must be chosen.
        let src = graph(&[
            (
                "@x/a",
                vec![
                    vd("1.0.0", false, &[("@x/shared", "^1")]),
                    vd("2.0.0", false, &[("@x/shared", "^2")]),
                ],
            ),
            ("@x/shared", vec![v("1.3.0", false), v("2.0.0", false)]),
        ]);
        let lock = resolve(&roots(&[("@x/a", "*"), ("@x/shared", "^1")]), "r", &src).unwrap();
        assert_eq!(
            versions_of(&lock),
            pairs(&[("@x/a", "1.0.0"), ("@x/shared", "1.3.0")])
        );
    }

    #[test]
    fn a_conflict_shows_the_dependency_paths() {
        let src = graph(&[
            (
                "@acme/billing",
                vec![vd("1.0.0", false, &[("@acme/money", "^1")])],
            ),
            (
                "@acme/payroll",
                vec![vd("1.0.0", false, &[("@acme/money", "^2")])],
            ),
            ("@acme/money", vec![v("1.0.0", false), v("2.0.0", false)]),
        ]);
        let err = resolve(
            &roots(&[("@acme/billing", "^1"), ("@acme/payroll", "^1")]),
            "r",
            &src,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("cannot resolve @acme/money"), "{err}");
        assert!(
            err.contains("veld.toml → @acme/billing@1.0.0 → @acme/money@^1"),
            "{err}"
        );
        assert!(
            err.contains("veld.toml → @acme/payroll@1.0.0 → @acme/money@^2"),
            "{err}"
        );
        assert!(err.contains("available versions: 1.0.0, 2.0.0"), "{err}");
    }

    #[test]
    fn a_direct_range_can_conflict_with_a_transitive_one() {
        let src = graph(&[
            (
                "@acme/billing",
                vec![vd("1.0.0", false, &[("@acme/money", "^1")])],
            ),
            ("@acme/money", vec![v("1.0.0", false), v("2.0.0", false)]),
        ]);
        let err = resolve(
            &roots(&[("@acme/billing", "*"), ("@acme/money", "^2")]),
            "r",
            &src,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("veld.toml → @acme/money@^2"), "{err}");
        assert!(
            err.contains("@acme/billing@1.0.0 → @acme/money@^1"),
            "{err}"
        );
    }

    #[test]
    fn a_missing_transitive_package_names_who_needs_it() {
        let src = graph(&[("@a/x", vec![vd("1.0.0", false, &[("@a/gone", "*")])])]);
        let err = resolve(&roots(&[("@a/x", "*")]), "r", &src)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("@a/gone") && err.contains("@a/x@1.0.0"),
            "{err}"
        );
    }

    #[test]
    fn consistent_cycles_resolve() {
        let src = graph(&[
            ("@a/x", vec![vd("1.0.0", false, &[("@a/y", "^1")])]),
            ("@a/y", vec![vd("1.2.0", false, &[("@a/x", "^1")])]),
        ]);
        let lock = resolve(&roots(&[("@a/x", "*")]), "r", &src).unwrap();
        assert_eq!(
            versions_of(&lock),
            pairs(&[("@a/x", "1.0.0"), ("@a/y", "1.2.0")])
        );
        assert_eq!(lock.get("@a/x").unwrap().dependencies, ["@a/y"]);
        assert_eq!(lock.get("@a/y").unwrap().dependencies, ["@a/x"]);
        // The lock of a cycle parses back.
        Lockfile::parse(&lock.to_toml()).unwrap();
    }

    #[test]
    fn inconsistent_cycles_fail_and_say_so() {
        // y requires x ^2, but x 2.0 requires y ^2 which does not exist; x 1.0 requires y.
        let src = graph(&[
            ("@a/x", vec![vd("1.0.0", false, &[("@a/y", "^1")])]),
            ("@a/y", vec![vd("1.0.0", false, &[("@a/x", "^2")])]),
        ]);
        let err = resolve(&roots(&[("@a/x", "*")]), "r", &src)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("@a/x") && err.contains("dependency cycle"),
            "{err}"
        );
    }

    #[test]
    fn a_cycle_back_to_a_different_version_than_chosen_fails() {
        let src = graph(&[
            (
                "@a/x",
                vec![
                    vd("1.0.0", false, &[("@a/y", "^1")]),
                    vd("2.0.0", false, &[("@a/y", "^1")]),
                ],
            ),
            ("@a/y", vec![vd("1.0.0", false, &[("@a/x", "=1.0.0")])]),
        ]);
        // x ^2 -> y -> x =1.0.0 contradicts itself.
        assert!(resolve(&roots(&[("@a/x", "^2")]), "r", &src).is_err());
        // x * backs up from 2.0.0 to 1.0.0, which is consistent.
        let ok = resolve(&roots(&[("@a/x", "*")]), "r", &src).unwrap();
        assert_eq!(ok.get("@a/x").unwrap().version, "1.0.0");
    }

    #[test]
    fn the_same_inputs_always_give_the_same_lock() {
        let src = graph(&[
            (
                "@x/a",
                vec![vd("1.0.0", false, &[("@x/c", "^1"), ("@x/b", "*")])],
            ),
            ("@x/b", vec![vd("1.0.0", false, &[("@x/c", ">=1.1")])]),
            (
                "@x/c",
                vec![v("1.0.0", false), v("1.1.0", false), v("1.9.0", false)],
            ),
            ("@x/d", vec![v("3.0.0", false)]),
        ]);
        let first = resolve(&roots(&[("@x/d", "*"), ("@x/a", "*")]), "r", &src)
            .unwrap()
            .to_toml();
        for _ in 0..5 {
            let again = resolve(&roots(&[("@x/a", "*"), ("@x/d", "*")]), "r", &src)
                .unwrap()
                .to_toml();
            assert_eq!(first, again);
        }
        assert!(first.contains("version = \"1.9.0\""));
    }

    #[test]
    fn a_locked_version_is_kept_when_it_still_fits() {
        let src = graph(&[("@a/x", vec![v("1.0.0", false), v("1.5.0", false)])]);
        let prefer = BTreeMap::from([("@a/x".to_string(), "1.0.0".to_string())]);
        let lock = resolve_preferring(&roots(&[("@a/x", "^1")]), "r", &src, &prefer).unwrap();
        assert_eq!(lock.packages[0].version, "1.0.0");
        let stale = BTreeMap::from([("@a/x".to_string(), "0.9.0".to_string())]);
        let lock = resolve_preferring(&roots(&[("@a/x", "^1")]), "r", &src, &stale).unwrap();
        assert_eq!(lock.packages[0].version, "1.5.0");
    }

    #[test]
    fn an_invalid_dependency_in_a_listing_is_not_silently_ignored() {
        let src = graph(&[("@a/x", vec![vd("1.0.0", false, &[("nonsense", "^1")])])]);
        let err = resolve(&roots(&[("@a/x", "*")]), "r", &src)
            .unwrap_err()
            .to_string();
        assert!(err.contains("invalid dependency"), "{err}");
    }
}
