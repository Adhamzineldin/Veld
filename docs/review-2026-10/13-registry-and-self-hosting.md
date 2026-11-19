# 13. Registry architecture and self-hosting plan

Evidence: registry and archive audit track (line-by-line review of `internal/registry` and `internal/server`, crafted archives run against the real extraction code, authorization matrix, live end-to-end run against `veld serve` on a disposable PostgreSQL 16 container, two MITM proxies), findings F-REG-01 to F-REG-23. The lead reviewer independently read `tarball.go`, `client.go`, `credentials.go`, the `pull` command and the schema, and confirmed F-REG-01, 02, 03, 09, 10 and 16 at the cited lines. Decision records: ADR-004 (separate binary), ADR-008 (manifest and lockfile).

## 13.1 What exists at 6a47f81 (verified)

Architecture: Go 1.22 `net/http` ServeMux server inside the CLI (`veld registry serve`), PostgreSQL via `lib/pq`, local filesystem blob storage, HMAC-SHA256 JWT (no external library), `vtk_` API tokens stored hashed, bcrypt passwords, TOTP, SMTP email, embedded single-page UI. Schema: users, orgs, org_members, packages, package_versions (UNIQUE package_id, version), tokens, email_verification_tokens (`internal/server/db/db.go:49-120`). The architecture is reasonable; the implementation is not safe to deploy.

Reproduced against the live server (any registered user can create an org; registration is open and email verification is never enforced):

| ID | Finding | Security | Who can trigger |
|---|---|---|---|
| F-REG-01 | The 5-minute "partial" JWT issued before the TOTP step is accepted as a full session: `/auth/me` returns 200 and `/tokens` mints an admin token (`handlers/auth.go:105-117`, `auth/middleware.go:70-100`; JWT has no purpose claim) | high | anyone with a victim's password; defeats 2FA |
| F-REG-02 | Publish builds the storage key from unvalidated `manifest.name` and `version` (`handlers/packages.go:177`; `storage/storage.go:33-36` cleans and joins without a containment check): wrote outside the storage root, overwrote another org's tarball, deleted it through DELETE | high | any registered user |
| F-REG-03 | Token scopes are not enforced except on publish (`packages.go:105`): a read-only token mints read/write/delete/admin tokens (`auth.go:393-439`) and deletes versions (`packages.go:276-338`) | high | holder of any token, for example a leaked read-only CI token |
| F-REG-09 | `veld pull` takes the SHA-256 from the same response and skips verification when the header is absent (`cmd/veld/registry.go:232-236`, `client.go:208-210`); both MITM proxies (header stripped; body and header replaced) pulled successfully; `http://` accepted silently | high | network attacker on http, or a malicious registry |
| F-REG-11 | Stored XSS sinks in the embedded UI (usernames, org text, versions, deprecation text; `escHtml` used on two lines only; no CSP) | high (source-supported; payloads stored and returned, not rendered in a browser) | any registered user or publisher |
| F-REG-10 | `Unpack`: no size, count or ratio limits (1.24 MiB archive expanded to 1 GiB in 2.9 s; 100,000 files from 1.4 MB); writes in place, partial files left on failure; `io.Copy` and `MkdirAll` errors dropped (`tarball.go:98-143`). Path containment held for `..`, absolute, drive-letter and backslash names; symlink, hardlink and device entries become empty regular files (safe by accident); NTFS alternate data stream names write hidden streams | medium | malicious package author or registry |
| F-REG-07 | Concurrent publish of the same version: the loser's cleanup deletes the winner's tarball while the DB row remains (10 of 25 and 16 of 25 downloads returned 404) | low security, high user impact | two CI jobs racing |
| F-REG-08 | Versions are hard-deletable and re-publishable with different content; "latest" is the most recently published, not the highest semver | medium | org owner or stolen token |
| F-REG-04, 05 | An org admin can demote or remove the sole owner; token `expires_at` never enforced | medium | org admin; expired-token holder |
| F-REG-06, 23 | Private packages: cannot be created through the API (publish hard-codes public), and when set in the database they leak through `/orgs/{org}` (anonymous, with member emails), `/packages` (any logged-in user) and `/versions` (no check); 401/403/404 split is an existence oracle | medium | anonymous or any user |
| F-REG-12 to 15 | No rate limit on login or TOTP (60 and 300 attempts, no 429); TOTP code replays; no body-size limit (200 MiB non-archive accepted as a package); JWT survives logout, 7-day lifetime, no password reset; cookie not `Secure`; CORS `*`; no CSP, X-Frame-Options or HSTS; no TLS option | medium | anonymous / network |
| F-REG-16 | `DefaultRegistry()` returns the first key of a Go map (`credentials.go:105-107`): the default registry varied 9/2/1 across 12 runs; credentials keyed by raw URL string; no scope-to-registry binding | medium | user logged in to two registries |
| F-REG-21 | No health endpoint (`/healthz` returns the SPA with 200), SIGTERM unhandled, no `schema_migrations` table (schema re-run on every start), no backup tooling, zero tests for `internal/registry` and `internal/server` | low | operator |
| F-REG-17, 18, 19, 20, 22 | plaintext credentials (standard practice; ACL inherited on Windows); `Pack` includes every `.veld` at any depth and follows file symlinks; SMTP proceeds without TLS and logs the verification token; pull can overwrite project files when `--out` points at the project and a pulled `veld.config.json` can carry a `postGenerate` hook; docs name commands that do not exist and a `--secret` example that fails the 16-character minimum | low to medium | various |

