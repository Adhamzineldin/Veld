//! Finding and reading a workspace from the disk (`spec/workspace.md`, section 3.1), and from a
//! list of virtual files (the playground). Both end in [`resolve`].

use std::fs;
use std::path::{Path, PathBuf};

use super::error::ConfigError;
use super::members::{any_matches, expand, expand_members, segments};
use super::paths::absolutize;
use super::raw::{FileConfig, parse_table};
use super::{CONFIG_FILE, Project, legacy, resolve::resolve};

/// Text that replaces what is on the disk, for files an editor has open and unsaved.
type Overlay<'a> = &'a dyn Fn(&Path) -> Option<String>;

fn read(path: &Path, overlay: Overlay<'_>) -> Result<String, ConfigError> {
    if let Some(text) = overlay(path) {
        return Ok(text);
    }
    fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Reads and resolves the workspace that the `veld.toml` at `path` belongs to: the file itself
/// when it has `[workspace]`, else the nearest workspace above that lists its directory, else
/// the file alone as a single file workspace. [`Project::current`] names the member when
/// `path` is a member's file below a root.
pub fn load(path: &Path) -> Result<Project, ConfigError> {
    load_with(path, &|_| None)
}

/// [`load`], where `overlay` may supply the text of any `veld.toml` instead of the disk (the
/// language server passes its open buffers).
pub fn load_with(
    path: &Path,
    overlay: &dyn Fn(&Path) -> Option<String>,
) -> Result<Project, ConfigError> {
    let path = absolutize(path)?;
    let text = read(&path, overlay)?;
    let dir = parent(&path)?;
    let table = parse_table(&text)?;
    legacy::check(&table)?;
    if table.contains_key("workspace") {
        return load_root(&dir, &text, None, overlay).map(|p| with_file(p, &path));
    }
    let Some((root_dir, root_text)) = workspace_above(&dir, overlay)? else {
        return resolve(&dir, &text, &[]).map(|p| with_file(p, &path));
    };
    let rel = relative(&root_dir, &dir);
    let ws = FileConfig::parse(&root_text)?.workspace.unwrap_or_default();
    if any_matches(&ws.exclude, &rel)? {
        return resolve(&dir, &text, &[]).map(|p| with_file(p, &path));
    }
    if any_matches(&ws.members, &rel)? {
        return load_root(&root_dir, &root_text, Some(&rel), overlay);
    }
    Err(ConfigError::workspace(format!(
        "`{}` is a member file, but the workspace `{}` neither lists `{rel}` in `members` nor in `exclude`; add the directory to one of them",
        path.display(),
        root_dir.join(CONFIG_FILE).display()
    )))
}

/// The root file is the file that was named (`--config other.toml`), not always `veld.toml`.
fn with_file(mut project: Project, file: &Path) -> Project {
    project.file = file.to_path_buf();
    project
}

fn parent(path: &Path) -> Result<PathBuf, ConfigError> {
    path.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| ConfigError::invalid("config path has no parent directory"))
}

/// `dir` below `root`, `/` separated.
fn relative(root: &Path, dir: &Path) -> String {
    dir.strip_prefix(root)
        .unwrap_or(dir)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The nearest `veld.toml` above `dir` that has a `[workspace]` table, with its text.
fn workspace_above(
    dir: &Path,
    overlay: Overlay<'_>,
) -> Result<Option<(PathBuf, String)>, ConfigError> {
    for ancestor in dir.ancestors().skip(1) {
        let file = ancestor.join(CONFIG_FILE);
        if !file.is_file() {
            continue;
        }
        let text = read(&file, overlay)?;
        let table = parse_table(&text)?;
        legacy::check(&table)?;
        if table.contains_key("workspace") {
            return Ok(Some((ancestor.to_path_buf(), text)));
        }
    }
    Ok(None)
}

fn load_root(
    root_dir: &Path,
    root_text: &str,
    current: Option<&str>,
    overlay: Overlay<'_>,
) -> Result<Project, ConfigError> {
    let ws = FileConfig::parse(root_text)?.workspace.unwrap_or_default();
    let dirs = disk_dirs(root_dir, &ws.members)?;
    let selected = expand(&ws.members, &ws.exclude, &dirs)?;
    let mut files = Vec::new();
    for rel in selected {
        let text = read(&root_dir.join(&rel).join(CONFIG_FILE), overlay)?;
        files.push((rel, text));
    }
    let mut project = resolve(root_dir, root_text, &files)?;
    if let Some(rel) = current {
        let dir = super::paths::normalize(&root_dir.join(rel));
        project.current = project
            .members
            .iter()
            .find(|m| m.dir == dir)
            .map(|m| m.name.clone());
    }
    Ok(project)
}

/// The directories that a `members` pattern can name and that hold a `veld.toml`,
/// relative to the root with `/`, found by walking only the segments the patterns spell out.
fn disk_dirs(root: &Path, patterns: &[String]) -> Result<Vec<String>, ConfigError> {
    let mut found = Vec::new();
    for pattern in patterns {
        let mut level = vec![String::new()];
        for part in segments(pattern)? {
            let mut next = Vec::new();
            for base in &level {
                if part == "*" {
                    next.extend(subdirs(root, base));
                } else {
                    next.push(join(base, part));
                }
            }
            level = next;
        }
        found.extend(
            level
                .into_iter()
                .filter(|d| root.join(d).join(CONFIG_FILE).is_file()),
        );
    }
    found.sort();
    found.dedup();
    Ok(found)
}

fn join(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_string()
    } else {
        format!("{base}/{name}")
    }
}

