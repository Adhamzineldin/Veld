//! Installing exactly what `veld.lock` pins into `.veld/packages`.

use std::path::{Path, PathBuf};

use veld_project::packages::{
    CONTRACTS_DIR, LockedPackage, Lockfile, MANIFEST_FILE, cache_dir, split_name,
};

use crate::error::{RegistryError, Result, io};
use crate::manifest::PackageManifest;
use crate::resolver::parse_range;
use crate::tarball::{Limits, read_archive, sha256_hex, verify_contents, write_files};

/// What `ensure_installed` did for one package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub name: String,
    pub version: String,
    /// `false` when the package was already in the cache and verified.
    pub downloaded: bool,
}

fn tarball_path(dir: &Path) -> PathBuf {
    let mut name = dir
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".tar.gz");
    dir.with_file_name(name)
}

/// Installs one downloaded archive: checks its SHA-256 against the lock, unpacks it into a
/// temporary directory next to the target, verifies the manifest checksums, and renames the
/// directory into place. The archive is kept beside it so the cache can be re-verified.
pub fn install_archive(root: &Path, locked: &LockedPackage, archive: &[u8]) -> Result<()> {
    let actual = sha256_hex(archive);
    if !actual.eq_ignore_ascii_case(&locked.sha256) {
        return Err(RegistryError::Checksum {
            what: format!("{}@{} tarball", locked.name, locked.version),
            expected: locked.sha256.clone(),
            actual,
        });
    }
    let files = read_archive(archive, &Limits::default())?;
    let manifest = verify_contents(&files, &locked.name, &locked.version)?;
    check_dependencies(locked, &manifest)?;

    let dest = cache_dir(root, &locked.name, &locked.version)
        .ok_or_else(|| RegistryError::Tarball(format!("unsafe package `{}`", locked.name)))?;
    let parent = dest
        .parent()
        .ok_or_else(|| RegistryError::Other("bad cache path".into()))?;
    std::fs::create_dir_all(parent).map_err(io(parent))?;
    let tmp = parent.join(format!(".tmp-{}-{}", locked.version, std::process::id()));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).map_err(io(&tmp))?;
    }
    write_files(&files, &tmp)?;
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(io(&dest))?;
    }
    std::fs::rename(&tmp, &dest).map_err(io(&dest))?;
    let tar = tarball_path(&dest);
    std::fs::write(&tar, archive).map_err(io(&tar))
}

