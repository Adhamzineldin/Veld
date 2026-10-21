# 19. Expected state of Veld at submission (readiness gate 2027-05-31, release 1.0.0)

This chapter states, dimension by dimension, where Veld is at the reviewed revision (6a47f81, verified in chapters 04 and 05) and where it is expected to be when the graduation release ships, if the plan in chapters 15 to 18 is executed at baseline capacity. Every "end state" row is a target with a verification method, not a result. Performance numbers are targets to be measured in M4 to M6 on the reference laptop defined in the evaluation protocol; none has been measured yet except where marked "measured today".

## 19.1 Product and positioning

| Dimension | Start (6a47f81) | End (1.0.0) | Verified by |
|---|---|---|---|
| Thesis | "contract-first, multi-stack generator for 7+ backends and 4+ frontends" with claims that outrun the code | "a minimal, fully checked contract language whose compiler emits dependency-free server interfaces, routes, validation and clients for a few stacks, with build-tool safety" | README, thesis chapter 1, support matrix generated from CI |
| Claimed targets | 8 backends, 10 frontends, 6 tools; 2 of 18 compile | tier 1: node-ts, typescript, react, python (Flask), go (net/http, chi), 3 service SDKs, openapi; tier 2 compile-checked nightly; the rest removed | `veld doctor` and README matrix generated from the latest CI run |
| Differentiators that can be demonstrated | none without caveats | server interfaces plus route wiring plus validation across three backends; zero-dependency output; breaking-change gate; non-destructive generation; external plugin protocol; single binary with LSP | demo script chapter 17 §17.4 |
| Interoperability | OpenAPI 3.0.3 export (valid, incomplete) | OpenAPI export complete (servers, security placeholder, defaults, aligned error schema), 3.1 if cheap; OpenAPI import documented as future work | redocly lint zero errors in CI |

## 19.2 Language and compiler

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Language definition | syntax described in README/CLAUDE.md; undocumented unions, WS, HEAD/OPTIONS accepted | written specification `docs/spec/` for lang 1.0 (models, enums, modules, actions, extends, Map, arrays, optional, defaults, deprecations, headers, middleware, status codes, imports incl. wildcard, constants, optional `veld 1` header); unions, WS, errors, constraints marked "reserved" and refused by emitters with a diagnostic | conformance suite: 30 to 40 fixtures with expected IR and diagnostics |
| Diagnostics | validator all errors with file:line; lexer/parser first error only, line only, sometimes wrong line; unterminated strings accepted | all errors from all stages, `file:line:col`, stable codes, snippet, hints, `--json`; error recovery continues after a syntax error | fixture expectations; CLI snapshot tests |
| Type system | flat `IsArray`/`IsMap` flags, no nesting; scalar names unusable as field names | recursive `TypeRef`; nested arrays and maps; reserved words usable as field and enum names; `date` and `datetime` distinct in every target | fixtures |
| IR | none (Go AST passed to emitters; `veld ast` dumps it) | `irVersion 1.0` with JSON Schema; `veld ir` command; the only cross-process contract | schema validation of every fixture's IR in CI |
| Imports | alias and relative work; `@alias/*` deletes the rest of the file silently; missing imports ignored | all three forms work; missing import is an error; aliases honoured by every command | fixtures; `veld validate` on `examples/microservices` |
| Robustness | fuzz-clean (measured today: 1.67M executions, no panic); 5 MB file under 1 s (measured today) | fuzz targets in CI nightly; same bounds kept | nightly fuzz job |
| Performance targets | not measured except the above | compile (parse, resolve, validate, lower) of a 2,000-line, 10-file project under 100 ms; `veld generate` for 50 models and 100 operations with the three tier-1 backends under 1 s excluding plugins; memory under 100 MB | benchmark in `evaluation/perf/` run at RC |
| Formatter | corrupts files, panics | lossless, idempotent, `fmt --check` | idempotence test on all fixtures |
| Breaking-change detection | dead in `generate`; shallow classifier | works in `generate --strict` (exit 4), `diff --json`; classifier covers models, nested models, enums, headers, middleware, status codes, paths, methods | fixture pairs per breaking class |
| Language metadata | `internal/language` unused by the compiler | single source of truth consumed by lexer, parser, LSP, grammars; drift test | CI |

