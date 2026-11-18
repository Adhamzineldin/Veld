# M2 implementation plan for T2 (safe generation, Node/TypeScript)

- Date: 2026-11-18
- Author: Mohamed (T2)
- Milestone: M2 "Core: IR, diagnostics, LSP engine", 2026-11-30 to 2027-01-10 (S3, S4, S5). Exit review 2027-01-08.
- Reviewers requested: Adham (T1), on the IR split and the B-027 decision; Menna (T4), on Python/Go parity and fixtures; Ahmed (T5), on CLI, exit codes and CI.
- Status: PROPOSED. Sections 3 and 4 need answers by 2026-12-02 (S3, day 3).
- Sources: team-roles PDF (T2 M2 tasks); `docs/review-2026-10/15-roadmap-to-june-2027.md` (M2 row, sprints 7 to 11); `csv/sprint-allocation.csv` (S3 to S5); `csv/backlog.csv` (B-026, B-027, B-024 as reviewer); chapter 08 §8.2 to §8.6; chapter 09 §9.4, §9.7; ADR-002, ADR-005 and its amendment, ADR-007, ADR-012; `docs/notes/2026-11-18-staged-writer-decisions.md`.

## 1. What M2 must deliver from T2

| Source | Commitment | Work package |
|---|---|---|
| PDF M2 #1, S3 "OutputFile API" | Route all built-in emitters through the safe writer and the shared compiler/IR interface | WP-A, WP-C |
| PDF M2 #2 | Output path validation, managed-file tracking, deterministic writes | Mostly done in M1 (`internal/output`); what remains is WP-A, plus the determinism job on two OSes (WP-F) |
| PDF M2 #3, S5 "tier-1 emitters on IR", B-026 | Node plain and Express, TypeScript client and React Hooks aligned with IR v1 | WP-B, WP-C |
| B-026 | Validation on by default, with deep validators | WP-D |
| PDF M2 #4 | Coordinate clean/watch handling and breaking-change checks with the CLI owner | WP-E |
| S3 "emitters on diag" | Emitters report unsupported constructs as diagnostics (AGENTS.md §1.7) | WP-A3 |
| S4 "IR lowering v0" | AST to IR adapter so emitters can migrate before IR v1 is final | WP-B |
| B-027 (planned for S2, not done) | Tier-3 removal PR | WP-G (needs decision D1) |

The parts of the M2 exit criteria (chapter 15) that T2 contributes to are: tier-1 emitters on IR; byte-identical generation on two OSes; validation on by default; generated output still compiles and round-trips.

## 2. Starting point (verified at 4fa9275, 2026-11-18)

- **Done in M1:**
  - the staged writer, manifest, output policy, `clean`, `init` and dry-run fixes (B-006, B-008, B-009);
  - determinism (B-012);
  - the Node/TS fixes (B-015);
  - `TestNodeToolchain`, which runs `tsc --strict` plus an Express round trip in CI;
  - 9 output-safety acceptance tests.
- **Emitters still write files themselves.** There are 225 `os.WriteFile` calls across `internal/emitter` and `internal/generators`. They only reach a staging directory, but they cannot be checked for path validity or duplicates.
  - Tier-1 calls: node 18, tsshared 9, typescript 7, react 5, python 22 (T4), go 16 (T4), openapi 1.
  - `EmitOptions.DryRun` is always false and still checked at 32 sites.
- **No IR yet.** `internal/ir`, `diag`, `semantic` and `compiler.Compile` do not exist. Every emitter reads `ast.AST` (90 files) and re-derives semantics itself: status codes, path parameters, inheritance, route order (`emitter.OrderBySpecificity`), header identifiers.
- **No golden-output tests.** Emitter tests assert substrings. `TestNodeToolchain` and `TestDeterministicOutput` are the only behavioural guards.
- **Carried over from M1:**
  - the `node-js` backend still leaks 5xx messages;
  - the node-ts service SDK drops GET `input`;
  - the gate parses the contract twice and is silently skipped on a parse error;
  - workspace locks collide in the working directory;
  - ADR-007 is still PROPOSED.

## 3. Dependency on T1 and how this plan absorbs it