/// Re-checks an installed package against the lock: the kept archive's SHA-256 and every
/// extracted file against the manifest. Any difference is a hard error.
pub fn verify_installed(root: &Path, locked: &LockedPackage) -> Result<()> {
    let dir = cache_dir(root, &locked.name, &locked.version)
        .ok_or_else(|| RegistryError::Tarball(format!("unsafe package `{}`", locked.name)))?;
    let tar = tarball_path(&dir);
    match std::fs::read(&tar) {
        Ok(bytes) => {
            let actual = sha256_hex(&bytes);
            if !actual.eq_ignore_ascii_case(&locked.sha256) {
                return Err(RegistryError::Checksum {
                    what: format!("cached tarball of {}@{}", locked.name, locked.version),
                    expected: locked.sha256.clone(),
                    actual,
                });
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(io(&tar)(e)),
    }
    let manifest_path = dir.join(MANIFEST_FILE);
    let mut files = std::collections::BTreeMap::new();
    files.insert(
        MANIFEST_FILE.to_string(),
        std::fs::read(&manifest_path).map_err(io(&manifest_path))?,
    );
    collect_veld(&dir.join(CONTRACTS_DIR), CONTRACTS_DIR, &mut files)?;
    let manifest = verify_contents(&files, &locked.name, &locked.version)?;
    check_dependencies(locked, &manifest)
}

/// The package's own manifest must declare exactly the dependencies the lock records for it,
/// so a lock cannot silently disagree with the contracts it installs.
fn check_dependencies(locked: &LockedPackage, manifest: &PackageManifest) -> Result<()> {
    let declared: Vec<&str> = manifest.dependencies.keys().map(String::as_str).collect();
    let recorded: Vec<&str> = locked.dependencies.iter().map(String::as_str).collect();
    if declared == recorded {
        return Ok(());
    }
    let show = |v: &[&str]| {
        if v.is_empty() {
            "none".to_string()
        } else {
            v.join(", ")
        }
    };
    Err(RegistryError::Manifest(format!(
        "{}@{} declares dependencies ({}) but veld.lock records ({}); run `veld add` to re-resolve",
        locked.name,
        locked.version,
        show(&declared),
        show(&recorded)
    )))
}

/// Checks the installed graph as a whole: every installed package's declared ranges must be
/// satisfied by the version the lock pins for that dependency.
fn verify_graph(root: &Path, lock: &Lockfile) -> Result<()> {
    for p in &lock.packages {
        let dir = cache_dir(root, &p.name, &p.version)
            .ok_or_else(|| RegistryError::Tarball(format!("unsafe package `{}`", p.name)))?;
        let path = dir.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&path).map_err(io(&path))?;
        let manifest = PackageManifest::parse(&text)?;
        for (dep, range) in &manifest.dependencies {
            let pinned = lock.get(dep).ok_or_else(|| {
                RegistryError::Resolve(format!(
                    "{}@{} depends on {dep}, which is not in veld.lock; run `veld add` to re-resolve",
                    p.name, p.version
                ))
            })?;
            let ok = parse_range(range)?;
            if !semver::Version::parse(&pinned.version).is_ok_and(|v| ok.matches(&v)) {
                return Err(RegistryError::Resolve(format!(
                    "{}@{} needs {dep}@{range}, but veld.lock pins {dep}@{}; run `veld add` to re-resolve",
                    p.name, p.version, pinned.version
                )));
            }
        }
    }
    Ok(())
}

/// Removes what `ensure_installed` put in the cache, after a failure.
fn roll_back(root: &Path, fresh: &[&LockedPackage]) {
    for p in fresh {
        if let Some(dir) = cache_dir(root, &p.name, &p.version) {
            let _ = std::fs::remove_dir_all(&dir);
            let _ = std::fs::remove_file(tarball_path(&dir));
        }
    }
}

fn collect_veld(
    dir: &Path,
    prefix: &str,
    out: &mut std::collections::BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir).map_err(io(dir))? {
        let entry = entry.map_err(io(dir))?;
        let path = entry.path();
        let name = format!("{prefix}/{}", entry.file_name().to_string_lossy());
        let kind = entry.file_type().map_err(io(&path))?;
        if kind.is_dir() {
            collect_veld(&path, &name, out)?;
        } else if kind.is_file() {
            out.insert(name, std::fs::read(&path).map_err(io(&path))?);
        } else {
            return Err(RegistryError::Tarball(format!(
                "cached package contains a non-regular entry `{name}`"
            )));
        }
    }
    Ok(())
}

/// Makes the cache match the lock: the whole graph, direct and transitive. Installed packages
/// are verified; missing ones are fetched with `fetch` (the only step that may touch the
/// network) and checked against the lock's SHA-256. On any failure, what this call installed is
/// removed again.
pub fn ensure_installed(
    root: &Path,
    lock: &Lockfile,
    fetch: &mut dyn FnMut(&LockedPackage) -> Result<Vec<u8>>,
) -> Result<Vec<Installed>> {
    let mut fresh: Vec<&LockedPackage> = Vec::new();
    let result = install_all(root, lock, fetch, &mut fresh);
    if result.is_err() {
        roll_back(root, &fresh);
    }
    result
}

fn install_all<'a>(
    root: &Path,
    lock: &'a Lockfile,
    fetch: &mut dyn FnMut(&LockedPackage) -> Result<Vec<u8>>,
    fresh: &mut Vec<&'a LockedPackage>,
) -> Result<Vec<Installed>> {
    let mut done = Vec::new();
    for p in &lock.packages {
        let missing = !is_installed(root, p);
        if split_name(&p.name).is_none() {
            return Err(RegistryError::Resolve(format!(
                "`{}` in veld.lock is not a valid package name",
                p.name
            )));
        }
        if missing {
            fresh.push(p);
            let bytes = fetch(p)?;
            install_archive(root, p, &bytes)?;
        } else {
            verify_installed(root, p)?;
        }
        done.push(Installed {
            name: p.name.clone(),
            version: p.version.clone(),
            downloaded: missing,
        });
    }
    verify_graph(root, lock)?;
    Ok(done)
}

