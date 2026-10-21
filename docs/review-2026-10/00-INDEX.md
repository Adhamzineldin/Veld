# Veld architecture review and delivery plan (2026-10-21)

Reviewed revision: `6a47f815f37ea4c286a6e765312df76743fa7133` on `master`, working tree clean except the review brief. Planning baseline 2026-10-21; readiness target 2027-05-31; submission and defense June 2027. This directory is new and does not modify any historical plan under `docs/`.

## Report (read in order)

| # | File | Content |
|---|---|---|
| 01 | `01-executive-recommendation.md` | product direction, retain/refactor/rewrite decision, stack, scope, risks |
| 02 | `02-methodology-and-evidence.md` | revision, environment, commands, audit tracks, evidence labels, limitations |
| 03 | `03-product-assessment.md` | problem, users, journeys, alternatives, DSL justification, thesis, deliverables, non-goals, research questions, evaluation plan |
| 04 | `04-current-architecture-and-capability-inventory.md` | verified capability inventory with coverage and verification method |
| 05 | `05-defect-register-and-documentation-discrepancies.md` | scales, P0 and P1 tables, documentation discrepancies, what to preserve |
| 06 | `06-rewrite-and-language-decision-matrices.md` | weighted strategy matrix, per-component language decisions |
| 07 | `07-target-architecture-and-repository-layout.md` | boundaries, AST/IR, directory tree, interfaces, deployables, ADR index |
| 08 | `08-compiler-language-cli-plan.md` | pipeline, IR shape, language 1.0 scope, safe generation, CLI, work breakdown |
| 09 | `09-emitters-and-generated-code-quality.md` | emitter inventory, tiers, zero-dependency definition, testing beyond snapshots |
| 10 | `10-external-emitter-system.md` | plugin protocol (proposed), manifest, exchange, SDK, hello emitter journey |
| 11 | `11-language-server-and-editors.md` | LSP state and plan, VS Code primary, JetBrains limited, metadata pipeline |
| 12 | `12-packages-installation-distribution.md` | installing Veld, ecosystem packages (manifest, lockfile, resolution), SDK distribution |
| 13 | `13-registry-and-self-hosting.md` | registry findings, scope, API and storage design, threat model, self-hosting, backup/restore |
| 14 | `14-security-testing-releases-governance.md` | security posture, CI layers, releases, OSS governance, release criteria |
| 15 | `15-roadmap-to-june-2027.md` | capacity, budget check, milestones, first eight weeks, critical path, fallbacks, gates |
| 16 | `16-team-allocation.md` | skills survey, ownership matrix, per-member plans, pairing, sprint table |
| 17 | `17-diagrams-evaluation-report-and-demo.md` | diagram index, evaluation execution, thesis outline, demo and fallback |
| 18 | `18-backlog-acceptance-decisions-first-actions.md` | backlog summary, release acceptance, unresolved decisions, first ten actions |
| 19 | `19-expected-state-at-submission.md` | start state versus expected end state at 1.0.0 per dimension (features, support, performance targets, security, operations, docs), with verification methods and stated limitations |
| 20 | `20-extended-scope-plan.md` | the 2026-10-21 decision to include every deferred item as an ordered queue (constraints, schema generator, auth, unions, streaming, WebSocket, Docker/CI, OpenAPI import, signing, Wasm plugins, hosted plugins, tier-2 revival, HA) with AI-assisted capacity, per-member load and the automatic cut |
| rules | `../../AGENTS.md` (repository root) | quality and process rules for every contributor and AI agent; protects the end state; lists the stale statements in `CLAUDE.md` |
| all | `VELD-REVIEW-COMBINED.md` | every chapter, ADR, diagram, CSV table, appendix and `AGENTS.md` in one file (for conversion to Word or PDF) |

## Supporting artifacts

| Path | Content |
|---|---|
| `adr/ADR-001` to `ADR-010` | architecture decision records (all PROPOSED, for acceptance in Sprint 0) |
| `diagrams/D1` to `D8` (`.mmd`) | editable Mermaid sources; each header states purpose, owner, update trigger, and current-versus-proposed labelling |
| `csv/defect-register.csv` | 153 findings with all required columns (ID, category, evidence, severities, priority, file:line, reproducer, expected/actual, impact, root cause, recommendation, regression check, effort, owner, reviewer, dependencies, lead verification, source track) |
| `csv/backlog.csv` | 58 backlog items with stable IDs, acceptance criteria, effort, owner, reviewer, dependencies, milestone |
| `csv/sprint-allocation.csv` | 16 sprints with per-member responsibilities and integration checkpoints |
| `appendix-external-research.md` | external facts with URLs and access dates, marked primary/secondary/unverified |
| `appendix-track-A` to `G` | raw audit-track notes (command logs, case tables, reproduction logs) |

## How findings are labelled

Evidence: reproduced defect, source-supported defect, suspected risk, design limitation, documentation mismatch, unverified. Severity scales (user impact, security, delivery priority P0 to P3) are defined in chapter 05 §5.1 and kept separate.

## Headline numbers

| Item | Value |
|---|---|
| Findings | 153 (88 reproduced, 19 source-supported, 34 documentation mismatches, 8 design limitations, 4 suspected risks) |
| Priority | 25 P0, 43 P1, 36 P2, 49 P3 |
| Security high | 6 (route shadowing bypassing middleware; registry 2FA bypass, storage traversal, unenforced scopes, optional integrity, stored XSS sinks) |
| Decision | selective replacement in Go; no rewrite |
| Capacity to readiness | 1,106 committed technical hours plus 277 reserve; 148 D1 hours (assumptions in chapter 15) |
| Planned technical work | 694 to 1,139 hours |
