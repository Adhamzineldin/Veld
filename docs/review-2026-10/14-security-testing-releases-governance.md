# 14. Security, testing, releases, open-source governance and ongoing maintenance

## 14.1 Security posture (summary of chapter 05 and 13)

Six findings carry security severity high at 6a47f81: route shadowing that bypasses middleware in generated Node routes (F-NODE-05), registry 2FA bypass (F-REG-01), storage path traversal on publish (F-REG-02), unenforced token scopes (F-REG-03), optional and self-referential download integrity (F-REG-09) and stored XSS sinks in the registry UI (F-REG-11). Twelve more are medium. None of them requires a redesign; all are fixed by the M1 to M3 items in chapters 08, 09 and 13.

Security work that is in scope for graduation:

- Threat models written down for the generator (hostile contract, hostile plugin, hostile package) and the registry (chapter 13 table), each with the controls that exist and tests that prove them.
- A `SECURITY.md` with a private reporting address, a 90-day disclosure window, and the supported versions table.
- `govulncheck` in CI; `npm audit --production` for the extension and SDK (the VS Code extension currently has 3 dev-only vulnerabilities, 2 high).
- Security self-review at the RC gate using the OWASP ASVS level 1 checklist for the registry; D1 records results, T3 and T1 sign.
- No telemetry of any kind in the CLI, server or extension (decision; documented in the privacy section of the README).

Out of scope, stated as future work: plugin sandboxing (chapter 10), package signing beyond checksums, registry-hosted plugins.

## 14.2 Testing strategy (fast versus scheduled)

| Layer | Runs on | Content | Owner |
|---|---|---|---|
| Fast CI (every push, under 10 minutes) | ubuntu, windows, macos | `go build`, `go vet`, `go test -race` (unit + conformance suite), language-metadata drift (`git status --porcelain`), import-boundary test, determinism test on fixtures, tier-1 generated-code compile (`tsc`, `compileall`, `go build`), VS Code extension compile, website build, `govulncheck` | T5 |
| Round-trip CI (every push to main and PRs touching emitters) | ubuntu, windows | tier-1 server + client harness, cross-language matrix (Node/Python/Go servers against the three clients), error cases | T4 |
| Registry CI (every push touching `internal/registry`, `internal/pkg`, `cmd/veld-registry`) | ubuntu with PostgreSQL 16 service | handler tests, authz matrix, publish race, limits, malicious archives, e2e publish/pull with lockfile | T3 |
| Scheduled nightly | ubuntu | tier-2 compile matrix (javac, dotnet, php -l, vue/svelte tsc), JetBrains Gradle build, parser and archive fuzzing (`go test -fuzz` 10 minutes each), installer smoke tests (npm, brew, release archive on a clean container), plugin protocol compatibility (hello emitter against the current host) | T4 / T5 |
| Release pipeline (tags) | ubuntu | GoReleaser build, checksums, cosign signatures, SBOM, GitHub attestation, wrapper publication, Docker image | T5 |

Parser fuzzing: the audit's 1.67 million executions found no panic; the fuzz targets written during the audit (`fe-audit/fz/`) should be moved into the repository as `FuzzLex`, `FuzzParse`, `FuzzValidate`, `FuzzDiff`, `FuzzUnpack`. Property tests: `fmt` idempotence on every fixture; `diff(a, a)` empty; `Unpack(Pack(dir))` equals `dir` for allowed files.

Generated-code conformance: the harness in chapter 09 §9.5 is the acceptance mechanism for every tier claim. A README support cell is generated from the latest CI run, never hand-edited.

Documentation examples that compile: every code block in `docs/guides` tagged `veld-example` is extracted by a script and run through `veld validate`; the tutorial project under `examples/node-typescript` is generated and compiled in fast CI (it is today the only example whose output passes `tsc --strict`).

Architecture drift detection: the import-boundary test (chapter 07 §7.1); a test that the registered emitter list equals the README matrix; the language-metadata drift job fixed to catch new files.

## 14.3 Releases

- GoReleaser configuration in the repository; tags `vX.Y.Z` (CLI), `registry/vX.Y.Z`, `sdk/vX.Y.Z`, `vscode/vX.Y.Z`.
- Each release: archives for linux/darwin/windows on amd64 and arm64, `checksums.txt`, cosign signatures (keyless), SBOM (Syft), GitHub artifact attestation (provenance), Docker image for the registry, Homebrew formula pushed to the tap, npm wrapper published with provenance, VS Code extension packaged (`vsce`) and published by a human.
- Changelog in Keep-a-Changelog format updated per sprint by the integration owner; release notes generated from it.
- Version support: the latest minor receives fixes; the previous minor receives security fixes for 3 months after the next minor; language versions follow ADR-009.
- Credentials: npm, Marketplace and tap tokens stored as GitHub environment secrets with required reviewers (two team members) for the `release` environment; no secrets in wrappers.
- Actions pinned by commit SHA (today pinned by tag).