## 19.3 CLI and generation safety

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Output writing | `RemoveAll` of output dirs before parsing; user files and even sources deleted; non-atomic | staged writer, `.veld-manifest.json`, swap on success, unmanaged files refused, atomic per file, previous output kept on failure | output-safety tests in CI |
| `clean`, `dry-run`, `init`, `watch` | `clean` deletes any directory; `dry-run` writes cache; `init` overwrites; `watch` wipes on bad saves | `clean` removes manifest files only; `dry-run` writes nothing; `init` never overwrites; `watch` keeps last good output, handles SIGTERM | CLI tests |
| Determinism | not deterministic (measured today: 1 of 6 runs differs) | byte-identical across runs and across Windows/Linux/macOS | CI determinism job |
| Config | flat beats nested; unknown keys ignored; `--out` sometimes ignored; `sh -c` hooks | nested wins; unknown keys warn; `--out` overrides; hooks via platform shell, failures fail the run, `--no-hooks` | config tests |
| Machine readability | none; colour always; exit 1 for everything | `--json` on validate/generate/lint/diff/doctor/version; `NO_COLOR`; exit codes 0/1/2/3/4 documented | CLI tests |
| Commands | `doctor` unregistered; registry commands only under `veld registry` | `doctor` reports toolchains, plugins, tiers; top-level `login/logout/push/pull` aliases | tests |
| Incremental cache | exists, always wiped | removed | n/a |

## 19.4 Generated code (tier 1)

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Node backend | runs under tsx; fails `tsc --strict`; `/:id` shadows static routes (middleware bypass); 500 leaks exception text; validation off and shallow | compiles under `tsc --strict`; static-before-param ordering from the IR; one error shape, no leak; validation on by default with enum, element type, integer, format, optional/null checks and query coercion; interfaces instead of classes; Express adapter and plain mode | compile + round trip in CI on 3 OSes |
| TypeScript client | path params unencoded; arrays lossy; no timeout/auth hook; strict errors | `encodeURIComponent`; repeated keys for arrays; `fetch` injection, headers callback, `timeoutMs`, per-call `RequestInit`; strict clean | round trip incl. `a/b?x` and arrays |
| React hooks | compile once SDK fixed | wrap the client; typed query keys | compile + a hook test |
| Python backend and client | routes have a syntax error; no registration; `Decimal` import missing; headers empty; malformed JSON gives 500 | `compileall` and `mypy --strict` on types clean; Flask adapter and plain mode; validators with `Decimal`; headers and typed query bound; 400 on malformed JSON; service receives models, not dicts; client SDK accepts its own models | compile + round trip |
| Go backend and client | `undefined: mux`; no validation; query/headers nil; date and datetime conflated | builds and vets; net/http and chi adapters; generated validators; query and header binding; `date` as a string-wrapped type, `datetime` as `time.Time`; client with query support | compile + round trip |
| Cross-language | not tested | Node, Python and Go servers each called by the three clients on one fixture | matrix job |
| Zero-dependency claim | true only for TS types/clients | precise and true for types, clients, validators in all tier-1 languages; adapters import only the user's framework | dependency grep in CI |
| Tier 2 (vue, svelte, javascript, typesonly, java, csharp, php, docs, graphql, schema) | mostly uncompilable | compile-checked nightly or demoted; emit a warning banner; README says "experimental" | nightly job |
| Removed | rust, angular, dart, kotlin, swift, mock, dockerfile, cicd, envconfig | absent from the binary; history tagged `frozen/<name>` | `veld doctor` |