Restart persistence works. Registration, login, org creation, publish, download and re-publish refusal of an existing version work in the happy path.

## 13.2 Scope decision for graduation

Keep the registry in scope, because Journey C (shared contracts with a lockfile) is a differentiator and the existing code is a reasonable base. Reduce the feature set to what can be made correct and tested:

| Feature | Graduation | Reason |
|---|---|---|
| Users, registration, login, password hashing | yes, with rate limiting and typed JWTs (`purpose: session \| partial`), revocable sessions (server-side session id), password reset by email if SMTP is configured, otherwise operator-only reset command | required |
| TOTP | keep only if F-REG-01 and replay protection are fixed and tested; otherwise disable the endpoints for 1.0 | a broken 2FA is worse than none |
| Organizations, roles owner/admin/member, last-owner protection, ownership transfer by owner only | yes | needed for scopes |
| Scoped tokens (read, write, delete, admin; optional org binding), expiry enforced, revocation list | yes | needed for CI |
| Public and private packages | yes: private is the default for new packages; visibility enforced by one authorization helper used by every handler | tenancy is the point of self-hosting |
| Version immutability | yes: a published version can never be overwritten; delete is replaced by `yank` (hidden from resolution, still downloadable by exact version with a warning) within 72 hours of publish by an owner, and `deprecate` (message) at any time | reproducibility |
| Search | name prefix search over the user's visible packages only | cheap; full-text deferred |
| Audit events | append-only `audit_events` table for login, token create/revoke, publish, yank, deprecate, member changes; exposed as an owner-only list endpoint | low cost; useful for the report's security chapter |
| Retention | none at graduation; blobs are small | n/a |
| Web UI | keep the embedded SPA but rewrite its rendering to a templating approach with escaping by default, add CSP; or reduce to a read-only package browser | F-REG-11 |
| Email | optional; verification required for publishing only when SMTP is configured | avoids blocking local setups |

## 13.3 Target design

API boundary `/api/v1` (unchanged shape where correct, additive elsewhere):

| Endpoint | Auth | Authorization helper call |
|---|---|---|
| `POST /auth/register`, `/auth/login`, `/auth/login/totp`, `/auth/logout`, `/auth/password-reset` | none / partial / session | rate-limited by IP and account |
| `GET /auth/me`, `POST /tokens`, `DELETE /tokens/{id}` | session only (API tokens cannot mint tokens) | `requireSession` |
| `POST /orgs`, `/orgs/{org}/members`, `DELETE .../members/{user}`, `POST /orgs/{org}/transfer` | session or admin-scoped token | `requireOrgRole(owner)` for role changes and transfer; last-owner guard everywhere |
| `GET /packages?org=` | optional | visibility filter |
| `GET /packages/{org}/{name}`, `/versions`, `/{ver}/download` | optional | `canRead(package)` on every one of them |
| `POST /packages` (publish) | token with `write` | `canWrite(org)`; manifest validated: `name` matches `^[a-z0-9][a-z0-9-]{0,63}$`, `version` strict semver, `org` must equal the token's org when org-bound |
| `POST /packages/{org}/{name}/{ver}/yank`, `/deprecate` | token with `delete` / `write` | `canWrite(org)` |
| `GET /orgs/{org}/audit` | session, owner | `requireOrgRole(owner)` |
| `GET /healthz` (process up), `GET /readyz` (DB ping, storage writable) | none | n/a |

