# 20. Extended scope plan (decision ADR-012, 2026-10-21)

This chapter is authoritative for everything the project owner added back on 2026-10-21. It extends, and where it conflicts overrides, chapters 03, 09, 10, 13, 15, 16, 18 and 19. The core plan (M0 to M3, the chapter 19 end state) is unchanged and protected; the additions form an ordered queue behind it.

## 20.1 What is added, with the design each item must follow

Every item below ships only when it meets the tier-1 bar: compiles on three OSes, round-trip tested, deterministic, declared in the emitter capability manifest, documented from CI evidence. Zero-runtime-dependency exceptions are listed explicitly.

| # | Item | Language or product surface | Design constraints (binding for implementers and agents) | Zero-dep exception | Estimate (h) |
|---|---|---|---|---|---|
| Q1 | Per-field constraints | `@min(n) @max(n) @minLength(n) @maxLength(n) @pattern("re") @format(email\|uri\|...)` on fields; IR `constraints`; validators in TS, Python, Go; OpenAPI mapping; `veld lint` warns on unsatisfiable combinations | validators generated, no library; regex dialect limited to the RE2-compatible subset so the three languages agree; tests include boundary values | none | 30 to 50 |
| Q2 | Database schema generator (proper) | `@entity` opt-in on models; `@id`, `@unique`, `@index`, `@table("name")`, `@relation(Model, field)`; dialects PostgreSQL and SQLite; Prisma output; quoted identifiers and defaults; `veld schema --migrations` writes a timestamped diff against the previous manifest (full schema always emitted) | DTOs are never tables unless `@entity`; `Map` and arrays become JSON columns with a documented note; relations only one-to-many and many-to-one in 1.0 | none | 30 to 60 |
| Q3 | Authentication schemes in the contract | module or action `auth: bearer` / `apiKey(header: "X-Key")` / `basic` / `oauth2(authorizationCode, scopes: [...])`; OpenAPI `securitySchemes`; server adapters wire the existing `middleware` hook with a typed `AuthContext`; clients get an `auth` config (static token, callback) | Veld never issues or validates tokens itself; it wires the user's handler | none | 30 to 60 |
| Q4 | Discriminated unions | `type Shape = Circle \| Square @discriminator(kind)`; IR `union`; TypeScript discriminated union; Python `Union` with a validator switch; Go interface plus custom `UnmarshalJSON`; OpenAPI `oneOf` with `discriminator`; validators check the discriminator first | undiscriminated unions are a validator error in 1.0 | none | 40 to 70 |
| Q5 | Streaming | `output: Event[] @stream(sse)` generates a server generator/iterator and a client async iterator; `output: file` chunked download with content-type; `input: file` multipart upload with a size limit from `@maxSize(n)` | all three stdlibs support SSE and multipart; uploads stream to a temp file, never to memory | none | 40 to 80 |
| Q6 | WebSocket actions (Node first) | `action Chat { method: WS path: /ws input: ClientMsg emit: ServerMsg }`; JSON messages typed both directions; validators applied per message; server adapter for Node; browser client uses the native `WebSocket` API | server adapter imports `ws` (declared, documented exception) | Node server: `ws` | 40 to 70 |
| Q7 | WebSocket for Python and Go, plus clients | Python adapter (`websockets`), Go adapter (`golang.org/x/net/websocket` or `nhooyr.io/websocket`), Python and Go clients; echo and typed round-trip tests | declared exceptions | Python: `websockets`; Go: one declared library | 40 to 80 |
| Q8 | Dockerfile, CI and env generators (fixed, tested) | templates per tier-1 stack; pinned Veld version with checksum verification in the Dockerfile; no bundled registry Postgres; nightly job builds each image and hits the health endpoint | tier-2 tools; never run by default; `tools` config block finally honoured | none | 15 to 30 |
| Q9 | OpenAPI import | `veld import openapi <file>` writes `.veld` files plus a report of unsupported constructs; subset: object/primitive/array/enum schemas, `allOf` single inheritance, `oneOf` with discriminator (to unions), JSON request bodies, path/query/header params, status codes, security schemes (to Q3) | import is lossy by design and says so; round trip `import(export(x)) == x` holds for every conformance fixture | none | 40 to 80 |
| Q10 | Package signing | `veld push --sign` with Sigstore keyless (GitHub or Google OIDC) or a minisign key pair; registry stores signatures; lockfile `"signed": true` makes `veld pull` require a valid signature; `veld-registry` verifies on publish | checksum path stays the default; signing is opt-in per package | none in the CLI (uses the Sigstore HTTP API) | 30 to 60 |
| Q11 | WebAssembly plugin transport (sandboxing) | manifest `transport: "wasm"`; host runs the module with Extism on wazero, no `AllowedPaths`, no network; same `EmitRequest`/`EmitResponse` JSON; hello emitter compiled with TinyGo and the Rust PDK; `veld doctor` shows the transport | native (stdio) plugins remain allowed but print "not sandboxed" once per run; Wasm plugins run without a trust flag | Go host dependency `extism/go-sdk` (pure Go) is a compiler dependency, not a generated-code dependency | 60 to 120 |
| Q12 | Registry-hosted plugins | package `kind: "plugin"` carrying a Wasm module (or per-platform binaries); `veld plugin install @org/name@ver` verifies signature (Q10) and records it in the lockfile; trust policy: Wasm runs by default, native requires `--trust-native` | depends on Q10 and Q11 | none | 40 to 80 |
| Q13 | Tier-2 revival: Rust backend (real axum router and main), Angular on the TS SDK, Dart, Kotlin and Swift client fixes, mock server wired to `veld mock` serving OpenAPI examples | all compile-checked nightly; Rust additionally `cargo check` in nightly; promoted to tier 1 only by the ADR-005 promotion rule | none | 80 to 130 |
| Q14 | High-availability hosting | Compose variant with two registry replicas behind Caddy, PostgreSQL primary plus streaming replica (or a managed database), S3-compatible storage implementation (MinIO in Compose); documented failover; drill in M6 | the single-host Compose stays the default | none | 40 to 70 |

