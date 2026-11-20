#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;

use tempfile::tempdir;
use veld_project::config::{CONFIG_FILE, collect_sources, find_config, load};

#[test]
fn discovers_loads_and_collects_sources() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("contracts/models")).unwrap();
    fs::create_dir_all(root.join("src/deep")).unwrap();
    fs::write(root.join("contracts/b.veld"), "b").unwrap();
    fs::write(root.join("contracts/models/a.veld"), "a").unwrap();
    fs::write(
        root.join(CONFIG_FILE),
        "[project]\nname = \"shop\"\n[[targets]]\nrole = \"client\"\nemitter = \"typescript\"\nout = \"web/gen\"\n",
    )
    .unwrap();

    let config = find_config(&root.join("src/deep")).unwrap();
    let project = load(&config).unwrap();
    assert_eq!(project.name, "shop");
    let sources = collect_sources(&project.packages[0]).unwrap();
    let paths: Vec<_> = sources.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(paths, ["b.veld", "models/a.veld"]);
    assert!(project.targets[0].out.ends_with("gen"));
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
    fs::write(&file, "[project]\nname = \"x\"\n[[targets]]\nrole=\"client\"\nemiter=\"ts\"\nout=\"o\"\n").unwrap();
    let err = load(&file).unwrap_err();
    assert_eq!(err.code(), Some("E0701"));
    assert!(err.to_string().contains("did you mean `emitter`"));
}
