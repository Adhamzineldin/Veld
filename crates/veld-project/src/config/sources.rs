//! Collecting `.veld` sources of a package, deterministically.

use std::fs;
use std::path::{Path, PathBuf};

use super::PackageSpec;
use super::error::ConfigError;

/// `(relative path with '/', text)` for every `**/*.veld` file, sorted by path.
///
/// Symlinks that resolve outside the package root are rejected; symlinks that
/// stay inside are skipped (the target is found at its real location).
pub fn collect_sources(pkg: &PackageSpec) -> Result<Vec<(String, String)>, ConfigError> {
    let root = fs::canonicalize(&pkg.root).map_err(|source| ConfigError::Io {
        path: pkg.root.clone(),
        source,
    })?;
    let mut found = Vec::new();
    walk(&root, &root, &mut found)?;
    found.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(found)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) -> Result<(), ConfigError> {
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| ConfigError::Io { path, source }
    };
    for entry in fs::read_dir(dir).map_err(io(dir))? {
        let entry = entry.map_err(io(dir))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(io(&path))?;
        if file_type.is_symlink() {
            check_link(root, &path)?;
        } else if file_type.is_dir() {
            walk(root, &path, out)?;
        } else if file_type.is_file() && path.extension().is_some_and(|e| e == "veld") {
            let text = fs::read_to_string(&path).map_err(io(&path))?;
            out.push((relative(root, &path)?, text));
        }
    }
    Ok(())
}

fn check_link(root: &Path, link: &Path) -> Result<(), ConfigError> {
    let target = fs::canonicalize(link).map_err(|source| ConfigError::Io {
        path: link.to_path_buf(),
        source,
    })?;
    if target.starts_with(root) {
        Ok(())
    } else {
        Err(ConfigError::SymlinkEscape {
            path: link.to_path_buf(),
        })
    }
}

fn relative(root: &Path, path: &Path) -> Result<String, ConfigError> {
    let rel = path
        .strip_prefix(root)
        .map_err(|_| ConfigError::SymlinkEscape {
            path: path.to_path_buf(),
        })?;
    let parts: Option<Vec<&str>> = rel.components().map(|c| c.as_os_str().to_str()).collect();
    parts
        .map(|p| p.join("/"))
        .ok_or_else(|| ConfigError::NonUtf8Path {
            path: PathBuf::from(path),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use veld_ir::PackageKind;

    fn spec(root: &Path) -> PackageSpec {
        PackageSpec {
            name: "p".into(),
            kind: PackageKind::Service,
            root: root.to_path_buf(),
            base_url: None,
            consumes: vec![],
        }
    }

    #[test]
    fn collects_sorted_relative_paths() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("models/deep")).unwrap();
        fs::write(tmp.path().join("z.veld"), "z").unwrap();
        fs::write(tmp.path().join("a.veld"), "a").unwrap();
        fs::write(tmp.path().join("models/deep/m.veld"), "m").unwrap();
        fs::write(tmp.path().join("models/readme.md"), "no").unwrap();
        let got = collect_sources(&spec(tmp.path())).unwrap();
        let paths: Vec<&str> = got.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(paths, ["a.veld", "models/deep/m.veld", "z.veld"]);
        assert_eq!(got[0].1, "a");
    }

    #[test]
    fn missing_root_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(collect_sources(&spec(&tmp.path().join("nope"))).is_err());
    }

    #[test]
    fn escaping_symlink_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = tmp.path().join("pkg");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&pkg).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("x.veld"), "x").unwrap();
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(&outside, pkg.join("link")).is_ok();
        #[cfg(windows)]
        let linked = std::os::windows::fs::symlink_dir(&outside, pkg.join("link")).is_ok();
        if !linked {
            eprintln!("skipping: symlink creation not permitted on this host");
            return;
        }
        assert!(matches!(
            collect_sources(&spec(&pkg)),
            Err(ConfigError::SymlinkEscape { .. })
        ));
    }
}
