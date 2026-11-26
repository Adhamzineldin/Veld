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

/// The message catalogs of one member: the directory name and every `*.toml` file in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSet {
    /// The directory relative to the member, `/` separators (`messages`).
    pub dir: String,
    /// `(file name, text)`, sorted by name.
    pub files: Vec<(String, String)>,
}

/// The catalogs of a package (`spec/platform.md` section 22): the `*.toml` files directly in its
/// messages directory, sorted by name. `None` when the package has no such directory, or no file.
/// Subdirectories, symlinks and files of another kind are not catalogs and are skipped.
///
/// # Errors
/// The directory of an explicit `[service] messages` does not exist, a file cannot be read or is
/// not UTF-8.
pub fn collect_catalogs(pkg: &PackageSpec) -> Result<Option<CatalogSet>, ConfigError> {
    let Some(spec) = &pkg.messages else {
        return Ok(None);
    };
    if !spec.dir.is_dir() {
        if spec.explicit {
            return Err(ConfigError::invalid(format!(
                "`[service] messages` names `{}`, which is not a directory",
                spec.name
            )));
        }
        return Ok(None);
    }
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| ConfigError::Io { path, source }
    };
    let mut files = Vec::new();
    for entry in fs::read_dir(&spec.dir).map_err(io(&spec.dir))? {
        let path = entry.map_err(io(&spec.dir))?.path();
        let is_file = fs::symlink_metadata(&path).is_ok_and(|m| m.is_file());
        let name = path.file_name().and_then(|n| n.to_str()).map(str::to_owned);
        match name {
            Some(name) if is_file && name.ends_with(".toml") => {
                let text = fs::read_to_string(&path).map_err(io(&path))?;
                files.push((name, text));
            }
            _ => {}
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok((!files.is_empty()).then(|| CatalogSet {
        dir: spec.name.clone(),
        files,
    }))
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
            prefix: String::new(),
            messages: None,
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

    fn with_messages(root: &Path, name: &str, explicit: bool) -> PackageSpec {
        let mut s = spec(root);
        s.messages = Some(crate::config::MessagesSpec {
            dir: root.join(name),
            name: name.into(),
            explicit,
        });
        s
    }

    #[test]
    fn catalogs_are_the_toml_files_of_the_directory_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("messages");
        fs::create_dir_all(dir.join("nested")).unwrap();
        fs::write(dir.join("pt-br.toml"), "b").unwrap();
        fs::write(dir.join("ar.toml"), "a").unwrap();
        fs::write(dir.join("README.md"), "no").unwrap();
        fs::write(dir.join("nested/fr.toml"), "no").unwrap();
        let got = collect_catalogs(&with_messages(tmp.path(), "messages", false))
            .unwrap()
            .unwrap();
        assert_eq!(got.dir, "messages");
        let names: Vec<&str> = got.files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["ar.toml", "pt-br.toml"]);
        assert_eq!(got.files[0].1, "a");
    }

    #[test]
    fn no_directory_or_no_file_is_no_catalog() {
        let tmp = tempfile::tempdir().unwrap();
        let named = with_messages(tmp.path(), "messages", false);
        assert_eq!(collect_catalogs(&named).unwrap(), None);
        fs::create_dir_all(tmp.path().join("messages")).unwrap();
        assert_eq!(collect_catalogs(&named).unwrap(), None);
        assert_eq!(collect_catalogs(&spec(tmp.path())).unwrap(), None);
    }

    #[test]
    fn a_directory_that_was_named_but_is_missing_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = with_messages(tmp.path(), "texts", true);
        assert!(collect_catalogs(&spec).is_err());
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
