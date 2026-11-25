//! The workspace model: `veld.toml`, open buffers over disk contents, and the analysis of
//! the whole project, which is recomputed from scratch on every change.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use lsp_types::Uri;
use veld_compiler::index::{self, Index};
use veld_compiler::{CompileInput, FileId, PackageInput, SourceFile};
use veld_ir::PackageKind;
use veld_project::config::{self, Project};

use crate::text::LineIndex;
use crate::uri::{path_key, path_to_uri};

/// Name of the package that holds `.veld` files that belong to no configured package.
pub const AD_HOC_PACKAGE: &str = "_adhoc";

const MAX_AD_HOC_FILES: usize = 500;
const SKIP_DIRS: [&str; 5] = ["node_modules", "target", ".git", "dist", "build"];

/// An open editor buffer.
#[derive(Debug, Clone)]
struct Buffer {
    path: PathBuf,
    text: String,
}

/// Open buffers and the workspace root.
#[derive(Debug, Default)]
pub struct Workspace {
    pub root: Option<PathBuf>,
    buffers: BTreeMap<String, Buffer>,
}

/// Where a file of the analysis lives.
#[derive(Debug)]
pub struct FileMeta {
    pub path: Option<PathBuf>,
    pub uri: Option<Uri>,
    pub lines: LineIndex,
}

/// Signatures from the last compilation that had no errors.
#[derive(Debug, Clone, Default)]
pub struct Sigs {
    /// Qualified declaration name to `sig`.
    pub decls: BTreeMap<String, String>,
    /// (service, endpoint) to the operation `sig`.
    pub ops: BTreeMap<(String, String), String>,
}

impl Sigs {
    fn from_ir(ir: &veld_ir::Ir) -> Self {
        let decls = ir
            .decls
            .iter()
            .map(|d| (d.qualified.clone(), d.sig.clone()))
            .collect();
        let ops = ir
            .services
            .iter()
            .flat_map(|s| {
                s.endpoints
                    .iter()
                    .map(|e| ((s.name.clone(), e.name.clone()), e.sig.clone()))
            })
            .collect();
        Self { decls, ops }
    }
}

/// One analysis of the whole workspace.
#[derive(Debug)]
pub struct Analysis {
    pub index: Index,
    /// Indexed by `FileId.0`.
    pub files: Vec<FileMeta>,
    by_key: BTreeMap<String, FileId>,
    pub sigs: Sigs,
    /// True when `sigs` come from an earlier, successful compilation.
    pub sigs_stale: bool,
    /// The `veld.toml` that was used, if any, and why it could not be loaded.
    pub config_path: Option<PathBuf>,
    pub config_error: Option<String>,
}

impl Analysis {
    #[must_use]
    pub fn meta(&self, id: FileId) -> Option<&FileMeta> {
        self.files.get(id.0 as usize)
    }

    #[must_use]
    pub fn file_for(&self, path: &Path) -> Option<FileId> {
        self.by_key.get(&path_key(path)).copied()
    }

    #[must_use]
    pub fn lines(&self, id: FileId) -> Option<&LineIndex> {
        self.meta(id).map(|m| &m.lines)
    }

    #[must_use]
    pub fn uri(&self, id: FileId) -> Option<&Uri> {
        self.meta(id)?.uri.as_ref()
    }
}

impl Workspace {
    #[must_use]
    pub fn new(root: Option<PathBuf>) -> Self {
        Self {
            root,
            buffers: BTreeMap::new(),
        }
    }

    pub fn open(&mut self, path: &Path, text: String) {
        self.buffers.insert(
            path_key(path),
            Buffer {
                path: path.to_path_buf(),
                text,
            },
        );
    }

    pub fn close(&mut self, path: &Path) {
        self.buffers.remove(&path_key(path));
    }

    #[must_use]
    pub fn buffer_text(&self, path: &Path) -> Option<&str> {
        self.buffers.get(&path_key(path)).map(|b| b.text.as_str())
    }