## 19.5 Extensibility (plugins)

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Mechanism | in-process Go registry only | VEP v1: executables speaking JSON over stdio, `--describe` manifest, capability declarations, host-mediated writes, timeout and size caps, version pinning | protocol tests with a stub plugin |
| SDK | none | `@veld/emitter-sdk` (TypeScript) with IR types generated from the schema, `defineEmitter`, test harness, `npm create veld-emitter` | SDK tests; hello emitter |
| Reference plugins | none | `veld-emitter-hello` on npm; `veld-emitter-docs` extracted from the built-in docs generator (if B-041 lands) | acceptance test on Windows and Linux from a clean project |
| Performance target | n/a | plugin invocation overhead (spawn, handshake, 1 MB IR transfer) under 200 ms on the reference laptop | measured at RC |
| Trust | n/a | documented: plugins are trusted code; no sandbox; path containment, timeout, pinning enforced | docs and tests |

## 19.6 Editor support

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Language server | single file; byte offsets; first error; no imports; no MethodNotFound; hangs clients on unknown methods | compiler-API based; overlay buffers; cross-file diagnostics, completion (context-aware, imported models), hover, definition, document symbols, formatting; references if time; UTF-16 correct; protocol conformant; debounced; `recover` | scripted session test in CI |
| Performance target | measured today: 34 ms per change on a small file; 745 ms on 2.4 MB blocking hover | diagnostics under 100 ms for the 2,000-line, 10-file project; hover unaffected by concurrent edits | measured at M4 |
| VS Code | 1,676-line TypeScript re-implementation, commands dead, no tests | thin `vscode-languageclient` launcher; generated grammar and snippets; commands wired; binary discovery with checksum-verified download; integration test; published to the Marketplace | extension test in CI; Marketplace listing |
| JetBrains | 4,090-line hand-written PSI plugin, drift | highlighting only plus LSP4IJ server definition (limited support), built nightly; Marketplace publication only if reserve allows | manual verification with screenshots |

## 19.7 Registry, packages and self-hosting

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Deployable | `veld registry serve` inside the CLI | `veld-registry` binary and Docker image; CLI no longer links PostgreSQL, SMTP or the UI | binary dependency check |
| Security | 2FA bypass; storage traversal; scopes unenforced; XSS sinks; no limits; no rate limits; sessions unrevocable | typed JWTs; scopes enforced by one helper; expiry; revocation; last-owner guard; content-addressed storage; manifest validation; limits; rate limiting; TOTP replay protection (or TOTP disabled); escaping and CSP; security headers; Secure cookie; CORS allow-list | authz matrix and abuse tests in CI; ASVS L1 checklist |
| Integrity | optional, self-referential checksum; `http://` accepted | checksum always required, anchored by `veld.lock.json`; `https` default | MITM tests |
| Packages | no manifest, no lockfile, "latest" by publish time | `veld.package.json`, `veld.lock.json`, semver resolution (exact, caret, x-range), cache, offline, scoped registries bound to one URL, immutable versions with yank and deprecate | reproducible `veld install --frozen` on two machines |
| Operations | no health endpoint, no migrations table, no backup, no TLS, no tests | `/healthz`, `/readyz`, versioned forward-only migrations, JSON logs, config check, Compose with Caddy TLS, `backup`/`restore`/`verify` commands, runbook | timed restore drill (target RTO 1 h, RPO 24 h) |
| Performance target | n/a | publish of a 1 MB package under 2 s; download under 500 ms on a LAN; 50 concurrent downloads without error | load test at RC |
| Scale assumptions | n/a | 50 users, 500 versions, 5 publishes and 500 downloads per day on 2 vCPU / 4 GB | documented |

## 19.8 Distribution and releases

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Channels | 8 wrappers, most placeholders; npm downloads `latest` without checksum; no LICENSE | GitHub Releases (6 platform builds incl. windows-arm64), npm, Homebrew tap, `go install`, pip; others moved to experimental | clean-machine smoke tests |
| Integrity | `checksums.txt` published, never verified | checksums verified by every wrapper; cosign signatures; SBOM; GitHub provenance attestation; actions pinned by SHA | release pipeline |
| Versioning | `0.1.0` hardcoded; tags to v0.3.24 | `1.0.0` stamped by GoReleaser; changelog; per-product tags | `veld --version --json` |

