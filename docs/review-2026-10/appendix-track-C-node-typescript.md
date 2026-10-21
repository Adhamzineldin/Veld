# Appendix: audit-track notes (C-node-typescript)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

# Node backend + TypeScript/framework frontend emitters: audit (Veld 6a47f81, 2026-10-21)

Working dirs: scratchpad/node-audit/e2e/{rich,v2,rsv,fw,brk,sb}. Binary: scratchpad/node-audit/veld.exe.
Nothing tracked was modified.

## 1. E2E trace log

| # | Step | Result |
|---|------|--------|
| 1 | `go build -o node-audit/veld.exe ./cmd/veld` | OK |
| 2 | `veld init` (stdin EOF) | Scaffolds veld/{app.veld,veld.config.json,models/*,modules/*}, README.md. Interactive prompts proceed on EOF. Config: backend out `../generated`, no frontend out. |
| 3 | Rich contract (8 scalars, optional, array, Map, enum @default, extends, prefix, GET+path+query, POST :201, PUT, PATCH, DELETE no output, headers, middleware, deprecated action/field, recursive model) | First attempt failed to parse: `headers: AuthHdr` with a quoted-name field model (`"X-Trace-Id": string` as a model field) gives `expected identifier`. Inline `headers: { "X-Trace-Id": string }` (the CLAUDE.md form) parses. |
| 4 | `veld generate` (config backend out `../generated`) | OK. Backend -> rich/generated, frontend -> rich/veld/generated (inside the veld source dir). No schemas/, no Zod, no errors/ dir (none declared). |
| 5 | npm i typescript@5 zod express @types/express @types/node tsx; `tsc --noEmit` strict, commonjs | **FAIL, 4 errors**: routes/items.routes.ts(85,43) TS2345 headers literal not assignable to class `SecureHeaders`; client/itemsApi.ts(30,66) and (35,61) TS2352 `SearchQuery` -> `Record<string,unknown>`; client/itemsApi.ts(60,63) TS2345 `SecureHeaders` -> `Record<string,string>`. With `--validate` a 5th: _validators.ts(55,13) TS2352. Same errors in react/vue/svelte output. |
| 6 | server.ts (express.json + Router + `itemsRouter(r, svc, mw)`) and client.ts (generated `ItemsClient`) run with tsx (tsx ignores type errors) | see below |
| 6a | GET /items/abc | 200 OK |
| 6b | GET with id `a/b?x=1#h` | **404** `Cannot GET /items/a/b`: path param not URL-encoded |
| 6c | POST create | 201 |
| 6d | DELETE | 204, SDK resolves undefined |
| 6e | PUT / PATCH | 200 |
| 6f | POST `{"i":"notanumber"}`, default build (no --validate) | **201**: no validation at all |
| 6g | same with `--validate` | 400 `{"error":"Validation failed: id: required string; ...","code":"VALIDATION_ERROR"}` (string list, not Zod issues) |
| 6h | Malformed JSON | 400 with Express HTML error page (body parser runs outside the generated try/catch) |
| 6i | GET /items/boom, /items/guarded, /items/old | **All three answered by getItem** (route shadowing). `middleware.authGuard` never ran. |
| 6j | Service throws plain Error | **500** `{"error":"db password=hunter2 at /srv/x"}` (raw message leaked). Error with .status=418,.code -> 418 `{"error":"teapot","code":"TEA"}` |
| 6k | `search({q:'a b&c=d%26e', page:2, tags:['x','y']})` | Scalar encoding correct; server received page:"2" (string) and tags:"x,y" (comma string, not array). Raw `?q=a%20b&c=d%26e` decodes correctly. |
| 6l | header action | Headers sent and mapped; with no headers sent the service is still called with undefined values and returns 200 (no check). |
| 6m | `--validate` garbage body | id:'notuuid', i:1.5, d:'notdecimal', dt:'zzz', tags:[1,2], meta:{a:1}, role:'hacker', maybe:null, extra key -> **201 accepted**. i:9007199254740993 accepted, becomes ...992. `?page=abc` -> service gets string, response assertion then 500. |

## 2. Claims verification

| Claim | Verdict | Evidence |
|-------|---------|----------|
| Zero runtime deps in generated output | **True for the default build.** Imports across types/, interfaces/, routes/, client/, index.ts, _validators.ts are relative only; no zod. `--backend-framework=express` adds `import('express')` type references (needs @types/express, types only). React/Vue/Svelte/Angular outputs import their framework (expected). | Zod is **never** generated (CLAUDE.md says Zod/Pydantic schemas are generated and lists schemas/schemas.ts: documentation mismatch). Types-only usage needs no install, but output does not compile under strict (F-NODE-06). |
| Framework agnostic (`router: any`) | **Partially.** Signature: `xRouter(router: any, service: IXService[, middleware: IMiddleware])`; calls `router.<verb>(path, [mw...], async (req:any,res:any)=>{})` and assumes Express semantics: req.params, req.query (pre-parsed), req.body (pre-parsed, needs express.json()), req.headers[lowercase], res.status(n).json(), res.status(204).end(), res.json(). | Fastify (reply.code().send(), no res.status) and Hono (c.json(), single context arg) will not work; Koa neither. It is Express-shaped, not agnostic. Middleware type is (req,res,next). Positive: no hard framework import in the default build. |
| Deterministic | **True.** Two generations into separate dirs: `diff -r` identical. Map ranges feed sorted keys or ordered outer loops (sort.Strings in validate.go:422, sortedStringKeys sdk.go:468, helpers.go:432 first-wins on ordered module loop). | Caveat: every file is rewritten each run (mtime changes even if content identical) because generate wipes and rewrites. |
| Output safety ("never writes outside --out", "user files never touched") | **False.** | F-NODE-01..04, 16, 17. |

Other checks:
- Type mapping: int/float -> number (no unsafe-int guard), decimal -> branded string, date/datetime/uuid -> branded strings (UUID, Decimal, ISODate, ISODateTime in types/_scalars.ts), enum -> string-literal union (Angular emits a real `enum`), Map -> Record<string,V>, extends -> `class X extends Base`, recursive `Node2 { children: Node2[] }` generates and terminates (no Zod, so no z.lazy question; validator recurses by call).
- Optional vs nullable: `maybe?: string` type excludes null but validator accepts null.
- Status codes: POST->201 (including POST with output Node2), DELETE no output->204, `Type:201` honored.
- SDK: VeldApiError(status, body: string, code?); parseErrorResponse gives code '' when the body has none. 204 handled; 200 with empty body would throw in res.json(). No timeout/AbortSignal, no fetch injection, only static `headers` in config (no per-request/async token hook). Content-Type: application/json is sent on GET too. DELETE sends body `{}`. resolveBase is browser-safe (globalThis.process?.env). `export const api = new VeldApiClient()` constructs at import.
- GET with `input` model: SDK serializes input into query, route passes req.query (consistent with the recent fix) but values are strings typed as the model.
- Reserved names: F-NODE-14. Actions named delete/get/new yield legal members and work.
- Per-framework frontends (all emitted into the same out dir as the TS SDK; backend also emitted because backend runs too):
  - react: core hooks import `react`; the other file imports `@tanstack/react-query`; both wrap client/itemsApi (import the TS SDK). Compiles with deps except the shared F-NODE-06 errors.
  - vue: imports `vue`; wraps TS SDK; same result. svelte: imports `svelte/store`; wraps TS SDK; same.
  - angular: **does not import the TS SDK**, uses HttpClient + rxjs; broken (F-NODE-15).
  - javascript: CJS `require`, but still emits types/*.ts, index.ts, routes/*.ts, interfaces/*.ts (mixed JS/TS). types-only: types plus leftover backend files.
  - The full client/ directory is duplicated into every framework output.

## 3. Findings

### F-NODE-01: `veld generate` deletes the whole output directory (including user files) before every run
- Category: generate-cli
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: low
- Location: cmd/veld/generate.go:40-50 (runClean), 757-760 (call); comment at 141-142 claims the opposite
- Trigger/reproducer: put my-notes.ts, usersub/a.txt, routes/stale.routes.ts in the out dir; `veld generate --force --out=./o1`. All gone.
- Expected vs actual: "user files are never touched; use `veld clean`" vs `os.RemoveAll(outDir)` on every non-dry-run generate (also workspace and watch).
- Impact: hand-added files, node_modules, co-located code in the out dir are lost; all mtimes reset, defeating tsc/vite incremental caches.
- Root cause: runClean invoked unconditionally; no manifest of generated files.
- Recommendation: refactor: write a manifest of emitted files and delete only stale manifest entries, never RemoveAll.
- Regression check: foreign file in out dir survives two generates; a removed module's file is deleted.
- Effort estimate: 6h-10h

### F-NODE-02: `--out` pointing at the project/source directory deletes the contract and sources
- Category: output-safety
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: low
- Location: internal/config/config.go:375 (only rejects "..", ".", "./", "../."), cmd/veld/generate.go:40
- Trigger/reproducer: sandbox sb/proj containing app.veld and notes.txt: `veld generate --input=app.veld --out=../proj`. On Windows RemoveAll partially failed (cwd in use) but app.veld and notes.txt were deleted, then the run errored "reading app.veld". Unix would remove everything. `--out=.`/`..` are refused. Root (`/`) deliberately not tested.
- Expected vs actual: refuse an out dir that contains the input, config, cwd or a filesystem root vs lexical check only.
- Impact: irreversible deletion of the user's source of truth; with F-NODE-01 any `--out=src` wipes code.
- Root cause: textual validation; recursive clean.
- Recommendation: fix: resolve to absolute and refuse if out is an ancestor-or-equal of cwd/config dir/input file or a root; fix F-NODE-01.
- Regression check: tests for out equal to cwd, parent, input's dir, symlinks.
- Effort estimate: 2h-3h

### F-NODE-03: A failing generate leaves the previous output deleted; watch does this on every save
- Category: watch
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: cmd/veld/generate.go:757-760 (clean before parse), cmd/veld/watch.go:270
- Trigger/reproducer: generate OK, append garbage to the contract, regenerate: parse error printed and the out dir no longer exists. In `veld watch`, saving a half-edited contract wipes generated code (debounce is 300ms; errors are printed and the loop continues, which is good), breaking the dev server until the contract is valid again.
- Expected vs actual: failed run is a no-op vs destructive.
- Impact: dev-loop breakage; CI leaves an empty out dir after a failure.
- Root cause: clean precedes parse; no staging directory.
- Recommendation: refactor: emit to a temp dir and swap on success only.
- Regression check: broken contract leaves previous output byte-identical.
- Effort estimate: 4h-8h (shared with F-NODE-01)

### F-NODE-04: Breaking-change gate and `--strict` never fire (lock deleted before the diff)
- Category: generate-cli
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: cmd/veld/generate.go:50 (diff.DeleteLock inside runClean) vs :205 (hasLock)
- Trigger/reproducer: generate; remove action DeleteItem; `veld generate --strict` -> exit 0, nothing reported; lock re-saved without the action.
- Expected vs actual: exit 1 with breaking changes (CLAUDE.md "Contract Safety Features") vs silent success.
- Impact: the advertised CI protection is non-functional.
- Root cause: runClean removes .veld.lock.json and .veld-cache.json ahead of the diff.
- Recommendation: fix: stop deleting the lock in generate/watch.
- Regression check: end-to-end: remove an action, run `generate --strict`, expect exit 1.
- Effort estimate: 1h-2h

### F-NODE-05: Static routes shadowed by earlier parametric routes (including middleware-protected ones)
- Category: emitter-node
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: high
- Location: internal/emitter/backend/node/routes.go:75-87 (actions emitted in declaration order)
- Trigger/reproducer: `GET /items/:id` declared before `GET /items/boom`, `/items/guarded` (middleware AuthGuard), `/items/old`. Requests to each are answered by getItem; authGuard never invoked.
- Expected vs actual: each action reached by its own handler vs later same-method static paths unreachable and answered by another action without the middleware.
- Impact: dead endpoints; protected action silently replaced by an unprotected one.
- Root cause: no specificity ordering and no validator/lint rule for overlapping routes (lint duplicate-route does not catch it).
- Recommendation: fix: emit static-segment routes before parametric ones per method and add a validator error for ambiguous overlap.
- Regression check: `/:id` then `/me` reaches the right handler under express.
- Effort estimate: 3h-5h

### F-NODE-06: Generated output does not compile under `tsc --strict`
- Category: emitter-ts
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: frontend/typescript/client.go (`query as Record<string, unknown>`, `extraHeaders: Record<string,string>`), backend/node/routes.go (headers object literal)
- Trigger/reproducer: any contract with a `query:` model or `headers:` block; `tsc --noEmit --strict` (errors in section 1 step 5). Every framework wrapper inherits them.
- Expected vs actual: clean compile vs 4 errors (5 with --validate).
- Impact: users must disable strict or edit generated files.
- Root cause: models are classes with methods, so literals/instances are not assignable to or from Record types; no test compiles the output.
- Recommendation: fix: emit plain interfaces (or cast via unknown) and add a tsc compile test.
- Regression check: CI generates the rich contract and runs tsc --strict --noEmit.
- Effort estimate: 1h-2h (casts) or 6h-10h (interface migration)

### F-NODE-07: Models are classes with getters/setters and branded scalars
- Category: emitter-ts
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/emitter/backend/node/types.go, tshelpers/tshelpers.go, types/_scalars.ts
- Trigger/reproducer: `createItem({ s: 'x', ... })` is a type error (needs full `Everything` instance with getS()/setS()/toJSON()); uuid/decimal/date fields need `as UUID` casts. JSON responses are plain objects typed as the class, so methods do not exist at runtime (same mechanism as the reproduced headers error).
- Expected vs actual: CLAUDE.md says "types/interfaces"; actual class-per-model, 147 lines for 3 models.
- Impact: typing lies (res.getS() compiles, throws) and ergonomic cost.
- Root cause: class-per-model design.
- Recommendation: refactor: emit interfaces; make branded scalars optional.
- Regression check: object-literal call compiles.
- Effort estimate: 6h-10h

### F-NODE-08: No input validation unless `--validate`; documented Zod/schemas do not exist
- Category: emitter-node
- Evidence status: documentation mismatch
- Severity (user impact): medium
- Security severity: medium
- Location: backend/node/routes.go:105-110, CLAUDE.md "Route handler features"/"Generated Output Structure"
- Trigger/reproducer: POST `{"i":"notanumber"}` on default build -> 201, service receives it.
- Expected vs actual: "Zod .parse(); ZodError -> 400" vs raw req.body.
- Impact: services trust unvalidated input by default.
- Root cause: validation moved to opt-in zero-dep validators; docs stale.
- Recommendation: refactor: make validation default-on (or document opt-in) and update CLAUDE.md.
- Regression check: default generate emits _validators.ts; docs aligned.
- Effort estimate: 2h-4h

### F-NODE-09: `--validate` validators are shallow
- Category: emitter-node
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Location: backend/node/validate.go:110-300
- Trigger/reproducer: accepted: role:'hacker' (enum), tags:[1,2], meta:{a:1}, i:1.5, i:9007199254740993, bad uuid/date/decimal, maybe:null, unknown extra keys passed through. @default not applied; missing required headers not checked; GET query values are strings typed as numbers; `?page=abc` becomes a 500 via output assertion. Recursion has no depth cap (deep JSON bounded only by body-size limit; not tested, suspected risk).
- Expected vs actual: runtime checks matching the TS types vs typeof-only checks.
- Impact: contract holes; prototype-ish keys in `meta` pass through.
- Root cause: validators check typeof only.
- Recommendation: refactor: validate enums, element types, integers, formats, optional/null, coerce query.
- Regression check: table-driven bad values per scalar.
- Effort estimate: 8h-14h

### F-NODE-10: Error handler leaks internal messages on 500 and mixes error formats
- Category: emitter-node
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Location: backend/node/routes.go:185-191
- Trigger/reproducer: service throws Error('db password=... at /srv/x') -> 500 body has the message. `err.status` from any thrown object is trusted. Malformed JSON bypasses it (Express HTML page). The identical catch block is repeated per action.
- Expected vs actual: generic 500 body vs raw message.
- Impact: information disclosure; inconsistent error contract.
- Root cause: catch copies err.message regardless of status.
- Recommendation: fix: generic message for status >= 500 and one shared handleError.
- Regression check: 500 body is constant.
- Effort estimate: 2h-3h

### F-NODE-11: Path parameters are not URL-encoded in the SDK
- Category: emitter-ts
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Location: internal/emitter/helpers.go:25 (ToTemplateLiteral), frontend/typescript/client.go (urlExpr)
- Trigger/reproducer: `getItem('a/b?x=1#h')` requested /items/a/b (404); '../admin' would traverse.
- Expected vs actual: encodeURIComponent(id) vs raw interpolation.
- Impact: IDs with reserved chars break; client-side path injection when IDs come from user input. Angular has the same pattern.
- Root cause: `${id}` emitted directly.
- Recommendation: fix: emit `${encodeURIComponent(id)}`.
- Regression check: SDK test with ids containing / ? # %.
- Effort estimate: 1h-2h

### F-NODE-12: Query serialization is lossy
- Category: emitter-ts
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: frontend/typescript/helpers.go (buildQueryString)
- Trigger/reproducer: `tags:['x','y']` -> `tags=x%2Cy`; server gets string "x,y", not an array; nested objects become "[object Object]".
- Expected vs actual: repeated keys or defined format vs String(value).
- Impact: array query fields unusable; server type says string[].
- Root cause: String(value) and no matching parse on routes.
- Recommendation: fix: repeat keys for arrays and normalize on the route side.
- Regression check: round-trip array/bool/int through SDK and route.
- Effort estimate: 3h-5h

### F-NODE-13: SDK lacks timeout/abort, fetch injection and dynamic auth hook
- Category: emitter-ts
- Evidence status: design limitation
- Severity (user impact): medium
- Security severity: low
- Location: frontend/typescript/client.go:35-45, helpers.go (VeldClientConfig)
- Trigger/reproducer: config is {baseUrl, headers} only; no signal/timeout/fetch/token callback/retries; Content-Type sent on GET; DELETE sends `{}`; empty 200 body throws; VeldApiError.body is a raw string; `code` is '' not undefined when absent.
- Expected vs actual: production-ready client vs minimal wrapper.
- Impact: users wrap or fork generated code.
- Root cause: minimal request helper.
- Recommendation: refactor: extend config with fetch, headers callback, timeoutMs, per-call RequestInit.
- Regression check: SDK tests with fake fetch.
- Effort estimate: 4h-6h

### F-NODE-14: Module named `index` clobbers types barrel; JS-global model names break output
- Category: emitter-node
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: none
- Location: backend/node/types.go / barrel.go (file naming); no reserved-name check in the validator
- Trigger/reproducer: `module index {...}` writes types/index.ts twice; final content `export * from './index'` (self-reference), all types lost; tsc: no exported member Date/Error/Object/Map/Promise. Model `Promise` yields `Promise<Promise>`.
- Expected vs actual: validation error or collision-safe naming.
- Impact: cryptic failures from valid-looking contracts.
- Root cause: file names derived from module names without reserved list; no shadowing check.
- Recommendation: fix: validator rule rejecting reserved module/model names, or suffix generated files.
- Regression check: module `index` / model `Date` yields a validation error.
- Effort estimate: 2h-4h

### F-NODE-15: Angular emitter output is invalid and incomplete
- Category: emitter-angular
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: none
- Location: frontend/angular/emitter.go (models ~L110, service generation)
- Trigger/reproducer: generate --frontend=angular with the rich contract: models/index.ts contains `X-Trace-Id: string;` (TS1131/TS1128/TS1005); references UUID/Decimal/ISODate which are not defined or imported there; `secure(headers)` accepts headers but never passes them to http.get; baseUrl hard-coded ''; no VeldApiError; path params unencoded.
- Expected vs actual: working service vs compile failure and silently dropped headers.
- Impact: Angular target unusable with headers or branded scalars.
- Root cause: independent reimplementation that shares neither the TS SDK nor type emitter.
- Recommendation: refactor: reuse the TS type emitter and quote non-identifier keys.
- Regression check: compile Angular output against @angular/core@17 in CI.
- Effort estimate: 6h-10h

### F-NODE-16: `--out`/`--input` resolution is inconsistent and can write outside the project
- Category: generate-cli
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: low
- Location: internal/config/config.go (BuildResolved), cmd/veld/generate.go
- Trigger/reproducer: (a) config in rich/veld/ with backend out "../generated": plain `veld generate` -> rich/generated; adding `--input=veld/app.veld` -> backend printed as "./../generated" and written relative to cwd = e2e/generated, outside the project. (b) `--out=./gen3` with that config changed only the frontend (rich/veld/gen3); the config backend out wins silently. (c) Scaffolded config puts frontend output inside veld/ next to sources.
- Expected vs actual: documented precedence (flags > config) and one path base.
- Impact: surprising locations; destructive with F-NODE-01.
- Root cause: ConfigDir vs cwd base differs per path; split-output precedence.
- Recommendation: fix: resolve relative paths against one base and print absolute resolved paths.
- Regression check: flag precedence and `--input` with config in a subdir.
- Effort estimate: 3h-5h

### F-NODE-17: Writes are non-atomic, no manifest, stale files handled by "delete everything"
- Category: output-safety
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: low
- Location: emitters call os.WriteFile directly (backend/node/routes.go, frontend/typescript/emitter.go:65); no manifest anywhere
- Trigger/reproducer: a failure midway (read-only-dir simulation not run) leaves a partial tree; with F-NODE-01 the prior good tree is already gone.
- Expected vs actual: temp+rename; manifest-based cleanup.
- Impact: torn output after crashes or Ctrl-C.
- Root cause: no output abstraction (codegen package exists but node/ts do not use it).
- Recommendation: refactor: route all writes through a staged writer.
- Regression check: inject write failure; old tree intact.
- Effort estimate: 6h-10h (overlaps F-NODE-01/03)

### F-NODE-18: Doc/UX mismatches in CLI output and CLAUDE.md
- Category: generate-cli
- Evidence status: documentation mismatch
- Severity (user impact): low
- Security severity: none
- Location: cmd/veld/generate.go (import-instructions block near the end), CLAUDE.md
- Trigger/reproducer: "Import instructions" always print users/User examples even with no such module; generated README lists modules only; CLAUDE.md output tree (schemas/, auth.routes.ts, Zod) stale; `--backend=none` is not a valid value and silently fell back to node.
- Expected vs actual: accurate instructions.
- Impact: new-user confusion.
- Root cause: static templates.
- Recommendation: fix: derive instructions from the AST and refresh CLAUDE.md.
- Regression check: snapshot of instructions for a contract without `users`.
- Effort estimate: 2h-3h

### F-NODE-19: Tests never compile or run the generated TypeScript
- Category: emitter-node
- Evidence status: source-supported defect
- Severity (user impact): medium
- Security severity: none
- Location: internal/emitter/**/*_test.go
- Trigger/reproducer: no `tsc`/`exec.Command`/node in any emitter test; 210 `func Test` total (node 10, typescript 6, react 5), all substring assertions on text.
- Expected vs actual: compile tests would have caught F-NODE-06 and 15.
- Impact: regressions in generated code ship unnoticed.
- Root cause: string-contains test style.
- Recommendation: fix: add golden files plus a `tsc --strict` smoke job per target.
- Regression check: the CI job described.
- Effort estimate: 6h-10h

## 4. Emitter quality assessment

- Sizes (Go lines): backend/node 2071 total (sdk.go 475, validate.go 427, routes.go 324); frontend/typescript 950 (emitter.go 324, client.go 247); frontend/javascript 779; react emitter 453; angular 220; emitter/helpers.go 744. Manageable, but there are no templates: everything is sb.WriteString(fmt.Sprintf(...)); the identical catch block is repeated per action.
- Duplication: node-ts / node-js / TS client / JS client / Angular / server-client each re-implement request helpers, query building, path interpolation and type mapping (Angular does not reuse the type emitter). The `codegen` and `lang/*` helper packages are used by go/rust but not by node/typescript.
- Adding a new backend: implement Emit, Summary, the IsBackend marker and EmitServiceSdk (mandatory even for plain backends), register via init() and a blank import in main.go. Frontend: Emit, Summary, IsFrontend. The plumbing is small and clean; the real cost is type mapping and route emission.
- Tests: 210 test functions under internal/emitter; style is strings.Contains on output; no golden files, no compile of generated TypeScript.
- Architecture: contract -> AST -> emitter isolation is sound and output is deterministic. Risk is concentrated in generated-code semantics (route order, validation depth, class-based models, SDK ergonomics, Angular) and in the CLI's destructive write strategy, not in the pipeline.
