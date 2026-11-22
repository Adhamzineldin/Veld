# Veld v1 archive

This directory holds the **v1 Go implementation of Veld**. It is **frozen and superseded by Veld 2.0**
(the Rust rewrite in the repository root). It is kept for reference only: do not build on it, do not
fix bugs here, and do not copy v1 behaviour into 2.0 without checking the specs in [`spec/`](../../spec).

The last commit before the archive move is tagged `v1-final`
(`git show v1-final`, `git checkout v1-final` for the full v1 tree at its original paths).

| Path | What |
|---|---|
| `go/cmd`, `go/internal`, `go/go.mod`, `go/go.sum` | The v1 compiler and CLI (module `github.com/Adhamzineldin/Veld`). It no longer builds from here: its import paths assume the repository root. Check out `v1-final` to build it. `internal/server` was the registry and now lives in `registry/` |
| `examples/` | The v1 examples (`veld.config.json` and v1 `.veld` files). `microservices/veld` was the starting point of the v2 `examples/microservices` |
| `distribution/` | v1 wrappers that are not maintained for 2.0: go, nuget, chocolatey, apt, composer |
| `docs/`, `notes/`, `scratch/`, `scripts/` | v1 docs, ADRs, execution plans, scratch files |
| `github-workflows/` | The v1 CI and release workflows |
| `AGENTS.md`, `CLAUDE.md` | The v1 agent instructions |

To move a v1 project to 2.0, run `veld migrate` (see [`CHANGELOG.md`](../../CHANGELOG.md)).
