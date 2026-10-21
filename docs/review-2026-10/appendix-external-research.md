# Appendix: External facts (accessed 2026-10-21)

Method: `WebFetch` was blocked by a PreToolUse hook for the whole session, so no page was read in full. Facts come from `WebSearch` result summaries that quote the cited pages. Each item is marked **primary** (the summary quoted the primary page), **secondary**, or **unverified** (prior knowledge only). Re-verify primary items by opening the URL before citing them in the graduation report. The first pass was delegated to a research track; the lead reviewer repeated the decision-relevant searches with domain restrictions.

## A.1 TypeSpec (Microsoft)

- TypeSpec 1.0 is GA; `@typespec/openapi3` and `@typespec/json-schema` stable; protobuf and client/server emitters preview. typespec.io/blog/typespec-1-0-GA-release (primary).
- Emitters are TypeScript npm packages built on `@typespec/compiler`; emitter framework moved to `@typespec/asset-emitter` (April 2025). typespec.io/docs/extending-typespec/emitters-basics (primary).
- Server emitters: `@typespec/http-server-csharp` 0.58.0-alpha.33 released on GitHub (2026); `http-server-js` exists (documentation PR merged 2026-10-16). A "Prepare typespec publish" PR moves packages to 1.17.0 / 0.87.0. github.com/microsoft/typespec/releases, PRs #12264, #12250, #12134 (primary, version numbers from release and PR titles).
- License: repository page lists MIT. github.com/microsoft/typespec (primary for the repo; npm package license not confirmed).
- VS Code / Visual Studio extensions exist (prior knowledge, unverified in-session).

## A.2 Smithy (AWS)

- Apache-2.0 license. github.com/smithy-lang/smithy (primary).
- Generators: smithy-typescript (Apache-2.0, "in development"), smithy-go (Apache-2.0), smithy-kotlin, smithy-python, smithy-java (1.0.0 GA April 2026 per a secondary blog), smithy-rs (server generator work in progress per awesome-smithy). github.com/smithy-lang, github.com/smithy-lang/awesome-smithy (primary and secondary as noted).
- smithy-language-server and smithy-intellij exist. github.com/awslabs/smithy-language-server, github.com/awslabs/smithy-intellij (primary).
- Distribution via Maven Central (smithy-codegen-core 1.72.1, July 2026). mvnrepository.com (secondary).

## A.3 OpenAPI and generators

- OpenAPI 3.2.0 released September 2025 with a 3.1.2 patch; adds QUERY method, streaming media types, tag hierarchy. spec.openapis.org/oas/v3.2 (primary via summary).
- openapi-generator: latest stable v7.26.0 tagged 2026-10-06; JVM (Java 11+); generates clients, server stubs, docs. github.com/OpenAPITools/openapi-generator/tags (primary).
- oapi-codegen: Apache-2.0, Go clients and servers. openapi-typescript: MIT, types only. Kiota: MIT, clients. openapi-typescript-codegen: MIT, clients. Hey API openapi-ts: MIT, SDKs plus Zod schemas. github.com pages for each (primary). orval, openapi-fetch licenses not found (unverified).
- Fern (Postman acquisition January 2026) and Stainless acquisition claims are vendor or single-source statements (unverified).

## A.4 Protocol Buffers, buf, Connect

- Plugin protocol: compiler sends a `CodeGeneratorRequest` on stdin, plugin writes a `CodeGeneratorResponse` on stdout, compiler writes files. buf.build/docs/generate (primary).
- buf local plugins: `plugins.local` executable on PATH (`protoc-gen-<name>`); remote plugins pinned as `buf.build/<org>/<plugin>:<version>` and executed on the BSR; both mixable in `buf.gen.yaml`. buf.build/docs/generate, buf.build/docs/bsr/remote-plugins (primary).
- `plugin.proto`: github.com/protocolbuffers/protobuf/blob/main/src/google/protobuf/compiler/plugin.proto (unverified URL, prior knowledge).
- Connect RPC languages: Go, TypeScript/JavaScript, Swift, Kotlin, Python; CNCF project. connectrpc.com (primary).

## A.5 TypeScript-only type-sharing tools

