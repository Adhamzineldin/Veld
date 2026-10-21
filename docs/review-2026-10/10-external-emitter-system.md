# 10. External Emitter System (Veld Emitter Protocol, VEP v1)

**Status:** PROPOSED DESIGN. Nothing in this chapter exists in the reviewed revision (6a47f81). The only extension point today is the in-process Go registry in `internal/emitter/emitter.go:90-118` (`RegisterBackend` / `RegisterFrontend` / `RegisterTool`, populated from `init()` functions and blank imports in `cmd/veld/main.go`). That is a built-in registry, not a plugin system: adding an emitter requires editing and rebuilding the compiler.

## 10.1 Problem addressed

- Veld's value proposition depends on supporting many targets, but every target is a Go package compiled into the binary. The team cannot maintain 20+ emitters, and third parties cannot add one without a fork.
- The emitter interface (`Emit(a ast.AST, outDir string, opts EmitOptions) error`, `emitter.go:43-45`) hands emitters the raw AST and a writable directory. Emitters write files themselves, so the host cannot enforce output ownership, determinism, or path containment.
- There is no serialized, versioned intermediate representation (IR). `veld ast` dumps the Go AST struct as JSON, which is an implementation detail, not a contract.

## 10.2 Options compared

| Option | Portability | Host complexity | Plugin-author complexity | Trust / isolation | Verdict |
|---|---|---|---|---|---|
| In-process Go plugins (`plugin` package) | Linux/macOS only; must match exact Go toolchain and dependency versions | Low | High (Go only, rebuild per Veld version) | None (same process) | Rejected. Unsupported on Windows, version-fragile. |
| HashiCorp go-plugin (gRPC over local socket) | All OS | Medium (gRPC codegen, handshake) | Medium | Process isolation only | Rejected for v1: adds protobuf/gRPC machinery; more than needed. |
| **Executable plugin, versioned JSON over stdin/stdout (protoc / buf model)** | All OS, any language | Low (exec, pipe, JSON) | Low (read JSON, write JSON) | Process isolation only; no sandbox | **Chosen for v1.** Proven by protoc (`CodeGeneratorRequest`/`Response`) and buf local plugins. |
| WebAssembly (Extism on wazero, pure Go, no cgo) | All OS; authors need a Wasm toolchain | Medium | Medium to high (PDK per language) | Real capability-based sandbox: no FS/network unless granted | Deferred to post-graduation. Keep the protocol JSON-shaped so a Wasm transport can be added without changing the schema. |
| Language-hosted extensions (embedded JS runtime loads TS plugins) | Requires Node at runtime | Medium | Low for TS authors | None | Rejected: ties the compiler to Node, contradicts single-binary distribution. |

External facts used (provisional; WebFetch was blocked during this review, so these rest on search-result summaries accessed 2026-10-21): buf local plugins are `protoc-gen-<name>` executables found on PATH that read the schema on stdin and write files on stdout, and remote plugins run on the Buf Schema Registry (buf.build/docs/generate, buf.build/docs/bsr/remote-plugins). Extism's Go SDK runs on wazero, a pure-Go runtime. WASI 0.3.0 was released 2026-06-11 (wasi.dev).

## 10.3 Design

### 10.3.1 Roles and trust boundary

```
veld CLI (host, trusted)                     plugin process (trusted by installation, NOT sandboxed)
+------------------------------+   stdin     +-----------------------------+
| compile -> IR (JSON)         | ----------> | read EmitRequest            |
| spawn plugin, enforce timeout|             | produce files in memory     |
| read EmitResponse            | <---------- | write EmitResponse (stdout) |
| validate paths, write files  |   stdout    | logs -> stderr              |
| record manifest, diagnostics |             +-----------------------------+
+------------------------------+
```

- The plugin never touches the output directory. It returns file contents; the host performs all writes. This is what makes output mediation, path containment, the generated-files manifest and determinism checks possible.
- Process isolation is not sandboxing. A plugin runs with the invoking user's privileges and can read any file, open network connections, or spawn processes. The host does not and cannot prevent this in v1. The enforceable controls are: (a) the host refuses to write any returned path that escapes `outDir`; (b) a wall-clock timeout kills the process; (c) stdout size is capped; (d) the plugin's version and protocol version are checked before execution; (e) plugins run only when named in `veld.config.json`, never discovered implicitly from the working directory.
- Policy: plugins are trusted code, with the same trust level as an npm or Go dependency. Users pin versions. Documentation must say this plainly. A future Wasm transport is the path to running untrusted plugins.

### 10.3.2 Discovery and installation (reuse existing package managers)