T1's sprint plan delivers `diag` in S3, `TypeRef` in S4, and IR v1 plus its schema and `veld ir` in S5. T2's "tier-1 emitters on IR" is also in S5, which puts the two on the same critical path in the same sprint as the exit review.

To take IR v1 off T2's critical path:

1. **Freeze the emitter-facing IR Go types early.** By 2026-12-02, T1 and T2 agree `internal/ir/ir.go`: `Program`, `Model`, `Field`, `TypeRef`, `Enum`, `Service`, `Operation`, `Segment`, `Header`, `Output`, `Span`, following chapter 08 §8.3. T1 owns the file; T2 drafts it in WP-B1 for T1 to edit and approve. After 2026-12-14, changes are additive only.
2. **T2 writes the S4 lowering v0** (`internal/ir/lower`, AST to IR), as `sprint-allocation.csv` assigns. Emitters migrate against v0 in S4. T1's `semantic.Lower` in S5 replaces the adapter behind the same types, and emitters do not change.
3. **Move semantics out of the emitters.** Facts emitters derive today move into lowering, so they are computed once:
   - default status codes (`SuccessStatusForAction`);
   - path segments and parameters;
   - literal-before-param route order;
   - flattened inheritance with provenance;
   - header wire names plus derived identifiers (F-MULTI-01);
   - used-model sets (`CollectTransitiveModels`).

   This is the point of ADR-002.

If T1's IR v1 slips past 2027-01-05, the exit review accepts "tier-1 TS emitters on IR via lowering v0". The emitter work does not wait for IR v1.

## 4. Decisions needed (by 2026-12-02)

| # | Question | Recommendation | Who decides |
|---|---|---|---|
| D1 | B-027 says delete rust, angular, dart, kotlin, swift, mock, dockerfile, cicd and envconfig in M2. ADR-012 (accepted) brings all of them back later: Q13 in S12–S13 (owners T4/T5) and Q8 in S11 (T5). | **Do not delete.** Mark them experimental: they stay registered, print a one-line warning when used, are listed as "experimental (not tested)" in `--help`, and stay on the legacy AST path (WP-A keeps that path working). Still remove the incremental cache (`--incremental`, `.veld-cache.json`), as chapter 08 §8.5 asks. Record the decision as an amendment to ADR-005. | T1 with T4, T5 |
| D2 | Who owns the `internal/ir` types and lowering v0? | T1 owns the types and the schema; T2 drafts the types (WP-B1) and writes lowering v0 (WP-B2, S4), with T1 as reviewer. | T1 |
| D3 | Validation default ON: unknown keys in a request body? | **Strip** them before the service sees them. That matches Pydantic's default (`extra="ignore"`), so the Python path behaves the same (T4), and it closes mass-assignment holes without breaking clients that send extra fields. Reject `null` for optional fields (chapter 08 §8.4). | T2 with T4 |
| D4 | F-NODE-07 (models are classes with getters/setters): change during the IR migration? | **No, not in M2.** The migration must stay byte-identical (WP-C rule). Proposed for M4 with a migration note. | T2 |
| D5 | Exit codes for refusals and for `--strict`. | Use chapter 08 §8.5: 1 diagnostics (and unsafe output path, unmanaged conflict), 2 usage or config, 3 plugin, 4 breaking change under `--strict`. T2 changes `generate`/`clean`/`init` once T5's B-046 helper exists. | T5 |

## 5. Work packages

Estimates are baseline hours before any AI-assistance multiplier (chapter 20). "Must" items fit the baseline capacity in §7; "stretch" items run only if the S3 measurement shows room.

### WP-0 Golden snapshots before anything moves (S3, must, 4–6 h)

Every later step is a refactor of generated output, so the safety net comes first (AGENTS.md §2: "golden output plus a compile test for any emitter change").