## 14.4 Open-source governance

| Item | Decision | Owner |
|---|---|---|
| License | MIT for the whole repository (README already claims it; no LICENSE file exists). Add `LICENSE` at the root in Sprint 0; verify that `cobra` (Apache-2.0: keep its LICENSE and NOTICE in the distribution archive), `lib/pq` (MIT) and `golang.org/x/crypto` (BSD-3-Clause) are compatible (they are permissive and compatible; confirm with the SPDX pages). Generated output carries no license header and is owned by the user; state this in the README ("output of Veld is yours"). | T5 |
| University IP policy | Unresolved: confirm with the department whether student-authored open-source code requires any assignment or notice, and whether the June submission can reference a public repository. Record the answer as ADR-011. | T1 with D1 |
| Contribution terms | Developer Certificate of Origin (sign-off), no CLA. | T5 |
| Files | `CONTRIBUTING.md` (build, test, fixture-writing, emitter tiers, how to add an external emitter), `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1), `SECURITY.md`, `CODEOWNERS` (chapter 16 matrix), issue templates (bug with contract and `veld --version --json`; feature; emitter proposal), PR template (tests, docs, changelog, tier impact), `GOVERNANCE.md` (two-maintainer review for `internal/`, one for docs). | D1 drafts, T5 validates |
| RFC/ADR process | ADRs in `docs/adr/` with the template used in this review; an RFC is an ADR in `proposed` state open for one sprint of comments. | D1 |
| Roadmap | `ROADMAP.md` regenerated from chapter 15 at each gate. | D1 |
| Telemetry and privacy | none; documented. | T5 |
| Accessibility | website and registry UI: keyboard navigation, contrast, alt text, checked with a browser accessibility audit before the Beta gate; docs in plain language. | D1 with T5 |
| First external contributor | target: one external contribution (a tier-2 fix or a plugin) merged before the readiness gate, as evidence for the "contributor accessibility" claim; `good first issue` labels on at least ten backlog items by M4. | T5 |
| Bug triage | weekly 20-minute triage in the sprint meeting; labels `P0..P3` as in chapter 05. | integration owner |

Maintenance after graduation: the team commits (in `GOVERNANCE.md`) to security fixes for six months after the release and to responding to issues within two weeks; anything beyond that is best-effort. The tier model keeps the maintained surface small enough for one or two volunteers.

## 14.5 Release criteria (measurable) and evidence

| Criterion | Measure | Evidence |
|---|---|---|
| Tier-1 correctness | round-trip harness green on three OSes for two consecutive sprints | CI run links in the report |
| Safety | output-safety tests (unmanaged file, `--out` at project root, failing compile keeps output, interrupted write) green | CI |
| Determinism | byte-identical generation across runs and OSes for all fixtures | CI job output |
| Plugin system | hello emitter installed from npm into a clean project generates on Windows and Linux; escape, timeout and capability tests pass | CI and a recorded demo |
| Editor | VS Code integration test green; multi-file diagnostics in the scripted session | CI |
| Registry | authz matrix, race, limits and malicious-archive tests green; restore drill recorded with timing | CI and runbook appendix |
| Security | zero open P0/P1 with security severity; `govulncheck` clean; ASVS L1 checklist completed | checklist in `docs/security/` |
| Docs | every support claim generated from CI; tutorial compiles; discrepancy list from chapter 05 closed | docs CI |
| Release | signed artifacts with checksums, SBOM and attestation; wrappers install on clean machines | release page and smoke-test logs |
| Governance | LICENSE, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, CODEOWNERS present; at least one external contribution merged | repository |

## 14.6 Priority order for the controls (so the checklist does not consume the project)

1. Sprint 0: LICENSE, CODEOWNERS, CHANGELOG, `go 1.26`, hygiene, CI on three OSes.
2. M1: tier-1 compile jobs, output-safety tests, malicious-archive tests, `govulncheck`.
3. M2: round-trip harness, registry CI with PostgreSQL, determinism job, import-boundary test.
4. M3: SECURITY.md, CONTRIBUTING.md, issue and PR templates (documentation work fits the exam period).
5. M4: GoReleaser with signatures, SBOM and attestations; installer smoke tests; nightly tier-2 and fuzz jobs.
6. M5: accessibility audit, ASVS checklist, runbook drill.
7. M6: release rehearsal, first external contributor outreach.
