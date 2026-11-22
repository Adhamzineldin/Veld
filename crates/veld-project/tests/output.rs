#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;

use tempfile::tempdir;
use veld_project::output::{PlanAction, WriteOptions, clean, write};
use veld_project::{FileSet, FileSetError};

const MANIFEST: &str = ".veld-manifest.json";

fn set(items: &[(&str, &str)]) -> FileSet {
    let mut fs = FileSet::new();
    for (p, c) in items {
        fs.add(*p, *c).unwrap();
    }
    fs
}

fn opts() -> WriteOptions {
    WriteOptions::default()
}

fn read(out: &Path, rel: &str) -> String {
    fs::read_to_string(out.join(rel)).unwrap()
}

/// Every entry in `out`, as sorted relative `/` paths (dirs end with `/`).
fn tree(out: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, acc: &mut Vec<String>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let rel = p
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if p.is_dir() {
                acc.push(format!("{rel}/"));
                walk(base, &p, acc);
            } else {
                acc.push(rel);
            }
        }
    }
    let mut acc = Vec::new();
    walk(out, out, &mut acc);
    acc.sort();
    acc
}

#[test]
fn first_write_creates_files_and_manifest() {
    let tmp = tempdir().unwrap();
    let out = tmp.path().join("gen");
    let plan = write(
        &out,
        &set(&[("index.ts", "i"), ("types/a.ts", "a")]),
        &opts(),
    )
    .unwrap();
    assert_eq!(
        plan.paths_with(PlanAction::Create),
        ["index.ts", "types/a.ts"]
    );
    assert_eq!(read(&out, "types/a.ts"), "a");
    assert_eq!(
        tree(&out),
        [".veld-manifest.json", "index.ts", "types/", "types/a.ts"]
    );
    let manifest = read(&out, MANIFEST);
    assert!(manifest.contains("\"types/a.ts\""));
    assert!(manifest.ends_with('\n'));
}

#[test]
fn rewrite_removes_stale_files_and_prunes_empty_dirs() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    write(
        out,
        &set(&[("keep.ts", "k"), ("old/dir/gone.ts", "g"), ("upd.ts", "1")]),
        &opts(),
    )
    .unwrap();
    let plan = write(
        out,
        &set(&[("keep.ts", "k"), ("upd.ts", "2"), ("new.ts", "n")]),
        &opts(),
    )
    .unwrap();
    assert_eq!(plan.action_of("old/dir/gone.ts"), Some(PlanAction::Delete));
    assert_eq!(plan.action_of("keep.ts"), Some(PlanAction::Unchanged));
    assert_eq!(plan.action_of("upd.ts"), Some(PlanAction::Update));
    assert_eq!(plan.action_of("new.ts"), Some(PlanAction::Create));
    assert_eq!(
        tree(out),
        [".veld-manifest.json", "keep.ts", "new.ts", "upd.ts"]
    );
    assert_eq!(read(out, "upd.ts"), "2");
}

#[test]
fn user_files_next_to_generated_files_survive() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    write(out, &set(&[("sub/a.ts", "a")]), &opts()).unwrap();
    fs::write(out.join("sub/user.ts"), "mine").unwrap();
    fs::write(out.join("notes.md"), "notes").unwrap();
    write(out, &set(&[("b.ts", "b")]), &opts()).unwrap();
    assert_eq!(read(out, "sub/user.ts"), "mine");
    assert_eq!(read(out, "notes.md"), "notes");
    assert!(!out.join("sub/a.ts").exists());
}

#[test]
fn conflicting_unmanaged_file_is_refused_then_forced() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    fs::write(out.join("a.ts"), "user content").unwrap();
    let files = set(&[("a.ts", "generated"), ("b.ts", "b")]);
    let err = write(out, &files, &opts()).unwrap_err();
    assert!(err.to_string().contains("a.ts"), "{err}");
    assert_eq!(read(out, "a.ts"), "user content");
    assert!(!out.join("b.ts").exists(), "refusal must be all-or-nothing");
    assert!(!out.join(MANIFEST).exists());
    write(
        out,
        &files,
        &WriteOptions {
            force: true,
            ..opts()
        },
    )
    .unwrap();
    assert_eq!(read(out, "a.ts"), "generated");
}

