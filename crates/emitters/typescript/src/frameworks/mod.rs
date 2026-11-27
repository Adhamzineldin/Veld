//! Framework helpers of the `typescript` client: React hooks and Vue composables.
//!
//! Each framework adds `runtime/<framework>.ts` (one shared implementation of the query and
//! mutation state machines) and `<package>/<framework>.ts` (thin, typed wrappers over the
//! generated client classes). These are the only generated files that import a package, and only
//! the framework the target chose. The plain client output never changes.

mod react;
mod vue;

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, FrameworkAdapter, FrameworkInfo, GenDiagnostic};
use veld_ir::{Method, PackageId};

use crate::client::RequestNames;
use crate::tree::{FileCtx, Tree};
use crate::types::ts_type;
use crate::util::{HEADER, add_file, doc_comment, rel_import};

/// The generated clients of one package, as the framework adapters need them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientModel {
    pub packages: Vec<PackageModel>,
    /// Whether an emitted declaration has rules: the framework runtime then also holds the form
    /// helpers (`useFormRules`), and the package files re-export them.
    pub rules: bool,
}

/// One package: its directory and the services whose clients get helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageModel {
    /// Directory of the package's files, empty or ending in `/`.
    pub prefix: String,
    /// Import lines for the model types the hooks mention, rendered for a file in `prefix`.
    pub imports: Vec<String>,
    pub services: Vec<ServiceModel>,
    /// Whether the package file re-exports the form helpers (the contract has rules).
    pub rules: bool,
}

/// One service and its endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceModel {
    /// PascalCase name (`Orders`).
    pub name: String,
    /// camelCase key in the client map (`orders`).
    pub key: String,
    /// The client class (`OrdersClient`).
    pub client: String,
    pub endpoints: Vec<EndpointModel>,
}

/// One endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointModel {
    pub docs: Option<String>,
    pub deprecated: Option<String>,
    /// The client method (`getOrder`).
    pub method: String,
    /// The hook name (`useGetOrder`).
    pub hook: String,
    /// The request type exported by `client.ts` (`GetOrderRequest`).
    pub request: String,
    /// TypeScript type of the success value (`void` for none).
    pub response: String,
    /// `true` for GET endpoints that are not streams, which become query hooks; the rest are
    /// mutations, except streams.
    pub query: bool,
    /// `true` for SSE (`get ... -> stream`) and `ws` endpoints, which become `use<Endpoint>Stream`.
    pub stream: bool,
    /// `true` for a `ws` endpoint: its stream hook can also `send`.
    pub ws: bool,
    /// The type the client sends on a `ws` channel (`never` when it sends nothing).
    pub message: String,
    /// The stream hook name (`useTicksStream`).
    pub stream_hook: String,
    /// `true` when the request has no parameters, so the hook defaults it to `{}`.
    pub empty_request: bool,
}

impl ClientModel {
    /// Whether any endpoint is a stream, so the framework runtime needs its streaming part.
    #[must_use]
    pub fn has_streams(&self) -> bool {
        self.packages
            .iter()
            .flat_map(|p| &p.services)
            .flat_map(|s| &s.endpoints)
            .any(|e| e.stream)
    }
}

/// Every client framework helper. The single source of truth for `frameworks(Role::Client)`.
pub const ADAPTERS: &[&dyn FrameworkAdapter<ClientModel>] = &[&react::React, &vue::Vue];

/// The registry as the core sees it.
#[must_use]
pub fn infos() -> Vec<FrameworkInfo> {
    veld_codegen::framework_infos(ADAPTERS)
}

/// Builds the model of the tree's services.
#[must_use]
pub fn model(tree: &Tree<'_>) -> ClientModel {
    let names = RequestNames::new(tree.ir, &tree.services);
    let packages = tree
        .packages()
        .into_iter()
        .map(|pkg| package_model(tree, &names, pkg))
        .collect();
    ClientModel {
        packages,
        rules: tree.uses_rules(),
    }
}

