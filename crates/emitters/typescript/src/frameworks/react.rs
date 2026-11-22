//! React hooks (`framework = "react"`, React 18). Only React built-ins are used.

use veld_codegen::{
    CodeWriter, FileSet, FrameworkAdapter, GenDiagnostic, LanguageVersion, Profile,
};

use super::{
    ClientModel, Flavor, PackageModel, framework_files, header, missing, write_clients_type,
    write_hooks,
};

/// The framework id, as written in `veld.toml`.
pub const ID: &str = "react";

const RUNTIME: &str = include_str!("../../runtime/react.ts");
const STREAM_RUNTIME: &str = include_str!("../../runtime/react_stream.ts");

/// The `react` framework (React 18).
#[derive(Debug, Clone, Copy)]
pub struct React;

impl FrameworkAdapter<ClientModel> for React {
    fn id(&self) -> &'static str {
        ID
    }

    fn framework_versions(&self) -> &'static [&'static str] {
        &["18"]
    }

    fn min_language(&self) -> LanguageVersion {
        LanguageVersion::es(2020)
    }

    fn files(&self, model: &ClientModel, profile: &Profile) -> Result<FileSet, Vec<GenDiagnostic>> {
        if profile.framework() != Some(ID) {
            return Ok(FileSet::new());
        }
        framework_files(ID, RUNTIME, STREAM_RUNTIME, model, package_source)
    }
}

pub(super) const FLAVOR: Flavor = Flavor {
    id: ID,
    title: "React hooks",
    imports: &[
        "import { Fragment, createContext, createElement, useContext } from 'react';",
        "import type { ReactElement, ReactNode } from 'react';",
    ],
    query_request: |req| format!("{req} | null"),
};

fn package_source(pkg: &PackageModel, path: &str) -> String {
    let mut w = CodeWriter::new();
    header(&mut w, pkg, &FLAVOR, path);
    for svc in &pkg.services {
        w.line(format!(
            "const {}Context = createContext<{} | null>(null);",
            svc.key, svc.client
        ));
    }
    w.blank();
    write_clients_type(&mut w, pkg);
    w.blank();
    write_provider(&mut w, pkg);
    for svc in &pkg.services {
        w.blank();
        w.line(format!(
            "/** The `{}` from the nearest `VeldProvider`, or `client` when given. */",
            svc.client
        ));
        w.open(format!(
            "export function use{}Client(client?: {}): {}",
            svc.name, svc.client, svc.client
        ));
        w.line(format!("const provided = useContext({}Context);", svc.key));
        w.line("const found = client ?? provided;");
        w.line(format!(
            "if (!found) {}",
            missing(
                svc,
                "render inside <VeldProvider clients={ ... }> or pass { client }"
            )
        ));
        w.line("return found;");
        w.close();
        write_hooks(&mut w, svc, &FLAVOR);
    }
    w.finish()
}

fn write_provider(w: &mut CodeWriter, pkg: &PackageModel) {
    w.line("/** Makes the configured clients available to the hooks below. */");
    w.open("export function VeldProvider(props: { clients: VeldClients; children?: ReactNode }): ReactElement");
    w.line("let tree: ReactNode = props.children;");
    for svc in &pkg.services {
        w.open(format!("if (props.clients.{})", svc.key));
        w.line(format!(
            "tree = createElement({}Context.Provider, {{ value: props.clients.{} }}, tree);",
            svc.key, svc.key
        ));
        w.close();
    }
    w.line("return createElement(Fragment, null, tree);");
    w.close();
}
