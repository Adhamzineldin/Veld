//! `veld setup`: wires every generated target into the app that consumes it.
//!
//! * **TypeScript** (`node`, `typescript` targets): the generated directory is an npm package
//!   (`@veld/<target>`, see `package.json` in the output). Setup adds a `file:` dependency on it
//!   to the app's `package.json`. A `file:` dependency is real package resolution (npm links the
//!   directory into `node_modules`), so it works the same for `tsc`, Node through `tsx`, Vite
//!   and test runners. A `tsconfig` `paths` mapping would only help `tsc`: Node and the bundlers
//!   would each need their own alias, and compiled output would keep the unresolved alias.
//! * **Java**: Maven gets a `build-helper-maven-plugin` `add-source` execution, Gradle a
//!   `sourceSets.main.java.srcDir(...)` line, so the hand-written code imports the generated
//!   packages as ordinary Java packages.
//!
//! Edits are format preserving and idempotent. Anything unsafe to edit is printed as a snippet.

pub mod gradle;
pub mod json_edit;
pub mod pom;
pub mod xml;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use veld_project::config::{EmitterRef, Project, TargetSpec};

use json_edit::Change;

/// One file that setup changes, with what it was and what it becomes.
#[derive(Debug, Clone)]
pub struct FileEdit {
    pub original: String,
    pub text: String,
    /// Human-readable lines for the report.
    pub lines: Vec<String>,
}

/// An edit that is not safe to make automatically: the snippet to paste instead.
#[derive(Debug, Clone)]
pub struct Manual {
    pub target: String,
    pub file: PathBuf,
    pub reason: String,
    pub snippet: String,
}

/// Everything setup would do.
#[derive(Debug, Default)]
pub struct Plan {
    pub edits: BTreeMap<PathBuf, FileEdit>,
    pub manual: Vec<Manual>,
    /// Lines that are not edits: targets already wired, skipped, or missing a build file.
    pub notes: Vec<String>,
    /// App directories whose `package.json` changed (`npm install` is needed there).
    pub npm_dirs: Vec<PathBuf>,
}

impl Plan {
    /// Whether any file would change.
    #[must_use]
    pub fn changes(&self) -> bool {
        self.edits.values().any(|e| e.original != e.text)
    }

    fn text_of(&self, path: &Path) -> Result<String> {
        match self.edits.get(path) {
            Some(e) => Ok(e.text.clone()),
            None => fs::read_to_string(path)
                .with_context(|| format!("cannot read `{}`", path.display())),
        }
    }

    fn record(&mut self, path: &Path, new_text: String, line: String) -> Result<()> {
        let original = match self.edits.get(path) {
            Some(e) => e.original.clone(),
            None => self.text_of(path)?,
        };
        let entry = self.edits.entry(path.to_path_buf()).or_insert(FileEdit {
            original,
            text: String::new(),
            lines: Vec::new(),
        });
        entry.text = new_text;
        entry.lines.push(line);
        Ok(())
    }
}