#[test]
fn identical_unmanaged_file_is_adopted() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    fs::write(out.join("a.ts"), "same").unwrap();
    let plan = write(out, &set(&[("a.ts", "same")]), &opts()).unwrap();
    assert_eq!(plan.action_of("a.ts"), Some(PlanAction::Unchanged));
    assert!(read(out, MANIFEST).contains("a.ts"));
    // Now managed: a later generation may replace it.
    write(out, &set(&[("a.ts", "changed")]), &opts()).unwrap();
    assert_eq!(read(out, "a.ts"), "changed");
}

#[test]
fn traversal_and_bad_paths_are_rejected() {
    let mut files = FileSet::new();
    for bad in ["../x", "/abs", "C:/x", "a\\b", "a/../../x", "", "a\0b"] {
        assert!(
            matches!(files.add(bad, "x"), Err(FileSetError::InvalidPath { .. })),
            "{bad:?}"
        );
    }
    assert!(files.is_empty());
}

#[test]
fn tampered_manifest_cannot_delete_outside_out() {
    let tmp = tempdir().unwrap();
    let out = tmp.path().join("gen");
    fs::create_dir(&out).unwrap();
    let victim = tmp.path().join("victim.txt");
    fs::write(&victim, "precious").unwrap();
    fs::write(
        out.join(MANIFEST),
        r#"{"version":1,"generator":"x","files":{"../victim.txt":"00"}}"#,
    )
    .unwrap();
    assert!(write(&out, &set(&[("a", "a")]), &opts()).is_err());
    assert!(clean(&out, false).is_err());
    assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
}

#[test]
fn symlink_inside_out_is_refused() {
    let tmp = tempdir().unwrap();
    let out = tmp.path().join("gen");
    let elsewhere = tmp.path().join("elsewhere");
    fs::create_dir_all(&out).unwrap();
    fs::create_dir_all(&elsewhere).unwrap();
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&elsewhere, out.join("link")).is_ok();
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_dir(&elsewhere, out.join("link")).is_ok();
    if !linked {
        eprintln!(
            "SKIPPED symlink_inside_out_is_refused: creating symlinks is not permitted on this host"
        );
        return;
    }
    let err = write(&out, &set(&[("link/x.ts", "x")]), &opts()).unwrap_err();
    assert!(err.to_string().contains("symlink"), "{err}");
    assert!(!elsewhere.join("x.ts").exists());
}

#[test]
fn dry_run_writes_nothing() {
    let tmp = tempdir().unwrap();
    let out = tmp.path().join("gen");
    let dry = WriteOptions {
        dry_run: true,
        ..opts()
    };
    let plan = write(&out, &set(&[("a.ts", "a")]), &dry).unwrap();
    assert_eq!(plan.action_of("a.ts"), Some(PlanAction::Create));
    assert!(!out.exists(), "dry run must not even create the out dir");

    write(&out, &set(&[("a.ts", "a"), ("b.ts", "b")]), &opts()).unwrap();
    let before = tree(&out);
    let manifest = read(&out, MANIFEST);
    let plan = write(&out, &set(&[("a.ts", "changed")]), &dry).unwrap();
    assert_eq!(plan.action_of("a.ts"), Some(PlanAction::Update));
    assert_eq!(plan.action_of("b.ts"), Some(PlanAction::Delete));
    assert_eq!(tree(&out), before);
    assert_eq!(read(&out, "a.ts"), "a");
    assert_eq!(read(&out, MANIFEST), manifest);
}

#[test]
fn dry_run_reports_conflicts_without_failing() {
    let tmp = tempdir().unwrap();
    fs::write(tmp.path().join("a.ts"), "user").unwrap();
    let dry = WriteOptions {
        dry_run: true,
        ..opts()
    };
    let plan = write(tmp.path(), &set(&[("a.ts", "gen")]), &dry).unwrap();
    assert_eq!(plan.conflicts(), ["a.ts"]);
}

#[test]
fn clean_keeps_user_modified_files() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    write(
        out,
        &set(&[("a.ts", "a"), ("d/b.ts", "b"), ("c.ts", "c")]),
        &opts(),
    )
    .unwrap();
    fs::write(out.join("c.ts"), "edited by user").unwrap();
    fs::write(out.join("d/mine.ts"), "mine").unwrap();
    fs::remove_file(out.join("a.ts")).unwrap();
    let report = clean(out, false).unwrap();
    assert_eq!(report.removed, ["d/b.ts"]);
    assert_eq!(report.modified, ["c.ts"]);
    assert_eq!(report.missing, ["a.ts"]);
    assert_eq!(tree(out), [MANIFEST, "c.ts", "d/", "d/mine.ts"]);
    assert_eq!(read(out, "c.ts"), "edited by user");
}