## 19.9 Quality, testing and CI

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Tests | 523 functions, substring assertions; 20 packages untested incl. the registry server | conformance suite; golden outputs; compile tests; round-trip harness; cross-language matrix; determinism; output-safety; registry handler/authz/race/limit tests; LSP scripted session; extension integration test; fuzz targets | CI |
| CI | ubuntu-only unit tests | fast job on 3 OSes under 10 minutes; round-trip job; registry job with PostgreSQL; nightly tier-2, fuzz, installers, JetBrains, plugin compatibility | workflow files |
| Coverage (indicative) | parser 54%, validator 54%, node 20%, python 29%, registry 0% | not a target in itself; the requirement is that every tier-1 claim has a compile and round-trip test | n/a |
| Docs correctness | 26 documented mismatches; examples that do not compile | support matrix generated from CI; tutorial examples extracted and validated; getting-started snippets taken from generated output | docs CI |

## 19.10 Governance, documentation and academic artifacts

| Dimension | Start | End | Verified by |
|---|---|---|---|
| Repository | 24.8 MB tracked, 82% a stray binary; scratch dirs; no LICENSE, CONTRIBUTING, SECURITY, CODE_OF_CONDUCT, CODEOWNERS, CHANGELOG | clean tree under 5 MB; MIT LICENSE; all governance files; DCO sign-off; CODEOWNERS; ADRs in `docs/adr/`; roadmap | repository |
| Documentation | README, scraped docs page, stale developer guide, four plan versions | spec, tutorials for the three journeys, plugin author guide, operator runbook, API reference, support matrix, architecture overview matching the code | docs site built in CI |
| Academic | proposal documents with inconsistent numbers | thesis report (11 chapters, chapter 17 §17.3) with evidence links; evaluation results for Q1 to Q5 and the user study; slides; live demo and recorded fallback | submission folder |
| Community | 4 git authors; no external contributions | at least one external contribution merged; `good first issue` set; six-month security-fix commitment | repository |

## 19.11 Extended end state (added 2026-10-21 by ADR-012; see chapter 20)

The project owner added the previously deferred items as an ordered queue behind the core. If the queue completes, the end state additionally contains, each at the tier-1 bar:

| Addition | End state | Verified by |
|---|---|---|
| Per-field constraints | `@min/@max/@minLength/@maxLength/@pattern/@format` enforced by generated validators in TS, Python, Go and reflected in OpenAPI | boundary-value fixtures |
| Database schema | `@entity`-based SQL (PostgreSQL, SQLite) and Prisma output with relations, indexes, quoted defaults, migration diffs | schema applied in CI |
| Auth schemes | bearer, API key, basic, OAuth2 metadata in the contract; OpenAPI security; middleware wiring; client auth config | protected-route round trip |
| Discriminated unions | typed unions across the three stacks with validators and OpenAPI `oneOf` | fixtures |
| Streaming | SSE outputs, chunked downloads, multipart uploads with size limits | round trips |
| WebSocket | typed bidirectional actions for Node, Python, Go servers and the three clients (declared adapter dependencies) | echo round trips |
| Docker, CI and env generators | tier-2 tools whose images build nightly | nightly job |
| OpenAPI import | subset importer with an unsupported-constructs report; export/import round trip on fixtures | round-trip test |
| Package signing | Sigstore keyless or minisign; verified on pull when the lockfile demands it | tamper test |
| WebAssembly plugins | sandboxed transport with a Wasm hello emitter | sandbox and latency tests |
| Registry-hosted plugins | `veld plugin install` with signature verification and trust policy | local registry test |
| Tier-2 revival | Rust backend compiles with a real router; Angular, Dart, Kotlin, Swift compile nightly; `veld mock` works | nightly job |
| High availability | replicated Compose variant with S3-compatible storage and a failover drill | drill record |

Performance targets added: WebSocket echo round trip under 5 ms locally; SSE first byte under 50 ms; Wasm plugin overhead under 2 times the stdio transport; HA failover under 60 s.

## 19.11a What will still be missing at submission (stated limitations, revised)

