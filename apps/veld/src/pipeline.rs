//! The shared pipeline: load config, collect sources, compile, render diagnostics, check
//! capabilities, generate per target, write. Commands compose these steps.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use veld_codegen::{
    DynGenerator, FileSet, GenDiagnostic, GenRequest, Role, check_capabilities, resolve_profile,
};
use veld_compiler::{CompileInput, CompileOutput, PackageInput, SourceFile};
use veld_ir::{Ir, PackageKind, ServiceId};
use veld_plugin_host::{HostConfig, PluginGenerator};
use veld_project::config::{self, EmitterRef, PackageSpec, Project, TargetSpec};
use veld_project::output::{self, Plan, PlanAction, WriteOptions};

use crate::error::user;
use crate::registry;
use crate::ui::Ui;

// ---------------------------------------------------------------------------
// Load and compile
// ---------------------------------------------------------------------------

/// Resolves `--config` (a file, a directory, or nothing) to the `veld.toml` it names.
pub fn config_file(config: Option<&Path>) -> Result<PathBuf> {
    Ok(match config {
        Some(p) if p.is_file() => p.to_path_buf(),
        Some(p) => config::find_config(p)?,
        None => config::find_config(
            &std::env::current_dir().context("cannot read the current directory")?,
        )?,
    })
}

/// Resolves `--config` to a loaded project.
pub fn load_project(config: Option<&Path>) -> Result<Project> {
    Ok(config::load(&config_file(config)?)?)
}

/// A project that compiled without errors.
#[derive(Debug)]
pub struct Compiled {
    pub project: Project,
    pub ir: Ir,
}

/// Compiles `project` and prints its diagnostics. Returns `None` after printing the failure
/// summary when there are errors.
pub fn compile_or_report(ui: &Ui, project: Project) -> Result<Option<Compiled>> {
    let started = std::time::Instant::now();
    let spin = ui.spinner("Compiling contracts");
    let out = compile_project(&project);
    spin.finish();
    let out = out?;
    ui.debug(
        1,
        &format!(
            "compiled {} in {}",
            plural(project.packages.len(), "package"),
            crate::ui::render::elapsed(started.elapsed())
        ),
    );
    let counts = report_diagnostics(ui, &project, &out);
    match out.ir {
        Some(ir) if counts.errors == 0 => Ok(Some(Compiled { project, ir })),
        _ => {
            eprintln!("{}", failure_summary(ui, counts));
            Ok(None)
        }
    }
}

/// Reads every package's `.veld` files and compiles them together.
pub fn compile_project(project: &Project) -> Result<CompileOutput> {
    // Registry packages are importable only by the packages that depend on them directly.
    let rules = veld_compiler::AccessRules {
        allowed: project.registry_access()?,
    };
    let compile_input = compile_input(project)?;
    Ok(veld_compiler::compile_prefixed(
        &compile_input,
        Some(&rules),
        &project.prefixes(),
    ))
}

fn compile_input(project: &Project) -> Result<CompileInput> {
    compile_input_from(project, |pkg| Ok(config::collect_sources(pkg)?))
}

/// The compiler input for `project`, with each package's sources supplied by `read`
/// (the working tree normally, a git ref for `veld diff --against`).
pub fn compile_input_from(
    project: &Project,
    read: impl Fn(&PackageSpec) -> Result<Vec<(String, String)>>,
) -> Result<CompileInput> {
    let mut packages = Vec::new();
    let mut consumes = BTreeMap::new();
    for pkg in &project.packages {
        packages.push(package_input(pkg, read(pkg)?));
        if pkg.kind == PackageKind::Service && !pkg.consumes.is_empty() {
            consumes.insert(pkg.name.clone(), pkg.consumes.clone());
        }
    }
    for rp in project.registry_packages()? {
        let files = rp
            .sources
            .into_iter()
            .map(|(path, text)| SourceFile { path, text })
            .collect();
        packages.push(PackageInput {
            name: rp.name,
            kind: PackageKind::Registry,
            files,
            base_url: None,
        });
    }
    Ok(CompileInput { packages, consumes })
}