Total additions: 555 to 1,040 hours.

## 20.2 Capacity under the AI-assisted assumption

| Quantity | Baseline (chapter 15) | Extended |
|---|---|---|
| Committed technical hours (five members, exam-adjusted, 15% coordination) | 1,106 | 1,106 actual |
| AI-assisted multiplier (1.5 on the 60% of effort that is implementation and tests; 1.0 elsewhere) | not applied | 1.3 blended |
| Equivalent committed capacity | 1,106 | 1,438 |
| Reserve (20%) | 277 | 360 |
| Planned technical work | 694 to 1,139 (mid 916) | 1,251 to 2,169 (mid 1,710) |
| Fit at the midpoint | 190 h of reserve left | 88 h of reserve left |
| Fit at the high estimate | 33 h of reserve used | 371 h short |

Sensitivity: with no AI gain (multiplier 1.0) the midpoint overshoots capacity by about 330 h, which is Q14, Q13, Q12, Q11 and Q10 in reverse order. At 16 h per week with the 1.3 multiplier, equivalent capacity is about 1,920 h and the midpoint fits with about 200 h of reserve. The multiplier is measured at the M1 exit: the integration owner records actual hours per backlog item; if the ratio of estimate to actual is below 1.15, chapter 20 §20.6 applies.

## 20.3 Ordered queue and schedule

Core (M0 to M3) is unchanged. The queue starts in S8 (2027-02-08) and is worked in this order; an item does not start until the previous one in the same owner's lane is accepted.

| Order | Item | Owner (lane) | Co-owner | Target sprints | Hard dependency |
|---|---|---|---|---|---|
| 1 | Q1 constraints | T1 (syntax, IR, validator) | T2, T4 (validators) | S8 to S9 | IR v1 (B-025) with reserved fields |
| 2 | Q2 database schema generator | T4 | T1 (annotations) | S8 to S9 | IR `entity` fields |
| 3 | Q3 auth schemes | T1 (syntax), T2 (SDK, middleware wiring, OpenAPI) | T4 (Python, Go wiring) | S9 to S10 | Q1 |
| 4 | Q4 unions | T1 (syntax, IR), T2 and T4 (emitters) | | S10 to S11 | Q1 |
| 5 | Q5 streaming | T2 (Node), T4 (Python, Go) | T1 (syntax) | S11 to S12 | IR `stream` |
| 6 | Q6 WebSocket Node | T2 | T1 (semantics) | S11 to S12 | Q5 syntax work |
| 7 | Q8 Docker/CI/env generators | T5 | | S11 | B-016 compile jobs |
| 8 | Q7 WebSocket Python, Go, clients | T4 | T2 | S12 to S13 | Q6 |
| 9 | Q9 OpenAPI import | T1 | D1 (report format) | S12 to S13 | Q3, Q4 |
| 10 | Q10 package signing | T3 | T5 (release keys) | S11 to S12 | B-044 lockfile |
| 11 | Q11 Wasm plugin transport | T2 | T4 (Wasm hello) | S12 to S13 | B-037 host runner |
| 12 | Q13 tier-2 revival (Rust, Angular, Dart, Kotlin, Swift, mock) | T4 (Rust, mock), T5 (Angular, mobile clients) | | S12 to S13 (parallel, lowest priority in each lane) | B-049 nightly job |
| 13 | Q12 registry-hosted plugins | T3 | T2 | S13 | Q10, Q11 |
| 14 | Q14 high availability | T3 | T5 | S13 | B-051 |
| freeze | extended-feature freeze and automatic cut review | all | | 2027-05-02 (end of S13) | |

