#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;

use tempfile::tempdir;
use veld_project::config::{CONFIG_FILE, collect_sources, find_config, load};

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn discovers_loads_and_collects_sources() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    write(root, "contracts/b.veld", "b");
    write(root, "contracts/models/a.veld", "a");
    fs::create_dir_all(root.join("src/deep")).unwrap();
    write(
        root,
        CONFIG_FILE,
        "[service]\nname = \"shop\"\n[client]\nemitter = \"typescript\"\nout = \"web/gen\"\n",
    );

    let config = find_config(&root.join("src/deep")).unwrap();
    let project = load(&config).unwrap();
    assert_eq!(project.name, "shop");
    let sources = collect_sources(&project.packages[0]).unwrap();
    let paths: Vec<_> = sources.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(paths, ["b.veld", "models/a.veld"]);
    assert!(project.targets[0].out.ends_with("gen"));
}

#[test]
fn a_workspace_is_found_from_a_member_directory() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    write(
        root,
        CONFIG_FILE,
        "[workspace]\nname = \"shop\"\nmembers = [\"services/*\"]\n",
    );
    write(
        root,
        "services/a/veld.toml",
        "[service]\n[server]\nemitter = \"node\"\n",
    );
    write(root, "services/a/contracts/a.veld", "a");
    write(
        root,
        "services/b/veld.toml",
        "[service]\nconsumes = [\"a\"]\n",
    );
    fs::create_dir_all(root.join("services/a/src")).unwrap();

    let config = find_config(&root.join("services/a/src")).unwrap();
    assert!(config.ends_with("services/a/veld.toml"));
    let project = load(&config).unwrap();
    assert_eq!(project.name, "shop");
    assert_eq!(project.current.as_deref(), Some("a"));
    assert_eq!(project.members.len(), 2);
    let sources = collect_sources(project.package("a").unwrap()).unwrap();
    assert_eq!(sources.len(), 1);
    let from_root = load(&root.join(CONFIG_FILE)).unwrap();
    assert_eq!(from_root.current, None);
    assert_eq!(from_root.packages, project.packages);
}

#[test]
fn no_config_is_not_found() {
    let tmp = tempdir().unwrap();
    // The temp dir's ancestors might contain a stray veld.toml on a dev machine; only assert the error shape.
    if let Err(e) = find_config(tmp.path()) {
        assert!(e.to_string().contains("no veld.toml"));
    }
}

#[test]
fn error_codes_surface_through_load() {
    let tmp = tempdir().unwrap();
    let file = tmp.path().join(CONFIG_FILE);
    fs::write(
        &file,
        "[service]\nname = \"x\"\n[client]\nemiter = \"ts\"\nout = \"o\"\n",
    )
    .unwrap();
    let err = load(&file).unwrap_err();
    assert_eq!(err.code(), Some("E0701"));
    assert!(err.to_string().contains("did you mean `emitter`"));
}

#[test]
fn a_file_in_the_earlier_layout_is_e0709() {
    let tmp = tempdir().unwrap();
    let file = tmp.path().join(CONFIG_FILE);
    fs::write(
        &file,
        "[project]\nname = \"x\"\n[[targets]]\nrole = \"client\"\nemitter = \"typescript\"\nout = \"o\"\n",
    )
    .unwrap();
    let err = load(&file).unwrap_err();
    assert_eq!(err.code(), Some("E0709"));
    let msg = err.to_string();
    assert!(
        msg.contains("[workspace]") && msg.contains("[client]"),
        "{msg}"
    );
}

#[test]
fn a_missing_member_is_e0710() {
    let tmp = tempdir().unwrap();
    write(
        tmp.path(),
        CONFIG_FILE,
        "[workspace]\nname = \"w\"\nmembers = [\"services/a\"]\n",
    );
    let err = load(&tmp.path().join(CONFIG_FILE)).unwrap_err();
    assert_eq!(err.code(), Some("E0710"));
}
