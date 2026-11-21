//! The pure core: compile, generate, format and export, over plain data.
//!
//! Input convention: a source file's path is `<package>/<path in package>.veld`, the same name
//! the CLI prints in diagnostics. A path without a directory belongs to the package `main`.
//! The optional [`ProjectIn`] adds what `veld.toml` would: package kinds, base URLs, route
//! prefixes and `consumes`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use veld_codegen::{
    DynGenerator, FileSet, GenDiagnostic, GenRequest, Role, check_capabilities, resolve_profile,
};
use veld_compiler::{
    CompileInput, CompileOutput, Diagnostic as CompilerDiagnostic, PackageInput, SourceFile,
    SourceMap,
};
use veld_ir::{Ir, PackageKind, ServiceId};

use crate::registry;

// --- input -------------------------------------------------------------------------------

/// One contract file.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceIn {
    pub path: String,
    pub text: String,
}

/// A contracts package, as `veld.toml` declares it.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PackageIn {
    pub name: String,
    /// `"service"` (default) or `"library"`.
    pub kind: String,
    pub base_url: Option<String>,
    /// Route prefix, already joined (`/api/v1`); empty for none.
    pub prefix: String,
    pub consumes: Vec<String>,
}

/// The project-level settings; every field is optional.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProjectIn {
    pub packages: Vec<PackageIn>,
}

/// One `[[targets]]` entry.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TargetIn {
    /// Names the generated package (`@veld/<name>`). Default `<role>-<emitter>`.
    pub name: Option<String>,
    pub emitter: String,
    pub role: String,
    pub framework: Option<String>,
    pub options: Map<String, Value>,
    /// The served service (server role); implied when the project has exactly one.
    pub service: Option<String>,
    /// The services a client covers; default all.
    pub services: Option<Vec<String>>,
    /// Whether a server also gets clients for its `consumes`. Default `true`.
    pub clients: Option<bool>,
}

// --- output ------------------------------------------------------------------------------

/// A compiler or generator diagnostic. Lines and columns are 1-based; `file` is empty and the
/// position 0 when the problem belongs to no file.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Diag {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub help: Option<String>,
    pub notes: Vec<String>,
    pub rendered: String,
}

/// What the contracts declare, for the summary line.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct Summary {
    pub services: usize,
    pub endpoints: usize,
    pub types: usize,
}

#[derive(Debug, Serialize)]
pub struct CompileResult {
    pub ok: bool,
    pub diagnostics: Vec<Diag>,
    pub summary: Option<Summary>,
}

/// A generated file.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OutFile {
    pub path: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct GenerateResult {
    pub ok: bool,
    pub files: Vec<OutFile>,
    pub diagnostics: Vec<Diag>,
}

#[derive(Debug, Serialize)]
pub struct FormatResult {
    pub ok: bool,
    pub text: String,
    pub diagnostics: Vec<Diag>,
}

#[derive(Debug, Serialize)]
pub struct OpenapiResult {
    pub ok: bool,
    pub text: String,
    pub diagnostics: Vec<Diag>,
}

// --- diagnostics -------------------------------------------------------------------------

fn input_error(message: impl Into<String>) -> Diag {
    let message = message.into();
    Diag {
        code: "INPUT".into(),
        severity: "error".into(),
        rendered: format!("error[INPUT]: {message}"),
        message,
        file: String::new(),
        line: 0,
        col: 0,
        end_line: 0,
        end_col: 0,
        help: None,
        notes: Vec::new(),
    }
}

fn convert(map: &SourceMap, d: &CompilerDiagnostic) -> Diag {
    let (line, col) = map
        .line_col(d.primary.file, d.primary.start)
        .unwrap_or((0, 0));
    let (end_line, end_col) = map
        .line_col(d.primary.file, d.primary.end)
        .unwrap_or((line, col));
    Diag {
        code: d.code.to_string(),
        severity: d.severity.as_str().to_string(),
        message: d.message.clone(),
        file: map.path(d.primary.file).unwrap_or_default().to_string(),
        line,
        col,
        end_line,
        end_col,
        help: d.help.clone(),
        notes: d.notes.clone(),
        rendered: map.render(d),
    }
}

