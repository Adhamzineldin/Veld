//! Vue composables (`framework = "vue"`, Vue 3.3 or newer). Only Vue's reactivity API and
//! `provide`/`inject` are used.

use veld_codegen::{
    CodeWriter, FileSet, FrameworkAdapter, GenDiagnostic, LanguageVersion, Profile,
};

use super::{
    ClientModel, Flavor, PackageModel, framework_files, header, missing, write_clients_type,
    write_hooks,
};

/// The framework id, as written in `veld.toml`.
pub const ID: &str = "vue";

const RUNTIME: &str = include_str!("../../runtime/vue.ts");

/// The `vue` framework (Vue 3).
#[derive(Debug, Clone, Copy)]
pub struct Vue;

impl FrameworkAdapter<ClientModel> for Vue {
    fn id(&self) -> &'static str {
        ID
    }

    fn framework_versions(&self) -> &'static [&'static str] {
        &["3"]
    }

    fn min_language(&self) -> LanguageVersion {
        LanguageVersion::es(2020)
    }

    fn files(&self, model: &ClientModel, profile: &Profile) -> Result<FileSet, Vec<GenDiagnostic>> {
        if profile.framework() != Some(ID) {
            return Ok(FileSet::new());
        }
        framework_files(ID, RUNTIME, model, package_source)
    }
}

const FLAVOR: Flavor = Flavor {
    id: ID,
    title: "Vue composables",
    imports: &[
        "import { inject } from 'vue';",
        "import type { InjectionKey, MaybeRefOrGetter, Plugin } from 'vue';",
    ],
    query_request: |req| format!("MaybeRefOrGetter<{req} | null>"),
};

fn package_source(pkg: &PackageModel, path: &str) -> String {
    let mut w = CodeWriter::new();
    header(&mut w, pkg, &FLAVOR, path);
    for svc in &pkg.services {
        w.line(format!(
            "const {}Key: InjectionKey<{}> = Symbol('veld:{}');",
            svc.key, svc.client, svc.key
        ));
    }
    w.blank();
    write_clients_type(&mut w, pkg);
    w.blank();
    write_plugin(&mut w, pkg);
    for svc in &pkg.services {
        w.blank();
        w.line(format!(
            "/** The `{}` installed by `createVeldPlugin`, or `client` when given. */",
            svc.client
        ));
        w.open(format!(
            "export function use{}Client(client?: {}): {}",
            svc.name, svc.client, svc.client
        ));
        w.line(format!(
            "const found = client ?? inject({}Key, null);",
            svc.key
        ));
        w.line(format!(
            "if (!found) {}",
            missing(
                svc,
                "install createVeldPlugin({ ... }) on the app or pass { client }"
            )
        ));
        w.line("return found;");
        w.close();
        write_hooks(&mut w, svc, &FLAVOR);
    }
    w.finish()
}

fn write_plugin(w: &mut CodeWriter, pkg: &PackageModel) {
    w.line("/** A Vue plugin that provides the configured clients to every component and composable. */");
    w.open("export function createVeldPlugin(clients: VeldClients): Plugin");
    w.open_with("return", " {");
    w.open("install(app)");
    for svc in &pkg.services {
        w.line(format!(
            "if (clients.{}) app.provide({}Key, clients.{});",
            svc.key, svc.key, svc.key
        ));
    }
    w.close_with("},");
    w.close_with("};");
    w.close();
}