Metadata schema additions: `package_versions.sha256` (already), `size_bytes`, `yanked_at`, `dependencies jsonb`; `packages.visibility` default `private`; `sessions` table (id, user, expires, revoked); `audit_events`; `schema_migrations` with forward-only numbered migrations embedded in the binary.

Storage abstraction: `Storage.Put(key, r io.Reader, limit int64) (sha256, size, error)` writes to a temporary file, fsyncs, renames; keys are derived by the server as `sha256/<first2>/<hash>.tar.gz` (content-addressed), never from user input; the DB row stores the key. The local filesystem implementation is the only one at graduation; the interface allows an S3 implementation later.

Publication atomicity and concurrency: (1) validate manifest; (2) stream the upload through `io.LimitReader` (default 20 MiB) into storage computing the hash; (3) verify the archive is a gzip tar whose entries are all regular files with allowed names and sizes (server-side `Unpack` dry run with the hardened reader); (4) in one transaction, insert the version row with `ON CONFLICT DO NOTHING RETURNING`; if nothing returned, the version exists: respond 409 and delete only the blob written in this request if no other row references the hash; (5) respond 201 with `sha256`. Idempotency: a retry with an identical archive for an existing version returns 200 with the same `sha256`; a different archive returns 409.

Resource limits: body 20 MiB (configurable), archive entries 2,000, per-entry 2 MiB, total uncompressed 50 MiB, JSON bodies 64 KiB, login 10 per minute per IP and 5 per minute per account, TOTP 5 per 5 minutes with last-used counter stored.

Threat model and controls (proportional to graduation scope):

