# T3 registry tracker (Eyad): stages A to H

Owner: T3 (Eyad). Living checklist for the registry, packages and security stream, from M0 to the defense. Tick a box only when its "done when" condition holds and the evidence (file, PR, CI run) is linked next to it.

Sources: `VELD_Team_Roles_Extended_Languages_Frameworks.pdf` (Eyad pages 5-6), `docs/review-2026-10/13-registry-and-self-hosting.md`, `12-packages-installation-distribution.md` §12.2, ADR-004, ADR-008, `csv/backlog.csv`, `AGENTS.md`.

People: Adham (T1), Mohamed (T2), Eyad (T3), Menna (T4), Ahmed (T5), Shahd (D1).

Legend: `[ ]` not started or in progress, `[x]` done (add the evidence link). Dates are deadlines from the review plan.

---

## Stage A: agree what we are building (M0, Oct 21 to Nov 1)

Order: A1 → A2 and A3 (before the Oct 27 workshop) → A7a (action list) → A5 → A6 → A7b (full contract) → A4. A8 and A9 run in parallel.

### A1. Scope of the 1.0 registry (`docs/registry/scope.md`)
- [ ] Read ch.13 §13.1 and §13.2 and the PDF M0 bullets
- [ ] Write the in-scope table: accounts, orgs and roles, scoped tokens, public/private packages, immutable versions + yank/deprecate, prefix search, audit events, web UI, optional email, 2FA (pending U4)
- [ ] Write the out-of-scope list: hosted public registry, mirrors/proxies, federation, Kubernetes, executable plugins (until Q12), full-text search
- [ ] Write the cut order if over budget at the M2 exit: search, audit UI, TOTP
- [ ] Write the responsibility split (accounts, scoped tokens, publication, pull, storage) as one paragraph each
- [ ] Adham approves at the Oct 27 workshop; Shahd records the decision
- Done when: the file is merged and the workshop decision log references it.

### A2. ADR-004: registry as its own binary
- [ ] Read `docs/review-2026-10/adr/ADR-004-registry-server-separate-binary.md`
- [ ] Decide and write down: binary name `veld-registry`; commands in the server (`serve`, `check-config`, `admin`, `backup`, `restore`, `verify`); commands left in the CLI (`login`, `logout`, `push`, `pull`, `install`)
- [ ] Decide code locations: `cmd/veld-registry/`, server `internal/registry/`, shared client and archive code `internal/pkg/`
- [ ] Agree distribution with Ahmed: Docker image + release archive, tag `registry/vX.Y.Z`, not in npm/pip/brew wrappers
- [ ] Post comments or amendments before Oct 27; ADR accepted by Nov 1; Shahd publishes it in `docs/adr/`
- Done when: ADR-004 status is ACCEPTED in `docs/adr/`.

### A3. ADR-008: package manifest and lockfile
- [ ] Read ADR-008 and ch.12 §12.2
- [ ] Decide the integrity string format (`sha256-<hex>` or SRI `sha256-<base64>`) and write it in the ADR
- [ ] Fix the manifest fields: `manifestVersion`, `name` (`@org/name`), `version`, `description`, `entry`, `dependencies`, `license`; name regex; strict semver
- [ ] Fix the packed-file allow-list: `.veld` files, `veld.package.json`, `README.md`, `LICENSE`; nothing else
- [ ] Fix the lockfile fields: `lockfileVersion`, per package `name`, `version`, `resolved`, `integrity`, `dependencies`
- [ ] Fix the range syntax (exact `1.2.0`, caret `^1.2.0`, x-range `1.x`) and the resolution rule (highest satisfying, one version per package, conflicts are errors naming both sides)
- [ ] Fix the scope binding key in `veld.config.json` (`"registries": {"@acme": "https://..."}`) and "no fallback"
- [ ] Confirm "data-only packages" and the cache path
- [ ] Review with Adham; Mohamed reviews; accepted by Nov 1
- Done when: ADR-008 status is ACCEPTED in `docs/adr/` with no open questions.

### A4. Threat model (`docs/registry/threat-model.md`)
- [ ] List assets: package bytes, package metadata, accounts, sessions, tokens, private package names, operator secrets, backups
- [ ] List attackers: anonymous user, registered user/malicious author, holder of a stolen token, org admin acting badly, malicious registry or network attacker, local attacker on a developer machine
- [ ] Draw the trust boundaries: CLI ↔ network ↔ server ↔ PostgreSQL/storage; archive contents are untrusted on both sides
- [ ] Go through STRIDE per boundary and fill the table: threat, asset, control, test that proves the control
- [ ] Map every review finding F-REG-01..23 to a row (each must have a control and a test)
- [ ] Adham reviews
- Done when: every row has a control and a named test; no F-REG ID is unmapped.

