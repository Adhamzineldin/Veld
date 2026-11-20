# Veld emitter plugin protocol — v1

Status: **DRAFT.**

A plugin is an executable that `veld.toml` pins by `command`, `version` and `sha256`. Veld runs it as a child process. The working directory is a fresh empty temporary directory, the environment is empty apart from `PATH`, and stdin and stdout are pipes. Each message is a single line of UTF-8 JSON terminated by `\n`. The plugin sends human logs to stderr, and Veld forwards them with the `[plugin:<name>]` prefix.

## 1. Handshake

Veld sends `{"type":"hello","protocol":"1","veld_version":"2.0.0"}`.

The plugin replies within 5 s:
```json
{ "type": "hello", "protocol": "1", "name": "kotlin", "version": "1.2.0",
  "roles": ["client"], "capabilities": ["constraints", "auth", "page"],
  "options_schema": { /* JSON Schema for [targets.options], optional */ } }
```

If the plugin speaks a different protocol major version, Veld fails with `E0706`.

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
- **Signature embedding:** plugins MUST embed the `sig` values given in the IR. CI for first-party plugins runs the signature conformance suite.
