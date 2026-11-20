//! `veld migrate`: v1 project to v2. Reads `veld.config.json` and the v1 `.veld` files, writes
//! v2 contracts and a `veld.toml` next to the v1 config. Existing files are never overwritten.

mod plan;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::cli::MigrateArgs;
use crate::error::{Exit, user};
use crate::fsutil;
use crate::pipeline;
use crate::ui::Ui;

const V1_CONFIG: &str = "veld.config.json";

pub fn run(ui: &Ui, args: &MigrateArgs) -> Result<Exit> {
    let (dir, v1) = read_v1_config(args.from.as_deref())?;
    let name = project_name(&dir);
    let cfg = plan::plan(&v1, &name, args.out.trim_matches('/'));
    let mut warnings = cfg.warnings.clone();
    let mut outputs: Vec<(PathBuf, String)> = Vec::new();
    for pkg in &cfg.packages {
        let src = dir.join(&pkg.source);
        let skip = dir.join(&pkg.dest);
        let files = read_v1_files(&src, &skip)?;
        if files.is_empty() {
            warnings.push(format!(
                "no v1 .veld files found in `{}`",
                pipeline::display_path(&src)
            ));
        }
        let result = veld_tools::migrate_v1(&files);
        warnings.extend(result.warnings.iter().map(|w| format!("{}: {w}", pkg.dest)));
        outputs.extend(
            result
                .files
                .into_iter()
                .map(|(rel, text)| (dir.join(&pkg.dest).join(rel), text)),
        );
    }
    outputs.push((dir.join("veld.toml"), cfg.toml.clone()));
    write_outputs(ui, &dir, &outputs, args.dry_run, &mut warnings)?;
    for w in &warnings {
        ui.warn(w);
    }
    Ok(Exit::Ok)
}

fn write_outputs(
    ui: &Ui,
    dir: &Path,
    outputs: &[(PathBuf, String)],
    dry_run: bool,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let (mut written, mut skipped) = (0, 0);
    for (path, text) in outputs {
        let shown = path
            .strip_prefix(dir)
            .unwrap_or(path)
            .display()
            .to_string()
            .replace('\\', "/");
        if dry_run {
            let state = if path.exists() {
                "exists, would be skipped"
            } else {
                "would write"
            };
            ui.info(&format!("{state}: {shown}"));
        } else if fsutil::create_new(path, text.as_bytes())? {
            written += 1;
            ui.info(&format!("wrote {shown}"));
        } else {
            skipped += 1;
            warnings.push(format!("`{shown}` already exists and was not overwritten"));
        }
    }
    if !dry_run {
        ui.info(&format!(
            "migrated: {written} file(s) written, {skipped} skipped; run `veld check` in {}",
            pipeline::display_path(dir)
        ));
    }
    Ok(())
}

/// The v1 config directory and its parsed JSON.
fn read_v1_config(from: Option<&Path>) -> Result<(PathBuf, serde_json::Value)> {
    let start = match from {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir().context("cannot read the current directory")?,
    };
    let candidates = [
        start.join(V1_CONFIG),
        start.join("veld").join(V1_CONFIG),
        start.clone(),
    ];
    let file = candidates
        .into_iter()
        .find(|c| c.is_file() && c.file_name().is_some_and(|n| n == V1_CONFIG))
        .ok_or_else(|| {
            user(format!(
                "no v1 `{V1_CONFIG}` found in `{}` or its `veld` subdirectory",
                start.display()
            ))
        })?;
    let text =
        fs::read_to_string(&file).with_context(|| format!("cannot read `{}`", file.display()))?;
    let json = serde_json::from_str(&text)
        .map_err(|e| user(format!("`{}` is not valid JSON: {e}", file.display())))?;
    let dir = fs::canonicalize(file.parent().unwrap_or(Path::new(".")))
        .context("cannot resolve the v1 config directory")?;
    Ok((strip_verbatim(dir), json))
}

/// `\\?\C:\x` becomes `C:\x`, so messages and relative paths stay readable on Windows.
fn strip_verbatim(p: PathBuf) -> PathBuf {
    let text = p.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => p,
    }
}

/// The directory name, or its parent's when the config sits in a conventional `veld/` folder.
fn project_name(dir: &Path) -> String {
    let leaf = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
    match leaf(dir).as_deref() {
        Some("veld") => dir.parent().and_then(leaf).unwrap_or_else(|| "app".into()),
        Some(n) => n.to_string(),
        None => "app".into(),
    }
}

/// Every v1 `.veld` file below `root`, relative with `/`, skipping registry copies,
/// `node_modules`, hidden folders and `skip` (the destination).
fn read_v1_files(root: &Path, skip: &Path) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    walk(root, root, skip, &mut out)?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn walk(root: &Path, dir: &Path, skip: &Path, out: &mut Vec<(String, String)>) -> Result<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries {
        let path = entry?.path();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if path.is_dir() {
            if !matches!(name.as_str(), "packages" | "node_modules")
                && !name.starts_with('.')
                && path != skip
            {
                walk(root, &path, skip, out)?;
            }
        } else if path.extension().is_some_and(|e| e == "veld") {
            let rel = path
                .strip_prefix(root)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let text = fs::read_to_string(&path)
                .with_context(|| format!("cannot read `{}`", path.display()))?;
            out.push((rel, text.replace("\r\n", "\n")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_name_skips_the_veld_folder() {
        assert_eq!(project_name(Path::new("/work/shop/veld")), "shop");
        assert_eq!(project_name(Path::new("/work/shop")), "shop");
    }

    #[test]
    fn read_v1_files_skips_packages_node_modules_and_the_destination() {
        let tmp = tempfile::tempdir().unwrap();
        for f in [
            "a.veld",
            "m/b.veld",
            "packages/x.veld",
            "node_modules/y.veld",
            "contracts/z.veld",
            "n.txt",
        ] {
            let p = tmp.path().join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, "x").unwrap();
        }
        let files = read_v1_files(tmp.path(), &tmp.path().join("contracts")).unwrap();
        let names: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
        assert_eq!(names, ["a.veld", "m/b.veld"]);
    }
}