fn package_input(pkg: &PackageSpec, sources: Vec<(String, String)>) -> PackageInput {
    let files = sources
        .into_iter()
        .map(|(path, text)| SourceFile { path, text })
        .collect();
    PackageInput {
        name: pkg.name.clone(),
        kind: pkg.kind,
        files,
        base_url: pkg.base_url.clone(),
    }
}

// ---------------------------------------------------------------------------
// The IR lock file
// ---------------------------------------------------------------------------

/// Name of the IR snapshot that `veld generate` writes next to `veld.toml`.
pub const LOCK_FILE: &str = "veld.lock.ir.json";

pub fn lock_path(project: &Project) -> PathBuf {
    project.root.join(LOCK_FILE)
}

/// The previous IR snapshot (`veld.lock.ir.json`), if there is a readable one.
pub fn read_lock(project: &Project) -> Option<Ir> {
    let text = std::fs::read_to_string(lock_path(project)).ok()?;
    serde_json::from_str(&text).ok()
}

/// The lock file's exact bytes: pretty JSON with a trailing newline. Deterministic.
pub fn lock_text(ir: &Ir) -> Result<String> {
    let mut text = serde_json::to_string_pretty(ir).context("cannot serialize the IR")?;
    text.push('\n');
    Ok(text)
}

/// Writes the lock file unless it already holds the same bytes (so a clean tree stays clean).
pub fn write_lock(project: &Project, ir: &Ir) -> Result<()> {
    let path = lock_path(project);
    let text = lock_text(ir)?;
    if std::fs::read_to_string(&path).is_ok_and(|old| old == text) {
        return Ok(());
    }
    crate::fsutil::atomic_write(&path, text.as_bytes())
}

/// Counts of what a compile found, for the one-line summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    pub errors: usize,
    pub warnings: usize,
}

/// Prints all diagnostics (and config warnings) to stderr and counts them.
/// `--quiet` hides warnings but never errors.
pub fn report_diagnostics(ui: &Ui, project: &Project, out: &CompileOutput) -> Counts {
    report_diagnostics_in(ui, project, out, None).0
}

/// [`report_diagnostics`] limited to the files in `scope`. The second value is the number of
/// errors in other files, which are counted but not printed.
pub fn report_diagnostics_in(
    ui: &Ui,
    project: &Project,
    out: &CompileOutput,
    scope: Option<&Scope>,
) -> (Counts, usize) {
    let mut counts = Counts::default();
    let mut hidden = 0;
    for w in &project.warnings {
        ui.warn(&format!("[{}] {}", w.code, w.message));
    }
    for d in &out.diagnostics {
        let shown = scope.is_none_or(|s| {
            out.sources
                .path(d.primary.file)
                .is_some_and(|p| s.has_qualified(p))
        });
        if !shown {
            hidden += usize::from(d.is_error());
            continue;
        }
        if d.is_error() {
            counts.errors += 1;
        } else {
            counts.warnings += 1;
            if ui.quiet {
                continue;
            }
        }
        ui.diagnostic(&out.sources.render(d));
    }
    (counts, hidden)
}

/// `"1 error"`, `"2 warnings"`.
pub fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}

/// Failure summary such as `✗ 2 errors, 1 warning`.
pub fn failure_summary(ui: &Ui, c: Counts) -> String {
    crate::ui::render::fail(
        &ui.theme_err(),
        &format!(
            "{}, {}",
            plural(c.errors, "error"),
            plural(c.warnings, "warning")
        ),
    )
}

/// Success summary such as `✓ 3 services, 24 endpoints, 31 types`.
pub fn success_summary(ui: &Ui, ir: &Ir) -> String {
    let endpoints: usize = ir.services.iter().map(|s| s.endpoints.len()).sum();
    let types = ir
        .decls
        .iter()
        .filter(|d| ir.package(d.package).kind != PackageKind::Prelude)
        .count();
    crate::ui::render::ok(
        &ui.theme_out(),
        &format!(
            "{}, {}, {}",
            plural(ir.services.len(), "service"),
            plural(endpoints, "endpoint"),
            plural(types, "type")
        ),
    )
}

