//! The embedded runtime: real `.java` files, byte-identical in every role except for the
//! package declaration.

use veld_codegen::{FileSet, GenDiagnostic};

use crate::tree::java_path;
use crate::util::{HEADER, add_file};

/// The package the embedded sources declare. It is replaced by `<base>.runtime`.
const SOURCE_PACKAGE: &str = "dev.veld.runtime";
/// The package the embedded adapter sources declare. It is replaced by `<base>.adapters`.
const ADAPTER_PACKAGE: &str = "dev.veld.adapters";

macro_rules! runtime_files {
    ($($name:literal),* $(,)?) => {
        /// `(simple name, source)` of every runtime class, in a fixed order.
        pub const RUNTIME: &[(&str, &str)] = &[$(($name, include_str!(concat!("../runtime/", $name, ".java")))),*];
    };
}

runtime_files!(
    "ClientCore",
    "ClientDefaults",
    "ClientOptions",
    "Codec",
    "EndpointSpec",
    "HandlerHooks",
    "Issue",
    "Json",
    "JsonParser",
    "JsonWriter",
    "Page",
    "RequestContext",
    "Router",
    "Schema",
    "ServerEndpoint",
    "UntypedError",
    "Veld",
    "VeldError",
    "VeldHandler",
    "VeldHttpError",
    "VeldRequest",
    "VeldResponse",
    "VeldTransportError",
    "Wire",
);

/// Source of the JDK `HttpServer` adapter.
pub const JDK_ADAPTER: &str = include_str!("../adapters/JdkHttpServerAdapter.java");
/// Source of the Spring Boot adapter.
pub const SPRING_ADAPTER: &str = include_str!("../adapters/SpringAdapter.java");

/// The Java package of the runtime for a base package.
#[must_use]
pub fn package(base: &str) -> String {
    format!("{base}.runtime")
}

/// The Java package of the adapters for a base package.
#[must_use]
pub fn adapter_package(base: &str) -> String {
    format!("{base}.adapters")
}

/// A runtime or adapter source with its package (and the runtime import) substituted.
fn substitute(source: &str, base: &str) -> String {
    let text = source
        .replace(
            &format!("package {SOURCE_PACKAGE};"),
            &format!("package {};", package(base)),
        )
        .replace(
            &format!("package {ADAPTER_PACKAGE};"),
            &format!("package {};", adapter_package(base)),
        )
        .replace(
            &format!("import {SOURCE_PACKAGE}."),
            &format!("import {}.", package(base)),
        );
    format!("{HEADER}\n{text}")
}

/// Adds the runtime classes to `files`.
pub fn emit(files: &mut FileSet, base: &str) -> Result<(), Vec<GenDiagnostic>> {
    let package = package(base);
    for (name, source) in RUNTIME {
        add_file(files, &java_path(&package, name), substitute(source, base))?;
    }
    Ok(())
}

/// Adds the JDK adapter, and the Spring adapter when `spring` is set.
pub fn emit_adapters(
    files: &mut FileSet,
    base: &str,
    spring: bool,
) -> Result<(), Vec<GenDiagnostic>> {
    let package = adapter_package(base);
    add_file(
        files,
        &java_path(&package, "JdkHttpServerAdapter"),
        substitute(JDK_ADAPTER, base),
    )?;
    if spring {
        add_file(
            files,
            &java_path(&package, "SpringAdapter"),
            substitute(SPRING_ADAPTER, base),
        )?;
    }
    Ok(())
}