- `cmd/veld/golden_test.go` generates each fixture for each tier-1 target and compares the tree with `cmd/veld/testdata/golden/<fixture>/<target>/`. `-update` rewrites the snapshots, and the diff is reviewed in the PR.
- **Fixtures:** the B-015 contract (`testdata/node/app.veld`), `determinismContract`, and `examples/node-typescript`. T4's conformance fixtures are added as they land (15 by S3, 30 by S4).
- **Targets:** node-ts plain, node-ts express, node-ts `--validate`, typescript, react, python flask, go chi, openapi.
- **Accept when:** snapshots are committed; the test fails on any one-byte change to an emitter; the review of the snapshot diff is part of the PR checklist.

### WP-A Emitters write through the host (S3, must, 12–18 h)

- **A1. Output API** in `internal/emitter`:
  ```go
  type Output interface { WriteFile(rel string, data []byte) error } // rel: slash-separated, clean, local
  ```
  - `output.Session` provides one per staged directory.
  - It validates `rel` with the manifest rules, so no `..`, absolute path or reserved name can get through.
  - It records each file and rejects a second write of the same path by a different emitter unless the bytes are identical. That case is the shared `tsshared` files, open question 4 of the writer note.
  - This is the in-process form of ADR-007's `[]OutputFile`. Plugins (B-037, M3) will return the same `{path, content, mode}` records into the same `Session`, so built-in and external emitters share one checked path.
