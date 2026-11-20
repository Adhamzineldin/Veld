//! `cargo xtask new-emitter <name> [--roles server,client]`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail};

use crate::edit;
use crate::templates;

/// How long `registry.rs` must have been untouched before the scaffolder edits it, so that it
/// never edits a file another contributor is in the middle of changing.
const SETTLE: Duration = Duration::from_secs(2);

/// What to create.
#[derive(Debug, Clone)]
pub struct NewEmitter {
    pub name: String,
    pub roles: Vec<String>,
}

impl NewEmitter {
    pub fn parse(name: &str, roles: Option<&str>) -> Result<Self> {
        let valid = name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        if !valid {
            bail!("`{name}` is not a valid emitter name: use lowercase letters and digits, starting with a letter");
        }
        let roles: Vec<String> = roles.unwrap_or("server,client").split(',').map(|r| r.trim().to_string()).collect();
        if roles.iter().any(|r| r != "server" && r != "client") || roles.is_empty() {
            bail!("`--roles` takes `server`, `client` or `server,client`");
        }
        let mut unique = roles.clone();
        unique.sort();
        unique.dedup();
        Ok(Self { name: name.to_string(), roles: unique })
    }

    fn struct_name(&self) -> String {
        let mut c = self.name.chars();
        c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
    }

    fn render(&self, template: &str) -> String {
        let roles_list: Vec<String> =
            self.roles.iter().map(|r| format!("Role::{}", if r == "server" { "Server" } else { "Client" })).collect();
        template
            .replace("{{name}}", &self.name)
            .replace("{{Struct}}", &self.struct_name())
            .replace("{{roles}}", &roles_list.join(", "))
            .replace("{{roles_text}}", &format!("role {}", self.roles.join(" and ")))
    }
}

/// The repository root: the parent of the `xtask` crate.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_default()
}

struct Edits {
    root_toml: String,
    app_toml: String,
    registry: String,
    registry_original: String,
}

/// Creates the crate and registers it. Everything is validated before anything is written.
pub fn run(root: &Path, spec: &NewEmitter, check_settled: bool) -> Result<Vec<String>> {
    let dir = root.join("crates/emitters").join(&spec.name);
    if dir.exists() {
        bail!("`{}` already exists; nothing was changed", dir.display());
    }
    let registry_path = root.join("apps/veld/src/registry.rs");
    if check_settled {
        ensure_settled(&registry_path)?;
    }
    let edits = plan_edits(root, spec)?;
    let mut created = Vec::new();
    for (rel, template) in [
        ("Cargo.toml", templates::CARGO_TOML),
        ("src/lib.rs", templates::LIB_RS),
        ("tests/conformance.rs", templates::CONFORMANCE_RS),
        ("README.md", templates::README_MD),
    ] {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap_or(&dir)).with_context(|| format!("cannot create `{}`", path.display()))?;
        fs::write(&path, spec.render(template)).with_context(|| format!("cannot write `{}`", path.display()))?;
        created.push(format!("crates/emitters/{}/{rel}", spec.name));
    }
    apply(root, &edits, &registry_path)?;
    Ok(created)
}

fn plan_edits(root: &Path, spec: &NewEmitter) -> Result<Edits> {
    let read = |rel: &str| fs::read_to_string(root.join(rel)).with_context(|| format!("cannot read `{rel}`"));
    let package = format!("veld-emit-{}", spec.name);
    let registry_original = read("apps/veld/src/registry.rs")?;
    let root_toml = edit::insert_member(&read("Cargo.toml")?, &format!("crates/emitters/{}", spec.name))?;
    let root_toml = edit::insert_workspace_dep(&root_toml, &package, &format!("crates/emitters/{}", spec.name))?;
    Ok(Edits {
        root_toml,
        app_toml: edit::insert_app_dep(&read("apps/veld/Cargo.toml")?, &package)?,
        registry: edit::insert_registry(&registry_original, &spec.name, &spec.struct_name())?,
        registry_original,
    })
}

fn ensure_settled(path: &Path) -> Result<()> {
    let modified = fs::metadata(path).and_then(|m| m.modified()).with_context(|| format!("cannot stat `{}`", path.display()))?;
    if SystemTime::now().duration_since(modified).is_ok_and(|age| age < SETTLE) {
        bail!("`{}` was modified a moment ago, so someone may be editing it; retry in a few seconds", path.display());
    }
    Ok(())
}