    /// Analyses the project. `prev` supplies signatures when this compilation has errors.
    #[must_use]
    pub fn analyze(&self, prev: Option<&Sigs>) -> Analysis {
        let (project, config_path, config_error) = self.load_project();
        let mut builder = InputBuilder::default();
        match &project {
            Some(p) => self.add_project_files(p, &mut builder),
            None => self.add_loose_files(&mut builder),
        }
        let used: BTreeSet<String> = builder.keys();
        for b in self.buffers.values() {
            let is_veld = b.path.extension().is_some_and(|e| e == "veld");
            if is_veld && !used.contains(&path_key(&b.path)) {
                builder.add_ad_hoc(&b.path, self.root.as_deref(), b.text.clone());
            }
        }
        let (input, paths) = builder.finish(project.as_ref());
        let prefixes = project.as_ref().map(Project::prefixes).unwrap_or_default();
        let index = index::index_prefixed(&input, &prefixes);
        let (sigs, stale) = match &index.ir {
            Some(ir) => (Sigs::from_ir(ir), false),
            None => (prev.cloned().unwrap_or_default(), true),
        };
        let mut by_key = BTreeMap::new();
        let files = index
            .files
            .iter()
            .map(|f| {
                let path = paths.get(&(f.package.clone(), f.rel_path.clone())).cloned();
                if let Some(p) = &path {
                    by_key.insert(path_key(p), f.id);
                }
                let text = index.sources.text(f.id).unwrap_or("");
                FileMeta {
                    uri: path.as_deref().and_then(path_to_uri),
                    path,
                    lines: LineIndex::new(text),
                }
            })
            .collect();
        Analysis {
            index,
            files,
            by_key,
            sigs,
            sigs_stale: stale,
            config_path,
            config_error,
        }
    }

    fn load_project(&self) -> (Option<Project>, Option<PathBuf>, Option<String>) {
        let Some(root) = &self.root else {
            return (None, None, None);
        };
        let Ok(path) = config::find_config(root) else {
            return (None, None, None);
        };
        // An open, unsaved `veld.toml` (the root's or a member's) wins over the disk.
        let result = config::load_with(&path, &|p| self.buffer_text(p).map(str::to_string));
        match result {
            Ok(p) => (Some(p), Some(path), None),
            Err(e) => (None, Some(path), Some(e.to_string())),
        }
    }

    fn add_project_files(&self, project: &Project, b: &mut InputBuilder) {
        for pkg in &project.packages {
            let mut files: BTreeMap<String, (PathBuf, String)> = BTreeMap::new();
            for (rel, text) in config::collect_sources(pkg).unwrap_or_default() {
                files.insert(rel.clone(), (pkg.root.join(&rel), text));
            }
            for buf in self.buffers.values() {
                if let Some(rel) = relative_to(&pkg.root, &buf.path)
                    && rel.ends_with(".veld")
                {
                    files.insert(rel, (buf.path.clone(), buf.text.clone()));
                }
            }
            for (rel, (path, text)) in files {
                b.add(&pkg.name, rel, path, text);
            }
        }
    }

    /// No usable `veld.toml`: every `.veld` file under the root is one ad-hoc package.
    fn add_loose_files(&self, b: &mut InputBuilder) {
        let Some(root) = &self.root else { return };
        let mut found = Vec::new();
        walk(root, 0, &mut found);
        for path in found {
            let text = match self.buffer_text(&path) {
                Some(t) => t.to_string(),
                None => fs::read_to_string(&path).unwrap_or_default(),
            };
            b.add_ad_hoc(&path, Some(root), text);
        }
    }
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 6 || out.len() >= MAX_AD_HOC_FILES {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for e in entries {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() && !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
            walk(&path, depth + 1, out);
        } else if ft.is_file() && path.extension().is_some_and(|x| x == "veld") {
            out.push(path);
        }
    }
}