/// `path` relative to `base`, with `/` separators (both absolute and normalised).
#[must_use]
pub fn relative(base: &Path, path: &Path) -> String {
    let a: Vec<Component> = base.components().collect();
    let b: Vec<Component> = path.components().collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = vec!["..".to_string(); a.len() - common];
    parts.extend(
        b[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// The directory of the app consuming `target`: its `app` key, else the nearest ancestor of
/// `out` (up to the project root) holding one of `markers`.
pub(crate) fn app_dir(project: &Project, target: &TargetSpec, markers: &[&str]) -> Option<PathBuf> {
    if let Some(app) = &target.app {
        return Some(app.clone());
    }
    let start = target.out.parent()?;
    start
        .ancestors()
        .take_while(|d| d.starts_with(&project.root))
        .find(|d| markers.iter().any(|m| d.join(m).is_file()))
        .map(Path::to_path_buf)
}

fn display(project: &Project, path: &Path) -> String {
    relative(&project.root, path)
}

/// The npm package name of a TypeScript target.
fn package_name(target: &TargetSpec) -> String {
    match target.options.get("package-name") {
        Some(toml::Value::String(s)) => s.clone(),
        _ => crate::pipeline::default_package_name(&target.name),
    }
}

/// Plans the targets `only` names, or all of them.
pub fn plan(project: &Project, targets: &[&TargetSpec]) -> Result<Plan> {
    let mut plan = Plan::default();
    for target in targets {
        match &target.emitter {
            EmitterRef::Builtin(id) if id == "node" || id == "typescript" => {
                plan_npm(&mut plan, project, target)?;
            }
            EmitterRef::Builtin(id) if id == "java" => plan_java(&mut plan, project, target)?,
            EmitterRef::Builtin(_) | EmitterRef::Plugin(_) => plan.notes.push(format!(
                "{}: nothing to wire (setup knows the node, typescript and java emitters)",
                target.name
            )),
        }
    }
    Ok(plan)
}

fn plan_npm(plan: &mut Plan, project: &Project, target: &TargetSpec) -> Result<()> {
    let Some(app) = app_dir(project, target, &["package.json"]) else {
        plan.notes.push(format!(
            "{}: no package.json found above `{}`; create one there (npm init) or set `app = \"...\"` on the target",
            target.name,
            display(project, &target.out)
        ));
        return Ok(());
    };
    let manifest = app.join("package.json");
    if !manifest.is_file() {
        plan.notes.push(format!(
            "{}: `{}` does not exist",
            target.name,
            display(project, &manifest)
        ));
        return Ok(());
    }
    let name = package_name(target);
    let spec = format!("file:{}", relative(&app, &target.out));
    let text = plan.text_of(&manifest)?;
    let shown = display(project, &manifest);
    match json_edit::set_entry(&text, "dependencies", &name, &spec) {
        Ok((_, Change::Unchanged)) => plan.notes.push(format!(
            "{}: already wired ({shown} depends on {name})",
            target.name
        )),
        Ok((new_text, change)) => {
            let verb = match change {
                Change::Updated { from } => format!("~ (was \"{from}\")"),
                Change::Added | Change::Unchanged => "+".to_string(),
            };
            plan.record(
                &manifest,
                new_text,
                format!(
                    "{verb} dependencies[\"{name}\"] = \"{spec}\"  ({})",
                    target.name
                ),
            )?;
            if !plan.npm_dirs.contains(&app) {
                plan.npm_dirs.push(app);
            }
        }
        Err(e) => plan.manual.push(Manual {
            target: target.name.clone(),
            file: manifest,
            reason: e.to_string(),
            snippet: format!("\"dependencies\": {{\n  \"{name}\": \"{spec}\"\n}}"),
        }),
    }
    Ok(())
}

fn plan_java(plan: &mut Plan, project: &Project, target: &TargetSpec) -> Result<()> {
    let markers = ["pom.xml", "build.gradle.kts", "build.gradle"];
    let Some(app) = app_dir(project, target, &markers) else {
        plan.notes.push(format!(
            "{}: no pom.xml or build.gradle(.kts) found above `{}`; add the directory as a source root yourself or set `app = \"...\"`",
            target.name,
            display(project, &target.out)
        ));
        return Ok(());
    };
    let rel = relative(&app, &target.out);
    if rel == "src/main/java" {
        plan.notes.push(format!(
            "{}: `{rel}` is already the standard source root of `{}`; nothing to do",
            target.name,
            display(project, &app)
        ));
        return Ok(());
    }
    let id = format!("veld-{}", crate::pipeline::slug(&target.name));
    if let Some(file) = markers.iter().map(|m| app.join(m)).find(|f| f.is_file()) {
        let text = plan.text_of(&file)?;
        let shown = display(project, &file);
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned());
        let (result, snippet) = match name.as_deref() {
            Some("pom.xml") => (
                pom::add_source(&text, &id, &rel).map_err(|e| e.to_string()),
                pom::snippet(&id, &rel),
            ),
            kts => {
                let kts = kts == Some("build.gradle.kts");
                let snippet = gradle::snippet(&id, &rel, kts);
                if gradle::safe(&text) {
                    (Ok(gradle::add_source(&text, &id, &rel, kts)), snippet)
                } else {
                    (
                        Err("no JVM plugin found in the build file".to_string()),
                        snippet,
                    )
                }
            }
        };
        match result {
            Ok((_, Change::Unchanged)) => plan.notes.push(format!(
                "{}: already wired ({shown} has `{rel}`)",
                target.name
            )),
            Ok((new_text, change)) => {
                let verb = match change {
                    Change::Updated { from } => format!("~ (was `{from}`)"),
                    Change::Added | Change::Unchanged => "+".to_string(),
                };
                plan.record(
                    &file,
                    new_text,
                    format!("{verb} source root `{rel}`  ({})", target.name),
                )?;
            }
            Err(reason) => plan.manual.push(Manual {
                target: target.name.clone(),
                file,
                reason,
                snippet,
            }),
        }
    }
    Ok(())
}

/// Writes every changed file (atomically, one by one).
pub fn apply(plan: &Plan) -> Result<()> {
    for (path, edit) in &plan.edits {
        if edit.original != edit.text {
            crate::fsutil::atomic_write(path, edit.text.as_bytes())?;
        }
    }
    Ok(())
}

/// Number of targets (of `targets`) that `veld setup` would still change something for.
#[must_use]
pub fn unwired(project: &Project, targets: &[&TargetSpec]) -> usize {
    targets
        .iter()
        .filter(|t| plan(project, &[t]).map(|p| p.changes()).unwrap_or(false))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use veld_project::config::load_str;

    fn project(root: &Path, toml: &str) -> Project {
        load_str(root, toml).unwrap()
    }

    const NPM_TOML: &str = "[service]
name = \"hello\"
[server]
name = \"api\"
emitter = \"node\"
out = \"server/src/generated\"
[client]
name = \"web\"
emitter = \"typescript\"
out = \"web/src/generated\"
";

    #[test]
    fn relative_paths() {
        let b = Path::new("/a/b");
        assert_eq!(relative(b, Path::new("/a/b/c/d")), "c/d");
        assert_eq!(relative(b, Path::new("/a/x")), "../x");
        assert_eq!(relative(b, b), ".");
    }

    #[test]
    fn wires_each_target_into_the_nearest_package_json() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("server")).unwrap();
        fs::create_dir_all(root.join("web")).unwrap();
        fs::write(
            root.join("server/package.json"),
            "{\n  \"name\": \"s\"\n}\n",
        )
        .unwrap();
        fs::write(root.join("web/package.json"), "{\n  \"name\": \"w\"\n}\n").unwrap();
        let p = project(root, NPM_TOML);
        let targets: Vec<&TargetSpec> = p.targets.iter().collect();
        let plan1 = plan(&p, &targets).unwrap();
        assert_eq!(plan1.edits.len(), 2);
        let server = &plan1.edits[&root.join("server/package.json")];
        assert!(
            server
                .text
                .contains("\"@veld/api\": \"file:src/generated\"")
        );
        assert_eq!(unwired(&p, &targets), 2);
        apply(&plan1).unwrap();
        let again = plan(&p, &targets).unwrap();
        assert!(!again.changes());
        assert_eq!(again.notes.len(), 2);
        assert_eq!(unwired(&p, &targets), 0);
    }

    #[test]
    fn an_app_key_overrides_the_search_and_missing_package_json_is_a_note() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let p = project(root, NPM_TOML);
        let targets: Vec<&TargetSpec> = p.targets.iter().collect();
        let plan1 = plan(&p, &targets).unwrap();
        assert!(!plan1.changes());
        assert!(
            plan1.notes[0].contains("no package.json"),
            "{:?}",
            plan1.notes
        );

        fs::create_dir_all(root.join("apps/site")).unwrap();
        fs::write(root.join("apps/site/package.json"), "{}").unwrap();
        let text = NPM_TOML.replace(
            "out = \"web/src/generated\"",
            "out = \"web/src/generated\"\napp = \"apps/site\"",
        );
        let p = project(root, &text);
        let plan2 = plan(&p, &[&p.targets[1]]).unwrap();
        let edit = &plan2.edits[&root.join("apps/site/package.json")];
        assert!(
            edit.text.contains("\"file:../../web/src/generated\""),
            "{}",
            edit.text
        );
    }

    #[test]
    fn an_unparseable_package_json_gives_a_snippet_and_no_edit() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("server")).unwrap();
        fs::write(root.join("server/package.json"), "{ // comment\n}").unwrap();
        let p = project(root, NPM_TOML);
        let plan1 = plan(&p, &[&p.targets[0]]).unwrap();
        assert!(plan1.edits.is_empty());
        assert_eq!(plan1.manual.len(), 1);
        assert!(plan1.manual[0].snippet.contains("@veld/api"));
    }

    #[test]
    fn package_name_option_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("server")).unwrap();
        fs::write(root.join("server/package.json"), "{}").unwrap();
        let text = NPM_TOML.replace(
            "out = \"server/src/generated\"
",
            "out = \"server/src/generated\"
[server.options]
package-name = \"@acme/api\"
",
        );
        let p = project(root, &text);
        let plan1 = plan(&p, &[&p.targets[0]]).unwrap();
        let edit = &plan1.edits[&root.join("server/package.json")];
        assert!(
            edit.text.contains("\"@acme/api\": \"file:src/generated\""),
            "{}",
            edit.text
        );
    }

    const JAVA_TOML: &str = "[service]
