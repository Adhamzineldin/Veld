# ADR-010: Keep a single repository; one Go module for binaries, npm workspace for TypeScript packages

- Status: PROPOSED (accept by 2026-11-01)
- Date: 2026-10-21
- Deciders: T5 (owner), T1; reviewer: T2
- Evidence: the repository already contains the CLI, registry, editors, website and distribution wrappers; the team is six people with no release engineer.

## Decision

- One Git repository (`Veld`). One Go module for `cmd/veld`, `cmd/veld-registry`, `cmd/generate-language` and all `internal/` packages.
- One npm workspace (`package.json` with `workspaces`) for `editors/vscode`, `website`, `sdk/emitter-ts`, and `plugins/hello-emitter`, so that one `npm ci` at the root installs everything CI needs.
- The JetBrains plugin keeps its Gradle build and is built only in a scheduled CI job (chapter 11).
- Independently versioned products: `veld` (CLI), `veld-registry`, `@veld/emitter-sdk`, the VS Code extension, the JetBrains plugin. They share one repository and one changelog with per-product sections; tags are prefixed (`v1.0.0`, `registry/v1.0.0`, `sdk/v0.3.0`, `vscode/v0.5.0`).

## Reasons

- Cross-cutting changes (IR schema, SDK types, docs) land in one pull request and are tested together.
- A six-person student team cannot afford release coordination across repositories.
- Separate deployables (ADR-004) do not require separate repositories.

## Consequences

- CI is split into fast per-path jobs (Go, TypeScript, docs) using path filters, and a scheduled full matrix (chapter 14).
- `CODEOWNERS` maps directories to the owners in chapter 16.
