//! `<package>/queries.ts`: query keys, query options and mutation options as plain objects.
//!
//! Enabled by the target option `query-options = true`. The file imports nothing from any
//! package: the objects have the shape `useQuery`, `useMutation` and `invalidateQueries` of
//! TanStack Query take, and they work just as well with SWR or a hook of your own. GET endpoints
//! (that are not streams) become queries, every other endpoint a mutation. Keys are built from the
//! canonical bytes of the request, so equal requests share a key whatever the property order.

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{DeclId, Endpoint, Method, PackageId, Service};

use crate::client::RequestNames;
use crate::tree::{FileCtx, Tree};
use crate::types::ts_type;
use crate::util::{add_file, doc_comment, method_key, ts_str};

/// File name of the module in a package directory.
pub const FILE: &str = "queries";

/// Writes `queries.ts` for every package of the tree and returns the module paths.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<Vec<String>, Vec<GenDiagnostic>> {
    let names = RequestNames::new(tree.ir, &tree.services);
    let mut modules = Vec::new();
    for pkg in tree.packages() {
        let module = format!("{}{FILE}", tree.pkg_prefix(pkg));
        let mut fc = FileCtx::new(tree, &module);
        let mut w = CodeWriter::new();
        for sid in tree
            .services
            .iter()
            .filter(|s| tree.ir.service(**s).package == pkg)
        {
            let svc = tree.ir.service(*sid);
            write_service(&mut w, &mut fc, &names, svc, pkg);
        }
        add_file(files, &format!("{module}.ts"), fc.finish(&w.finish()))?;
        modules.push(module);
    }
    Ok(modules)
}

fn is_query(ep: &Endpoint) -> bool {
    ep.method == Method::Get && !ep.success.stream
}

fn is_stream(ep: &Endpoint) -> bool {
    ep.success.stream || ep.method == Method::Ws
}

fn has_empty_request(ep: &Endpoint) -> bool {
    ep.params.iter().all(|p| p.stream)
}

/// The union of the declared error classes of `ep` and the two transport failures.
fn error_union(fc: &mut FileCtx<'_, '_>, ep: &Endpoint) -> String {
    let mut seen: Vec<DeclId> = Vec::new();
    let mut parts: Vec<String> = Vec::new();
    for e in &ep.errors {
        if !seen.contains(&e.decl) {
            seen.push(e.decl);
            parts.push(fc.type_ref(e.decl));
        }
    }
    parts.push(fc.rt("VeldHttpError"));
    parts.push(fc.rt("VeldTransportError"));
    parts.join(" | ")
}

fn write_service(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    pkg: PackageId,
) {
    let pascal = naming::pascal(&svc.name);
    let key = naming::camel(&svc.name);
    let class = format!("{pascal}Client");
    fc.import(&fc.tree.client_module(pkg), &class, true);
    let endpoints: Vec<&Endpoint> = svc.endpoints.iter().filter(|e| !is_stream(e)).collect();
    for ep in &endpoints {
        let req = names.get(svc.id, &ep.name);
        fc.import(&fc.tree.client_module(pkg), &req, false);
        w.line(format!(
            "/** What `{}` of `{}` can fail with: a declared error (test it with `X.matches(e)`), or one of the two transport failures. */",
            ep.name, svc.name
        ));
        let union = error_union(fc, ep);
        w.line(format!(
            "export type {pascal}{}Error = {union};",
            naming::pascal(&ep.name)
        ));
        w.blank();
    }
    write_queries(w, fc, names, svc, &endpoints, (&pascal, &key, &class));
    w.blank();
    write_mutations(w, names, svc, &endpoints, (&pascal, &key, &class));
    w.blank();
}

fn write_queries(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    endpoints: &[&Endpoint],
    (pascal, key, class): (&str, &str, &str),
) {
    let canonical = fc.rt("canonical");
    let signal = fc.rt("AbortSignalLike");
    w.line(format!(
        "/**\n * Query keys and query options of `{}`. `defaultClient` makes the client when a call does not pass one,\n * so there is no shared instance: pass `new {class}({{ baseUrl }})` as the last argument, or build your own set.\n */",
        svc.name
    ));
    w.open(format!(
        "export function create{pascal}Queries(defaultClient: () => {class} = () => new {class}())"
    ));
    w.line(format!("const all = ['veld', {}] as const;", ts_str(key)));
    w.open_with("const keys =", " {");
    w.line("all,");
    for ep in endpoints.iter().filter(|e| is_query(e)) {
        let req = names.get(svc.id, &ep.name);
        let default = if has_empty_request(ep) { " = {}" } else { "" };
        w.line(format!(
            "{}: (req: {req}{default}) => [...all, {}, {canonical}(req)] as const,",
            method_key(&ep.name),
            ts_str(&ep.name)
        ));
    }
    w.close_with("};");
    w.open_with("return", " {");
    w.line("keys,");
    for ep in endpoints.iter().filter(|e| is_query(e)) {
        write_query(w, fc, names, svc, ep, class, &signal);
    }
    w.close_with("};");
    w.close();
    w.blank();
    w.line(format!(
        "/** Query keys and query options of `{}`, with a default client. */",
        svc.name
    ));
    w.line(format!(
        "export const {key}Queries = create{pascal}Queries();"
    ));
}

fn write_query(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    ep: &Endpoint,
    class: &str,
    signal: &str,
) {
    doc_comment(w, ep.docs.as_deref(), ep.deprecated.as_deref());
    let req = names.get(svc.id, &ep.name);
    let ret = match &ep.success.ty {
        Some(t) => ts_type(fc, t),
        None => "void".to_string(),
    };
    let default = if has_empty_request(ep) { " = {}" } else { "" };
    let m = method_key(&ep.name);
    w.open_with(
        format!("{m}: (req: {req}{default}, client: {class} = defaultClient()) =>"),
        " ({",
    );
    w.line(format!("queryKey: keys.{m}(req),"));
    w.line(format!(
        "queryFn: (ctx: {{ signal?: {signal} }}): Promise<{ret}> => client.{m}(req, {{ signal: ctx.signal }}),"
    ));
    w.close_with("}),");
}

fn write_mutations(
    w: &mut CodeWriter,
    names: &RequestNames,
    svc: &Service,
    endpoints: &[&Endpoint],
    (pascal, key, class): (&str, &str, &str),
) {
    w.line(format!(
        "/** Mutation options of `{}`: every endpoint that is not a plain GET. */",
        svc.name
    ));
    w.open(format!(
        "export function create{pascal}Mutations(defaultClient: () => {class} = () => new {class}())"
    ));
    w.open_with("return", " {");
    for ep in endpoints.iter().filter(|e| !is_query(e)) {
        let req = names.get(svc.id, &ep.name);
        let m = method_key(&ep.name);
        let ret = format!("ReturnType<{class}['{m}']>");
        let default = if has_empty_request(ep) { " = {}" } else { "" };
        doc_comment(w, ep.docs.as_deref(), ep.deprecated.as_deref());
        w.open_with(
            format!("{m}: (client: {class} = defaultClient()) =>"),
            " ({",
        );
        w.line(format!(
            "mutationKey: ['veld', {}, {}] as const,",
            ts_str(key),
            ts_str(&ep.name)
        ));
        w.line(format!(
            "mutationFn: (req: {req}{default}): {ret} => client.{m}(req),"
        ));
        w.close_with("}),");
    }
    w.close_with("};");
    w.close();
    w.blank();
    w.line(format!(
        "/** Mutation options of `{}`, with a default client. */",
        svc.name
    ));
    w.line(format!(
        "export const {key}Mutations = create{pascal}Mutations();"
    ));
}