fn from_gen(d: &GenDiagnostic, target: &str) -> Diag {
    let (file, line, col) = d
        .origin
        .as_ref()
        .map_or((String::new(), 0, 0), |o| (o.file.clone(), o.line, o.col));
    Diag {
        code: d.code.to_string(),
        severity: "error".into(),
        rendered: format!("target `{target}`: error[{}]: {}", d.code, d.message),
        message: d.message.clone(),
        file,
        line,
        col,
        end_line: line,
        end_col: col,
        help: None,
        notes: Vec::new(),
    }
}

fn target_error(target: &str, message: &str) -> Diag {
    let mut d = input_error(format!("target `{target}`: {message}"));
    d.code = "E0803".into();
    d.rendered = format!("error[E0803]: {}", d.message);
    d
}

fn has_error(out: &CompileOutput) -> bool {
    out.diagnostics.iter().any(CompilerDiagnostic::is_error)
}

// --- compile -----------------------------------------------------------------------------

/// The compiler input and the facts the generators' glue needs.
struct Built {
    input: CompileInput,
    prefixes: BTreeMap<String, String>,
    packages: Vec<PackageIn>,
}

fn group_sources(
    files: &[SourceIn],
    packages: &mut Vec<PackageIn>,
) -> BTreeMap<String, Vec<SourceFile>> {
    let mut sources: BTreeMap<String, Vec<SourceFile>> = BTreeMap::new();
    for f in files {
        let (pkg, rel) = f.path.split_once('/').unwrap_or(("main", f.path.as_str()));
        if !packages.iter().any(|p| p.name == pkg) {
            packages.push(PackageIn {
                name: pkg.to_string(),
                ..PackageIn::default()
            });
        }
        sources
            .entry(pkg.to_string())
            .or_default()
            .push(SourceFile {
                path: rel.to_string(),
                text: f.text.clone(),
            });
    }
    sources
}

fn build(files: &[SourceIn], project: &ProjectIn) -> Built {
    let mut packages: Vec<PackageIn> = project.packages.clone();
    let mut sources = group_sources(files, &mut packages);
    // The CLI lists libraries first, then services, each by name; so does the compiler input.
    packages.sort_by(|a, b| (a.kind != "library", &a.name).cmp(&(b.kind != "library", &b.name)));
    let mut input = CompileInput::default();
    let mut prefixes = BTreeMap::new();
    for p in &packages {
        let kind = if p.kind == "library" {
            PackageKind::Library
        } else {
            PackageKind::Service
        };
        let mut files = sources.remove(&p.name).unwrap_or_default();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        input.packages.push(PackageInput {
            name: p.name.clone(),
            kind,
            files,
            base_url: p.base_url.clone(),
        });
        if kind == PackageKind::Service && !p.consumes.is_empty() {
            input.consumes.insert(p.name.clone(), p.consumes.clone());
        }
        if !p.prefix.is_empty() {
            prefixes.insert(p.name.clone(), p.prefix.clone());
        }
    }
    Built {
        input,
        prefixes,
        packages,
    }
}

fn compile_built(built: &Built) -> CompileOutput {
    veld_compiler::compile_prefixed(&built.input, None, &built.prefixes)
}

fn summary(ir: &Ir) -> Summary {
    Summary {
        services: ir.services.len(),
        endpoints: ir.services.iter().map(|s| s.endpoints.len()).sum(),
        types: ir
            .decls
            .iter()
            .filter(|d| ir.package(d.package).kind != PackageKind::Prelude)
            .count(),
    }
}

fn diags(out: &CompileOutput) -> Vec<Diag> {
    out.diagnostics
        .iter()
        .map(|d| convert(&out.sources, d))
        .collect()
}

/// Compiles the contracts; the IR is dropped, only diagnostics and a summary come back.
pub fn compile(files: &[SourceIn], project: &ProjectIn) -> CompileResult {
    let out = compile_built(&build(files, project));
    let ok = !has_error(&out);
    CompileResult {
        ok,
        summary: out.ir.as_ref().filter(|_| ok).map(summary),
        diagnostics: diags(&out),
    }
}

// --- generate ----------------------------------------------------------------------------

fn parse_role(role: &str) -> Option<Role> {
    match role {
        "server" => Some(Role::Server),
        "client" => Some(Role::Client),
        _ => None,
    }
}