// ---------------------------------------------------------------------------
// Reporting scope (`veld check PATH...`)
// ---------------------------------------------------------------------------

/// The contract files a report is limited to.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// `<package>/<path in package>`, the form compiler diagnostics use.
    qualified: BTreeSet<String>,
    /// `<path in package>`, the form IR origins (and lint findings) use.
    relative: BTreeSet<String>,
}

impl Scope {
    /// Whether a compiler diagnostic path (`pkg/rel.veld`) is in scope.
    pub fn has_qualified(&self, path: &str) -> bool {
        self.qualified.contains(path)
    }

    /// Whether a package-relative path is in scope.
    pub fn has_relative(&self, path: &str) -> bool {
        self.relative.contains(path)
    }
}

/// Package-relative contract path to the name of the package that owns it (first wins), so a
/// finding that only knows the relative path can be printed as `<package>/<path>`.
pub fn file_packages(project: &Project) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for pkg in &project.packages {
        for (rel, _) in config::collect_sources(pkg).unwrap_or_default() {
            map.entry(rel).or_insert_with(|| pkg.name.clone());
        }
    }
    map
}
/// Resolves files and directories given on the command line to the project's contract files.
/// A path that matches no contract is an error, so a typo is never a silent pass.
pub fn scope_of(project: &Project, paths: &[PathBuf]) -> Result<Scope> {
    let wanted: Vec<PathBuf> = paths
        .iter()
        .map(|p| {
            std::fs::canonicalize(p)
                .map_err(|e| user(format!("cannot read `{}`: {e}", p.display())))
        })
        .collect::<Result<_, _>>()?;
    let mut scope = Scope::default();
    let mut matched = vec![false; wanted.len()];
    for pkg in &project.packages {
        let root = std::fs::canonicalize(&pkg.root).unwrap_or_else(|_| pkg.root.clone());
        for (rel, _) in config::collect_sources(pkg)? {
            let abs = root.join(&rel);
            for (i, w) in wanted.iter().enumerate() {
                if abs.starts_with(w) {
                    matched[i] = true;
                    scope.qualified.insert(format!("{}/{rel}", pkg.name));
                    scope.relative.insert(rel.clone());
                }
            }
        }
    }
    if let Some(i) = matched.iter().position(|m| !m) {
        return Err(user(format!(
            "`{}` is not a contract of this project (no .veld file under it belongs to a package)",
            paths[i].display()
        )));
    }
    Ok(scope)
}

// ---------------------------------------------------------------------------
// Generate
// ---------------------------------------------------------------------------

/// One target, resolved against the IR and ready to run.
pub struct Job<'a> {
    pub target: &'a TargetSpec,
    generator: Generator,
    role: Role,
    services: Vec<ServiceId>,
    consumes: Vec<ServiceId>,
    options: serde_json::Value,
}

/// A built-in generator, or a plugin that has already completed its handshake.
enum Generator {
    Builtin(&'static dyn DynGenerator),
    Plugin(Box<PluginGenerator>),
}

impl Generator {
    fn get(&self) -> &dyn DynGenerator {
        match self {
            Self::Builtin(g) => *g,
            Self::Plugin(p) => p.as_ref(),
        }
    }
}

/// Resolves an `emitter` to a generator. Plugins are started once here to read their
/// handshake (roles and capabilities); the hash pin is checked before that.
fn resolve_generator(project: &Project, target: &TargetSpec) -> Result<Generator, Vec<String>> {
    let name = &target.name;
    match &target.emitter {
        EmitterRef::Builtin(id) => registry::find(id).map(Generator::Builtin).ok_or_else(|| {
            vec![format!(
                "target `{name}`: unknown emitter `{id}` (built-in: {})",
                registry::ids().join(", ")
            )]
        }),
        EmitterRef::Plugin(p) => {
            let spec = project
                .plugins
                .iter()
                .find(|s| &s.name == p)
                .ok_or_else(|| {
                    vec![format!(
                        "target `{name}`: plugin `{p}` is not declared under `[plugins.{p}]`"
                    )]
                })?;
            PluginGenerator::connect(spec, HostConfig::new(project.root.clone()))
                .map(|g| Generator::Plugin(Box::new(g)))
                .map_err(|e| vec![format!("target `{name}`: error[{}]: {e}", e.code())])
        }
    }
}

fn role_of(r: config::Role) -> Role {
    match r {
        config::Role::Server => Role::Server,
        config::Role::Client => Role::Client,
    }
}

fn service_ids(ir: &Ir, packages: &[String]) -> Vec<ServiceId> {
    packages
        .iter()
        .flat_map(|p| ir.services_of_package(p).map(|s| s.id))
        .collect()
}

/// The targets to run: all of them, or the one called `only`.
pub fn select_targets<'a>(project: &'a Project, only: Option<&str>) -> Result<Vec<&'a TargetSpec>> {
    let targets: Vec<&TargetSpec> = project
        .targets
        .iter()
        .filter(|t| only.is_none_or(|name| t.name == name))
        .collect();
    if !targets.is_empty() {
        return Ok(targets);
    }
    Err(match only {
        Some(name) => {
            let known: Vec<&str> = project.targets.iter().map(|t| t.name.as_str()).collect();
            user(format!(
                "no target named `{name}` (known targets: {})",
                known.join(", ")
            ))
        }
        None => user("veld.toml defines no `[[targets]]`; add one (see `veld init`)"),
    })
}

