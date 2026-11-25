//! Parity: for every example under `examples/` and every website playground preset, the files
//! the wasm engine generates must be BYTE-IDENTICAL to what the real `veld` CLI writes for the
//! same target. The cases run here natively; each verified case is also dumped (inputs plus
//! the SHA-256 of every expected file) to `target/veld-wasm-parity/cases.json`, which
//! `website/scripts/test-wasm.mjs` replays against the built `.wasm`, so the compiled
//! module is held to the same bytes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use veld_project::config::{self, EmitterRef, Role};
use veld_wasm::api::{self, PackageIn, ProjectIn, SourceIn, TargetIn};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

/// The  binary, built (or found up to date) by cargo and located from its JSON messages.
fn cli() -> PathBuf {
    static BIN: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BIN.get_or_init(|| {
        let out = Command::new(env!("CARGO"))
            .args(["build", "-p", "veld", "--message-format=json"])
            .current_dir(repo())
            .output()
            .unwrap();
        assert!(out.status.success(), "cannot build the veld CLI");
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|m| m["target"]["name"] == "veld" && m["target"]["kind"][0] == "bin")
            .find_map(|m| m["executable"].as_str().map(PathBuf::from))
            .expect("cargo reported no veld executable")
    })
    .clone()
}

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| repo().join("target"), PathBuf::from)
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// A case: contracts, the project settings, and the targets to compare.
struct Case {
    name: String,
    files: Vec<SourceIn>,
    project: ProjectIn,
    /// `(target, out dir relative to the project)`
    targets: Vec<(TargetIn, String)>,
    /// A directory holding `veld.toml` and the contracts, ready for the CLI.
    dir: tempfile::TempDir,
}

fn copy_contracts(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap().flatten() {
        let p = entry.path();
        let dest = to.join(entry.file_name());
        if p.is_dir() {
            fs::create_dir_all(&dest).unwrap();
            copy_contracts(&p, &dest);
        } else if p.extension().is_some_and(|e| e == "veld") {
            fs::copy(&p, &dest).unwrap();
        }
    }
}

/// Every `veld.toml` under `dir`, as `(path relative to `root` with /, text)`, sorted.
fn toml_files(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let mut entries: Vec<_> = fs::read_dir(dir).unwrap().flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if !matches!(name.as_str(), "node_modules" | "target" | "dist" | ".veld") {
                toml_files(root, &p, out);
            }
        } else if name == "veld.toml" {
            let rel = p
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, fs::read_to_string(&p).unwrap()));
        }
    }
}

/// The targets of a project as the CLI resolved them, in the form the wasm API takes, with each
/// `out` relative to the workspace root.
fn cli_targets(project: &config::Project) -> Vec<(TargetIn, String)> {
    project
        .targets
        .iter()
        .map(|t| {
            let EmitterRef::Builtin(emitter) = &t.emitter else {
                panic!("plugin target in an example")
            };
            let options = serde_json::to_value(&t.options)
                .unwrap()
                .as_object()
                .cloned()
                .unwrap_or_default();
            let target = TargetIn {
                name: Some(t.name.clone()),
                emitter: emitter.clone(),
                role: if t.role == Role::Server {
                    "server"
                } else {
                    "client"
                }
                .into(),
                framework: t.framework.clone(),
                options,
                service: t.service.clone(),
                services: (t.role == Role::Client).then(|| t.services.clone()),
                clients: Some(t.clients),
            };
            let out = t.out.strip_prefix(&project.root).unwrap();
            (target, out.to_string_lossy().replace('\\', "/"))
        })
        .collect()
}