fn package_model(tree: &Tree<'_>, names: &RequestNames, pkg: PackageId) -> PackageModel {
    let prefix = tree.pkg_prefix(pkg);
    let mut fc = FileCtx::new(tree, &format!("{prefix}hooks"));
    let mut services = Vec::new();
    for sid in tree
        .services
        .iter()
        .filter(|s| tree.ir.service(**s).package == pkg)
    {
        let svc = tree.ir.service(*sid);
        let mut endpoints = Vec::new();
        for ep in &svc.endpoints {
            let request = names.get(svc.id, &ep.name);
            let response = match &ep.success.ty {
                Some(t) => ts_type(&mut fc, t),
                None => "void".to_string(),
            };
            let stream = ep.success.stream || ep.method == Method::Ws;
            let message = match ep.params.iter().find(|p| p.stream) {
                Some(p) => ts_type(&mut fc, &p.ty),
                None => "never".to_string(),
            };
            let base = request.strip_suffix("Request").unwrap_or(&request);
            endpoints.push(EndpointModel {
                docs: ep.docs.clone(),
                deprecated: ep.deprecated.clone(),
                method: crate::util::method_key(&ep.name),
                hook: format!("use{base}"),
                stream_hook: format!("use{base}Stream"),
                response: if ep.method == Method::Ws && ep.success.ty.is_none() {
                    "never".to_string()
                } else {
                    response
                },
                request,
                query: ep.method == Method::Get && !stream,
                stream,
                ws: ep.method == Method::Ws,
                message,
                empty_request: ep.params.iter().all(|p| p.stream)
                    && !crate::version::needs_request(ep),
            });
        }
        services.push(ServiceModel {
            name: naming::pascal(&svc.name),
            key: naming::camel(&svc.name),
            client: format!("{}Client", naming::pascal(&svc.name)),
            endpoints,
        });
    }
    let imports = fc
        .imports
        .render(&format!("{prefix}hooks.ts"), &tree.runtime);
    PackageModel {
        prefix,
        imports,
        services,
        rules: tree.uses_rules(),
    }
}

/// The form helpers every framework shares (`formRules`, `FormModel`, `FormRules`).
const FORM_SHARED: &str = include_str!("../../runtime/form_shared.ts");

/// The embedded sources of one framework's `runtime/<id>.ts`: the base, and the parts appended
/// only when the contract streams or has rules.
struct RuntimeParts {
    runtime: &'static str,
    stream: &'static str,
    form: &'static str,
}

/// The files of one framework: `runtime/<id>.ts` and `<package>/<id>.ts` for every package.
fn framework_files(
    id: &str,
    parts: &RuntimeParts,
    model: &ClientModel,
    package: fn(&PackageModel, &str) -> String,
) -> Result<FileSet, Vec<GenDiagnostic>> {
    let mut files = FileSet::new();
    // The streaming helpers are appended only when the contract streams, and the form helpers
    // only when it has rules, so the runtime of a contract with neither is unchanged.
    let mut runtime = parts.runtime.to_string();
    if model.has_streams() {
        runtime.push_str(parts.stream);
    }
    if model.rules {
        runtime.push_str(FORM_SHARED);
        runtime.push_str(parts.form);
    }
    add_file(&mut files, &format!("runtime/{id}.ts"), runtime)?;
    for pkg in &model.packages {
        let path = format!("{}{id}.ts", pkg.prefix);
        add_file(&mut files, &path, package(pkg, &path))?;
    }
    Ok(files)
}

/// What differs between the frameworks in the shared per-package file.
struct Flavor {
    id: &'static str,
    /// What the file holds, for its header comment.
    title: &'static str,
    /// Lines importing the framework package itself.
    imports: &'static [&'static str],
    /// Wraps the request type of a query hook (`MaybeRefOrGetter<R | null>` or `R | null`).
    query_request: fn(&str) -> String,
    /// The form helper types this framework's runtime exports, next to `FormModel` and `FormRules`.
    form_types: &'static str,
}