/// Resolves one target: finds its generator and computes its services, consumes and options.
pub fn build_job<'a>(
    project: &Project,
    ir: &Ir,
    target: &'a TargetSpec,
) -> Result<Job<'a>, Vec<String>> {
    let name = &target.name;
    let generator = resolve_generator(project, target)?;
    let role = role_of(target.role);
    if !generator.get().roles().contains(&role) {
        let id = match &target.emitter {
            EmitterRef::Builtin(id) => id.clone(),
            EmitterRef::Plugin(p) => format!("plugin:{p}"),
        };
        return Err(vec![format!(
            "target `{name}`: emitter `{id}` cannot produce the `{}` role",
            role.name()
        )]);
    }
    let (services, consumes) = match role {
        Role::Server => server_services(project, ir, target)?,
        Role::Client => (service_ids(ir, &target.services), Vec::new()),
    };
    let options =
        job_options(target).map_err(|e| vec![format!("target `{name}`: invalid options: {e}")])?;
    Ok(Job {
        target,
        generator,
        role,
        services,
        consumes,
        options,
    })
}

/// A server serves its own service and, when `clients` is on, gets clients for its `consumes`.
fn server_services(
    project: &Project,
    ir: &Ir,
    target: &TargetSpec,
) -> Result<(Vec<ServiceId>, Vec<ServiceId>), Vec<String>> {
    let Some(svc) = target.service.as_deref() else {
        return Err(vec![format!(
            "target `{}`: a server target needs `service`",
            target.name
        )]);
    };
    let consumed = match project.package(svc) {
        Some(pkg) if target.clients => service_ids(ir, &pkg.consumes),
        _ => Vec::new(),
    };
    Ok((service_ids(ir, &[svc.to_string()]), consumed))
}

/// The npm name of a target's generated package: `@veld/<target name>`, made npm-safe.
/// A target name as a lowercase, URL-safe slug (`Orders_API` becomes `orders-api`).
pub fn slug(target: &str) -> String {
    let slug: String = target
        .chars()
        .map(|c| match c.to_ascii_lowercase() {
            c if c.is_ascii_alphanumeric() || c == '-' || c == '.' => c,
            _ => '-',
        })
        .collect();
    let slug = slug.trim_matches(|c| c == '-' || c == '.');
    if slug.is_empty() {
        "generated".to_string()
    } else {
        slug.to_string()
    }
}

/// The npm name of a target's generated package: `@veld/<target name>`, made npm-safe.
pub fn default_package_name(target: &str) -> String {
    format!("@veld/{}", slug(target))
}