Veld does not ship its own plugin package manager. A plugin is an executable resolved in this order:

1. `command` in config (explicit path or program name), e.g. `"command": "./node_modules/.bin/veld-emitter-elixir"`.
2. `veld-emitter-<name>` on `PATH` (protoc convention).
3. `node_modules/.bin/veld-emitter-<name>` relative to the config directory when `package` is set.

```json
{
  "emitters": [
    { "name": "elixir", "package": "@acme/veld-emitter-elixir", "version": "1.2.0",
      "out": "../backend/lib/generated", "options": { "module": "MyApp.Api" } },
    { "name": "hello", "command": "./tools/veld-emitter-hello", "version": "0.1.0", "out": "./docs/api" }
  ]
}
```

Installation is the user's package manager (`npm i -D @acme/veld-emitter-elixir`, `go install`, `cargo install`, or a downloaded binary). `veld doctor` reports each configured plugin: found / not found / version mismatch / protocol mismatch.

### 10.3.3 Manifest (returned by `--describe`)

```json
{
  "manifestVersion": 1,
  "name": "@acme/veld-emitter-elixir",
  "version": "1.2.0",
  "protocolVersions": [1],
  "irVersions": [">=1.0 <2.0"],
  "kind": "backend",
  "capabilities": {
    "headers": true, "maps": true, "extends": "flatten", "recursiveModels": true,
    "statusCodes": true, "middleware": "ignored", "deprecated": true
  },
  "optionsSchema": { "$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object",
    "properties": { "module": { "type": "string" } }, "additionalProperties": false },
  "deterministic": true
}
```

`capabilities` lets the host warn before emission when a contract uses a feature the plugin declares unsupported (`false`) or degraded (`"ignored"`, `"flatten"`). This is how unsupported semantics fail clearly instead of silently generating incorrect code.

### 10.3.4 Exchange

Invocation: `veld-emitter-elixir --emit` with one `EmitRequest` JSON document on stdin; the plugin writes exactly one `EmitResponse` JSON document on stdout and exits 0. Non-zero exit, invalid JSON, or timeout means plugin failure: the host writes nothing for that emitter and reports the failure with the stderr tail.

```jsonc
// EmitRequest (host -> plugin)
{
  "protocolVersion": 1,
  "host": { "name": "veld", "version": "0.5.0" },
  "irVersion": "1.0",
  "ir": { /* normalized IR, see chapter 08 section 8.3 */ },
  "options": { "module": "MyApp.Api" },
  "outDir": "lib/generated",            // informational; plugin must not write here
  "env": { "os": "windows", "pathSeparator": "\\" }
}
```

```jsonc
// EmitResponse (plugin -> host)
{
  "protocolVersion": 1,
  "files": [
    { "path": "my_app/api/types.ex", "content": "defmodule ...", "encoding": "utf-8" },
    { "path": "my_app/api/router.ex", "content": "...", "encoding": "utf-8", "executable": false }
  ],
  "diagnostics": [
    { "severity": "warning", "code": "ELIXIR_MIDDLEWARE_IGNORED",
      "message": "middleware AuthGuard on Users.Create is not supported by this emitter",
      "span": { "file": "veld/modules/users.veld", "line": 14, "column": 5 } }
  ],
  "summary": [ { "dir": "my_app/api/", "files": "types.ex, router.ex" } ]
}
```

Rules enforced by the host:

- `path` must be relative, use `/`, contain no `..` segments, no absolute prefix, no drive letter, and after joining must stay inside `outDir` (checked on the cleaned absolute path with a trailing-separator prefix test, not a string prefix on the entry name).
- `files` are sorted by path before writing; duplicate paths are an error.
- Total response size cap (default 64 MiB) and timeout (default 120 s, configurable per plugin).
- Diagnostics with `severity: "error"` abort writing for that emitter.
- Writes go through the same manifest and atomic-write path as built-in emitters (chapter 08 section 8.6).

Progress and logging: stderr lines are forwarded to the user prefixed with the plugin name when `--verbose` is set, otherwise buffered and shown only on failure. No streaming protocol in v1; a plugin that needs progress reporting writes to stderr. Cancellation: the host kills the process on Ctrl-C or timeout.

### 10.3.5 Compatibility negotiation

1. Host runs `--describe`, parses the manifest.
2. Host checks `protocolVersions` contains a version the host speaks (v1 only at graduation).
3. Host checks the host IR version satisfies the plugin's `irVersions` range.
4. Host checks `version` equals the pinned `version` in config (exact match; range syntax deliberately unsupported in v1).
5. Any mismatch is a hard error with a one-line remedy. `--allow-plugin-mismatch` downgrades steps 3 and 4 to warnings for local development.