fn header(w: &mut CodeWriter, pkg: &PackageModel, flavor: &Flavor, path: &str) {
    w.line(HEADER);
    w.line(format!(
        "// {} for the generated clients. The only import from a package is `{}`.",
        flavor.title, flavor.id
    ));
    w.blank();
    for line in flavor.imports {
        w.line(*line);
    }
    let runtime = rel_import(path, &format!("runtime/{}", flavor.id));
    let streams = pkg
        .services
        .iter()
        .any(|s| s.endpoints.iter().any(|e| e.stream));
    let hooks = if streams {
        "useMutation, useQuery, useStream"
    } else {
        "useMutation, useQuery"
    };
    let types = if streams {
        "ChannelResult, MutationOptions, MutationResult, QueryOptions, QueryResult, StreamOptions, StreamResult"
    } else {
        "MutationOptions, MutationResult, QueryOptions, QueryResult"
    };
    w.line(format!("import {{ {hooks} }} from '{runtime}';"));
    w.line(format!("import type {{ {types} }} from '{runtime}';"));
    if pkg.rules {
        w.line(format!(
            "export {{ formRules, useFormRules }} from '{runtime}';"
        ));
        w.line(format!(
            "export type {{ {} }} from '{runtime}';",
            flavor.form_types
        ));
    }
    let client = rel_import(path, &format!("{}client", pkg.prefix));
    let mut names: Vec<&str> = pkg
        .services
        .iter()
        .flat_map(|s| {
            std::iter::once(s.client.as_str()).chain(s.endpoints.iter().map(|e| e.request.as_str()))
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    w.line(format!(
        "import type {{ {} }} from '{client}';",
        names.join(", ")
    ));
    for line in &pkg.imports {
        w.line(line);
    }
    w.blank();
}

/// One hook per endpoint of `svc`.
fn write_hooks(w: &mut CodeWriter, svc: &ServiceModel, flavor: &Flavor) {
    let client = &svc.client;
    let lookup = format!("use{}Client", svc.name);
    for ep in &svc.endpoints {
        w.blank();
        doc_comment(w, ep.docs.as_deref(), ep.deprecated.as_deref());
        let method = &ep.method;
        let (req, res) = (&ep.request, &ep.response);
        if ep.stream {
            write_stream_hook(w, svc, ep, flavor, &lookup);
        } else if ep.query {
            let wrapped = (flavor.query_request)(req);
            let default = if ep.empty_request { " = {}" } else { "" };
            w.open(format!(
                "export function {}(req: {wrapped}{default}, opts: QueryOptions<{client}> = {{}}): QueryResult<{res}>",
                ep.hook
            ));
            w.line(format!("const client = {lookup}(opts.client);"));
            w.line(format!(
                "return useQuery((r, init) => client.{method}(r, init), req, opts.enabled);"
            ));
        } else {
            w.open(format!(
                "export function {}(opts: MutationOptions<{client}> = {{}}): MutationResult<{req}, {res}>",
                ep.hook
            ));
            w.line(format!("const client = {lookup}(opts.client);"));
            w.line(format!(
                "return useMutation((r, init) => client.{method}(r, init));"
            ));
        }
        w.close();
    }
}

/// `use<Endpoint>Stream(req, opts?)`: Server-Sent Events or a WebSocket channel.
fn write_stream_hook(
    w: &mut CodeWriter,
    svc: &ServiceModel,
    ep: &EndpointModel,
    flavor: &Flavor,
    lookup: &str,
) {
    let client = &svc.client;
    let wrapped = (flavor.query_request)(&ep.request);
    let default = if ep.empty_request { " = {}" } else { "" };
    let (res, method) = (&ep.response, &ep.method);
    let (ret, generics) = if ep.ws {
        (
            format!("ChannelResult<{res}, {}>", ep.message),
            format!("<{}, {res}, {}>", ep.request, ep.message),
        )
    } else {
        (format!("StreamResult<{res}>"), String::new())
    };
    w.open(format!(
        "export function {}(req: {wrapped}{default}, opts: StreamOptions<{client}> = {{}}): {ret}",
        ep.stream_hook
    ));
    w.line(format!("const client = {lookup}(opts.client);"));
    w.line(format!(
        "return useStream{generics}((r, init) => client.{method}(r, init), req, opts);"
    ));
}

/// The clients map type, `{ orders?: OrdersClient; ... }`.
fn write_clients_type(w: &mut CodeWriter, pkg: &PackageModel) {
    w.open("export interface VeldClients");
    for svc in &pkg.services {
        w.line(format!("{}?: {};", svc.key, svc.client));
    }
    w.close();
}

/// The "no client" error shared by both frameworks.
fn missing(svc: &ServiceModel, hint: &str) -> String {
    format!("throw new Error('use{}Client: {hint}');", svc.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use veld_codegen::{LanguageVersion, Profile};

    #[test]
    fn the_registry_lists_react_and_vue_with_their_versions() {
        let infos = infos();
        let list: Vec<(&str, &[String])> = infos
            .iter()
            .map(|i| (i.id.as_str(), i.versions.as_slice()))
            .collect();
        assert_eq!(list[0].0, "react");
        assert_eq!(list[0].1, ["18".to_string()]);
        assert_eq!(list[1].0, "vue");
        assert_eq!(list[1].1, ["3".to_string()]);
        assert!(
            infos
                .iter()
                .all(|i| i.min_language == Some(LanguageVersion::es(2020)))
        );
    }

    #[test]
    fn a_profile_without_the_framework_emits_nothing_extra() {
        let model = ClientModel {
            packages: Vec::new(),
            rules: false,
        };
        for adapter in ADAPTERS {
            let files = adapter.files(&model, &Profile::unversioned()).unwrap();
            assert_eq!(files.len(), 0, "{}", adapter.id());
        }
    }
}
