# ADR-003: External emitters run as executables speaking versioned JSON over stdin/stdout

- Status: PROPOSED (accept by 2026-11-01; protocol frozen at the end of M3, 2027-02-07)
- Date: 2026-10-21
- Deciders: T2 (owner), T1, T4; reviewer: T3 (security)
- Evidence: chapter 10; no plugin mechanism exists at 6a47f81 (`internal/emitter/emitter.go:90-118` is an in-process registry).

## Decision

Adopt the protoc/buf model: an emitter is an executable `veld-emitter-<name>` (or an explicit `command` in config). The host sends one `EmitRequest` JSON document (protocol version, host version, IR v1, options) on stdin; the plugin writes one `EmitResponse` JSON document (files, diagnostics, summary) on stdout and logs on stderr. The host performs all file writes after validating paths, applies timeouts and size caps, and records files in the output manifest. A `--describe` call returns the plugin manifest for compatibility checks.

WebAssembly (Extism on wazero) is deferred; the JSON schema is designed so a Wasm transport can carry the same documents later. HashiCorp go-plugin and in-process Go plugins are rejected (chapter 10, section 10.2).

## Reasons

- Any language can implement it with a JSON library; no Veld-specific runtime required.
- Process isolation and host-mediated writes give output safety without a sandbox.
- The approach is proven at scale by protoc plugins and buf local plugins (buf.build/docs/generate, accessed 2026-10-21 via search summary).

## Security statement (must be documented verbatim for users)

Plugins are trusted code. The host does not restrict a plugin's filesystem or network access. The host does guarantee that a plugin cannot write outside its configured output directory through the protocol, that a hung plugin is killed, and that a plugin is only executed when it is named with a pinned version in `veld.config.json`.

## Consequences

- Requires ADR-002 (IR) first.
- A TypeScript SDK, a hello emitter, and one extracted built-in emitter are the acceptance tests (chapter 10, section 10.4).
- The plugin protocol is versioned separately from the IR and the compiler (ADR-009).