#[test]
fn clean_dry_run_removes_nothing() {
    let tmp = tempdir().unwrap();
    write(tmp.path(), &set(&[("a.ts", "a")]), &opts()).unwrap();
    let before = tree(tmp.path());
    let report = clean(tmp.path(), true).unwrap();
    assert_eq!(report.removed, ["a.ts"]);
    assert_eq!(tree(tmp.path()), before);
}

#[test]
fn directory_in_the_way_leaves_previous_output_intact() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    write(out, &set(&[("a.ts", "old-a"), ("z.ts", "old-z")]), &opts()).unwrap();
    let manifest = read(out, MANIFEST);
    fs::create_dir(out.join("userdir")).unwrap();
    fs::write(out.join("userdir/f"), "f").unwrap();
    let files = set(&[("a.ts", "new-a"), ("userdir", "collides with a directory")]);
    assert!(write(out, &files, &opts()).is_err());
    assert_eq!(read(out, "a.ts"), "old-a");
    assert_eq!(read(out, "z.ts"), "old-z");
    assert_eq!(read(out, MANIFEST), manifest);
    assert_eq!(read(out, "userdir/f"), "f");
    assert_eq!(
        tree(out),
        [
            ".veld-manifest.json",
            "a.ts",
            "userdir/",
            "userdir/f",
            "z.ts"
        ]
    );
}

#[test]
fn file_where_a_parent_directory_is_needed_is_blocked_cleanly() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    fs::write(out.join("blocker"), "user file").unwrap();
    let files = set(&[("a.ts", "a"), ("blocker/child.ts", "c")]);
    assert!(write(out, &files, &opts()).is_err());
    assert_eq!(tree(out), ["blocker"]);
}

#[test]
fn no_staging_dir_is_left_behind() {
    let tmp = tempdir().unwrap();
    write(tmp.path(), &set(&[("a", "1")]), &opts()).unwrap();
    write(tmp.path(), &set(&[("a", "2")]), &opts()).unwrap();
    assert!(
        tree(tmp.path())
            .iter()
            .all(|p| !p.starts_with(".veld-staging-"))
    );
}

#[test]
fn manifest_is_deterministic() {
    let files = set(&[("z/last.ts", "z"), ("a.ts", "a"), ("m/mid.ts", "m")]);
    let a = tempdir().unwrap();
    let b = tempdir().unwrap();
    write(a.path(), &files, &opts()).unwrap();
    write(b.path(), &files, &opts()).unwrap();
    write(b.path(), &files, &opts()).unwrap();
    let (ma, mb) = (
        fs::read(a.path().join(MANIFEST)).unwrap(),
        fs::read(b.path().join(MANIFEST)).unwrap(),
    );
    assert_eq!(ma, mb);
    let text = String::from_utf8(ma).unwrap();
    assert!(!text.contains('\r'));
    assert!(text.find("a.ts").unwrap() < text.find("m/mid.ts").unwrap());
    assert!(text.find("m/mid.ts").unwrap() < text.find("z/last.ts").unwrap());
}

#[test]
fn insertion_order_does_not_matter() {
    let forward = set(&[("a", "1"), ("b", "2")]);
    let backward = set(&[("b", "2"), ("a", "1")]);
    let (x, y) = (tempdir().unwrap(), tempdir().unwrap());
    write(x.path(), &forward, &opts()).unwrap();
    write(y.path(), &backward, &opts()).unwrap();
    assert_eq!(
        fs::read(x.path().join(MANIFEST)).unwrap(),
        fs::read(y.path().join(MANIFEST)).unwrap()
    );
}

#[test]
fn never_removes_the_out_directory_itself() {
    let tmp = tempdir().unwrap();
    let out = tmp.path().join("gen");
    write(&out, &set(&[("only/a.ts", "a")]), &opts()).unwrap();
    write(&out, &FileSet::new(), &opts()).unwrap();
    assert!(out.is_dir());
    assert_eq!(tree(&out), [".veld-manifest.json"]);
    clean(&out, false).unwrap();
    assert!(out.is_dir());
    assert!(tree(&out).is_empty());
}

fn force() -> WriteOptions {
    WriteOptions {
        force: true,
        ..opts()
    }
}