### A5. Authorization matrix (`docs/registry/authz-matrix.md`)
- [ ] Rows (actors): anonymous; logged-in non-member; member; admin; owner; token with `read`, `write`, `delete`, `admin`; org-bound token used on another org; expired token; 2FA-pending session
- [ ] Columns: every action from A7a
- [ ] Fill each cell: allow, deny (status code), or 404-hide
- [ ] Write the rules in words under the table (private by default; uniform 404; only sessions mint tokens; admins cannot touch owners; last owner is protected)
- [ ] Adham reviews
- Done when: no empty cell; the table can be pasted into a Go test table.

### A6. Data model (`docs/registry/data-model.md`)
- [ ] Tables and columns with types and constraints: `users`, `sessions`, `orgs`, `org_members`, `packages`, `package_versions` (sha256, size, storage key, yanked_at, deprecated, dependencies jsonb), `tokens`, `audit_events`, `email_verification_tokens`, `password_reset_tokens`, `schema_migrations`
- [ ] Mark every UNIQUE/foreign key/CHECK and why it exists (immutability, last owner, uniqueness)
- [ ] Mermaid ER diagram; hand the content to Shahd for diagram D5
- [ ] Draft `0001_init.sql` (not merged until Stage C)
- Done when: Shahd has redrawn D5 from it and Adham has reviewed the constraints.

### A7. API contract (`docs/registry/api-v1.md`)
- [ ] A7a: list of resources and actions only (feeds A5)
- [ ] A7b: per endpoint: method, path, auth, required scope/role, request body, response body, status codes
- [ ] One error shape for every endpoint
- [ ] Limits per endpoint (body sizes, rate limits)
- [ ] Hand to Ahmed (CLI consumer) and agree before Stage E
- Done when: Ahmed confirms the CLI can be built from it.

### A8. Local development setup (`deploy/dev/` + `docs/registry/dev-setup.md`)
- [ ] Install Docker Desktop; run `postgres:16` with a named volume
- [ ] Write `deploy/dev/docker-compose.yml` (PostgreSQL only for now) and an example env file with no real secrets
- [ ] Decide the local storage folder (git-ignored)
- [ ] Write the steps to start, reset and inspect the dev database
- [ ] Another teammate follows the steps on a clean machine
- Done when: a teammate starts the dev database from the doc alone.

### A9. Acceptance tests written first (behind the `registryaccept` build tag)
- [ ] Malicious archive table: `..` traversal, absolute path, drive letter, NTFS alternate data stream, backslash names, symlink, hardlink, device, 1 GiB bomb, 100,000 entries, truncated gzip, partial extraction
- [ ] Integrity cases: missing checksum, wrong checksum, lockfile hash vs server hash, `http://` URL
- [ ] Authorization cases generated from A5 (as a data table until handlers exist)
- [ ] Run command written at the top of the file, like `cmd/veld/output_safety_test.go`
- Done when: the tests compile behind the tag, fail for the right reason, and the default `go test ./...` stays green.

### Team items in Stage A (not owned by Eyad; track because they block A)
- [ ] Shahd: skills survey answered (Eyad) by Oct 30
- [ ] Shahd: ADR set published in `docs/adr/`
- [ ] Ahmed: CODEOWNERS lists Eyad for registry paths
- [ ] Oct 30: M0 exit review (ADRs accepted, backlog imported)

---

## Stage B: safe building blocks (M1, Nov 2 to 29)

- [ ] B10. `internal/pkg` archive library: deterministic Pack, hardened Unpack, `FuzzUnpack` (B-019; Adham reviews)
- [ ] B11. Client trust rules: checksum always required, https default, explicit registry selection, per-URL credentials (B-018; Adham reviews)
- [ ] B12. Identity: password hashing, revocable sessions, password reset, login rate limits (B-020/B-032; Adham reviews)
- [ ] B13. API tokens: hashed, scoped, expiring, org-bound, minted only by sessions (B-020)
- [ ] B14. TOTP with replay protection, or disabled (decision U4 by Jan 8)
- [ ] B15. Single `authorize()` helper + authorization matrix tests (B-020; Adham reviews)
- [ ] B16. UI safety rules: escaping, CSP, security headers, Secure cookie, CORS allow-list (B-022; Ahmed reviews)
- [ ] B17. Registry CI job with PostgreSQL (with Ahmed, S2 pairing)
- [ ] S2 integration owner: vertical slice from a clean clone at the Nov 27 review