### 10.3.6 Determinism

The plugin contract requires identical output for identical `EmitRequest`. The host offers `veld generate --check-determinism`, which runs each emitter twice and diffs the responses; CI for the reference plugin runs this. Plugins must not embed timestamps, absolute paths, or random IDs.

## 10.4 Plugin SDK and development journey ("hello emitter")

SDK language decision: TypeScript first (`@veld/emitter-sdk`), because the people most likely to write an emitter for a web-API tool are JavaScript developers, and npm already solves distribution. The Go host's own request/response types are exported from `internal/ir` and `internal/emit/plugin` so a Go plugin can import them; a separate Go SDK package is not planned for graduation.

```ts
// hello-emitter/src/index.ts  (proposed SDK surface)
import { defineEmitter } from "@veld/emitter-sdk";

export default defineEmitter({
  name: "veld-emitter-hello",
  version: "0.1.0",
  kind: "tool",
  capabilities: { headers: true, maps: true, extends: "preserve", recursiveModels: true },
  emit(ir, ctx) {
    const lines = ["# API summary", ""];
    for (const m of ir.models) lines.push(`- model ${m.name} (${m.fields.length} fields)`);
    for (const s of ir.services) for (const op of s.operations)
      lines.push(`- ${op.method} ${s.prefix}${op.path} -> ${op.name}`);
    ctx.file("API.md", lines.join("\n") + "\n");
    if (ir.services.some(s => s.operations.some(o => o.middleware.length)))
      ctx.warn("HELLO_MIDDLEWARE_IGNORED", "middleware is not represented in API.md");
  },
});
```

Journey (target: under 30 minutes for a TypeScript developer):

1. `npm create veld-emitter@latest hello` scaffolds the package above with a `bin` entry `veld-emitter-hello`, tests, and a fixture IR.
2. `npm test` runs the SDK test harness: it loads `fixtures/*.ir.json` (shipped with the SDK, generated from the conformance suite), calls `emit`, and snapshots the `EmitResponse`.
3. `npm link`, then in a Veld project add `{ "name": "hello", "package": "veld-emitter-hello", "version": "0.1.0", "out": "./docs" }` to `veld.config.json`.
4. `veld doctor` shows the plugin as found with a compatible protocol; `veld generate` writes `docs/API.md` and lists it in the manifest.
5. `npm publish`; a user runs `npm i -D veld-emitter-hello` and the same config works.

Acceptance test for the graduation release: one reference emitter (the hello emitter above, plus one non-trivial external emitter extracted from the current built-ins; see chapter 09 for the candidate) installed from a registry into a clean project, executed by a release binary, with output byte-identical across two runs and two operating systems.

## 10.5 What is explicitly not promised

- No sandboxing of plugin filesystem or network access in v1.
- No remote plugin execution (buf-style) and no plugin hosting in the Veld registry at graduation; plugins are distributed through npm, Go, or GitHub releases. The Veld registry remains data-only (contracts) at graduation; see chapter 13.
- No incremental or streaming protocol.
- No version ranges for plugin pins.

## 10.6 Cost and sequence

| Step | Depends on | Estimate | Owner (chapter 16) |
|---|---|---|---|
| IR schema v1, JSON Schema, `veld ir` command | compiler IR work (ch. 08) | 24 to 40 h | T1 |
| Host: process runner, timeout, path mediation, manifest integration | output manifest (ch. 08 section 8.6) | 24 to 36 h | T2 |
| `--describe` handshake, compatibility checks, `veld doctor` integration | host runner | 8 to 14 h | T2 |
| TypeScript SDK, IR types generated from JSON Schema, test harness | IR schema | 24 to 40 h | T4 |
| Hello emitter and `npm create` scaffold | SDK | 8 to 16 h | T4 |
| Extract one built-in emitter to external (acceptance) | SDK, host | 16 to 30 h | T4 with T2 |
| Docs: plugin author guide, protocol reference | all above | 10 to 16 h (D1 writes, T2 validates) | D1 |

Total: 114 to 192 h of technical work, roughly 10 to 17 percent of the team's committed technical capacity (chapter 15). This is affordable only if the emitter matrix is reduced as recommended in chapter 09.

## 10.7 Success demonstration

- A plugin written in TypeScript, installed from npm into a fresh project, produces files through a release binary of Veld on Windows and Linux.
- A plugin that returns `../escape.txt` is refused with a clear error and no file is written outside `outDir`.
- A plugin that hangs is killed after the timeout, and `veld generate` exits non-zero with the plugin's stderr tail.
- A contract using a feature the plugin declares unsupported produces a diagnostic before emission.
