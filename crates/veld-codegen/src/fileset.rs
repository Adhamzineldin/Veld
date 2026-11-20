//! A validated, deterministic set of generated files (relative path -> bytes).

use std::collections::BTreeMap;

use thiserror::Error;

/// Name of the manifest file written into every output directory.
pub const MANIFEST_NAME: &str = ".veld-manifest.json";
/// Prefix of the temporary staging directory created inside an output directory.
pub const STAGING_PREFIX: &str = ".veld-staging-";

/// Why a generated file path was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PathError {
    #[error("path is empty")]
    Empty,
    #[error("path contains a NUL byte")]
    Nul,
    #[error("path contains a backslash; use `/` separators")]
    Backslash,
    #[error("path is absolute")]
    Absolute,
    #[error("path has an empty component")]
    EmptyComponent,
    #[error("path contains a `.` component")]
    DotComponent,
    #[error("path contains a `..` component")]
    Traversal,
    #[error("path has a drive prefix or contains `:`")]
    Colon,
    #[error("path component ends with `.` or space or has a character invalid on Windows")]
    InvalidComponent,
    #[error("path is reserved for Veld's own bookkeeping")]
    Reserved,
}

/// Errors from building a [`FileSet`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FileSetError {
    #[error("invalid output path `{path}`: {reason}")]
    InvalidPath { path: String, reason: PathError },
    #[error("duplicate output path `{0}`")]
    Duplicate(String),
    #[error("output path `{file}` is used as a file but `{other}` needs it to be a directory")]
    Collision { file: String, other: String },
}

/// Checks that `path` is a safe relative path with `/` separators.
pub fn validate_rel_path(path: &str) -> Result<(), PathError> {
    if path.is_empty() {
        return Err(PathError::Empty);
    }
    if path.contains('\0') {
        return Err(PathError::Nul);
    }
    if path.contains('\\') {
        return Err(PathError::Backslash);
    }
    if path.starts_with('/') {
        return Err(PathError::Absolute);
    }
    path.split('/').try_for_each(validate_component)?;
    reserved_check(path)
}

fn validate_component(comp: &str) -> Result<(), PathError> {
    match comp {
        "" => Err(PathError::EmptyComponent),
        "." => Err(PathError::DotComponent),
        ".." => Err(PathError::Traversal),
        _ if comp.contains(':') => Err(PathError::Colon),
        _ if comp.ends_with('.') || comp.ends_with(' ') => Err(PathError::InvalidComponent),
        _ if comp
            .chars()
            .any(|c| matches!(c, '<' | '>' | '"' | '|' | '?' | '*') || c.is_control()) =>
        {
            Err(PathError::InvalidComponent)
        }
        _ => Ok(()),
    }
}

fn reserved_check(path: &str) -> Result<(), PathError> {
    let first = path.split('/').next().unwrap_or_default();
    if path == MANIFEST_NAME || first.starts_with(STAGING_PREFIX) {
        return Err(PathError::Reserved);
    }
    Ok(())
}

/// Generated files, keyed by relative path, iterated in sorted order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileSet {
    files: BTreeMap<String, Vec<u8>>,
}

impl FileSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a file. The path must be relative, `/`-separated and free of `..`.
    pub fn add(
        &mut self,
        path: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<(), FileSetError> {
        let path = path.into();
        validate_rel_path(&path).map_err(|reason| FileSetError::InvalidPath {
            path: path.clone(),
            reason,
        })?;
        if self.files.contains_key(&path) {
            return Err(FileSetError::Duplicate(path));
        }
        self.check_collision(&path)?;
        self.files.insert(path, bytes.into());
        Ok(())
    }

    fn check_collision(&self, path: &str) -> Result<(), FileSetError> {
        for (idx, _) in path.match_indices('/') {
            let ancestor = &path[..idx];
            if self.files.contains_key(ancestor) {
                return Err(FileSetError::Collision {
                    file: ancestor.to_string(),
                    other: path.to_string(),
                });
            }
        }
        let prefix = format!("{path}/");
        if let Some((existing, _)) = self.files.range(prefix.clone()..).next()
            && existing.starts_with(&prefix)
        {
            return Err(FileSetError::Collision {
                file: path.to_string(),
                other: existing.clone(),
            });
        }
        Ok(())
    }

    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    pub fn contains(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Files in sorted path order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.files.iter().map(|(p, b)| (p.as_str(), b.as_slice()))
    }

    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_paths() {
        let mut fs = FileSet::new();
        fs.add("a.ts", "x").unwrap();
        fs.add("dir/sub/b.ts", b"y".to_vec()).unwrap();
        assert_eq!(fs.len(), 2);
        assert_eq!(fs.get("a.ts"), Some(&b"x"[..]));
        assert_eq!(fs.paths().collect::<Vec<_>>(), ["a.ts", "dir/sub/b.ts"]);
    }

    #[test]
    fn rejects_bad_paths() {
        let cases = [
            ("", PathError::Empty),
            ("../x", PathError::Traversal),
            ("a/../x", PathError::Traversal),
            ("/abs", PathError::Absolute),
            ("C:/x", PathError::Colon),
            ("a\\b", PathError::Backslash),
            ("a\0b", PathError::Nul),
            ("a//b", PathError::EmptyComponent),
            ("a/", PathError::EmptyComponent),
            ("./a", PathError::DotComponent),
            ("a/b.", PathError::InvalidComponent),
            (".veld-manifest.json", PathError::Reserved),
            (".veld-staging-1/x", PathError::Reserved),
        ];
        for (path, reason) in cases {
            assert_eq!(validate_rel_path(path), Err(reason), "{path:?}");
        }
    }

    #[test]
    fn manifest_name_is_only_reserved_at_top_level() {
        assert_eq!(validate_rel_path("sub/.veld-manifest.json"), Ok(()));
    }

    #[test]
    fn rejects_duplicates_and_collisions() {
        let mut fs = FileSet::new();
        fs.add("a", "1").unwrap();
        assert_eq!(fs.add("a", "2"), Err(FileSetError::Duplicate("a".into())));
        assert!(matches!(
            fs.add("a/b", "3"),
            Err(FileSetError::Collision { .. })
        ));
        fs.add("d/x", "1").unwrap();
        assert!(matches!(
            fs.add("d", "3"),
            Err(FileSetError::Collision { .. })
        ));
        fs.add("dd", "ok").unwrap();
    }
}