## Stage C: the server becomes a service (M2, Nov 30 to Jan 10)

- [ ] C18. `cmd/veld-registry`: config precedence, `check-config`, JSON logs, SIGTERM, `/healthz`, `/readyz` (B-031; Ahmed reviews)
- [ ] C19. Versioned embedded migrations
- [ ] C20. Content-addressed storage behind an interface
- [ ] C21. Orgs: roles, last-owner guard, ownership transfer
- [ ] C22. Atomic publish with limits, server-side archive check, idempotency, audit event (B-021; Mohamed reviews)
- [ ] C23. Read endpoints with per-version sha256 and dependencies; visibility helper; uniform 404 (B-032/B-038; Adham reviews)
- [ ] C24. Yank, deprecate, prefix search, audit list
- [ ] C25. Server test suite and registry e2e in CI (B-033; Menna reviews); Jan 8 M2 exit
- [ ] C26. Move into `internal/pkg` and `internal/registry` during the M2 layout change

## Stage D: package contract frozen (M3, Jan 11 to Feb 7)

- [ ] D27. Lockfile v1 and resolver design frozen (with Adham and Ahmed; Menna adds lockfile fixtures)
- [ ] D28. Failure rules: no incomplete or corrupt package accepted on either side
- [ ] D29. Deployment and backup/restore design written
- [ ] S7 integration owner: Feb 5 checkpoint; security review of Mohamed's plugin protocol (B-036, B-037)

## Stage E: packages inside real projects (M4, Feb 8 to Mar 21)

- [ ] E30. `veld push` with manifest validation and the file allow-list
- [ ] E31. Resolver, lockfile write/read, `veld install`, `--frozen` (B-044; Adham reviews, S9 pairing)
- [ ] E32. Cache, offline mode, scope-to-registry binding
- [ ] E33. CLI commands and setup instructions with Ahmed (B-046)
- [ ] E34. Compiler integration with Adham: imports from `veld/packages`; `generate --strict` catches breaking pulled updates
- [ ] E35. Verify scoped tokens, private packages and lockfile-consistent pulls; Mar 5 `install --frozen` checkpoint; Alpha gate

## Stage F: self-hostable (M5, Mar 22 to Apr 18)

- [ ] F36. Dockerfile and Compose with PostgreSQL and Caddy; clean-VM test with Ahmed (B-051; Menna reviews)
- [ ] F37. Operator tools: `admin create-user`, closed registration, quotas
- [ ] F38. `backup`, `restore`, `verify`; timed restore drill on Apr 2 (RTO 1 h, RPO 24 h)
- [ ] F39. Runbook validated by executing every step (Shahd edits)
- [ ] F40. (Q10, optional) package signing with Ahmed; decision U13 by Mar 21 (B-068)
- [ ] S12 integration owner: Apr 16 Beta gate

## Stage G: hardened and released (M6, Apr 19 to May 16)

- [ ] G41. ASVS L1 self-review and govulncheck; Adham co-signs; Shahd records (B-056)
- [ ] G42. Release-candidate fixes; load test (publish 1 MB < 2 s, download < 500 ms, 50 concurrent)
- [ ] G43. Release `registry/vX.Y.Z` with Ahmed
- [ ] G44. (Q12, Q14 optional) hosted plugins with Mohamed, HA with Ahmed; drop unfinished by May 2 (B-070, B-073)
- [ ] G45. Thesis chapter 8 and the registry part of chapter 4

## Stage H: demonstrated (M7 and June)

- [ ] H46. Clean demo registry, sample packages, restore fallback; decision U9 by May 1
- [ ] H47. Defense demo: push, pull, lockfile, tamper, refusal; explain scopes, integrity, recovery

---

## Decisions owned by Eyad

- [ ] U4: keep or disable TOTP for 1.0, by Jan 8
- [ ] U13: Sigstore keyless or minisign for Q10, by Mar 21
- [ ] U9: demo registry hosting, by May 1

## Recurring review duties

- [ ] Security review of anything touching archives, auth or the plugin host (with Adham)
- [ ] Reviews owed: B-028 (Adham), B-036 and B-037 (Mohamed), B-035 and B-045 (Ahmed), B-053 (Menna)