/// The options handed to the generator. The built-in TypeScript emitters get `package-name`
/// defaulted to `@veld/<target name>` (the generator never sees the target name itself).
fn job_options(target: &TargetSpec) -> Result<serde_json::Value, serde_json::Error> {
    let ts =
        matches!(&target.emitter, EmitterRef::Builtin(id) if id == "node" || id == "typescript");
    if !ts {
        return if target.options.is_empty() {
            Ok(serde_json::Value::Null)
        } else {
            serde_json::to_value(&target.options)
        };
    }
    let mut options = target.options.clone();
    if !options.contains_key("package-name") {
        options.insert(
            "package-name".to_string(),
            toml::Value::String(default_package_name(&target.name)),
        );
    }
    serde_json::to_value(&options)
}

fn format_gen(d: &GenDiagnostic, target: &str) -> String {
    let at = d
        .origin
        .as_ref()
        .map(|o| format!("\n --> {}:{}:{}", o.file, o.line, o.col))
        .unwrap_or_default();
    format!("target `{target}`: error[{}]: {}{at}", d.code, d.message)
}

/// Checks capabilities, then generates the target's files.
pub fn run_job(ir: &Ir, job: &Job<'_>) -> Result<FileSet, Vec<String>> {
    let req = GenRequest {
        ir,
        role: job.role,
        services: job.services.clone(),
        consumes: job.consumes.clone(),
        framework: job.target.framework.clone(),
    };
    let name = &job.target.name;
    if req.services.is_empty() {
        return Err(vec![format!(
            "target `{name}`: selects no services (does its package declare a `service` block?)"
        )]);
    }
    if let Err(d) = resolve_profile(job.generator.get(), &req, &job.options) {
        return Err(vec![format_gen(&d, name)]);
    }
    let diags = check_capabilities(ir, &req, &job.generator.get().capabilities(job.role));
    if !diags.is_empty() {
        return Err(diags.iter().map(|d| format_gen(d, name)).collect());
    }
    job.generator
        .get()
        .generate_dyn(&req, &job.options)
        .map_err(|diags| diags.iter().map(|d| format_gen(d, name)).collect())
}

/// What `generate` did (or, for a dry run, would do) for one target.
#[derive(Debug)]
pub struct Written {
    pub name: String,
    pub out: PathBuf,
    pub plan: Plan,
}

/// Plans every target first and writes only when no target has a conflict, so one bad
/// target never leaves a partial result behind.
pub fn write_all(outputs: &[(&TargetSpec, FileSet)], opts: WriteOptions) -> Result<Vec<Written>> {
    let plan_only = WriteOptions {
        dry_run: true,
        ..opts
    };
    let mut written = Vec::new();
    for (target, files) in outputs {
        let plan = output::write(&target.out, files, &plan_only)
            .map_err(|e| user(format!("target `{}`: {e}", target.name)))?;
        written.push(Written {
            name: target.name.clone(),
            out: target.out.clone(),
            plan,
        });
    }
    if opts.dry_run || written.iter().any(|w| !w.plan.conflicts().is_empty()) {
        return Ok(written);
    }
    for ((target, files), w) in outputs.iter().zip(written.iter_mut()) {
        w.plan = output::write(&target.out, files, &opts)
            .map_err(|e| user(format!("target `{}`: {e}", target.name)))?;
    }
    Ok(written)
}

/// File counts of a plan: created, updated, deleted, unchanged.
pub fn tally(plan: &Plan) -> [usize; 4] {
    let n = |a: PlanAction| plan.paths_with(a).len();
    [
        n(PlanAction::Create),
        n(PlanAction::Update),
        n(PlanAction::Delete),
        n(PlanAction::Unchanged),
    ]
}

