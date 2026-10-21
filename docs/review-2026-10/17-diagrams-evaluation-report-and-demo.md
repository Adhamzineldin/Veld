# 17. Diagrams, academic evaluation, report outline and demonstration plan

## 17.1 Diagrams (Mermaid sources in `diagrams/`)

| File | Purpose | Shows current vs proposed | Owner (content / drawing) | Update trigger |
|---|---|---|---|---|
| `D1-system-context.mmd` | deployables and who talks to whom | solid = current, dashed = proposed (plugin process, `veld-registry`) | T1 / D1 | any ADR that adds or removes a deployable |
| `D2-compiler-pipeline.mmd` | pipeline stages and the allowed dependency direction | proposed, with current package names in brackets | T1 / D1 | changes to `internal/compiler`, `internal/ir`, layout |
| `D3-plugin-trust.mmd` | host and plugin exchange, what the host enforces and does not | proposed | T2 / D1 | VEP schema or host-rule change |
| `D4-registry-publish-consume.mmd` | publish and pull sequence, where integrity is verified, where atomicity is added | current with proposed steps marked [P] | T3 / D1 | changes to `cmd/veld/registry.go`, `internal/registry`, `handlers/packages.go` |
| `D5-registry-er.mmd` | tables and relations | current schema plus proposed `audit_events` | T3 / D1 | any migration |
| `D6-editor-lsp.mmd` | editor, server, compiler API interaction incl. unsaved buffers | current transport, proposed engine | T5 / D1 | changes to `internal/lsp` or editor clients |
| `D7-selfhost-backup.mmd` | compose topology and backup/restore flow | proposed | T3 / D1 | changes to `deploy/` or the runbook |
| `D8-roadmap-critical-path.mmd` | milestones, gates, critical path | proposed | T1 / D1 | every sprint review |

Redundant diagram types (class diagrams per package, per-command flowcharts) are deliberately not produced; D1 adds a diagram only when it explains a decision in the report.

## 17.2 Evaluation (planned; no results exist)

The plan in chapter 03 §3.9 is the authoritative list. Execution responsibilities: T4 runs Q1 to Q3 and Q5 with scripts under `evaluation/`; T2 runs Q4 with the SDK users; D1 coordinates the user study, consent and scheduling; T4 runs the technical part of each session. Results are recorded with collection date, commit hash, machine description and raw data files, and are reported in the thesis with their limitations (small N, student participants, single environment).

Baselines must be fixed before the Alpha gate: the OpenAPI corpus list (Q1), the hand-written Express + fetch reference app (Q2), the Zod and Pydantic validators (Q3), the three implementations at 6a47f81 (Q5).

## 17.3 Graduation report outline (mapped to evidence and contributors)

| Chapter | Content | Evidence sources | Contributors |
|---|---|---|---|
| 1 Introduction and problem definition | contract drift problem, thesis statement, research questions (ch. 03 §3.5, §3.8), contributions | this review ch. 03 | D1 writes; T1 validates |
| 2 Background and related work | OpenAPI, TypeSpec, Smithy, protobuf/buf, TS-only tools, LSP, plugin architectures | appendix-external-research (re-verified with primary pages) | D1; T1, T2 validate |
| 3 Requirements | functional and quality requirements, traceability to ADRs and backlog | ch. 03 §3.6, ch. 18 | D1; all owners |
| 4 Architecture | boundaries, IR, plugin protocol, LSP engine, registry; ADRs | ch. 07 to 13, diagrams | T1, T2, T3, T5 write sections; D1 edits |
| 5 Implementation | compiler changes, emitters, host and SDK, server, VS Code | pull requests, CHANGELOG | each owner |
| 6 Testing and quality assurance | conformance suite, compile and round-trip matrix, fuzzing, registry tests, CI | CI run links, coverage | T4, T5 |
| 7 Evaluation | Q1 to Q5 results, user study | `evaluation/` data | T4; D1 presents |
| 8 Security | threat models, fixed findings with before/after evidence, ASVS checklist | ch. 05 register, ch. 13, ch. 14 | T3; T1 reviews |
| 9 Limitations and future work | tiers not reached, Wasm plugins, OpenAPI import, registry-hosted plugins, JetBrains | ch. 03 §3.7, ch. 10 §10.5 | D1; T1 validates |
| 10 Project management | plan versus actual, capacity model, gates, risks that materialized | ch. 15, ch. 16, sprint reviews | D1; T1 validates |
| 11 Conclusion | | | D1 |
| Appendices | ADRs, defect register before/after, runbook, user-study protocol, generated-code samples | this review | D1 |

Every chapter draft is due per the D1 schedule in chapter 16 §16.4; each technical statement cites a test, CI run, finding ID or command output.

## 17.4 Live demonstration scenario (defensible) and offline fallback

Live (12 minutes, from a clean VM image prepared in M7, no network needed except the local registry):

1. `npx veld init` in an empty folder; open in VS Code; show the extension starting `veld lsp`; introduce a typo in a type name; show the diagnostic with the exact range and the cross-file definition jump.
2. Add a `POST /users` action with a header block; `veld generate`; show the manifest and that a hand-written file in the output directory survives regeneration.
3. Run the generated Express server with a 20-line service implementation; call it with the generated TypeScript client; show 201, a 400 with validation details, and a 404 for a path with special characters encoded correctly.
4. Switch the backend to Python in the config; regenerate; run Flask; call it with the same TypeScript client (cross-language round trip).
5. Remove a field; `veld generate --strict` fails with the breaking change named; `--force` proceeds and the lockfile updates.
6. Install the hello emitter from npm; add it to the config; `veld doctor` shows it; generate; show `API.md`; show a plugin returning `../escape.txt` being refused.
7. `veld push` the contract to the local self-hosted registry (compose, already running); `veld pull` it into a second project; show `veld.lock.json`; tamper with the archive on disk through the local proxy and show the integrity failure.

Offline fallback: a recorded video of the same script, plus a pre-generated folder for each step so that any step can be shown from static files if the VM fails. The recording is produced at the RC gate and refreshed at the readiness gate.

Questions the demo is designed to pre-empt: "is this just OpenAPI with extra steps?" (step 3 to 5: server interfaces, validation, breaking-change gate), "can anyone extend it?" (step 6), "is the registry safe to self-host?" (step 7 and the security chapter).