- **A2. Migrate tier-1 TypeScript emitters** to `Output`: node, tsshared, typescript, react, the node SDK (39 write sites), then openapi.
  - The change is mechanical: `os.MkdirAll` plus `os.WriteFile(filepath.Join(outDir, ...))` becomes `out.WriteFile(rel, ...)`.
  - The legacy staging-directory path stays for every emitter not yet migrated, including Python/Go until T4 is ready (A2 does not touch T4's packages without T4).
  - Delete `EmitOptions.DryRun` and its 32 checks in one commit.
- **A3. Unsupported features fail loudly.**
  - Add `emitter.UnsupportedError{Feature, Action/Model, Span}`, converted to a `diag.Diagnostic` with code `E_UNSUPPORTED_FEATURE` when T1's `diag` lands.
  - Node and TS emitters use it, instead of emitting wrong code, for the constructs chapter 08 §8.4 defers from lang 1.0 that they cannot represent correctly today. The exact list is fixed in S3 by a test per construct (candidates: union types, `WS`/`stream:`/`emit:` actions, HEAD/OPTIONS methods). One table-driven test per construct.
- **Accept when:**
  - golden snapshots are unchanged;
  - `TestNodeToolchain`, `TestDeterministicOutput` and the output-safety tests pass;
  - a unit test shows that an emitter writing `../x` or the manifest name is refused.

### WP-B IR types draft and lowering v0 (S3 draft, S4 implementation, must, 16–24 h)

- **B1** (S3, 4–6 h): draft `internal/ir/ir.go` from chapter 08 §8.3 for T1 (decision D2). The IR has no maps, only sorted slices, for determinism. It carries `irVersion` and `langVersion`, plus `Span` on every node.
- **B2** (S4, 10–14 h): `internal/ir/lower.FromAST(ast.AST) (*ir.Program, error)`. It moves the semantics listed in §3 out of the emitters, including `OrderBySpecificity`, which becomes the order of `Service.Operations` with a recorded reason.
- **B3** (S4, 2–4 h): IR JSON goldens for every fixture (input to T1's schema work); `go test` checks lowering is deterministic (lower twice, byte-compare the JSON).
- **Accept when:**
  - the 11 B-015 round-trip checks pass on output produced from IR (after WP-C1);
  - IR JSON is byte-identical across runs;
  - the checkpoint "IR v0 for one fixture" on 2026-12-11 is met by B3 on the B-015 fixture.

### WP-C Tier-1 TypeScript emitters on IR (S4 to S5, must, 22–34 h)

Order, one commit per step, each byte-identical against WP-0 snapshots unless the commit says otherwise:

1. **C1 Host:** `generate()` compiles once. It runs lowering v0, or `compiler.Compile` when T1 ships it, and passes `*ir.Program` to emitters that implement the new interface:
   ```go
   type IREmitter interface { EmitIR(p *ir.Program, out Output, opts EmitOptions) error }
   ```
   The host prefers `EmitIR`; everything else keeps `Emit(ast.AST, outDir, opts)` (ADR-002 §4 adapter). The same compile result feeds the breaking-change gate (WP-E).
2. **C2** `tsshared` types and type barrel.
3. **C3** node interfaces, routes, middleware, `_respond.ts`, barrel.
4. **C4** node validators (start of WP-D).
5. **C5** typescript client.
6. **C6** react hooks.
7. **C7** node-ts service SDK. Fix its dropped GET `input` here; the snapshot changes for that case only.

Once every tier-1 TS emitter is migrated, an import-boundary test (AGENTS.md §1.4) checks that these packages no longer import `internal/ast`. Python and Go follow the same pattern with T4 (PDF Menna M2 #1), with T2 as reviewer.

**Accept when:** every C step is green on WP-0, `TestNodeToolchain` and `TestDeterministicOutput`, and the import-boundary test passes for node, tsshared, typescript and react.

### WP-D Validation on by default, with deep validators (S5, must for enum, format and null checks; stretch for coercion; 16–28 h)

B-026, F-NODE-08, F-NODE-09. Chapter 15's risk note applies: keep enum and format checks, defer query coercion if time runs short.

- **Must:**
  - `validate` defaults to true; `--no-validate` and `"validate": false` turn it off (documented, CHANGELOG under "Changed").
  - Validators check:
    - enum membership;
    - element types of arrays and maps;
    - `int` via `Number.isSafeInteger`;
    - formats for `uuid`, `date`, `datetime` (RFC 3339 with offset) and `decimal`;
    - required fields;
    - `null` rejected for optional fields;
    - unknown keys stripped (D3);
    - required headers present, which closes the gap where an undefined header reached the service, noted in B-015;
    - a recursion depth cap.
  - `@default` is applied before the service is called.
  - A validation failure gives 400 with `code: VALIDATION_ERROR` and a stable list of `{path, message}`; parity with Pydantic is agreed with T4.
- **Stretch:**
  - query coercion (`page=2` becomes the number 2; `page=abc` gives 400 instead of a 500 from the output assertion);
  - repeated keys for array query fields on both client and server (F-NODE-12).
- **Tests:**
  - table-driven validator tests (one case per rule, valid and invalid);
  - error cases in `roundtrip.ts`, taken from the audit's §6m inputs (`role:'hacker'`, `tags:[1,2]`, bad uuid/date/decimal, `i:1.5`, `maybe:null`, extra key), each expected to give 400 or be stripped;
  - the round trip runs with validation on by default.
- **Accept when** the B-026 criterion "round-trip error cases pass; `--no-validate` documented" holds.

### WP-E Coordination with the CLI owner: generate order, gate, clean/watch (S5, must, 6–10 h)

PDF M2 #4, with T5 (CLI) and T1 (diff on IR).

- One compile per run. The pipeline follows chapter 08 §8.6: compile, diff against the lock, gate, emit to staging, swap, manifest, lock, hooks. `computePreChanges` and its second parse go away, so a parse error stops the run with diagnostics before the gate instead of skipping it silently (remainder of B-007; T1 owns the gate, T2 owns the host side).
- `diff` runs on IR when T1 switches it in S5; the gate uses the `Compile` result unchanged.
- Remove the incremental cache and `--incremental` (D1, chapter 08 §8.5). This deletes `Options.Partial` from `internal/output`, which simplifies the writer.
- `watch` uses the same single compile, keeps the previous output on error (already true) and handles SIGTERM. Exit codes follow D5 once B-046 is in.
- Workspace entries keep their lock and cache in the config directory, not the working directory. This fixes the collision found in M1, with T1, because the lock is T1's.

### WP-F Carry-overs and hardening (S3 to S5, must, 4–8 h)

- `node-js` backend: use the same `_respond` logic (F-NODE-10 for the JS path). The source is generated from one template shared with node-ts, not copied.
- ADR-007 acceptance by T1 (writer note review), ideally at the S3 start.
- Determinism on two operating systems: the job runs once T5's OS matrix is in (S5). `TestDeterministicOutput` already exists; it only needs to run on Windows and Linux in CI.
- Snapshot of the M1 hours actually spent, for the chapter 20 multiplier measurement (integration owner's request).

### WP-G B-027 (S3, after D1, 3–5 h)

As decided under D1. With the recommendation:

- experimental warning and `--help` labels for the Q8/Q13 targets;
- remove the incremental cache (shared with WP-E);
- rewrite the README support matrix to tiers. The matrix is generated from CI per AGENTS.md §1.6; until that generator exists, the README states the tiers with a link to the CI jobs that back each tier-1 claim.

## 6. Out of scope for M2 (and where it goes)

- VEP v1 schema and plugin host (B-036, B-037): M3, starting S6. WP-A's `Output` and the `Session` path checks are the parts the host will reuse.
- Models as interfaces instead of classes (F-NODE-07): M4, decision D4.
- SDK timeouts, abort and auth hooks (F-NODE-13), reserved module/model names (F-NODE-14, T1 validator, B-028): not T2's M2 work.
- Q1 to Q14 extended features: no earlier than S8 (ADR-012).

## 7. Schedule and capacity

Baseline capacity for T2 in M2 is about 50 to 55 h. That is 12 h/week over six weeks, with the 2026-12-21 to 2027-01-03 break at 50% and 15% for coordination (chapter 15 §15.1). Must items total 80 to 120 h at baseline estimates, so the plan only fits if the assisted pace seen in M1 holds. B-006 to B-015 were estimated at 27 to 46 h and done in one AI-assisted working session on 2026-11-18; that is not yet a recorded hours measurement (WP-F). If the S3 checkpoint shows less than half of WP-0, WP-A and WP-B1 done, drop the WP-D stretch, then C6 and C7 (React and SDK stay on the AST path for M2), in that order.

| Sprint | Dates | T2 work | Checkpoint |
|---|---|---|---|
| S3 | 2026-11-30 to 12-13 | D1–D5 answered (by 12-02); WP-0 snapshots; WP-B1 IR types draft; WP-A1, A2 (TS family), A3; WP-G; WP-F `node-js` responder | 12-11: IR v0 for one fixture (T4's checkpoint; B3 on the B-015 fixture is T2's part). T2 internal: all tier-1 TS emitters write through `Output`, snapshots unchanged. |
| S4 | 12-14 to 12-27 (break from 12-21) | WP-B2, B3 lowering v0; WP-C1 to C3 | 12-23: node backend emits from IR with snapshots unchanged; IR JSON goldens for every fixture available to T1 |
| S5 | 12-28 to 2027-01-10 (break to 01-03) | WP-C4 to C7; WP-D must; WP-E; determinism on two OSes | 01-08: M2 exit review. Tier-1 TS emitters on IR, validation on by default, round trip green with error cases, byte-identical on two OSes. |

## 8. Risks

| Risk | Likelihood | Effect | Response |
|---|---|---|---|
| IR v1 lands late in S5 | high | emitters migrate twice | Emitters target the frozen v0 types (§3); IR v1 changes are additive after 12-14 |
| A migration silently changes output | medium | users see unexplained diffs; tests still green | WP-0 snapshots before any change; every step byte-identical unless the commit says why |
| Validation default ON breaks existing users | medium | 400s where requests used to pass | CHANGELOG "Changed" entry; `--no-validate`; error body lists every failing path |
| Conflicts with T4 in shared packages (`internal/emitter`, `tsshared`, host) | medium | merge churn | T2 owns the host and `Output`; T4 owns `backend/python` and `backend/go`; changes to `internal/emitter` helpers are announced in the team channel before they land |
| Winter break and January finals | high | S5 is short | S5 contains no new design; anything not started by 01-03 moves to S6 slack, not to the exit review |

## 9. Definition of done for every T2 change in M2

As AGENTS.md §2, plus:

- WP-0 golden snapshots are unchanged, or the diff is reviewed and explained in the commit;
- `TestNodeToolchain` passes (TypeScript strict compile and Express round trip);
- `TestDeterministicOutput` passes;
- output-safety tests pass;
- coverage of `internal/output` and any new package stays at or above 70%;
- CHANGELOG entry for user-visible changes;
- a commit message that names the backlog item and finding.