fn service_ids(ir: &Ir, packages: &[String]) -> Vec<ServiceId> {
    packages
        .iter()
        .flat_map(|p| ir.services_of_package(p).map(|s| s.id))
        .collect()
}

/// `@veld/<name>`, lowercased and made npm-safe (mirrors `pipeline::slug`).
fn package_name(target: &str) -> String {
    let slug: String = target
        .chars()
        .map(|c| match c.to_ascii_lowercase() {
            c if c.is_ascii_alphanumeric() || c == '-' || c == '.' => c,
            _ => '-',
        })
        .collect();
    let slug = slug.trim_matches(|c| c == '-' || c == '.');
    format!("@veld/{}", if slug.is_empty() { "generated" } else { slug })
}

/// The options handed to the generator (mirrors `pipeline::job_options`).
fn job_options(t: &TargetIn, name: &str) -> Value {
    let mut options = t.options.clone();
    if matches!(t.emitter.as_str(), "node" | "typescript") && !options.contains_key("package-name")
    {
        options.insert("package-name".into(), Value::String(package_name(name)));
    }
    if options.is_empty() {
        Value::Null
    } else {
        Value::Object(options)
    }
}

type Services = (Vec<ServiceId>, Vec<ServiceId>);

/// Which services a target serves and consumes (mirrors `veld-project` and `pipeline`).
fn resolve_services(t: &TargetIn, role: Role, built: &Built, ir: &Ir) -> Result<Services, String> {
    let services: Vec<&PackageIn> = built
        .packages
        .iter()
        .filter(|p| p.kind != "library")
        .collect();
    if role == Role::Client {
        let names = t
            .services
            .clone()
            .unwrap_or_else(|| services.iter().map(|p| p.name.clone()).collect());
        return Ok((service_ids(ir, &names), Vec::new()));
    }
    let name = match (&t.service, services.as_slice()) {
        (Some(s), _) => s.clone(),
        (None, [only]) => only.name.clone(),
        (None, _) => return Err("a server target needs `service`".into()),
    };
    let Some(pkg) = services.iter().find(|p| p.name == name) else {
        return Err(format!("unknown service `{name}`"));
    };
    let consumed = if t.clients.unwrap_or(true) {
        service_ids(ir, &pkg.consumes)
    } else {
        Vec::new()
    };
    Ok((service_ids(ir, &[name]), consumed))
}

fn run_target(ir: &Ir, built: &Built, t: &TargetIn) -> Result<FileSet, Vec<Diag>> {
    let name = t
        .name
        .clone()
        .unwrap_or_else(|| format!("{}-{}", t.role, t.emitter));
    let fail = |m: &str| vec![target_error(&name, m)];
    let role = parse_role(&t.role).ok_or_else(|| fail(&format!("unknown role `{}`", t.role)))?;
    let generator: &dyn DynGenerator = registry::find(&t.emitter)
        .ok_or_else(|| fail(&format!("unknown emitter `{}`", t.emitter)))?;
    if !generator.roles().contains(&role) {
        return Err(fail(&format!(
            "emitter `{}` cannot produce the `{}` role",
            t.emitter, t.role
        )));
    }
    let (services, consumes) = resolve_services(t, role, built, ir).map_err(|m| fail(&m))?;
    let req = GenRequest {
        ir,
        role,
        services,
        consumes,
        framework: t.framework.clone(),
    };
    if req.services.is_empty() {
        return Err(fail("selects no services (is there a `service` block?)"));
    }
    let options = job_options(t, &name);
    let from = |ds: &[GenDiagnostic]| ds.iter().map(|d| from_gen(d, &name)).collect::<Vec<_>>();
    resolve_profile(generator, &req, &options).map_err(|d| from(&[d]))?;
    let caps = check_capabilities(ir, &req, &generator.capabilities(role));
    if !caps.is_empty() {
        return Err(from(&caps));
    }
    generator.generate_dyn(&req, &options).map_err(|d| from(&d))
}