#[test]
fn user_edited_generated_file_is_refused_not_overwritten() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    write(out, &set(&[("a.ts", "gen1"), ("b.ts", "b")]), &opts()).unwrap();
    fs::write(out.join("a.ts"), "my edit").unwrap();
    let files = set(&[("a.ts", "gen2"), ("b.ts", "b2")]);
    let err = write(out, &files, &opts()).unwrap_err().to_string();
    assert!(err.contains("you edited generated file"), "{err}");
    assert!(err.contains("a.ts") && err.contains("--force"), "{err}");
    assert_eq!(read(out, "a.ts"), "my edit");
    assert_eq!(read(out, "b.ts"), "b", "refusal is all-or-nothing");
    write(out, &files, &force()).unwrap();
    assert_eq!(read(out, "a.ts"), "gen2");
}

#[test]
fn user_edited_file_is_not_deleted_when_it_is_no_longer_generated() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    write(out, &set(&[("a.ts", "a"), ("old.ts", "old")]), &opts()).unwrap();
    fs::write(out.join("old.ts"), "my edit").unwrap();
    let files = set(&[("a.ts", "a")]);
    let err = write(out, &files, &opts()).unwrap_err().to_string();
    assert!(err.contains("you edited generated file"), "{err}");
    assert_eq!(read(out, "old.ts"), "my edit");
    write(out, &files, &force()).unwrap();
    assert!(!out.join("old.ts").exists());
}

#[test]
fn user_edit_is_refused_even_when_the_generated_content_is_unchanged() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    let files = set(&[("a.ts", "a")]);
    write(out, &files, &opts()).unwrap();
    fs::write(out.join("a.ts"), "my edit").unwrap();
    assert!(write(out, &files, &opts()).is_err());
    assert_eq!(read(out, "a.ts"), "my edit");
}

#[test]
fn dry_run_reports_user_modified_files() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    write(out, &set(&[("a.ts", "a")]), &opts()).unwrap();
    fs::write(out.join("a.ts"), "my edit").unwrap();
    let dry = WriteOptions {
        dry_run: true,
        ..opts()
    };
    let plan = write(out, &set(&[("a.ts", "new")]), &dry).unwrap();
    assert_eq!(plan.action_of("a.ts"), Some(PlanAction::Modified));
    assert_eq!(plan.conflicts(), ["a.ts"]);
    assert_eq!(read(out, "a.ts"), "my edit");
}

#[test]
fn file_kept_by_clean_is_still_reported_as_user_modified() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    write(out, &set(&[("a.ts", "a"), ("c.ts", "c")]), &opts()).unwrap();
    fs::write(out.join("c.ts"), "my edit").unwrap();
    let report = clean(out, false).unwrap();
    assert_eq!(report.modified, ["c.ts"]);
    let files = set(&[("a.ts", "a"), ("c.ts", "c")]);
    let err = write(out, &files, &opts()).unwrap_err().to_string();
    assert!(err.contains("you edited generated file"), "{err}");
    assert!(!err.contains("not created by Veld"), "{err}");
    assert_eq!(read(out, "c.ts"), "my edit");
    // The kept-files manifest lists only the kept file.
    let manifest = read(out, MANIFEST);
    assert!(
        manifest.contains("c.ts") && !manifest.contains("a.ts"),
        "{manifest}"
    );
    write(out, &files, &force()).unwrap();
    assert_eq!(read(out, "c.ts"), "c");
}

#[test]
fn clean_that_keeps_nothing_leaves_no_manifest() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    write(out, &set(&[("a.ts", "a")]), &opts()).unwrap();
    clean(out, false).unwrap();
    assert!(!out.join(MANIFEST).exists());
}

#[test]
fn stale_staging_dirs_are_removed_at_the_start_of_a_write() {
    let tmp = tempdir().unwrap();
    let out = tmp.path();
    let stale = out.join(".veld-staging-dead-1-0");
    fs::create_dir_all(stale.join("files/sub")).unwrap();
    fs::write(stale.join("files/sub/x"), "x").unwrap();
    fs::write(out.join("user.txt"), "mine").unwrap();
    write(out, &set(&[("a.ts", "a")]), &opts()).unwrap();
    assert_eq!(tree(out), [MANIFEST, "a.ts", "user.txt"]);
    // A no-op write cleans up too, and a dry run touches nothing.
    fs::create_dir_all(out.join(".veld-staging-dead-2-0")).unwrap();
    let dry = WriteOptions {
        dry_run: true,
        ..opts()
    };
    write(out, &set(&[("a.ts", "a")]), &dry).unwrap();
    assert!(out.join(".veld-staging-dead-2-0").exists());
    write(out, &set(&[("a.ts", "a")]), &opts()).unwrap();
    assert!(!out.join(".veld-staging-dead-2-0").exists());
}
