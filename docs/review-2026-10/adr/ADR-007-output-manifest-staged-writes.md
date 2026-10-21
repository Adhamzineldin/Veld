# ADR-007: Generated output is written through a staged writer with a manifest; never `RemoveAll`

- Status: PROPOSED (accept by 2026-11-01; implement in M1, before any other emitter work)
- Date: 2026-10-21
- Deciders: T2 (owner), T1; reviewer: T3
- Evidence (reproduced by the Node/TS audit track and confirmed by the lead reviewer at the cited lines):
  - `cmd/veld/generate.go:40-52` (`runClean`) calls `os.RemoveAll` on every output directory, removes `.veld-cache.json` and deletes `.veld.lock.json`; it is invoked at `generate.go:758-760` before the contract is parsed.
  - Consequences reproduced: user files inside the output directory are deleted (F-NODE-01); `--out=../proj` deleted the project's own `.veld` sources because `internal/config/config.go:371-379` only inspects the last path segment (F-NODE-02); a contract with a syntax error wipes the previous good output and `veld watch` does so on every save (F-NODE-03); the breaking-change gate and `--strict` never fire because the lock is deleted before the diff runs (F-NODE-04).
  - Emitters call `os.WriteFile` directly; there is no record of which files Veld produced (F-NODE-17).

## Decision

1. Emitters (built-in and external) never write to disk. They return `[]OutputFile{Path, Content, Mode}` to the emission host (chapter 08, section 8.6).
2. The host writes every file to a staging directory beside the output directory (`<out>.veld-staging-<pid>`), then swaps: files whose content is unchanged are left untouched (mtime preserved), changed files are renamed into place one by one, and files listed in the previous manifest but absent from the new set are deleted. Files not in the previous manifest are never deleted; if a generated path collides with an unmanaged file, generation fails with the path named and `--overwrite-unmanaged` is required to proceed.
3. `.veld-manifest.json` in the output directory records `veldVersion`, `langVersion`, `irVersion`, emitter name and version, and for each file its path and SHA-256. `veld clean` deletes exactly the manifest's files and then empty directories; `veld diff` reports files modified by hand since generation.
4. Parsing and validation always complete before any filesystem change. A failed compile leaves the previous output and the lock untouched.
5. Output path safety: `out` is resolved to an absolute path and refused when it equals or contains the config directory, the input file's directory, the current working directory, a filesystem root, or the user's home directory. The resolved absolute paths are printed in `--verbose` and `--dry-run`.
6. `--dry-run` runs the whole pipeline including emitters and prints the would-be file list and a content diff against the manifest, writing nothing.
7. The breaking-change lock is written only after a successful generation and is never deleted by generate or watch; only `veld clean --lock` removes it.

## Reasons

- "Veld never touches developer business logic files" and "Veld never writes outside the --out directory" are headline promises in CLAUDE.md and README; today both are false.
- A manifest is also the mechanism that lets external plugins share the same safety guarantees (ADR-003).

## Alternatives rejected

- Keep `RemoveAll` but warn: still destroys user files on a typo.
- Per-emitter cleanup: duplicates logic in 20 packages and does not cover plugins.

## Consequences

- Every emitter's `Emit` signature changes to return files (or write through a `codegen.Writer` that collects them). This is a mechanical change across `internal/emitter/**` and is scheduled before the IR migration so the two refactors do not collide.
- Estimated cost 20 to 30 h including tests (output-safety tests become part of the conformance suite).
