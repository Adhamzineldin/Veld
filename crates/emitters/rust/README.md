# Rust emitter

Veld generates a complete Cargo crate for each Rust server or client target. The server
provides a service trait and an Axum router. The typed client accepts a custom transport or
uses the selected reqwest or ureq adapter. The blocking ureq adapter supports HTTP and SSE.
Contracts with WebSocket endpoints require a different client transport.

A target that has settings (a service URL for each service it calls, the rule output mode of a
server with rules) also gets `runtime_config.rs`: a typed runtime config with one check, readers
for nested values, flat maps and environment variables, and `clients(&runtime, options)`. Generated
code reads no environment variable of its own (`spec/targets/rust.md` section 15).

The [book catalog example](../../../examples/rust-catalog) runs a handwritten service and
command-line client against the generated crates. The [target specification](../../../spec/targets/rust.md)
describes the type mappings, generated signatures, framework options and streaming behavior.

## Development checks

Run `cargo test -p veld-emit-rust` from the repository root. With
`VELD_REQUIRE_TOOLCHAINS=1`, missing toolchains fail the suite. The toolchain tests build and
lint generated crates, exercise Rust, Node and TypeScript peers over HTTP and streaming
connections, and check canonical identity and wire encodings.