fn apply(root: &Path, edits: &Edits, registry_path: &Path) -> Result<()> {
    write_atomic(&root.join("Cargo.toml"), &edits.root_toml)?;
    write_atomic(&root.join("apps/veld/Cargo.toml"), &edits.app_toml)?;
    // The registry is edited last, and only if nobody changed it since it was read.
    let now = fs::read_to_string(registry_path).with_context(|| format!("cannot read `{}`", registry_path.display()))?;
    if now != edits.registry_original {
        bail!("`{}` changed while the emitter was being created; add its line by hand (see the README)", registry_path.display());
    }
    write_atomic(registry_path, &edits.registry)
}

fn write_atomic(path: &Path, text: &str) -> Result<()> {
    if fs::read_to_string(path).is_ok_and(|old| old == text) {
        return Ok(());
    }
    let tmp = path.with_extension("xtask-tmp");
    fs::write(&tmp, text).with_context(|| format!("cannot write `{}`", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("cannot replace `{}`", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let w = |rel: &str, text: &str| {
            let p = tmp.path().join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        };
        w("Cargo.toml", "[workspace]\nmembers = [\n  \"crates/emitters/node\",\n  \"apps/veld\",\n]\n\n[workspace.dependencies]\nveld-emit-node = { path = \"crates/emitters/node\" }\n");
        w("apps/veld/Cargo.toml", "[dependencies]\nveld-emit-node.workspace = true\n");
        w("apps/veld/src/registry.rs", "use veld_codegen::DynGenerator;\nuse veld_emit_node::Node;\n\npub fn generators() -> &'static [&'static dyn DynGenerator] {\n    &[&Node]\n}\n");
        tmp
    }

    #[test]
    fn names_and_roles_are_validated() {
        assert!(NewEmitter::parse("kotlin", None).is_ok());
        for bad in ["", "Kotlin", "1kt", "kot-lin", "kot_lin"] {
            assert!(NewEmitter::parse(bad, None).is_err(), "{bad}");
        }
        assert_eq!(NewEmitter::parse("go", Some("client")).unwrap().roles, ["client"]);
        assert!(NewEmitter::parse("go", Some("both")).is_err());
    }

    #[test]
    fn creates_the_crate_registers_it_and_refuses_a_second_time() {
        let repo = fake_repo();
        let spec = NewEmitter::parse("kotlin", Some("client")).unwrap();
        let created = run(repo.path(), &spec, false).unwrap();
        assert_eq!(created.len(), 4);
        let dir = repo.path().join("crates/emitters/kotlin");
        let lib = fs::read_to_string(dir.join("src/lib.rs")).unwrap();
        assert!(lib.contains("pub struct Kotlin;") && lib.contains("&[Role::Client]") && lib.contains("Capabilities::default()"));
        assert!(!lib.contains("{{"), "no placeholder is left: {lib}");
        let cargo = fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("name = \"veld-emit-kotlin\""));
        let root = fs::read_to_string(repo.path().join("Cargo.toml")).unwrap();
        assert!(root.contains("\"crates/emitters/kotlin\",") && root.contains("veld-emit-kotlin = { path"));
        let registry = fs::read_to_string(repo.path().join("apps/veld/src/registry.rs")).unwrap();
        assert!(registry.contains("&Kotlin, // xtask:new-emitter kotlin") && registry.contains("use veld_emit_kotlin::Kotlin;"));
        assert!(fs::read_to_string(repo.path().join("apps/veld/Cargo.toml")).unwrap().contains("veld-emit-kotlin.workspace = true"));
        let before = fs::read_to_string(repo.path().join("Cargo.toml")).unwrap();
        let err = run(repo.path(), &spec, false).unwrap_err().to_string();
        assert!(err.contains("already exists"), "{err}");
        assert_eq!(before, fs::read_to_string(repo.path().join("Cargo.toml")).unwrap());
    }

    #[test]
    fn a_registry_that_is_being_edited_stops_the_scaffold_before_anything_is_written() {
        let repo = fake_repo();
        let registry = repo.path().join("apps/veld/src/registry.rs");
        fs::write(&registry, "pub fn generators() -> &'static [u8] {\n    &[\n").unwrap();
        let spec = NewEmitter::parse("kotlin", None).unwrap();
        assert!(run(repo.path(), &spec, false).is_err());
        assert!(!repo.path().join("crates/emitters/kotlin").exists());
        // A file touched a moment ago is also refused.
        fs::write(&registry, "fn generators() {}\n").unwrap();
        assert!(run(repo.path(), &spec, true).unwrap_err().to_string().contains("modified a moment ago"));
    }
}
