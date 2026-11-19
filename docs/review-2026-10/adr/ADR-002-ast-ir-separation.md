# ADR-002: Separate the syntax tree from a versioned, serializable IR

- Status: PROPOSED (accept by 2026-11-01)
- Date: 2026-10-21
- Deciders: T1 (owner), T2; reviewer: T4
- Evidence: `internal/ast/ast.go` is passed directly to every emitter (`internal/emitter/emitter.go:43-45`), `veld ast` dumps that struct as JSON, and `CollectTransitiveModels` plus per-emitter type-mapping code re-derive the same facts in each emitter (`internal/emitter/helpers.go`, `internal/emitter/lang/*`).

## Context

Today "only AST JSON passes between stages" (CLAUDE.md). The AST is a syntax-shaped Go struct (`Model.Extends` as a name, `Field.Type` as a string with `IsArray`/`IsMap` flags, status codes parsed by emitters). Every emitter resolves inheritance, path parameters and type references itself, which is where emitters diverge (chapter 09). There is no stable, versioned representation to hand to an external plugin.

## Decision

1. Keep the AST as the output of parsing, with source spans, and keep it internal.
2. Add `internal/ir`: a normalized, fully resolved, versioned representation (`irVersion` "1.0") produced by semantic analysis. It contains flattened models (inheritance resolved, with the `extends` chain preserved as metadata), structured type references (scalar, array, map, ref, optional), services and operations with pre-extracted path parameters, query and header bindings, output status codes, deprecations, and a source map from every IR node to file, line and column.
3. Publish a JSON Schema for the IR and a `veld ir` command. The IR JSON is the only thing that crosses a process boundary (plugins, tests, tooling). Inside the process, Go structs are used directly; there is no serialization between lexer, parser, validator and built-in emitters.
4. Built-in emitters migrate from AST to IR incrementally: an adapter builds IR from AST so that emitters can be moved one at a time; emitters not migrated by the Beta gate are frozen (chapter 09).

## Reasons

- One place to resolve semantics removes the per-emitter divergence that the audits found.
- Plugins need a stable contract; the AST is not one.
- Serializing only at process boundaries keeps the LSP fast and the code simple.

## Alternatives rejected

- Serialize at every stage (CLAUDE.md's "only AST JSON passes between stages" taken literally): no benefit inside one process, costs time and complexity.
- Make the AST itself the public contract: it would freeze syntax details into the plugin API and force every plugin to re-implement inheritance and path parsing.

## Consequences

- `internal/ir` and its JSON Schema are versioned artifacts (ADR-009).
- Conformance fixtures (chapter 08) pair `.veld` inputs with expected IR JSON and expected diagnostics.
- `veld ast` remains for debugging but is documented as unstable.