/// `p` relative to `root` with `/` separators, when `p` is inside `root`.
fn relative_to(root: &Path, p: &Path) -> Option<String> {
    let mut rest = p.components();
    for rc in root.components() {
        let pc = rest.next()?;
        let (a, b) = (
            rc.as_os_str().to_string_lossy(),
            pc.as_os_str().to_string_lossy(),
        );
        let same = if cfg!(windows) {
            a.eq_ignore_ascii_case(&b)
        } else {
            a == b
        };
        if !same {
            return None;
        }
    }
    let parts: Vec<String> = rest
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

#[derive(Default)]
struct InputBuilder {
    packages: BTreeMap<String, Vec<SourceFile>>,
    paths: BTreeMap<(String, String), PathBuf>,
    ad_hoc_used: BTreeSet<String>,
}

impl InputBuilder {
    fn add(&mut self, package: &str, rel: String, path: PathBuf, text: String) {
        self.paths.insert((package.to_string(), rel.clone()), path);
        self.packages
            .entry(package.to_string())
            .or_default()
            .push(SourceFile { path: rel, text });
    }

    fn keys(&self) -> BTreeSet<String> {
        self.paths.values().map(|p| path_key(p)).collect()
    }

    /// Adds a file to the ad-hoc package, named by its path under `root` (or its file name).
    fn add_ad_hoc(&mut self, path: &Path, root: Option<&Path>, text: String) {
        let mut rel = root
            .and_then(|r| relative_to(r, path))
            .or_else(|| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "untitled.veld".to_string());
        while self.ad_hoc_used.contains(&rel) {
            rel = format!("dup_{rel}");
        }
        self.ad_hoc_used.insert(rel.clone());
        self.add(AD_HOC_PACKAGE, rel, path.to_path_buf(), text);
    }

    fn finish(
        self,
        project: Option<&Project>,
    ) -> (CompileInput, BTreeMap<(String, String), PathBuf>) {
        let mut consumes = BTreeMap::new();
        let mut packages = Vec::new();
        let configured: Vec<_> = project.map(|p| p.packages.clone()).unwrap_or_default();
        for pkg in &configured {
            if pkg.kind == PackageKind::Service && !pkg.consumes.is_empty() {
                consumes.insert(pkg.name.clone(), pkg.consumes.clone());
            }
        }
        for (name, files) in self.packages {
            let spec = configured.iter().find(|p| p.name == name);
            packages.push(PackageInput {
                kind: spec.map_or(PackageKind::Service, |p| p.kind),
                base_url: spec.and_then(|p| p.base_url.clone()),
                name,
                files,
            });
        }
        // A configured package with no files still exists, so `consumes` stays valid.
        for pkg in configured {
            if !packages.iter().any(|p| p.name == pkg.name) {
                packages.push(PackageInput {
                    name: pkg.name,
                    kind: pkg.kind,
                    files: Vec::new(),
                    base_url: pkg.base_url,
                });
            }
        }
        (CompileInput { packages, consumes }, self.paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, text: &str) {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    fn project_dir() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        write(
            d,
            "veld.toml",
            "[workspace]\nname = \"shop\"\nmembers = [\"iam\", \"app\"]\n",
        );
        write(d, "iam/veld.toml", "[service]\ncontracts = \".\"\n");
        write(
            d,
            "app/veld.toml",
            "[service]\ncontracts = \".\"\nconsumes = [\"iam\"]\n",
        );
        write(d, "iam/users.veld", "model User { id: uuid }\n");
        write(
            d,
            "app/orders.veld",
            "use iam:users.{User}\nmodel Order { owner: User }\n",
        );
        tmp
    }

    #[test]
    fn maps_files_to_packages_through_the_config() {
        let tmp = project_dir();
        let ws = Workspace::new(Some(tmp.path().to_path_buf()));
        let a = ws.analyze(None);
        assert!(a.config_error.is_none());
        assert_eq!(a.index.error_count(), 0);
        let f = a.file_for(&tmp.path().join("app/orders.veld")).unwrap();
        assert_eq!(a.index.file(f).unwrap().package, "app");
        assert!(a.sigs.decls.contains_key("iam:users.User"));
        assert!(!a.sigs_stale);
    }

    #[test]
    fn buffers_override_disk_and_new_files_join_their_package() {
        let tmp = project_dir();
        let mut ws = Workspace::new(Some(tmp.path().to_path_buf()));
        let orders = tmp.path().join("app/orders.veld");
        ws.open(&orders, "model Order { owner: Nope }\n".into());
        let a = ws.analyze(None);
        assert!(a.index.diagnostics.iter().any(|d| d.is_error()));
        assert!(a.sigs_stale || a.sigs.decls.is_empty());
        let fresh = tmp.path().join("app/extra.veld");
        ws.open(&orders, "model Order { id: uuid }\n".into());
        ws.open(
            &fresh,
            "use orders.{Order}\nmodel Extra { o: Order }\n".into(),
        );
        let a = ws.analyze(None);
        assert_eq!(
            a.index.error_count(),
            0,
            "{}",
            a.index.sources.render_all(&a.index.diagnostics)
        );
        let f = a.file_for(&fresh).unwrap();
        assert_eq!(a.index.file(f).unwrap().package, "app");
    }

    #[test]
    fn files_outside_any_package_form_one_ad_hoc_package() {
        let tmp = project_dir();
        let mut ws = Workspace::new(Some(tmp.path().to_path_buf()));
        write(tmp.path(), "scratch/a.veld", "model A { b: B }\n");
        let (a_path, b_path) = (
            tmp.path().join("scratch/a.veld"),
            tmp.path().join("scratch/b.veld"),
        );
        ws.open(&a_path, "model A { b: B }\n".into());
        ws.open(&b_path, "model B { x: int }\n".into());
        let a = ws.analyze(None);
        let fa = a.file_for(&a_path).unwrap();
        let fb = a.file_for(&b_path).unwrap();
        assert_eq!(a.index.file(fa).unwrap().package, AD_HOC_PACKAGE);
        assert_eq!(a.index.file(fb).unwrap().package, AD_HOC_PACKAGE);
    }

    #[test]
    fn without_a_config_every_veld_file_under_the_root_is_loaded() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "a.veld", "model A { x: int }\n");
        write(tmp.path(), "sub/b.veld", "model B { a: A }\n");
        let ws = Workspace::new(Some(tmp.path().to_path_buf()));
        let a = ws.analyze(None);
        assert_eq!(
            a.index
                .files
                .iter()
                .filter(|f| f.package == AD_HOC_PACKAGE)
                .count(),
            2
        );
    }

    #[test]
    fn a_broken_config_is_reported_and_falls_back_to_ad_hoc() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "veld.toml", "[project\n");
        write(tmp.path(), "a.veld", "model A { x: int }\n");
        let a = Workspace::new(Some(tmp.path().to_path_buf())).analyze(None);
        assert!(a.config_error.is_some());
        assert_eq!(a.index.error_count(), 0);
    }

    #[test]
    fn signatures_survive_a_broken_edit() {
        let tmp = project_dir();
        let mut ws = Workspace::new(Some(tmp.path().to_path_buf()));
        let good = ws.analyze(None);
        ws.open(
            &tmp.path().join("iam/users.veld"),
            "model User { id: }\n".into(),
        );
        let bad = ws.analyze(Some(&good.sigs));
        assert!(bad.sigs_stale);
        assert!(bad.sigs.decls.contains_key("iam:users.User"));
    }

    #[test]
    fn relative_paths_are_component_wise() {
        assert_eq!(
            relative_to(Path::new("/r/x"), Path::new("/r/x/a/b.veld")).as_deref(),
            Some("a/b.veld")
        );
        assert!(relative_to(Path::new("/r/x"), Path::new("/r/xy/a.veld")).is_none());
    }
}