/// `path` relative to the current directory when it lies below it, for short messages.
pub fn display_path(path: &Path) -> String {
    let shown = std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf());
    let text = shown.display().to_string().replace('\\', "/");
    if text.is_empty() {
        ".".to_string()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use veld_project::config::load_str;

    fn project(extra: &str) -> Project {
        let dir = std::env::temp_dir().join("veld-pipeline-unit");
        load_str(&dir, &format!("[project]\nname = \"shop\"\n{extra}")).unwrap()
    }

    fn empty_ir() -> Ir {
        Ir {
            ir_version: String::new(),
            packages: vec![],
            decls: vec![],
            services: vec![],
            consumes: BTreeMap::new(),
            examples: vec![],
        }
    }

    #[test]
    fn plural_forms() {
        assert_eq!(plural(1, "error"), "1 error");
        assert_eq!(plural(0, "warning"), "0 warnings");
        assert_eq!(plural(2, "service"), "2 services");
    }

    #[test]
    fn failure_summary_is_plain_without_colour() {
        let ui = Ui::plain();
        assert_eq!(
            failure_summary(
                &ui,
                Counts {
                    errors: 2,
                    warnings: 1
                }
            ),
            "✗ 2 errors, 1 warning"
        );
    }

    #[test]
    fn unknown_target_name_lists_known_ones() {
        let p = project(
            "[[targets]]\nname = \"web\"\nrole = \"client\"\nemitter = \"typescript\"\nout = \"gen\"\n",
        );
        let err = select_targets(&p, Some("api")).unwrap_err().to_string();
        assert!(
            err.contains("no target named `api`") && err.contains("web"),
            "{err}"
        );
        assert_eq!(select_targets(&p, Some("web")).unwrap().len(), 1);
        assert_eq!(select_targets(&p, None).unwrap().len(), 1);
    }

    #[test]
    fn no_targets_is_a_user_error() {
        let p = project("");
        assert!(
            select_targets(&p, None)
                .unwrap_err()
                .to_string()
                .contains("no `[[targets]]`")
        );
    }

    #[test]
    fn unknown_emitter_wrong_role_and_plugins_are_reported() {
        let ir = empty_ir();
        let check = |extra: &str, needle: &str| {
            let p = project(extra);
            let err = build_job(&p, &ir, &p.targets[0]).err().unwrap();
            assert!(err[0].contains(needle), "{err:?}");
        };
        check(
            "[[targets]]\nrole = \"client\"\nemitter = \"cobol\"\nout = \"gen\"\n",
            "unknown emitter `cobol`",
        );
        check(
            "[[targets]]\nrole = \"client\"\nemitter = \"node\"\nout = \"gen\"\n",
            "cannot produce the `client` role",
        );
        check(
            "[[targets]]\nrole = \"client\"\nemitter = \"plugin:kotlin\"\nout = \"gen\"\n[plugins.kotlin]\ncommand = \"x\"\n",
            "plugin `kotlin`: executable `x` not found on PATH",
        );
    }

    #[test]
    fn package_names_are_npm_safe() {
        assert_eq!(default_package_name("api"), "@veld/api");
        assert_eq!(default_package_name("Orders_API"), "@veld/orders-api");
        assert_eq!(default_package_name("--"), "@veld/generated");
    }

    #[test]
    fn options_are_converted_to_json() {
        let ir = empty_ir();
        let p = project(
            "[[targets]]\nrole = \"client\"\nemitter = \"typescript\"\nout = \"gen\"\n[targets.options]\nvalidate = true\n",
        );
        let job = build_job(&p, &ir, &p.targets[0]).ok().unwrap();
        assert_eq!(
            job.options,
            serde_json::json!({ "validate": true, "package-name": "@veld/client-typescript" })
        );
        let p = project(
            "[[targets]]
role = \"client\"
emitter = \"typescript\"
out = \"gen\"
[targets.options]
package-name = \"@acme/sdk\"
",
        );
        let job = build_job(&p, &ir, &p.targets[0]).ok().unwrap();
        assert_eq!(
            job.options,
            serde_json::json!({ "package-name": "@acme/sdk" })
        );
    }

    #[test]
    fn a_target_with_no_services_is_an_error_not_a_panic() {
        let ir = empty_ir();
        let p =
            project("[[targets]]\nrole = \"client\"\nemitter = \"typescript\"\nout = \"gen\"\n");
        let job = build_job(&p, &ir, &p.targets[0]).ok().unwrap();
        let err = run_job(&ir, &job).err().unwrap();
        assert!(err[0].contains("selects no services"), "{err:?}");
    }

    #[test]
    fn display_path_is_relative_below_cwd() {
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(display_path(&cwd.join("a").join("b")), "a/b");
        assert_eq!(display_path(&cwd), ".");
    }
}