/// Compiles the contracts and runs one target through the real generator.
pub fn generate(files: &[SourceIn], target: &TargetIn, project: &ProjectIn) -> GenerateResult {
    let built = build(files, project);
    let out = compile_built(&built);
    let mut diagnostics = diags(&out);
    let failed = |diagnostics| GenerateResult {
        ok: false,
        files: Vec::new(),
        diagnostics,
    };
    let ir = match out.ir.as_ref() {
        Some(ir) if !has_error(&out) => ir,
        _ => return failed(diagnostics),
    };
    match run_target(ir, &built, target) {
        Ok(set) => GenerateResult {
            ok: true,
            files: set
                .iter()
                .map(|(path, bytes)| OutFile {
                    path: path.to_string(),
                    text: String::from_utf8_lossy(bytes).into_owned(),
                })
                .collect(),
            diagnostics,
        },
        Err(more) => {
            diagnostics.extend(more);
            failed(diagnostics)
        }
    }
}

// --- format and export -------------------------------------------------------------------

/// Formats one contract with the canonical formatter.
pub fn format(text: &str) -> FormatResult {
    match veld_tools::format(text) {
        Ok(text) => FormatResult {
            ok: true,
            text,
            diagnostics: Vec::new(),
        },
        Err(ds) => {
            let mut map = SourceMap::default();
            map.add("contract.veld", text);
            FormatResult {
                ok: false,
                text: text.to_string(),
                diagnostics: ds.iter().map(|d| convert(&map, d)).collect(),
            }
        }
    }
}

/// The OpenAPI document of every service, as `veld export openapi` prints it.
pub fn export_openapi(files: &[SourceIn], project: &ProjectIn) -> OpenapiResult {
    let out = compile_built(&build(files, project));
    let diagnostics = diags(&out);
    match out.ir.as_ref() {
        Some(ir) if !has_error(&out) => {
            let all: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
            let mut text =
                serde_json::to_string_pretty(&veld_tools::openapi(ir, &all)).unwrap_or_default();
            text.push('\n');
            OpenapiResult {
                ok: true,
                text,
                diagnostics,
            }
        }
        _ => OpenapiResult {
            ok: false,
            text: String::new(),
            diagnostics,
        },
    }
}

// --- describe ----------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct FrameworkDesc {
    pub id: String,
    pub versions: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct EmitterDesc {
    pub id: String,
    pub roles: Vec<String>,
    pub frameworks: BTreeMap<String, Vec<FrameworkDesc>>,
    /// The language-version option, e.g. `java` with `17`, `21`; absent when unversioned.
    pub language: Option<LanguageDesc>,
}

