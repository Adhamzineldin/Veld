# ADR-006: The language server is the single language engine; VS Code is the primary editor; JetBrains moves to LSP4IJ later

- Status: PROPOSED (accept by 2026-11-01)
- Date: 2026-10-21
- Deciders: T5 (owner), T1; reviewer: T4
- Evidence (LSP and editors audit track, chapter 05 F-LSP items):
  - `veld lsp` advertises only `textDocumentSync: 1`, completion, hover and definition (`internal/lsp/handler.go:234-243`), never resolves imports (`handler.go:261-273`, `324-365`), uses byte offsets for positions, and reports only the first error on a whole line.
  - The VS Code extension does not use the server at all: `editors/vscode/src/extension.ts:81-1675` is a 1,676-line TypeScript re-implementation of lexing, validation and completion; its three contributed commands and two settings are never registered.
  - The JetBrains plugin is a third implementation: about 4,090 lines of hand-written Kotlin PSI code (`VeldLexer.kt`, `VeldParser.kt`, `VeldAnnotator.kt`), with diagnostics from a `veld validate` subprocess on files on disk rather than the editor buffer.
  - `internal/language` is imported only by `cmd/generate-language`; the lexer, parser and LSP hard-code their own keyword lists, so the "single source of truth" claim does not hold for the compiler.

Three implementations of the language exist, and the two in editors already disagree with the compiler (both reject the valid `headers:` directive).

## Decision

1. The Go language server, built on the shared compiler API (ADR-002), is the only implementation of diagnostics, completion, hover, definition and (later) references, document symbols and formatting. Keyword and directive lists in the server come from `internal/language`, and a Go test asserts the parser accepts exactly those lists.
2. VS Code is the primary graduation editor. The extension becomes a thin `vscode-languageclient` launcher plus TextMate grammar generated from `internal/language`; the hand-written language service is deleted. Commands (`validate`, `generate`, `generate --dry-run`) are wired to the CLI via `veld.executablePath`.
3. Server discovery in VS Code: use `veld.executablePath` if set; otherwise `veld` on PATH; otherwise offer to download the matching release asset for the platform from GitHub Releases, verify its SHA-256 against the release's checksum file, and store it under the extension's global storage. Version compatibility is checked with `veld --version --json`; a mismatch shows a message with the required version. Offline with no binary: syntax highlighting only, with a status-bar hint.
4. JetBrains receives limited support at graduation: the existing PSI plugin keeps syntax highlighting only; its hand-written annotator and completion are removed to stop drift; diagnostics, completion and navigation are provided through LSP4IJ pointing at the same server. LSP4IJ 0.21.x is current and listed as compatible with all IntelliJ flavours (plugins.jetbrains.com listing, nightly 0.21.1 dated 2026-10-20, search summary accessed 2026-10-21); JetBrains' own LSP API became available to all IntelliJ IDEA users with the 2025.3 unified distribution and is being open-sourced in the 2026.2 cycle (blog.jetbrains.com, search summaries). The JetBrains work is scheduled after the Beta gate and is the first item cut if capacity drops.

## Reasons

- One engine removes two sources of drift and the ongoing cost of re-implementing every language change three times (audit estimate: 20 to 32 h/month for both editors as built versus 2 to 3 h/month for a thin VS Code client after a one-time 60 to 100 h server investment).
- VS Code has the largest audience for a TypeScript-centric tool and the cheapest client mechanism.

## Alternatives rejected

- Keep the TypeScript service as the VS Code engine: fast locally, but it is already wrong and will diverge again.
- JetBrains native LSP API as the primary JetBrains path: closed source until 2026.2 and tied to the unified distribution; LSP4IJ is open and already compatible with the 2024.2+ range.

## Consequences

- Chapter 11 sequences the server work (M2) before the VS Code rewrite (M4) and the LSP4IJ step (post-Beta).
- `editors/vscode/out/` is untracked; a lockfile is committed; a CI job compiles the extension.