fn is_installed(root: &Path, p: &LockedPackage) -> bool {
    cache_dir(root, &p.name, &p.version)
        .is_some_and(|d| d.join(MANIFEST_FILE).is_file() && d.join(CONTRACTS_DIR).is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::PackageManifest;
    use crate::tarball::pack;

    fn package(version: &str, body: &str) -> (LockedPackage, Vec<u8>) {
        let files = vec![("m.veld".to_string(), body.as_bytes().to_vec())];
        let m = PackageManifest::new("@acme/lib", version, None, &files);
        let packed = pack(&m, &files).unwrap();
        (
            LockedPackage {
                name: "@acme/lib".into(),
                version: version.into(),
                sha256: packed.sha256.clone(),
                registry: "https://r".into(),
                dependencies: Vec::new(),
            },
            packed.bytes,
        )
    }

    fn lock_of(p: &LockedPackage) -> Lockfile {
        Lockfile {
            packages: vec![p.clone()],
        }
    }

    #[test]
    fn installs_then_reuses_the_cache_without_fetching() {
        let tmp = tempfile::tempdir().unwrap();
        let (locked, bytes) = package("1.0.0", "model M {}");
        let mut fetched = 0;
        let mut fetch = |_: &LockedPackage| {
            fetched += 1;
            Ok(bytes.clone())
        };
        let first = ensure_installed(tmp.path(), &lock_of(&locked), &mut fetch).unwrap();
        assert!(first[0].downloaded);
        let second = ensure_installed(tmp.path(), &lock_of(&locked), &mut fetch).unwrap();
        assert!(!second[0].downloaded);
        assert_eq!(fetched, 1);
        let dir = cache_dir(tmp.path(), "@acme/lib", "1.0.0").unwrap();
        assert!(dir.join("contracts/m.veld").is_file());
        assert!(dir.join(MANIFEST_FILE).is_file());
    }

    #[test]
    fn checksum_mismatch_against_the_lock_is_a_hard_error() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut locked, bytes) = package("1.0.0", "model M {}");
        locked.sha256 = "0".repeat(64);
        let err = install_archive(tmp.path(), &locked, &bytes).unwrap_err();
        assert!(matches!(err, RegistryError::Checksum { .. }), "{err}");
        let dir = cache_dir(tmp.path(), "@acme/lib", "1.0.0").unwrap();
        assert!(!dir.exists(), "nothing is installed after a failure");
    }

    #[test]
    fn a_tampered_cached_file_or_tarball_or_lock_fails_loudly() {
        let tmp = tempfile::tempdir().unwrap();
        let (locked, bytes) = package("1.0.0", "model M {}");
        install_archive(tmp.path(), &locked, &bytes).unwrap();
        let dir = cache_dir(tmp.path(), "@acme/lib", "1.0.0").unwrap();

        // The lock pins a different hash than the cached archive.
        let mut wrong_lock = locked.clone();
        wrong_lock.sha256 = "1".repeat(64);
        assert!(verify_installed(tmp.path(), &wrong_lock).is_err());

        // An extracted file is edited.
        std::fs::write(dir.join("contracts/m.veld"), "model Evil {}").unwrap();
        let err = verify_installed(tmp.path(), &locked).unwrap_err();
        assert!(matches!(err, RegistryError::Checksum { .. }), "{err}");
        std::fs::write(dir.join("contracts/m.veld"), "model M {}").unwrap();
        verify_installed(tmp.path(), &locked).unwrap();

        // An unlisted file appears.
        std::fs::write(dir.join("contracts/extra.veld"), "model X {}").unwrap();
        assert!(verify_installed(tmp.path(), &locked).is_err());
        std::fs::remove_file(dir.join("contracts/extra.veld")).unwrap();

        // The kept archive is replaced.
        std::fs::write(tarball_path(&dir), b"junk").unwrap();
        assert!(verify_installed(tmp.path(), &locked).is_err());
    }

    #[test]
    fn a_package_with_the_wrong_identity_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut locked, bytes) = package("1.0.0", "model M {}");
        locked.version = "9.9.9".into();
        locked.sha256 = sha256_hex(&bytes);
        assert!(install_archive(tmp.path(), &locked, &bytes).is_err());
    }

    fn pkg_with(name: &str, version: &str, deps: &[(&str, &str)]) -> (LockedPackage, Vec<u8>) {
        let files = vec![(
            "m.veld".to_string(),
            format!("model {version} {{}}").into_bytes(),
        )];
        let m = PackageManifest::new(name, version, None, &files).with_dependencies(
            deps.iter()
                .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
                .collect(),
        );
        let packed = pack(&m, &files).unwrap();
        (
            LockedPackage {
                name: name.into(),
                version: version.into(),
                sha256: packed.sha256.clone(),
                registry: "https://r".into(),
                dependencies: deps.iter().map(|(a, _)| (*a).to_string()).collect(),
            },
            packed.bytes,
        )
    }

    #[test]
    fn installs_the_whole_graph_and_checks_ranges() {
        let tmp = tempfile::tempdir().unwrap();
        let (billing, b_bytes) = pkg_with("@acme/billing", "1.0.0", &[("@acme/money", "^1")]);
        let (money, m_bytes) = pkg_with("@acme/money", "1.2.0", &[]);
        let lock = Lockfile {
            packages: vec![billing, money],
        };
        let mut fetch = |p: &LockedPackage| {
            Ok(if p.name == "@acme/billing" {
                b_bytes.clone()
            } else {
                m_bytes.clone()
            })
        };
        let done = ensure_installed(tmp.path(), &lock, &mut fetch).unwrap();
        assert_eq!(done.len(), 2);
        assert!(
            cache_dir(tmp.path(), "@acme/money", "1.2.0")
                .unwrap()
                .is_dir()
        );
    }

    #[test]
    fn a_failure_midway_leaves_nothing_installed() {
        let tmp = tempfile::tempdir().unwrap();
        let (a, a_bytes) = pkg_with("@acme/a", "1.0.0", &[]);
        let (mut b, b_bytes) = pkg_with("@acme/b", "1.0.0", &[]);
        b.sha256 = "0".repeat(64);
        let lock = Lockfile {
            packages: vec![a, b],
        };
        let mut fetch = |p: &LockedPackage| {
            Ok(if p.name == "@acme/a" {
                a_bytes.clone()
            } else {
                b_bytes.clone()
            })
        };
        assert!(ensure_installed(tmp.path(), &lock, &mut fetch).is_err());
        assert!(!cache_dir(tmp.path(), "@acme/a", "1.0.0").unwrap().exists());
        assert!(!cache_dir(tmp.path(), "@acme/b", "1.0.0").unwrap().exists());
    }

    #[test]
    fn a_lock_that_disagrees_with_the_manifest_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut billing, bytes) = pkg_with("@acme/billing", "1.0.0", &[("@acme/money", "^1")]);
        billing.dependencies.clear();
        let err = install_archive(tmp.path(), &billing, &bytes)
            .unwrap_err()
            .to_string();
        assert!(err.contains("declares dependencies"), "{err}");
    }

    #[test]
    fn a_locked_version_outside_a_declared_range_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let (billing, b_bytes) = pkg_with("@acme/billing", "1.0.0", &[("@acme/money", "^1")]);
        let (money, m_bytes) = pkg_with("@acme/money", "2.0.0", &[]);
        let lock = Lockfile {
            packages: vec![billing, money],
        };
        let mut fetch = |p: &LockedPackage| {
            Ok(if p.name == "@acme/billing" {
                b_bytes.clone()
            } else {
                m_bytes.clone()
            })
        };
        let err = ensure_installed(tmp.path(), &lock, &mut fetch)
            .unwrap_err()
            .to_string();
        assert!(err.contains("needs @acme/money@^1"), "{err}");
        assert!(
            !cache_dir(tmp.path(), "@acme/billing", "1.0.0")
                .unwrap()
                .exists()
        );
    }
}