Gates (updated): core feature freeze 2027-04-18 (unchanged); extended-feature freeze 2027-05-02; RC 2027-05-16; readiness 2027-05-31. Queue items accepted after May 2 are not possible; items with open P1 defects at the RC gate are demoted to tier 2 ("experimental") in the release notes rather than removed.

## 20.4 Per-member additions (midpoint hours, to validate against the survey)

| Member | Baseline midpoint (chapter 16) | Added by the queue | Total | Equivalent capacity (288 committed + 72 reserve) | Gap at midpoint |
|---|---|---|---|---|---|
| T1 | about 215 | constraints 40, unions 55, auth syntax 15, OpenAPI import 60, streaming/WS semantics 20 = 190 | 405 | 360 | 45 over |
| T2 | about 200 | streaming 50, WS Node 55, auth SDK/middleware 30, Wasm transport 90 = 225 | 425 | 360 | 65 over |
| T3 | about 200 | signing 45, hosted plugins 60, HA with S3 storage 75 = 180 | 380 | 360 | 20 over |
| T4 | about 210 | DB schema 45, WS Python/Go 60, Rust 40, mobile clients 30, mock 9, constraint and union validators 20 = 204 | 414 | 360 | 54 over |
| T5 | about 190 | Docker/CI/env 22, Angular 10, CI for all queue items 35, auth OpenAPI 10, AGENTS.md enforcement 20, nightly expansion 25 = 122 | 312 | 360 | 48 under |

Reading: at the midpoint every lane except T5 is over its equivalent capacity, which is why the queue is ordered and the cut is automatic. T5 absorbs cross-cutting CI work and is the first backup for T2 and T4 queue items. If the survey shows a member with less than 12 h per week, that lane's lowest-order queue items are dropped first.

## 20.5 Rules that protect the end state (summary; full text in `AGENTS.md`)

- No queue item starts before its hard dependency is accepted; "accepted" means merged with tests, docs and changelog, and green in CI.
- No queue item may weaken a core guarantee: output safety, determinism, zero runtime dependencies (except the declared WebSocket adapters), the IR as the only cross-process contract, the import-boundary rules, the tier promotion rule.
- Every queue item adds conformance fixtures first, then the implementation, then compile and round-trip tests, in that order, in separate small commits.
- Lean code: a feature that needs more than about 1.5 times its estimate is paused and re-designed in a one-page note before continuing.
- Tests are not optional and are never weakened to pass; coverage floors in `AGENTS.md` apply to every package touched.

## 20.6 Automatic cut order (applied without a meeting)

If the M1 exit measurement shows a multiplier below 1.15, or any core gate slips by a sprint, drop from the end of the queue: Q14, Q12, Q13, Q11, Q10, Q9, Q7, Q8, Q6, Q5, then Q4. Q1, Q2 and Q3 are kept unless the core itself is at risk, in which case the chapter 15 §15.6 fallback applies to the core first and the queue is suspended entirely.

## 20.7 Research questions added by the extension

- Q6: Does AI-assisted development change the estimate-to-actual ratio for a student team, and does the ratio differ between greenfield features (the queue) and defect fixing (M1)? Measured from the tracker; reported with the multiplier assumption it validates or refutes.
- Q7: What does a WebAssembly plugin transport cost in latency and author effort compared with the stdio transport, for the same emitter? Measured with the hello emitter in both transports.

## 20.8 Updated statement of what is not in 1.0.0

Only what fails the automatic cut, plus: Kubernetes manifests, federated registries, gRPC or binary transports, a hosted public registry operated by the team, and any emitter beyond the registered list. Everything else the project owner asked for is in the queue above.