/// An example project exactly as its `veld.toml` files declare it. The CLI resolves the workspace
/// from the disk; the wasm API resolves the same files as virtual files. They must agree, and
/// the case runs the wasm side's answer against the CLI's output.
fn example_case(name: &str, root: &Path) -> Case {
    let project = config::load(&root.join("veld.toml")).unwrap();
    let mut tomls = Vec::new();
    toml_files(root, root, &mut tomls);
    let dir = tempfile::tempdir().unwrap();
    for (rel, text) in &tomls {
        let path = dir.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    let mut files = Vec::new();
    for pkg in &project.packages {
        let rel = pkg.root.strip_prefix(&project.root).unwrap();
        fs::create_dir_all(dir.path().join(rel)).unwrap();
        copy_contracts(&pkg.root, &dir.path().join(rel));
        for (path, text) in config::collect_sources(pkg).unwrap() {
            files.push(SourceIn {
                path: format!("{}/{path}", pkg.name),
                text,
            });
        }
    }
    let sources: Vec<SourceIn> = tomls
        .iter()
        .map(|(path, text)| SourceIn {
            path: path.clone(),
            text: text.clone(),
        })
        .collect();
    let resolved = api::workspace(&sources);
    assert!(resolved.ok, "{name}: {:?}", resolved.diagnostics);
    let wasm_targets: Vec<(TargetIn, String)> =
        resolved.targets.into_iter().zip(resolved.outs).collect();
    assert_eq!(
        serde_json::to_value(&wasm_targets).unwrap(),
        serde_json::to_value(cli_targets(&project)).unwrap(),
        "{name}: the CLI and the wasm API resolve different targets"
    );
    let packages: Vec<PackageIn> = project
        .packages
        .iter()
        .map(|pkg| PackageIn {
            name: pkg.name.clone(),
            kind: if pkg.kind == veld_ir::PackageKind::Library {
                "library".into()
            } else {
                "service".into()
            },
            base_url: pkg.base_url.clone(),
            prefix: pkg.prefix.clone(),
            consumes: pkg.consumes.clone(),
        })
        .collect();
    assert_eq!(
        serde_json::to_value(&resolved.project).unwrap(),
        serde_json::to_value(ProjectIn {
            packages: packages.clone()
        })
        .unwrap(),
        "{name}: the CLI and the wasm API resolve different packages"
    );
    Case {
        name: name.into(),
        files,
        project: resolved.project,
        targets: wasm_targets,
        dir,
    }
}

/// Every framework of every generator from `describe()`, for each service where it needs one.
fn matrix(project: &ProjectIn) -> Vec<TargetIn> {
    let services: Vec<&PackageIn> = project
        .packages
        .iter()
        .filter(|p| p.kind != "library")
        .collect();
    let mut out = Vec::new();
    for e in api::describe().emitters {
        for role in &e.roles {
            let frameworks = &e.frameworks[role];
            let names: Vec<Option<String>> = if frameworks.is_empty() {
                vec![None]
            } else {
                frameworks.iter().map(|f| Some(f.id.clone())).collect()
            };
            for fw in names {
                let mut base = TargetIn {
                    emitter: e.id.clone(),
                    role: role.clone(),
                    framework: fw,
                    ..TargetIn::default()
                };
                if e.id == "typescript" && base.framework.as_deref() == Some("react") {
                    let mut q = base.clone();
                    q.options.insert("query-options".into(), json!(true));
                    out.push(q);
                }
                if role == "server" {
                    for s in &services {
                        base.service = Some(s.name.clone());
                        out.push(base.clone());
                    }
                } else {
                    out.push(base);
                }
            }
        }
    }
    out
}

/// The target tables of one member, as `veld.toml` text; `out` is written relative to the member.
fn target_tables(targets: &[&(TargetIn, String)]) -> String {
    let mut t = String::new();
    for (target, out) in targets {
        let table = &target.role;
        t.push_str(&format!("[[{table}]]\n"));
        t.push_str(&format!("name = \"{}\"\n", target.name.as_ref().unwrap()));
        t.push_str(&format!("emitter = \"{}\"\n", target.emitter));
        if let Some(f) = &target.framework {
            t.push_str(&format!("framework = \"{f}\"\n"));
        }
        t.push_str(&format!("out = \"../{out}\"\n"));
        if !target.options.is_empty() {
            t.push_str(&format!("[{table}.options]\n"));
            for (k, v) in &target.options {
                t.push_str(&format!("{k} = {v}\n"));
            }
        }
        t.push('\n');
    }
    t
}

/// The `veld.toml` files of a preset as a workspace: a root, a member per package (its contracts
/// are the member directory) and an app `clients` holding every client target.
fn toml_for(project: &ProjectIn, targets: &[(TargetIn, String)]) -> Vec<(String, String)> {
    let mut members: Vec<String> = project
        .packages
        .iter()
        .map(|p| format!("\"{}\"", p.name))
        .collect();
    members.push("\"clients\"".into());
    let mut files = vec![(
        "veld.toml".to_string(),
        format!(
            "[workspace]\nname = \"playground\"\nmembers = [{}]\n",
            members.join(", ")
        ),
    )];
    for p in &project.packages {
        let table = if p.kind == "library" {
            "library"
        } else {
            "service"
        };
        let mut t = format!("[{table}]\ncontracts = \".\"\n");
        if let Some(u) = &p.base_url {
            t.push_str(&format!("base-url = \"{u}\"\n"));
        }
        if !p.consumes.is_empty() {
            let list: Vec<String> = p.consumes.iter().map(|c| format!("\"{c}\"")).collect();
            t.push_str(&format!("consumes = [{}]\n", list.join(", ")));
        }
        t.push('\n');
        // A server target with no `service` (a preset's default) serves the only service.
        let services: Vec<&str> = project
            .packages
            .iter()
            .filter(|q| q.kind != "library")
            .map(|q| q.name.as_str())
            .collect();
        let only = (services.len() == 1).then(|| services[0]);
        let mine: Vec<&(TargetIn, String)> = targets
            .iter()
            .filter(|(t, _)| {
                t.role == "server" && t.service.as_deref().or(only) == Some(p.name.as_str())
            })
            .collect();
        t.push_str(&target_tables(&mine));
        files.push((format!("{}/veld.toml", p.name), t));
    }
    let clients: Vec<&(TargetIn, String)> =
        targets.iter().filter(|(t, _)| t.role == "client").collect();
    files.push((
        "clients/veld.toml".to_string(),
        format!("[app]\n\n{}", target_tables(&clients)),
    ));
    files
}

/// A playground preset: contracts on disk plus `preset.json`, run against the whole matrix.
fn preset_case(id: &str, root: &Path) -> Case {
    let meta: Value = serde_json::from_slice(&fs::read(root.join("preset.json")).unwrap()).unwrap();
    let project: ProjectIn = serde_json::from_value(meta["project"].clone()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    for p in &project.packages {
        let from = root.join(&p.name);
        fs::create_dir_all(dir.path().join(&p.name)).unwrap();
        copy_contracts(&from, &dir.path().join(&p.name));
        collect(&from, &p.name, &from, &mut files);
    }
    let mut targets: Vec<(TargetIn, String)> = matrix(&project)
        .into_iter()
        .enumerate()
        .map(|(i, mut t)| {
            t.name = Some(format!("t{i}"));
            (t, format!("out/t{i}"))
        })
        .collect();
    // Match the browser's preset target, including its implicit package name and framework.
    let mut default: TargetIn = serde_json::from_value(meta["target"].clone()).unwrap();
    default.name = Some(format!("{}-{}", default.role, default.emitter));
    targets.push((default, "out/default".into()));
    for (rel, text) in toml_for(&project, &targets) {
        let path = dir.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    Case {
        name: format!("preset:{id}"),
        files,
        project,
        targets,
        dir,
    }
}

fn collect(root: &Path, pkg: &str, dir: &Path, out: &mut Vec<SourceIn>) {
    let mut entries: Vec<_> = fs::read_dir(dir).unwrap().flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            collect(root, pkg, &p, out);
        } else if p.extension().is_some_and(|x| x == "veld") {
            let rel = p
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.push(SourceIn {
                path: format!("{pkg}/{rel}"),
                text: fs::read_to_string(&p).unwrap(),
            });
        }
    }
}

fn read_tree(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            read_tree(root, &p, out);
        } else {
            let rel = p
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, fs::read(&p).unwrap());
        }
    }
}