tRPC, ts-rest, Hono RPC, zod-openapi, Zodios, Elysia Eden: TypeScript-only, types inferred from code, no multi-language generation (zod-openapi exports OpenAPI). Unverified in-session (prior knowledge).

## A.6 GraphQL Code Generator

Plugin-based codegen from GraphQL schemas and operations, many languages. the-guild.dev/graphql/codegen. Unverified in-session.

## A.7 Plugin technologies for a Go host

- Extism: Go SDK runs on wazero; manifest `AllowedPaths` maps host directories into the guest (requires `EnableWasi: true`); network access is not restricted by the SDK itself, hosts implement allow-lists; a downstream project pins `github.com/extism/go-sdk v1.7.1`. github.com/extism/go-sdk, extism.org/docs/concepts/host-functions (primary; version from a downstream pin, secondary).
- wazero: pure Go, no cgo, 1.0 since March 2023 with API stability promise. github.com/wazero/wazero (primary).
- WASI 0.3.0 released 2026-06-11. wasi.dev/releases/wasi-p3 (primary).
- HashiCorp go-plugin: separate OS process over gRPC, process isolation only (unverified version).

## A.8 Language Server Protocol and JetBrains

- LSP 3.18 is the current specification (site navigation: 3.18 current, 3.19 upcoming); changelog dates 3.18.0 to 2026-06-04 (date format inferred). microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification (primary).
- `positionEncoding` introduced in 3.17; client offers `general.positionEncodings`, server selects; UTF-16 mandatory default. 3.17 specification page (primary).
- JetBrains: LSP API available to all IntelliJ IDEA users via the unified distribution (2025.3); Community Edition sunset; the LSP client API is being open-sourced in the 2026.2 cycle (planned for 2026.1.4 stable). blog.jetbrains.com/platform/2025/09/..., blog.jetbrains.com/platform/2026/06/open-sourcing-the-lsp-client-api-in-intellij-idea-2026-2/ (primary).
- LSP4IJ (Red Hat): stable 0.21.0, nightly 0.21.1-20261020; listed as compatible with "all flavours of IntelliJ"; minimum 2024.2+. plugins.jetbrains.com/plugin/23257-lsp4ij, github.com/redhat-developer/lsp4ij/releases (primary; the GitHub releases page is stale at 0.19.3).

## A.9 Go and GoReleaser

- Go 1.26 released 2026-02-10; Go 1.27 released 2026-08-19; patch releases go1.27.2 (2026-10-08) and go1.26.10 (2026-10-19). Policy: each major release supported until two newer major releases exist. go.dev/doc/devel/release (primary). Therefore on 2026-10-21 supported: 1.26 and 1.27. Veld's `go 1.22` is out of support.
- GoReleaser v2.18: Syft SBOMs, cosign signing (cosign v3 `--bundle`), Homebrew, Scoop, Winget publishing; npm publishing Pro-only. goreleaser.com (primary via summary).

## A.10 Supply chain

- SLSA v1.2 current; Build track levels L1 to L3. slsa.dev (primary via summary).
- GitHub artifact attestations: `actions/attest-build-provenance` v4; needs `id-token: write` and `attestations: write`; free for public repositories. docs.github.com (primary via summary).
- cosign, npm provenance, OpenSSF Scorecard: unverified in-session.

## A.11 Licenses

MIT, Apache-2.0 and BSD-3-Clause are permissive and mutually compatible; Apache-2.0 requires preserving LICENSE and NOTICE on redistribution. Veld's dependencies: cobra (Apache-2.0), lib/pq (MIT), golang.org/x/crypto (BSD-3-Clause). SPDX pages: spdx.org/licenses/MIT.html, Apache-2.0.html, BSD-3-Clause.html (unverified in-session; standard knowledge).

## A.12 Who already covers server interfaces and route wiring

TypeSpec (preview C# and JS server emitters), Smithy (TypeScript developer-preview server, Java GA, Rust WIP), openapi-generator (server stubs for many frameworks), oapi-codegen (Go servers), Connect/buf (RPC server stubs). Veld's breadth across eight backend emitters is wider on paper, but chapter 05 shows two of them emitting uncompilable code at 6a47f81, so the claim must be earned through the compile matrix in chapter 09.