name = \"svc\"
[server]
name = \"api\"
emitter = \"java\"
out = \"svc/gen/java\"
";

    #[test]
    fn maven_is_edited_and_a_standard_source_root_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("svc")).unwrap();
        fs::write(
            root.join("svc/pom.xml"),
            "<project>\n  <artifactId>x</artifactId>\n</project>\n",
        )
        .unwrap();
        let p = project(root, JAVA_TOML);
        let plan1 = plan(&p, &[&p.targets[0]]).unwrap();
        let edit = &plan1.edits[&root.join("svc/pom.xml")];
        assert!(edit.text.contains("<source>gen/java</source>"));
        assert!(edit.text.contains("<id>veld-api</id>"));
        apply(&plan1).unwrap();
        assert!(!plan(&p, &[&p.targets[0]]).unwrap().changes());

        let std = JAVA_TOML.replace("svc/gen/java", "svc/src/main/java");
        let p = project(root, &std);
        let plan2 = plan(&p, &[&p.targets[0]]).unwrap();
        assert!(!plan2.changes() && plan2.notes[0].contains("standard source root"));
    }

    #[test]
    fn gradle_is_edited_or_a_snippet_is_printed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("svc")).unwrap();
        fs::write(root.join("svc/build.gradle.kts"), "plugins { java }\n").unwrap();
        let p = project(root, JAVA_TOML);
        let plan1 = plan(&p, &[&p.targets[0]]).unwrap();
        let edit = &plan1.edits[&root.join("svc/build.gradle.kts")];
        assert!(
            edit.text
                .contains("sourceSets[\"main\"].java.srcDir(\"gen/java\")")
        );

        fs::write(
            root.join("svc/build.gradle.kts"),
            "rootProject.name = \"x\"\n",
        )
        .unwrap();
        let plan2 = plan(&p, &[&p.targets[0]]).unwrap();
        assert!(plan2.edits.is_empty());
        assert!(plan2.manual[0].snippet.contains("veld:begin veld-api"));
    }
}
