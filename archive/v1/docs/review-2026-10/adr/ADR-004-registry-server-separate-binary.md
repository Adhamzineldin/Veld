# ADR-004: Move the registry server into its own binary (`veld-registry`) in the same repository

- Status: PROPOSED (accept by 2026-11-01; implement in M4)
- Date: 2026-10-21
- Deciders: T3 (owner), T5 (distribution); reviewer: T1
- Evidence: `cmd/veld/serve.go` wires `internal/server` into the CLI; `go.mod` links `github.com/lib/pq` into every CLI build; the embedded registry SPA (`internal/server/handlers/web.go`, `//go:embed`) ships inside the developer CLI; the tracked `veld` binary at the repository root is 20,450,930 bytes (`ls -la`, 2026-10-21). CLAUDE.md states "No separate registry binary exists" as a rule; this ADR changes that rule.

## Decision

Create `cmd/veld-registry` and move `veld serve` there. The CLI keeps `veld login/logout/push/pull` (client side). The registry is distributed as a Docker image (default self-hosting path, chapter 13) and as a standalone binary on GitHub Releases. The npm/pip/Homebrew wrappers ship only the CLI.

## Reasons

1. The developer CLI should not carry a PostgreSQL driver, SMTP code, TOTP, session handling and a SPA that no CLI user needs. Binary size, attack surface and dependency licensing all shrink.
2. Operators upgrade the registry on a different cadence from developers upgrading the CLI; one binary forces them to move together.
3. Separate binaries let CI build and test the server with a PostgreSQL service container without slowing every CLI job.

## Alternatives rejected

- Build tags (`-tags registry`) in one `cmd`: fragile, confuses wrappers that download "the" binary.
- A separate repository: adds release coordination for a six-person team; the registry shares `internal/registry` tarball and manifest code with the client. Monorepo retained (ADR-010).

## Consequences

- `CLAUDE.md` and docs must be updated (documentation currently asserts a single binary).
- The `packages/*` wrappers do not change.
- Migration cost: 6 to 10 hours (move command, update docs, add release target), excluding the hardening work in chapter 13.
