# ADR-012: Extended scope for 1.0.0 under an AI-assisted capacity assumption, with an ordered queue and an automatic cut date

- Status: ACCEPTED by the project owner on 2026-10-21 (team members to confirm in Sprint 0)
- Date: 2026-10-21
- Deciders: project owner; T1 (plan), T2, T3, T4, T5 (owners of the added items); D1 (traceability)
- Supersedes: the "deferred" lists in chapter 03 §3.7, chapter 10 §10.5, chapter 19 §19.11, and the tier-3 removals in ADR-005 for the items named below. Chapter 20 is the authoritative extended plan.

## Context

The baseline plan (chapters 15 to 19) delivered a reduced scope that fit 1,106 committed hours. The project owner decided to include every deferred item: per-field constraints, discriminated unions, authentication schemes in the contract, WebSocket actions, streaming (SSE, chunked download, multipart upload), OpenAPI import, a proper database schema generator, Dockerfile/CI/env generators, a WebAssembly plugin transport (sandboxing), registry-hosted plugins, package signing, high-availability hosting, and the revival of the frozen emitters (Rust, Angular, Dart, Kotlin, Swift) and the mock server. The stated justification is that AI-assisted development will make the team faster, provided code stays lean, follows SOLID principles and is fully tested.

## Decision

1. All items above are in scope for 1.0.0 as an **ordered extended queue** (chapter 20 §20.3) that starts only after the core is in place (M1 to M3: output safety, compile and round-trip tests, IR, diagnostics, plugin protocol freeze, registry hardening).
2. The core end state of chapter 19 remains the graduation deliverable and is never traded for a queue item. Queue items are implemented strictly in order; an item is "in" only when it meets the same bar as tier 1 (compile, round trip, determinism, docs generated from CI).
3. **Automatic cut:** at the extended-feature freeze on 2027-05-02, every queue item not accepted is dropped from 1.0.0 without discussion and documented as future work. The core feature freeze stays at 2027-04-18.
4. Capacity is planned with an AI-assisted multiplier of 1.5 on implementation and test writing (assumed 60 percent of effort) and 1.0 on design, review, evaluation, operations drills and documentation validation, giving a blended 1.3. This is an assumption to be measured at the M1 exit (actual hours against estimates) and corrected.
5. The IR v1 schema reserves the fields the queue needs (`constraints`, `union`, `auth`, `transport`, `stream`, `entity`) from the start so that adding them later does not bump the IR major version.
6. Code quality is a gate, not a goal: `AGENTS.md` at the repository root defines the rules every contributor and AI agent must follow; CI enforces the checkable ones.

## Consequences

- Planned technical work rises from 694 to 1,139 hours to 1,251 to 2,169 hours (chapter 20 §20.2). Under the 1.3 multiplier the midpoint consumes almost the entire reserve; the high estimate is not deliverable. The automatic cut is what keeps the graduation deliverable safe.
- Zero-runtime-dependency exceptions are introduced and declared: WebSocket adapters import a WebSocket library in Node, Python and Go; everything else keeps the rule.
- The hardening window for queue items is two weeks (May 2 to May 16) instead of four; defects in queue items found after May 16 are fixed only if P0.
- The thesis gains research questions on AI-assisted development productivity (measured, not assumed) and on plugin sandboxing.

## What would reverse this decision

Measured productivity at the M1 exit (2026-11-29) below 1.15 times the baseline estimates, or any core gate missed by more than one sprint. In either case the queue is truncated at the point chapter 20 §20.6 specifies, without moving the readiness date.
