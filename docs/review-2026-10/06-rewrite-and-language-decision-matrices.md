# 06. Refactor versus rewrite, and language decisions

## 6.1 Strategies compared

| Criterion (weight) | S1 Stabilize and incrementally refactor | S2 Selectively replace weak subsystems, keep working components | S3 Rewrite core or whole platform |
|---|---|---|---|
| Time to reliable delivery by 2027-05-31 (0.25) | 4: fixes are small and local, but boundaries stay weak | **5**: fixes plus four targeted replacements (output writer, formatter, VS Code client, registry hardening) fit the budget (chapter 15 §15.2) | 1: 44k lines cannot be re-implemented part-time in 32 weeks while also evaluating and writing |
| Preservation of behaviour and fixtures (0.10) | 5 | 4: conformance fixtures are written before replacements and pin behaviour | 2: behaviour must be re-derived |
| Technical risk (0.15) | 3: destructive output path and three language implementations remain structural risks | **4**: risks removed where they are, nothing else disturbed | 1: unproven foundation on the critical path |
| Team learning and contributor accessibility (0.10) | 4 | 4 | 2: new language or new codebase for everyone |
| Maintainability after graduation (0.15) | 2: emitter matrix and duplicated language logic remain | **4**: tiers, IR, single engine | 5 in theory, unknowable in practice |
| Tooling, packaging, performance (0.05) | 4 | 4 | 3 (depends on language) |
| Plugin support (0.10) | 2: hard to add without an IR | **4**: IR and host are the first replacements | 4 |
| Academic contribution (0.10) | 2: "we fixed bugs" | **4**: measurable before/after on safety, determinism, editor parity, plugin protocol | 3: "we rewrote it" with little evaluation time |
| Weighted score | 3.35 | **4.25** | 2.05 |

Scores are judgements by the lead reviewer on a 1 to 5 scale, informed by the audits; they are not measurements. Sensitivity: if the team survey shows strong Rust or TypeScript skills and weak Go, S3's learning score rises to 4 and its delivery score to 2, giving 2.5; S2 still wins. If the team has only one Go-capable member, S2's delivery score drops to 4 and S1's to 3; S2 still wins. The ranking is robust because S3 loses on the two heaviest criteria regardless of skills.

Recommendation: **S2.** Rejected: S1 because it leaves the four systemic causes (chapter 04 §4.5) in place and offers little to evaluate; S3 because it cannot be delivered and would not fix the actual problems, which are in boundaries and tests, not in the implementation language.

What would reverse it: evidence that the parser or validator is unsalvageable (the frontend audit found the opposite), or a team survey showing that fewer than two members can work in Go.

## 6.2 Language decisions per component

| Component | Current | Decision | Language and stack | Reason | Migration cost | Compatibility impact | Confidence |
|---|---|---|---|---|---|---|---|
| Compiler frontend (lexer, parser, validator, loader) | Go, hand-written | retain, refactor | Go 1.26 | fuzz-clean, fast, small grammar; defects are local (chapter 08 §8.1) | 40 to 70 h for diagnostics, spans, TypeRef | none for users; AST JSON (`veld ast`) changes and is declared unstable | high |
| Semantic IR and compiler API | absent | add | Go; JSON Schema for the serialized form | required by plugins, LSP and diff | 40 to 60 h | new `veld ir` command | high |
| Output writer and emission host | Go, `os.WriteFile` and `RemoveAll` in the CLI | replace | Go | destructive today; needed by plugins | 20 to 30 h | behaviour change: unmanaged files preserved; `clean` only removes manifest files | high |
| Built-in emitters, tier 1 | Go string builders | retain, fix, migrate to IR | Go | close to working for Node/TS; Python and Go fixable in days | 80 to 130 h | output changes (interfaces instead of classes, validation default on) are breaking for existing users and announced in the changelog | high |
| Built-in emitters, tier 2 and 3 | Go | freeze or remove per ADR-005 | Go | unverifiable within capacity | 5 to 8 h to remove | targets disappear from the binary; documented | high |
| External emitter protocol and host | absent | add | Go host; JSON over stdio; TypeScript SDK | chapter 10 | 114 to 192 h | new | medium (new work, scope controlled) |
| Formatter | Go, token-stream | rewrite (small) | Go | corrupts files | 12 to 20 h | `fmt --write` disabled until then | high |
| Diff and lock | Go | refactor onto IR | Go | classifier shallow; gate dead | 10 to 18 h | lock file format changes (versioned) | high |
| Language server | Go, in the CLI binary | retain, extend on the compiler API | Go | the only viable single engine | 60 to 100 h | protocol fixes change client behaviour positively | high |
| VS Code extension | TypeScript re-implementation (1,676 lines) | replace with thin client | TypeScript, `vscode-languageclient` | duplication is the problem | 20 to 40 h | commands finally work | high |
| JetBrains plugin | Kotlin PSI (4,090 lines) | reduce to highlighting plus LSP4IJ, post-Beta | Kotlin | third implementation | 20 to 40 h | limited support at graduation | medium (depends on reserve) |
| Registry client, tarball, credentials | Go (495 lines) | retain, harden, extend (manifest, lockfile, resolver) | Go | small, salvageable | 50 to 80 h | lockfile added; checksum required; `https` default | high |
| Registry server | Go in the CLI binary, no tests | retain architecture, harden, split binary | Go, PostgreSQL, Docker | architecture fine; implementation unsafe | 80 to 140 h | separate deployable; API additive | medium (largest unknown) |
| Registry UI | hand-written HTML/JS, XSS sinks | rewrite rendering layer or reduce to read-only | HTML/JS with escaping, CSP; no framework needed | small surface | 8 to 14 h | none | high |
| Website | Vite, React 19, TypeScript | retain; add CI; LiveDemo either compiled from the Go engine via WebAssembly (stretch) or removed | TypeScript | works; duplicates the generator | 2 to 4 h (CI) | none | high |
| Distribution wrappers | npm, pip, brew, go, nuget, chocolatey, apt, composer | keep npm, brew, go, pip; move the rest to experimental | JavaScript, Python, Ruby formula | chapter 12 | 20 to 30 h | fewer channels | high |
| CI and release | GitHub Actions, hand-written release | retain Actions; adopt GoReleaser | YAML, GoReleaser | signing, SBOM, provenance for free | 12 to 20 h | none | high |

## 6.3 If a rewrite had been recommended

It is not. For completeness, the conditions a rewrite would have needed: a precise failure list that the current implementation cannot fix in place (none found), preservation of the conformance fixtures as the specification (these must be written first in any strategy), a compatibility layer (`veld ast` output and config format), side-by-side comparison on every fixture, a viability milestone before 2026-11-29, abort criteria (fixtures not passing by that date), and a cutoff date after which the old implementation is removed (not before the Beta gate). These conditions are recorded so that any future proposal to rewrite can be judged against them.
