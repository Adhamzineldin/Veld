# Veld emitter plugin protocol — v1

Status: **DRAFT.**

A plugin is an executable that `veld.toml` pins by `command`, `version`, `sha256` and an optional `entry`. When `entry` is set, the `sha256` covers that script file and Veld appends it to `command` unless `command` already contains it; otherwise the `sha256` covers the resolved executable. A mismatch is `E0705`, checked before every run. Veld runs it as a child process. The working directory is a fresh empty temporary directory, the environment is empty apart from `PATH` (on Windows also `SystemRoot`, `SystemDrive`, `windir`, `TEMP`, `TMP`, `PATHEXT` and `ComSpec`, without which processes such as Node cannot start), and stdin and stdout are pipes. Each message is a single line of UTF-8 JSON terminated by `\n`. The plugin sends human logs to stderr, and Veld forwards them with the `[plugin:<name>]` prefix. Because of that, an argument of `command` that names an existing file relative to the project root (for example `node plugins/x/main.js`) is made absolute before the process starts.

## 1. Handshake

Veld sends `{"type":"hello","protocol":"1","veld_version":"2.0.0"}`.

The plugin replies within 5 s:
```json
{ "type": "hello", "protocol": "1", "name": "kotlin", "version": "1.2.0",
  "roles": ["server", "client"], "capabilities": ["constraints", "auth", "page"],
  "options_schema": { /* JSON Schema for [targets.options], optional */ },
  "frameworks": { "server": [ { "id": "ktor", "versions": ["2"], "min_language": "1.9" } ], "client": [] },
  "language": { "id": "kotlin", "versions": ["1.9", "2.0"] } }
```

If the plugin speaks a different protocol major version, Veld fails with `E0706`.

`frameworks` and `language` are **optional**, so a plugin written against the first draft of protocol v1 keeps working. They let the host validate a target with the same central check as the built-in generators (`spec/config.md`, "Language version and framework version"):

- `frameworks` maps a role (`server`, `client`) to the frameworks the plugin supports. Each entry has an `id`, optional `versions` (strings, the first is the default; absent or empty means the framework is unversioned) and an optional `min_language` (the oldest language version its output needs, `"17"`, `"1.9"` or `"es2020"`). Absent `frameworks`, or an absent role, means **no frameworks**: any `framework` in `veld.toml` is `E0803`.
- `language` has the language `id` and its supported `versions` (the first is the default). The target selects one with the option named after the `id`, here `kotlin = "2.0"`. An unknown value is `E0802`; a framework whose `min_language` is newer than the selected version is `E0803`. Absent `language` means **any version**: nothing is checked and the option is passed through to the plugin untouched.
- `[targets.options] framework-version = "2"` is checked against the framework's `versions` (`E0803` when it is not listed or the framework has none). The host passes `options` to the plugin unchanged, so the plugin reads the language and framework version there.
- A malformed declaration (a non-string `id`, `versions` that is not an array of strings, a `min_language` that is not a version, a duplicate framework id, an unknown role) is `E0706`.

`veld plugin test` generates once for every declared language version and for every declared framework with each of its versions, and fails when one of them is rejected or fails.

## 2. Generate

Veld sends:
```json
{ "type": "generate", "role": "client", "ir": { /* spec/ir.md */ },
  "services": [0, 2], "options": { }, "consumes": [1] }
```
The plugin replies within 60 s (configurable, maximum 600 s):
```json
{ "type": "result",
  "files": [ { "path": "client/api.kt", "content": "…", "executable": false } ],
  "diagnostics": [ { "code": "E0801", "severity": "error", "message": "…", "ir_ref": "decl:3" } ] }
```

## 3. Host guarantees

- **Files the host refuses (`E0707`):**
  - absolute paths, or paths containing `..`, a drive prefix, a NUL byte or a backslash
  - more than 10,000 files, a single file over 10 MiB, or more than 200 MiB in total
- **Exit and timeout:** a timeout or a non-zero exit code is `E0708`. The previous output is left untouched.
- **How files are written:** files are written only through the staged writer and the manifest, the same path built-in generators use.
- **Capability check:** the host checks the plugin's capabilities before it sends `generate`.
- **Target check:** the host validates the target's `framework`, `framework-version` and language option against the `hello` declarations (`E0802`, `E0803`) before it sends `generate`, with the same code as for built-in generators.
- **Signature embedding:** plugins MUST embed the `sig` values given in the IR. CI for first-party plugins runs the signature conformance suite.
