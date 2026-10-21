# ADR-001: Retain Go for the compiler, CLI, language server and registry server

- Status: PROPOSED (for team acceptance in Sprint 0, by 2026-11-01)
- Date: 2026-10-21
- Deciders: T1 (owner), T2, T3, T5; reviewer: whole team
- Evidence: chapter 06 decision matrices; chapter 02 baseline (build, vet and all tests green at 6a47f81 on Go 1.26.0)

## Context

The reviewed revision is a single Go module (`github.com/Adhamzineldin/Veld`) of 43,886 non-test and 12,180 test lines (tracked `.go` files, measured with `wc -l` on `git ls-files`). The pipeline (lexer, parser, validator, loader, emitters, LSP, registry client and server) builds and passes `go test ./...` on Go 1.26.0. The team has roughly 1,100 committed technical hours between now and the readiness target (chapter 15). The prompt asked whether a rewrite in Rust or TypeScript would be a better foundation.

## Decision

Keep Go for every process that ships as a binary: the compiler and CLI (`cmd/veld`), the language server (same binary), and the registry server (to be split into its own binary, see ADR-004). Raise `go.mod` from `go 1.22` to `go 1.26`, the oldest release still supported on 2026-10-21 (Go 1.26 released 2026-02-10 and Go 1.27 released 2026-08-19 per go.dev/doc/devel/release; each major release is supported until two newer ones exist).

TypeScript is used only where it is the native language of the host: the VS Code extension, the website, the registry SPA, and the emitter plugin SDK (ADR-003). Kotlin remains only in the JetBrains plugin.

## Reasons

1. Delivery risk. Re-implementing 44k lines in another language with part-time students in 32 weeks is not credible; a rewrite would consume the entire capacity and leave no time for evaluation and the report.
2. The weaknesses found (chapter 05) are in boundaries, output safety, registry robustness and the emitter matrix, not in the choice of Go. None of them is fixed by changing language.
3. Go's single static binary, cross-compilation and `go:embed` match the distribution model (wrappers download one binary per platform).
4. Contributor accessibility: Go's small surface helps a student team and future contributors more than Rust's learning curve; Node-based single-binary packaging is still less mature than Go's.

## Alternatives rejected

- Rust: better for a long-lived compiler with heavy performance needs; Veld's inputs are small (contracts of hundreds of lines), so performance is not a differentiator. Cost: team learning plus full rewrite.
- TypeScript: easiest for emitter authors, which is why the plugin SDK is in TypeScript, but as the compiler host it would require Node at runtime or Node SEA/bun packaging and would contradict the single-binary promise.

## Consequences

- The compiler API (chapter 08) is a Go package; LSP and CLI consume it in-process.
- Emitter authors outside Go are served by the plugin protocol, not by the host language.
- If, by the migration decision deadline (2026-11-29), the frontend audit had shown the parser to be unsalvageable, the response is a parser rewrite in Go behind the same API, not a language change. The audit (chapter 05, F-FE items) does not show that.

## What would reverse this decision

A demonstrated, measured need that Go cannot meet (none identified), or a team skills survey (chapter 16) showing that fewer than two technical members can work in Go while four or more are strong in Rust or TypeScript.
