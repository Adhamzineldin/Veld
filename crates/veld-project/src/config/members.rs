//! `members` and `exclude` patterns (`spec/workspace.md`, section 4.1). Pure: the caller says
//! which directories hold a `veld.toml`, so the CLI (from the disk) and the playground (from
//! its virtual files) match the same directories.

use std::collections::BTreeSet;

use super::error::ConfigError;
use super::raw::FileConfig;

/// Splits and checks one pattern. Only whole `*` segments are wildcards.
pub(super) fn segments(pattern: &str) -> Result<Vec<&str>, ConfigError> {
    let bad = |why: &str| {
        ConfigError::workspace(format!(
            "`{pattern}` is not a valid member pattern: {why} (a path with `/` and whole-segment `*` only)"
        ))
    };
    if pattern.is_empty() || pattern.starts_with('/') || pattern.contains('\\') {
        return Err(bad("it must be a relative path with `/` as the separator"));
    }
    if pattern.len() > 1 && pattern.as_bytes().get(1) == Some(&b':') {
        return Err(bad("it must not be an absolute path"));
    }
    let parts: Vec<&str> = pattern.split('/').collect();
    for part in &parts {
        match *part {
            "" | "." => return Err(bad("it has an empty or `.` segment")),
            ".." => return Err(bad("it has a `..` segment")),
            "*" => {}
            p if p.contains(['*', '?', '[', ']']) => {
                return Err(bad("`**`, `?`, `[...]` and `*` inside a name are not supported"));
            }
            _ => {}
        }
    }
    Ok(parts)
}

/// Whether the pattern (already valid) matches the directory `dir` (relative, `/` separated).
fn matches(parts: &[&str], dir: &str) -> bool {
    let dir_parts: Vec<&str> = dir.split('/').collect();
    dir_parts.len() == parts.len()
        && parts
            .iter()
            .zip(&dir_parts)
            .all(|(pat, d)| *pat == "*" || pat == d)
}

/// Whether any pattern matches `dir`. A bad pattern is an error.
pub fn any_matches(patterns: &[String], dir: &str) -> Result<bool, ConfigError> {
    for pattern in patterns {
        if matches(&segments(pattern)?, dir) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The members of a workspace: the directories in `dirs` (those that hold a `veld.toml`,
/// relative to the root, `/` separated) that `members` selects and `exclude` does not,
/// ordered by path as bytes.
pub fn expand(
    members: &[String],
    exclude: &[String],
    dirs: &[String],
) -> Result<Vec<String>, ConfigError> {
    let mut found = BTreeSet::new();
    for pattern in members {
        let parts = segments(pattern)?;
        let hits: Vec<&String> = dirs.iter().filter(|d| matches(&parts, d)).collect();
        if !parts.contains(&"*") && hits.is_empty() {
            return Err(ConfigError::workspace(format!(
                "member `{pattern}` does not exist or has no veld.toml"
            )));
        }
        found.extend(hits.into_iter().cloned());
    }
    let mut kept = Vec::new();
    for dir in found {
        if !any_matches(exclude, &dir)? {
            kept.push(dir);
        }
    }
    for pattern in exclude {
        segments(pattern)?;
    }
    Ok(kept)
}

/// The ordered member directories a root file selects, given the directories that hold a
/// `veld.toml`. Empty when the file has no `[workspace]` table.
pub fn expand_members(root_text: &str, dirs: &[String]) -> Result<Vec<String>, ConfigError> {
    let cfg = FileConfig::parse(root_text)?;
    match cfg.workspace {
        Some(ws) => expand(&ws.members, &ws.exclude, dirs),
        None => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn star_matches_one_directory_level() {
        let dirs = s(&["apps/web", "services/a", "services/b", "services/b/inner"]);
        let got = expand(&s(&["services/*", "apps/*"]), &[], &dirs).unwrap();
        assert_eq!(got, ["apps/web", "services/a", "services/b"]);
    }

    #[test]
    fn exclude_wins_and_duplicates_are_one_member() {
        let dirs = s(&["services/a", "services/legacy"]);
        let got = expand(
            &s(&["services/*", "services/a"]),
            &s(&["services/legacy"]),
            &dirs,
        )
        .unwrap();
        assert_eq!(got, ["services/a"]);
    }

    #[test]
    fn a_plain_entry_must_exist_and_a_star_entry_may_match_nothing() {
        let err = expand(&s(&["services/x"]), &[], &s(&["services/a"])).unwrap_err();
        assert_eq!(err.code(), Some("E0710"));
        assert!(expand(&s(&["libs/*"]), &[], &s(&["services/a"])).unwrap().is_empty());
    }

    #[test]
    fn members_are_ordered_by_bytes() {
        let dirs = s(&["b", "a/z", "a-b", "a"]);
        let got = expand(&s(&["*", "a/*"]), &[], &dirs).unwrap();
        assert_eq!(got, ["a", "a-b", "a/z", "b"]);
    }

    #[test]
    fn unsupported_syntax_is_e0710() {
        for p in [
            "**", "a/**", "svc-*", "a?", "a[b]", "/a", "../a", "a/../b", "", "a//b", "a\\b",
            "C:/a", "./a",
        ] {
            let err = expand(&s(&[p]), &[], &s(&["a"])).unwrap_err();
            assert_eq!(err.code(), Some("E0710"), "{p}");
        }
        let err = expand(&s(&["a"]), &s(&["**"]), &s(&["a"])).unwrap_err();
        assert_eq!(err.code(), Some("E0710"));
    }

    #[test]
    fn expand_members_reads_the_root_file() {
        let dirs = s(&["services/a", "apps/w"]);
        let text = "[workspace]\nname = \"w\"\nmembers = [\"services/*\"]\n";
        assert_eq!(expand_members(text, &dirs).unwrap(), ["services/a"]);
        assert!(expand_members("[service]\nname = \"a\"\n", &dirs).unwrap().is_empty());
    }
}
