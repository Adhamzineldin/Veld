# Unsafe file-writing paths in generate, clean, watch and `--out` (M0 / WP1)

- Date: 2026-10-24
- Author: Mohamed (T2)
- Commit inspected: `008bad2` (master)
- Purpose: M0 task "review generator, clean, watch and --out behaviours; isolate unsafe file-writing paths". This note is the input for the output-safety tests (WP2), the writer API note with T1 (due 2026-10-29) and backlog items B-006 to B-009 and B-012.
- Status: inventory only. No code was changed.

## 1. Summary

Every destructive behaviour comes from one function, `runClean` (`cmd/veld/generate.go:40-52`). It runs `os.RemoveAll` on every output directory and deletes `.veld-cache.json` and `.veld.lock.json`. It is called before the contract is parsed and on every save in `veld watch`. Path validation in `internal/config/config.go:371-379` only looks at the last path segment, so `RemoveAll` can be aimed at the contract sources. Emitters write with `os.WriteFile` directly, and no record exists of which files Veld produced. That is why "delete everything" is the only cleanup strategy available today.

ADR-007 (staged writer + `.veld-manifest.json`) addresses all of the P0 rows below. Section 5 lists the decisions the writer API note has to settle.

## 2. Reproduced on Windows 11 at 008bad2

All runs were in a scratch copy of a fresh `veld init` project (Node TS backend, TypeScript frontend, defaults). The binary was built with `go build -o veld.exe ./cmd/veld` (Go 1.27.0).

| Finding | Steps | Observed |
|---|---|---|
| F-NODE-01 | `veld generate --force`; create `generated/my-notes.ts`; `veld generate --force` | exit 0; `my-notes.ts` deleted |
| F-NODE-02 / F-CLI-02 | `veld generate --force --out=../veld` (config in `veld/`) | exit 1 "reading ...\veld\app.veld"; all 6 `.veld` files and the config deleted, the `veld/` folder gone |
| F-NODE-03 / F-CLI-01 | generate OK; append `garbage {{{` to `veld/modules/users.veld`; `veld generate --force` | exit 1 with parse error; `generated/` gone; `veld/.veld.lock.json` gone |
| F-NODE-04 / F-CLI-01 | generate OK; delete `action GetUser` from `users.veld`; `veld generate --strict` | exit 0, no breaking-change report; lock re-written without the action |
| F-CLI-06 | file listing before and after `veld generate --dry-run` | new file `veld/.veld-cache.json` |

Not reproduced in this session (taken from the defect register, source-checked only): F-CLI-03 (`veld clean` with `out: ./src`), F-NODE-17 (partial tree after a mid-run write failure), F-CLI-05 (`init` overwriting `README.md`), and the watch-mode variant of F-NODE-03.

## 3. Inventory

### A. Code that deletes files or directories

| # | Location | What it does | Unsafe because | Findings | Fixed by |
|---|---|---|---|---|---|
| A1 | `cmd/veld/generate.go:40-52` `runClean` | `os.RemoveAll` on each of `rc.OutputDirs()`, removes `.veld-cache.json`, calls `diff.DeleteLock` | Deletes files Veld did not create; deletes the breaking-change baseline | F-NODE-01, F-CLI-03 | B-006 |
| A2 | `cmd/veld/generate.go:758-760` | `runClean(rc)` before workspace handling, parsing and validation (only skipped for `--dry-run`) | A failing run leaves no output | F-NODE-03, F-CLI-01 | B-006, B-007 |
| A3 | `cmd/veld/generate.go:1021` vs A2 | `computePreChanges` reads the lock **after** A2 deleted it | `--strict`, `--force` and the interactive prompt can never fire | F-NODE-04, F-CLI-01 | B-007 |
| A4 | `cmd/veld/generate.go:1166` (`veld clean`) | `runClean(rc)` with no confirmation | Deletes any directory `out` points at, including user code | F-CLI-03 | B-009 |
| A5 | `cmd/veld/watch.go:56` | `runClean(rc)` before the initial generation | Same as A2 | F-NODE-03 | B-006 |
| A6 | `cmd/veld/watch.go:270` | `runClean(currentRC)` on **every** debounced change | Saving a half-edited contract wipes the output the dev server imports | F-NODE-03 | B-006 |
| A7 | `internal/diff/lock.go:94` `DeleteLock` | Removes `.veld.lock.json`; called from A1 | Lock should only be removed by an explicit `clean --lock` (ADR-007 §7) | F-NODE-04 | B-007 |
| A8 | `internal/emitter/backend/java/errors.go:47-51` | Emitter deletes 9 named files in `models/` and `RemoveAll(models/errors)`, errors ignored with `_ =` | Deletes outside any central control; violates AGENTS.md §3 "no `_ = f()` on I/O" | — | B-006 (deletes move to the host); Java is outside tier 1 (ADR-005) |
| A9 | `internal/emitter/backend/csharp/errors.go:35-43` | Same pattern: deletes 10 named files in `Models/`, errors ignored | Same as A8 | — | B-006; C# is outside tier 1 |

Temporary directories removed with `defer os.RemoveAll` in `cmd/veld/contract.go:359,367` are created by Veld itself and are safe.

### B. Output path validation

| # | Location | Current behaviour | Gap | Findings | Fixed by |
|---|---|---|---|---|---|
| B1 | `internal/config/config.go:371-379` `validateOutPath` | Rejects only when the last segment is `..`, `.` or a separator | `../veld` (the config dir), absolute paths, home and drive root are all accepted | F-NODE-02, F-CLI-02 | B-008 |
| B2 | `internal/config/config.go:411-428` | Relative `out`, `backendOut` and `frontendOut` resolve against the config dir | Resolved absolute paths are never shown to the user before deletion | F-CLI-02 | B-008 (print in `--verbose` / `--dry-run`) |
| B3 | `internal/config/config.go:481-513` `workspaceOutputDirs` | Workspace entry `out` / `frontendConfig.out` are resolved but **never validated** | A workspace entry can point anywhere, and A1 deletes it | F-CLI-02 | B-008 |