#[derive(Debug, Serialize)]
pub struct LanguageDesc {
    pub id: String,
    pub versions: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Describe {
    pub version: String,
    pub emitters: Vec<EmitterDesc>,
}

fn describe_emitter(g: &dyn DynGenerator) -> EmitterDesc {
    let frameworks = g
        .roles()
        .iter()
        .map(|r| {
            let list = g
                .frameworks(*r)
                .into_iter()
                .map(|f| FrameworkDesc {
                    id: f.id,
                    versions: f.versions,
                })
                .collect();
            (r.name().to_string(), list)
        })
        .collect();
    EmitterDesc {
        id: g.id().to_string(),
        roles: g.roles().iter().map(|r| r.name().to_string()).collect(),
        frameworks,
        language: g.language().map(|l| LanguageDesc {
            id: l.id,
            versions: l.versions,
        }),
    }
}

/// The engine version and the generators it carries.
pub fn describe() -> Describe {
    Describe {
        version: env!("CARGO_PKG_VERSION").to_string(),
        emitters: registry::generators()
            .iter()
            .map(|g| describe_emitter(*g))
            .collect(),
    }
}

// --- JSON entry points (what the wasm layer calls) ---------------------------------------

fn parse<T: serde::de::DeserializeOwned>(what: &str, text: &str) -> Result<T, Box<Diag>> {
    serde_json::from_str(text).map_err(|e| Box::new(input_error(format!("invalid {what}: {e}"))))
}

fn project_of(text: Option<&str>) -> Result<ProjectIn, Box<Diag>> {
    text.map_or_else(|| Ok(ProjectIn::default()), |t| parse("project", t))
}

/// [`compile`] over JSON text.
pub fn compile_json(files: &str, project: Option<&str>) -> CompileResult {
    match (parse::<Vec<SourceIn>>("files", files), project_of(project)) {
        (Ok(f), Ok(p)) => compile(&f, &p),
        (Err(d), _) | (_, Err(d)) => CompileResult {
            ok: false,
            diagnostics: vec![*d],
            summary: None,
        },
    }
}

/// [`generate`] over JSON text.
pub fn generate_json(files: &str, target: &str, project: Option<&str>) -> GenerateResult {
    match (
        parse::<Vec<SourceIn>>("files", files),
        parse("target", target),
        project_of(project),
    ) {
        (Ok(f), Ok(t), Ok(p)) => generate(&f, &t, &p),
        (Err(d), _, _) | (_, Err(d), _) | (_, _, Err(d)) => GenerateResult {
            ok: false,
            files: Vec::new(),
            diagnostics: vec![*d],
        },
    }
}

/// [`export_openapi`] over JSON text.
pub fn export_openapi_json(files: &str, project: Option<&str>) -> OpenapiResult {
    match (parse::<Vec<SourceIn>>("files", files), project_of(project)) {
        (Ok(f), Ok(p)) => export_openapi(&f, &p),
        (Err(d), _) | (_, Err(d)) => OpenapiResult {
            ok: false,
            text: String::new(),
            diagnostics: vec![*d],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO: &str = "model Greeting {\n  message: string\n}\n\nservice Hello at \"/hello\" {\n  get \"/{name}\" greet(name: string) -> Greeting | NotFound\n}\n";

    fn files(text: &str) -> Vec<SourceIn> {
        vec![SourceIn {
            path: "hello/hello.veld".into(),
            text: text.into(),
        }]
    }

    fn target(emitter: &str, role: &str, framework: Option<&str>) -> TargetIn {
        TargetIn {
            emitter: emitter.into(),
            role: role.into(),
            framework: framework.map(Into::into),
            ..TargetIn::default()
        }
    }

    #[test]
    fn compiles_and_summarizes() {
        let r = compile(&files(HELLO), &ProjectIn::default());
        assert!(r.ok, "{:?}", r.diagnostics);
        assert_eq!(r.summary.map(|s| (s.services, s.endpoints)), Some((1, 1)));
    }

    #[test]
    fn reports_a_diagnostic_with_position() {
        let r = compile(
            &files("model A {\n  x: Missing\n}\n"),
            &ProjectIn::default(),
        );
        assert!(!r.ok);
        let d = &r.diagnostics[0];
        assert_eq!((d.file.as_str(), d.line), ("hello/hello.veld", 2));
        assert!(d.code.starts_with('E') && d.rendered.contains(&d.code));
    }

    #[test]
    fn generates_every_builtin_target() {
        for (emitter, role, fw) in [
            ("node", "server", Some("express")),
            ("typescript", "client", Some("react")),
            ("java", "server", Some("jdk")),
            ("java", "client", None),
            ("rust", "server", Some("axum")),
        ] {
            let p = ProjectIn::default();
            let r = generate(&files(HELLO), &target(emitter, role, fw), &p);
            assert!(
                r.ok && !r.files.is_empty(),
                "{emitter}/{role}: {:?}",
                r.diagnostics
            );
        }
    }

    #[test]
    fn target_problems_are_diagnostics() {
        let p = ProjectIn::default();
        let bad = generate(&files(HELLO), &target("cobol", "server", None), &p);
        assert!(!bad.ok && bad.diagnostics[0].message.contains("unknown emitter"));
        let wrong = generate(&files(HELLO), &target("node", "server", Some("rails")), &p);
        assert!(!wrong.ok && wrong.diagnostics.iter().any(|d| d.code == "E0803"));
    }

    #[test]
    fn compile_errors_block_generation() {
        let r = generate(
            &files("model A {\n  x: Missing\n}\n"),
            &target("node", "server", None),
            &ProjectIn::default(),
        );
        assert!(!r.ok && r.files.is_empty() && !r.diagnostics.is_empty());
    }

    #[test]
    fn formats_and_exports() {
        assert!(format(HELLO).ok);
        assert!(!format("model {").ok);
        let o = export_openapi(&files(HELLO), &ProjectIn::default());
        assert!(o.ok && o.text.contains("\"openapi\""));
    }

    #[test]
    fn bad_json_is_an_input_diagnostic() {
        let r = compile_json("not json", None);
        assert!(!r.ok && r.diagnostics[0].code == "INPUT");
    }

    #[test]
    fn describes_the_generators() {
        let d = describe();
        let ids: Vec<&str> = d.emitters.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["java", "node", "typescript", "rust"]);
    }
}