| Threat | Control |
|---|---|
| Malicious archive (traversal, symlinks, bombs) | server-side validation on publish and the hardened client `Unpack`: allow-list of entry types (regular file, directory), name validation, limits, temp dir then rename, all I/O errors checked |
| Malicious package author | data-only packages; nothing is executed; a pulled `veld.config.json` is never used as the project config (ADR-008 manifest replaces it; hooks live only in the consumer's own config) |
| Compromised token | scopes enforced; expiry; revocation; org binding; audit events |
| Malicious registry operator or network attacker | lockfile integrity (ADR-008); `https` required unless `--allow-insecure-registry`; checksum always required |
| Private package enumeration | one visibility helper; uniform 404 for unauthorized reads |
| Secret leakage | tokens hashed; secrets never logged; verification token not logged; config validation refuses short secrets at start |
| Supply chain of the registry itself | Docker image built by CI from the tagged source with checksums and attestation (chapter 14) |

Executable plugins are not hosted by the registry (ADR-003, ADR-008). If that changes after graduation, plugins need signing and a separate trust policy; this is stated as future work.

Hosting decision: a dedicated binary `veld-registry` (ADR-004), distributed as a Docker image and release archive; the registry UI stays embedded in that binary; the public website (`website/`) is a separate static site with no code shared and none needed.

## 13.4 Self-hosting and operations

Default deployment: single host with Docker Compose (`deploy/docker-compose.yml`): `veld-registry`, `postgres:16` with a named volume, `caddy` for TLS and the custom domain, artifact volume. Kubernetes, replication and high availability are out of scope and documented as such.

Minimum requirements (assumptions: up to 50 users, 500 package versions, 5 publishes and 500 downloads per day, artifacts under 1 MB each): 2 vCPU, 4 GB RAM, 20 GB disk, one DNS name. Operating cost: a small cloud VM in this class is in the range of 10 to 25 USD per month at common providers (order-of-magnitude assumption; verify with the chosen provider), plus backup storage of a few GB.

Operator features to build:

| Item | Design |
|---|---|
| Configuration validation | `veld-registry check-config` validates the file and environment, refuses a secret shorter than 32 characters, refuses `sslmode=disable` unless `--allow-insecure-db`, prints the effective config with secrets masked |
| Secrets | environment variables or Docker secrets; never in the image |
| TLS | terminated by Caddy; the server also accepts `--tls-cert/--tls-key` for bare-metal |
| Persistent volumes | `pgdata`, `/data/packages` |
| Bootstrap administration | `veld-registry admin create-user --owner-of <org>` for the first account; registration can be closed with `REGISTRATION=invite` |
| Health and readiness | `/healthz`, `/readyz`; compose `healthcheck` |
| Logs | JSON lines to stdout; request id; no secrets |
| Metrics | a minimal `/metrics` text endpoint (requests, publishes, errors) only if time allows; not required |
| Quotas | per-org package count and total bytes (config), enforced on publish |
| Upgrades | image tag bump; migrations run at start, forward-only, each in a transaction; the binary refuses to start if the DB schema is newer than it knows |
| Migration failure | start aborts with the failed migration number; operator restores from backup or applies the fix; application rollback to the previous image is only safe if no migration has been applied since the backup, which the `backup-manifest.json` records |
| Private networks, custom domains | Caddy config samples; air-gapped: image and compose files transferable; `veld` clients use `--registry` with the internal name; no telemetry or outbound calls exist |

Backup and restore (to be tested in M5 and timed):

1. `veld-registry backup --out /backups/<ts>/`: runs `pg_dump -Fc` through the configured DSN, archives `/data/packages`, writes `backup-manifest.json` (schema version, app version, SHA-256 of both archives, counts of users, packages, versions).
2. Off-host copy by the operator's tool (rclone, scp); the runbook shows one example.
3. `veld-registry restore --from /backups/<ts>/`: refuses if the app version in the manifest is newer than the binary; restores the DB and artifacts; runs `veld-registry verify` which recomputes the SHA-256 of every blob referenced by `package_versions` and reports missing or mismatched blobs.

Proposed objectives: RPO 24 hours (nightly backup), RTO 1 hour on a clean host with the runbook. Acceptance check for the Beta gate: a recorded restore drill on a clean VM, timed, with `verify` reporting zero mismatches and a client `veld install --frozen` succeeding against the restored registry.

Minimum operator runbook (D1 writes, T3 validates by executing every step): install, first user and org, TLS, backup, restore, upgrade, migration failure recovery, rotating the JWT secret (invalidates sessions), revoking a leaked token, closing registration.

## 13.5 Work plan and estimates (owner T3; reviewer T1 for security items)

| Item | Findings | Estimate (h) | Sprint |
|---|---|---|---|
| Hardened `Unpack` (limits, temp dir, errors, entry allow-list) and `Pack` filtering; checksum-required pull; `https` default; deterministic registry selection | F-REG-09, 10, 16, 18 | 12 to 20 | S1 |
| Typed JWTs with purpose, scope enforcement helper, expiry, last-owner guard, session revocation | F-REG-01, 03, 04, 05, 14 | 10 to 18 | S1 to S2 |
| Manifest validation, content-addressed storage, atomic publish, limits, idempotency | F-REG-02, 07, 08, 13 | 12 to 20 | S2 to S3 |
| Visibility helper and uniform 404; private default; member email removal | F-REG-06, 23 | 6 to 10 | S3 |
| Rate limiting, TOTP replay counter, security headers, cookie flags, CORS allow-list | F-REG-12, 15 | 6 to 10 | S3 to S4 |
| Separate binary, versioned migrations, `/healthz`, `/readyz`, SIGTERM, JSON logs, config check | ADR-004, F-REG-21 | 10 to 16 | S3 to S4 |
| UI rendering rewrite with escaping and CSP | F-REG-11 | 8 to 14 | S4 to S5 |
| Server test suite (`httptest`, fake storage, PostgreSQL service in CI): authz matrix, publish races, limits | F-CLI-23 | 16 to 30 | S2 to S5 |
| Manifest, lockfile, resolver, cache, scoped registries (client) | ADR-008 | 36 to 56 | S8 to S10 |
| Compose, Caddy, backup/restore/verify commands, runbook, drill | section 13.4 | 16 to 24 | S11 to S12 |
| Total | | 132 to 218 | |

The registry stream is the only one that can expand without bound; the scope table in 13.2 is the fence. If the stream exceeds its sprint budget by more than 20 percent at the M2 exit review, search, audit UI and TOTP are cut, in that order.