**Default layout observation:** the `veld init` defaults (node-ts + typescript, no split chosen) produce `backendConfig.out: "../generated"` and no `frontendConfig.out`. The frontend therefore falls back to `out`, which defaults to `./generated` and resolves against the config dir, so it lands in `veld/generated/`, inside the contracts folder. It isn't destructive on its own, but it puts generated code next to the sources that A1 deletes around. Raise it with T5, who owns `init`.

### C. Writes with no record of what Veld produced

| # | Location | What it does | Gap | Findings | Fixed by |
|---|---|---|---|---|---|
| C1 | 85 files under `internal/emitter/**` and `internal/generators/**` | Each emitter calls `os.WriteFile` / `os.MkdirAll` itself | No manifest, no atomic swap, no collision check with user files | F-NODE-17 | B-006 (emitters return `[]OutputFile`) |
| C2 | `internal/emitter/tsshared/{types,errors,constants}.go` | Shared TS writers called by node, typescript, angular | Never check `DryRun` themselves; they rely on callers returning early | — | B-006 |
| C3 | Every emitter's `Emit` (e.g. `backend/node/main.go:54`, `backend/java/main.go:85`) | `if opts.DryRun { return nil }` | `--dry-run` never runs emitters, so it cannot list or diff the files it would write (ADR-007 §6 requires that) | — | B-009 |
| C4 | `cmd/veld/generate.go:185-189` `writeGeneratedReadme` | Writes `README.md` into each output dir (guarded by `DryRun`) | Not in any manifest; collides with a user `README.md` in a shared folder | F-NODE-01 | B-006 |
| C5 | `cmd/veld/generate.go:191-200` | Cache save is **not** guarded by `DryRun` | `--dry-run` writes `.veld-cache.json` | F-CLI-06 | B-009 |
| C6 | `cmd/veld/generate.go:120` | `_ = c.Save(rc.ConfigDir)` on the incremental no-op path | Error ignored (AGENTS.md §3) | — | B-009 |
| C7 | `cmd/veld/generate.go:141-142` | Comment says "No directory wipe — user files are never touched" | False because of A2; misleads reviewers | F-NODE-01 | B-006 (delete the comment with the code) |

### D. Writes outside the output directory

| # | Location | What it does | Note | Owner |
|---|---|---|---|---|
| D1 | `cmd/veld/init.go:28-33, 655` | Refuses only if a config file exists, then `os.WriteFile` on every scaffold file | Overwrites an existing `README.md` / `veld/*.veld` (F-CLI-05) | T5 (B-009 acceptance) |
| D2 | `cmd/veld/init.go:788` | `_ = os.WriteFile("veld/veld.config.json", ...)` | Error ignored | T5 |
| D3 | `internal/setup/setup.go:362, 416, 460` | `veld setup` edits the user's `package.json`, `tsconfig.json`, Vite config | Writing outside `out` is intended here, but it must be documented and must never run as part of `generate` | T2 to document |
| D4 | `cmd/veld/generate.go:198`, `internal/diff/lock.go:85` | Cache and lock files in the config dir | Expected location; lock already uses temp-file + rename | T1 (B-007) |

### E. Out of scope for this note

These are explicit single-file outputs chosen by the user, or registry writes owned by T3: `cmd/veld/contract.go:179, 219, 268, 580` (`openapi -o`, `graphql`, `schema`, `docs`), `contract.go:632` (`veld fmt` rewrites `.veld` files in place), `cmd/veld/registry.go:214-238` (`registry pull` unpacks into a directory, covered by B-019 / F-REG-09, F-REG-10), and `registry.go:645` (credentials file).

## 4. Mapping to backlog

| Backlog | Rows closed | Owner / reviewer |
|---|---|---|
| B-006 staged writer + manifest | A1, A2, A5, A6, A8, A9, C1, C2, C4, C7 | T2 / T1 |
| B-007 gate ordering, lock never deleted | A3, A7 | T1 / T2 |
| B-008 out path safety | B1, B2, B3 | T2 / T5 |
| B-009 dry-run, clean, init | A4, C3, C5, C6, D1, D2 | T2 / T5 |

## 5. Questions for the writer API note (T1 pairing, 2026-10-29)

1. **Refusal rule:** ADR-007 §5 refuses an `out` that equals or contains the config dir, input dir, cwd, home or root. Should an `out` that sits **inside** the config dir but contains `.veld` sources (e.g. `out: "models"`) also be refused? With a manifest it is no longer destructive, but generated files could collide with contracts.
2. **Migration:** existing projects have no manifest. On the first run of the new writer, should existing files in `out` count as unmanaged (fail and require `--overwrite-unmanaged`), or should a one-time adoption flag exist?
3. **Generated `README.md`** (C4): should it stay a managed file, or be dropped?
4. **`tsshared` writes** (C2): these files are produced by both the node backend and the typescript frontend into the same folder. Which emitter owns them in the manifest when backend and frontend share an output dir?
5. **Exit codes** (with T5): the code for a refused path and for an unmanaged collision, and how `--strict` interacts with the exit-code taxonomy in B-046.
6. **Failing tests before the fix:** do the WP2 tests land behind a build tag in a non-blocking CI job, or on a branch merged together with B-006?