/// The sub-directories of `root/base` (real directories, not links), as `base/name`.
fn subdirs(root: &Path, base: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root.join(base)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .map(|name| join(base, &name))
        .collect()
}

/// Resolves a workspace from virtual files: `(path relative to the root with /, text)` for
/// every `veld.toml`, the root's included as `veld.toml`. The same two pure functions as
/// [`load`] decide the members and the targets, so a playground and the CLI agree.
pub fn load_virtual(root_dir: &Path, files: &[(String, String)]) -> Result<Project, ConfigError> {
    let text_of = |path: &str| {
        files
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, t)| t.as_str())
    };
    let root_text = text_of(CONFIG_FILE)
        .ok_or_else(|| ConfigError::workspace("the workspace has no root `veld.toml`"))?;
    let dirs: Vec<String> = files
        .iter()
        .filter_map(|(p, _)| p.strip_suffix("/veld.toml").map(str::to_string))
        .collect();
    let selected = expand_members(root_text, &dirs)?;
    let members: Vec<(String, String)> = selected
        .into_iter()
        .filter_map(|rel| {
            let text = text_of(&format!("{rel}/veld.toml"))?.to_string();
            Some((rel, text))
        })
        .collect();
    resolve(&super::paths::normalize(root_dir), root_text, &members)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn shop(root: &Path) {
        write(
            root,
            "veld.toml",
            "[workspace]\nname = \"shop\"\nmembers = [\"services/*\", \"apps/*\"]\nexclude = [\"services/legacy\"]\n",
        );
        write(
            root,
            "services/bookings/veld.toml",
            "[service]\nconsumes = [\"payments\"]\n[server]\nemitter = \"node\"\n",
        );
        write(root, "services/payments/veld.toml", "[service]\n");
        write(root, "services/legacy/veld.toml", "[service]\n");
        write(
            root,
            "apps/web/veld.toml",
            "[app]\n[client]\nemitter = \"typescript\"\n",
        );
    }

    #[test]
    fn a_workspace_loads_from_the_root_and_from_a_member() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        shop(root);
        let from_root = load(&root.join("veld.toml")).unwrap();
        assert_eq!(from_root.name, "shop");
        assert_eq!(from_root.current, None);
        let names: Vec<_> = from_root.members.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["web", "bookings", "payments"]);
        let from_member = load(&root.join("services/bookings/veld.toml")).unwrap();
        assert_eq!(from_member.current.as_deref(), Some("bookings"));
        assert_eq!(from_member.root, absolutize(root).unwrap());
        assert_eq!(from_member.members, from_root.members);
        assert_eq!(from_member.targets, from_root.targets);
    }

    #[test]
    fn an_excluded_member_is_a_workspace_of_its_own() {
        let tmp = tempfile::tempdir().unwrap();
        shop(tmp.path());
        write(
            tmp.path(),
            "services/legacy/veld.toml",
            "[service]\nname = \"legacy\"\n",
        );
        let p = load(&tmp.path().join("services/legacy/veld.toml")).unwrap();
        assert_eq!(p.name, "legacy");
        assert_eq!(p.members.len(), 1);
    }

    #[test]
    fn a_member_file_no_workspace_claims_is_e0710() {
        let tmp = tempfile::tempdir().unwrap();
        shop(tmp.path());
        write(
            tmp.path(),
            "tools/gen/veld.toml",
            "[service]\nname = \"gen\"\n",
        );
        let err = load(&tmp.path().join("tools/gen/veld.toml")).unwrap_err();
        assert_eq!(err.code(), Some("E0710"));
        let msg = err.to_string();
        assert!(msg.contains("members") && msg.contains("exclude"), "{msg}");
    }

    #[test]
    fn a_lone_member_file_is_a_single_file_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            tmp.path(),
            "veld.toml",
            "[service]\nname = \"hello\"\n[server]\nemitter = \"node\"\n",
        );
        let p = load(&tmp.path().join("veld.toml")).unwrap();
        assert_eq!((p.name.as_str(), p.members.len()), ("hello", 1));
        assert_eq!(p.targets[0].name, "hello");
        assert!(p.targets[0].out.ends_with("src/generated"));
    }

    #[test]
    fn virtual_files_resolve_like_the_disk() {
        let tmp = tempfile::tempdir().unwrap();
        shop(tmp.path());
        let disk = load(&tmp.path().join("veld.toml")).unwrap();
        let mut files = Vec::new();
        for rel in [
            "veld.toml",
            "services/bookings/veld.toml",
            "services/payments/veld.toml",
            "services/legacy/veld.toml",
            "apps/web/veld.toml",
        ] {
            files.push((
                rel.to_string(),
                fs::read_to_string(tmp.path().join(rel)).unwrap(),
            ));
        }
        let virt = load_virtual(tmp.path(), &files).unwrap();
        assert_eq!(virt, disk);
    }

    #[test]
    fn a_member_below_a_root_with_root_only_tables_is_e0711() {
        let tmp = tempfile::tempdir().unwrap();
        shop(tmp.path());
        write(
            tmp.path(),
            "services/payments/veld.toml",
            "[service]\n[registry]\nurl = \"https://r\"\n",
        );
        let err = load(&tmp.path().join("veld.toml")).unwrap_err();
        assert_eq!(err.code(), Some("E0711"));
        assert!(err.to_string().contains("workspace root"), "{err}");
    }
}
