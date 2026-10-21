# ADR-005: Emitter support tiers; an emitter is "supported" only if CI compiles and round-trips its output

- Status: PROPOSED (accept by 2026-11-01; apply in M2)
- Date: 2026-10-21
- Deciders: T4 (owner), T2; reviewer: T1
- Evidence (multi-emitter and Node audit tracks; the lead reviewer reproduced the Python and Go failures on the unmodified `veld init` scaffold):
  - Python default output: `SyntaxError` at `routes/auth_routes.py:13` (`LoginInputSchema(**body = data  # ...`); routes are never registered; the imported `schemas` module is never written (F-MULTI-03).
  - Go output: `undefined: mux` in every route file (F-MULTI-06, F-CLI-19).
  - Rust output has no router, no `axum` dependency, handler/trait type mismatches; it cannot compile (F-MULTI-07).
  - Java, C#, PHP emit framework-less classes with syntax errors (`Role role = "user"`); they neither compile nor route (F-MULTI-08).
  - Header names such as `Idempotency-Key` become identifiers in every language (F-MULTI-01, root cause `internal/parser/parser.go:961`).
  - Angular output is invalid TypeScript (F-NODE-15); Dart, Kotlin and Swift clients have compile or runtime defects (F-MULTI-09).
  - All emitter tests assert substrings; `go test ./...` is green while most outputs do not compile (F-MULTI-02, F-NODE-19).
  - Maintenance estimate for a full compile matrix of 8 backends and 10 frontends: 75 to 110 hours per month (audit estimate; assumptions: one maintainer per emitter, one breaking upstream change per quarter), which exceeds the team's entire committed capacity of roughly 35 hours per week.

## Decision

Three tiers, enforced by CI and shown by `veld doctor` and in the README support matrix.

| Tier | Meaning | CI requirement | Graduation members |
|---|---|---|---|
| 1 Supported | Output compiles and passes request/response round-trip tests for every conformance fixture on every push; documented; breaking changes announced | compile + round-trip on ubuntu, windows, macos (fast job) | Backends: `node-ts` (plain and Express adapter), `python` (Flask adapter and plain), `go` (net/http plain and chi adapter). Frontends: `typescript`, `react`. Service SDKs: TypeScript, Python, Go. Tools: `openapi`. |
| 2 Experimental | Output compiles for every fixture in a scheduled CI job; no round-trip guarantee; emitting prints a one-line warning; may ship types and interfaces only | compile-only in the nightly matrix with the needed toolchain | Frontends: `vue`, `svelte`, `javascript`, `typesonly`. Backends: `java`, `csharp`, `php`, `javascript`. Tools: `docs`, `graphql`, `schema` (SQL/Prisma) after the quoting fixes. |
| 3 Frozen / removed | Not registered in the release binary; code deleted from `main` (kept in history and tagged `frozen/<name>`) or kept behind `-tags experimental` only if a team member owns it | none | `rust` backend, `angular` frontend (until rewritten on the TS SDK), `dart`, `kotlin`, `swift` clients, `mock` (dead code), `dockerfile`, `cicd`, `envconfig` generators (download a 404 asset, bundle a default-credential Postgres), Rust/Java/C#/PHP service SDKs. |

Promotion rule: an emitter moves up a tier only when its CI requirement has been green for two consecutive sprints and it has an owner and a backup in chapter 16.

Demotion rule: a tier-1 emitter whose CI is red for more than one sprint is demoted to tier 2 for the release and the README is updated in the same pull request.

## Reasons

- The thesis is "correct server interfaces, routes, validation and clients from one contract". Shipping eighteen targets of which two work undermines it more than shipping six that are verified.
- The team cannot afford more than tier-1 round-trip plus tier-2 compile checks (chapter 09 cost table).
- Deleting frozen code reduces the surface that every cross-cutting refactor (output writer, IR migration) must touch.

## Alternatives rejected

- Keep everything and label it "beta": the labels already exist in docs and were not true; users cannot tell which targets work.
- Fix everything: 150 to 250 hours for Rust, Java, C#, PHP, Dart, Kotlin, Swift alone, with no round-trip tests, and ongoing cost that no one owns.

## Amendment 2026-10-21 (ADR-012)

The project owner's extended-scope decision moves the tier-3 items back into scope as queue items with the same promotion rule: `rust`, `angular`, `dart`, `kotlin`, `swift` and `mock` are revived as tier 2 (Q13, backlog B-071, B-072); `dockerfile`, `cicd` and `envconfig` return as tier-2 tools after being fixed and tested (Q8, B-066); WebSocket and streaming become tier-1 extended features for the three tier-1 backends (Q5 to Q7) with declared dependency exceptions. Tier 3 is therefore empty at 1.0.0 unless the automatic cut of chapter 20 §20.6 removes an item, in which case it is frozen as originally described. The tier bar and the promotion and demotion rules are unchanged.

## Consequences

- README, CLAUDE.md, website and `test.md` support claims must be rewritten (chapter 05, D-items).
- The academic report presents the matrix honestly: supported, experimental, frozen, with CI evidence for each.
- Rust and Angular are the first candidates for community contribution through the plugin protocol; Rust's current code is a useful starting point for a contributor but not for the team before the Beta gate.