- Any queue item removed by the automatic cut on 2027-05-02 (order in chapter 20 §20.6); the thesis lists them as future work with the reason.
- Tier-2 targets compile but are not round-trip tested.
- JetBrains support is limited to highlighting plus LSP4IJ, possibly unpublished.
- Evaluation is small-N with student participants; performance figures are from one reference machine.
- Kubernetes manifests, federated registries, gRPC, generics beyond `Map` and arrays are out by decision.

## 19.12 Completeness and production-readiness estimate (judgement with explicit weights)

Two different percentages are asked for and they are not the same thing. "Completeness" measures how much of the 1.0.0 end state (sections 19.1 to 19.10) exists and works at 6a47f81. "Production readiness" measures how much of what a production user needs (correctness, safety, security, tests, operability) is in place for that area. Scores are the lead reviewer's judgement from the verified inventory in chapters 04 and 05; the weights express importance to the thesis. Another reviewer could reasonably move any cell by 10 points; the totals are robust to that.

| Area | Weight | Complete vs end state | Production ready | Basis |
|---|---|---|---|---|
| Language, lexer, parser, validator, loader | 15% | 65% | 55% | fuzz-clean, fast, all semantic errors reported; missing spans, all-errors syntax reporting, nested generics, IR, wildcard import, formatter |
| Generation safety and CLI | 10% | 40% | 15% | commands exist; output path destructive, gate dead, non-deterministic, config precedence wrong |
| Tier-1 emitters (Node/TS/React, Python, Go) | 20% | 50% | 25% | Node/TS about 70% and runs; Python about 35% and Go about 40% (do not compile); no compile tests |
| Validation of inputs | 5% | 30% | 20% | opt-in, `typeof` only; documented Zod/Pydantic absent |
| External plugin system | 10% | 0% | 0% | nothing exists |
| Language server and editors | 10% | 30% | 20% | single-file LSP; VS Code works on its own engine; JetBrains hand-written |
| Registry and package resolution | 10% | 45% | 10% | publish/pull happy path works; six security-high defects; no lockfile, manifest or immutability |
| Self-hosting and operations | 5% | 10% | 5% | no health endpoint, migrations table, backups, TLS, tests |
| Distribution and releases | 5% | 40% | 25% | `go install` works, release builds exist; npm/pip need work, no checksums verified, no signing |
| Testing and CI | 5% | 35% | 20% | 523 unit tests; nothing compiles generated code; ubuntu only |
| Documentation and governance | 5% | 35% | 15% | extensive docs with 26 verified mismatches; no LICENSE or governance files |
| **Weighted total** | 100% | **about 40%** | **about 20%** | |

Reading: roughly two fifths of the graduation end state exists, and roughly one fifth of what production use requires. Measured against the current README's claims (18 targets, Zod, deterministic, never touches user files) the completeness would be lower, around 30%, because several claimed capabilities do not work. The single most valuable slice, Node backend plus TypeScript client, is individually about 70% complete and about 45% production ready: it runs end to end, but on a generator that can delete the user's files and with a route-ordering defect that bypasses middleware.

What moves the numbers fastest (chapter 15, M1): the staged output writer, the gate ordering fix, the header-identifier fix, the Python and Go compile fixes and the compile-test CI job. Those items alone lift production readiness of the vertical slice from about 45% to about 75% within four weeks, because they remove the data-loss and does-not-compile failures rather than adding features.

## 19.13 One-paragraph summary

At 6a47f81 Veld is a promising compiler frontend attached to a destructive output path, an unverified emitter matrix and an unsafe registry, described by documentation that claims more than exists. At 1.0.0 it should be a small, specified language with a conformance suite and a versioned IR; a safe, deterministic generator with three verified backends, three verified clients and a real plugin protocol with a published SDK; one language engine serving VS Code and, in limited form, JetBrains; a hardened registry with lockfile-based reproducibility and a tested self-hosting path; signed releases through a few trustworthy channels; and an evaluation that measures the claims. The difference between the two states is the content of the thesis.