/// Runs the CLI for one target. `None` when the CLI refuses the target.
fn cli_files(case: &Case, target: &TargetIn, out: &str) -> Option<BTreeMap<String, Vec<u8>>> {
    let status = Command::new(cli())
        .args(["generate", target.name.as_deref().unwrap()])
        .current_dir(case.dir.path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    if !status.status.success() {
        return None;
    }
    let root = case.dir.path().join(out);
    let mut files = BTreeMap::new();
    read_tree(&root, &root, &mut files);
    files.retain(|k, _| !k.starts_with(".veld-"));
    Some(files)
}

/// Compares one target; returns the dump entry.
fn check(case: &Case, target: &TargetIn, out: &str) -> Value {
    let label = format!("{} / {}", case.name, target.name.as_deref().unwrap());
    let wasm = api::generate(&case.files, target, &case.project);
    let expected = cli_files(case, target, out);
    let Some(expected) = expected else {
        assert!(
            !wasm.ok,
            "{label}: the CLI refused the target but wasm generated it"
        );
        return json!({ "name": label, "ok": false, "files": case.files, "project": case.project, "target": target });
    };
    assert!(
        wasm.ok,
        "{label}: the CLI generated but wasm failed: {:?}",
        wasm.diagnostics
    );
    let got: BTreeMap<&str, &str> = wasm
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.text.as_str()))
        .collect();
    let want: Vec<&str> = expected.keys().map(String::as_str).collect();
    let have: Vec<&str> = got.keys().copied().collect();
    assert_eq!(have, want, "{label}: the file lists differ");
    let mut hashes = BTreeMap::new();
    for (path, bytes) in &expected {
        assert_eq!(
            got[path.as_str()].as_bytes(),
            bytes.as_slice(),
            "{label}: {path} differs"
        );
        hashes.insert(path.clone(), sha(bytes));
    }
    json!({ "name": label, "ok": true, "files": case.files, "project": case.project, "target": target, "expected": hashes })
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    let mut examples: Vec<_> = fs::read_dir(repo().join("examples"))
        .unwrap()
        .flatten()
        .collect();
    examples.sort_by_key(|e| e.file_name());
    for e in examples
        .into_iter()
        .filter(|e| e.path().join("veld.toml").exists())
    {
        cases.push(example_case(
            &format!("example:{}", e.file_name().to_string_lossy()),
            &e.path(),
        ));
    }
    let presets = repo().join("website/src/playground/presets");
    let mut dirs: Vec<_> = fs::read_dir(presets).unwrap().flatten().collect();
    dirs.sort_by_key(|e| e.file_name());
    for e in dirs.into_iter().filter(|e| e.path().is_dir()) {
        cases.push(preset_case(&e.file_name().to_string_lossy(), &e.path()));
    }
    cases
}

#[test]
fn wasm_output_is_byte_identical_to_the_cli() {
    let mut dump = Vec::new();
    let mut compared = 0;
    for case in cases() {
        assert!(
            api::compile(&case.files, &case.project).ok,
            "{} does not compile",
            case.name
        );
        for (target, out) in &case.targets {
            let entry = check(&case, target, out);
            if case.name.starts_with("example:") || out == "out/default" {
                assert_eq!(
                    entry["ok"], true,
                    "{}: the CLI must generate this example or preset default",
                    entry["name"]
                );
            }
            compared += usize::from(entry["ok"] == true);
            dump.push(entry);
        }
    }
    assert!(compared >= 20, "only {compared} targets were compared");
    let dir = target_dir().join("veld-wasm-parity");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("cases.json"), serde_json::to_vec(&dump).unwrap()).unwrap();
    eprintln!("parity: {compared} targets byte-identical to the CLI");
}
