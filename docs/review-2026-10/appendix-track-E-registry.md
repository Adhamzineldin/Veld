# Appendix: audit-track notes (E-registry)

> Provenance: raw notes produced by a delegated audit track (Claude Sonnet subagent) during the 2026-10-21 review of commit 6a47f81, reproduced verbatim. They are evidence logs, not conclusions: the lead reviewer's verified conclusions are in chapters 04, 05 and 13, and the column `lead_verified` in `csv/defect-register.csv` marks the items independently confirmed. Commands were run in disposable scratch directories; no tracked file was modified; no public registry was contacted.

# Veld registry subsystem audit

Commit 6a47f815f37ea4c286a6e765312df76743fa7133, reviewed 2026-10-21. Windows 11 host, Go 1.26, Docker 29.7.2 (postgres:16 container `veld-audit-pg`, removed at end).
No tracked file was modified (`git diff` clean; only pre-existing untracked docs). No real registry was contacted. CLI runs used `USERPROFILE=<scratch>\home`, so the real `~/.veld` was untouched (verified).

Artifacts (all under `...\scratchpad\reg-audit\`): `registrycopy\` (verbatim copy of tarball.go + `extract_test.go`), `e2e\main.go` (API harness), `proxy\main.go` (MITM), logs `unpack-small.log`, `unpack-small2.log`, `unpack-many.log`, `e2e-run1.log`, `e2e-run2.log`, `server*.log`.

Evidence legend: REPRODUCED = I ran it and saw the result. SOURCE = read in code, not executed. Platform caveat: archive tests ran on Windows (GOOS=windows). Linux behaviour for backslash/drive-letter names is inferred from the code (inert there).

---------------------------------------------------------------------------------------------------

## 1. Archive handling (internal/registry/tarball.go and callers)

Callers: `Pack` <- cmd/veld/registry.go:132 (push). `Unpack` <- registry.go:238 (pull). `VerifySHA` <- registry.go:233. `UnpackReader` has no other caller. The server never opens the archive (see F-REG-13).

1. Path containment (tarball.go:116-127). `package/` prefix is stripped (117-119, not required: unprefixed names are accepted). `filepath.Clean(name)` (122), then `strings.HasPrefix(name, "..")` -> silently `continue` (123-125); then `filepath.Join(destDir, name)` (127). There is no `filepath.Rel`/prefix check against destDir after the Join, no `filepath.IsAbs`/`VolumeName` check, no symlink-evaluation of destDir.
   - `..` forms are skipped silently (REPRODUCED: `package/../../escape.txt`, `../../escape2.txt`, `package/a/../../../escape3.txt` -> nothing written outside; only `ok.veld` extracted). False positive: any legitimate file whose name starts with `..` (e.g. `..foo`) is silently dropped.
   - Absolute names: `Join` re-roots them under dest (REPRODUCED: `/tmp/abs-escape.txt` -> `dest\tmp\abs-escape.txt`). Real absolute Windows path -> `os.Create` fails with "filename ... syntax is incorrect" and Unpack returns the error (no escape). `C:\Windows\Temp\x` same (error, nothing written outside).
   - Drive-relative `C:name` (REPRODUCED): no error, creates a file `dest\C` with an NTFS alternate data stream `veld-driverel.txt`. Likewise `package/a.veld:hidden` writes a hidden ADS on `a.veld`. Contained to dest, but invisible data can be smuggled on Windows.
   - Backslash names (REPRODUCED on Windows): `package\..\..\bs-escape.txt` and `..\..\bs-escape2.txt` were skipped (Clean normalises `\`); `package\sub\file.veld` extracted to `dest\package\sub\file.veld` (prefix strip did not match because it only strips the forward-slash prefix). On Linux a backslash is a literal filename character (SOURCE, inert).
2. Entry types (tarball.go:129-141). Only `TypeDir` is special-cased. EVERYTHING else (symlink, hardlink, char/block device, FIFO, PAX global header) falls through to `os.Create` + `io.Copy`. REPRODUCED: symlink entry -> empty regular file `dest\link` (mode rw-rw-rw-, not a symlink); a following entry `link/pwned.txt` then fails (`MkdirAll` error ignored at 134, `os.Create` error returned at 136) so nothing is written through the "link". Hardlink, device, FIFO, `pax_global_header` -> empty regular files named `hl`, `dev`, `fifo`, `pax_global_header`. So symlink/hardlink attacks are neutralised by accident (never materialised), not by design. A pre-existing symlink or junction inside destDir/outDir WOULD be followed by `os.Create` (SOURCE, not tested; Windows symlink creation needs privilege). PAX/GNU long names are decoded by archive/tar into `hdr.Name` and go through the same Clean check.
3. Size limits: none. No per-entry size (`hdr.Size` unused), no total size, no entry count, no ratio check, `io.Copy(out, tr)` at 139 has no `io.LimitReader`. REPRODUCED: 1.24 MiB archive -> 1.00 GiB file written, 2.9 s, err=nil. 1.4 MB archive with 100,000 one-byte entries -> 100,000 files created in 4 m 04 s (Defender-affected NTFS), err=nil. Download is also unbounded: `Client.Download` copies the whole body to a temp file (client.go:209) and `Client.do` uses `io.ReadAll` (client.go:69).
4. Checked I/O: `tr.Next` error is checked and returned (107-113); gzip.NewReader error checked (99). NOT checked: `os.MkdirAll` (130,134), `io.Copy` (139), `out.Close` (140), `VerifySHA`'s io.Copy (153), Pack's `tw.WriteHeader` (73), `io.Copy` (74), `tw.Close/gw.Close/tmp.Close` (78-80), `Download`'s io.Copy (209). REPRODUCED: truncated gzip -> file `big.veld` left at 144,776 of 400,000 bytes, and Unpack returned `unexpected EOF` only because the next `Next()` call failed; a failure inside the LAST entry's body would be reported as success if the tar trailer were also tolerated (the copy error is dropped).
5. Partial extraction: extraction writes directly into the final outDir (registry.go:214, 238); no temp dir + rename; no cleanup. REPRODUCED: archive `good1, good2, C:\bad.txt, never` -> error, but `good1.veld` and `good2.veld` remain on disk. Re-pulling a newer version over an older one also leaves stale files from the old version (no removal), and silently overwrites files that exist in outDir.
6. Checksum: Pack computes sha256 over the written tar.gz (tarball.go:51-82) and push throws it away (`_ = sha`, registry.go:157). The SERVER recomputes the sha while streaming (packages.go:178-202) and stores it in `package_versions.tarball_sha`; the download endpoint returns it in header `X-Veld-SHA256` (packages.go:270). The client reads that header from the SAME HTTP response as the body (client.go:208), and verifies only `if remoteSHA != ""` (registry.go:232). It is verified BEFORE extraction (233 vs 238) and after the full download to a temp file. Comparison is `got != expected` string compare (tarball.go:155), not constant time (irrelevant here). Source of truth = the registry's own response header: it protects against corruption and (REPRODUCED) against a server-side on-disk tamper (`integrity check failed: SHA-256 mismatch`), but gives NO authenticity: a MITM or a malicious registry controls both header and body, and omitting the header disables verification (REPRODUCED, F-REG-09). No signature, no lockfile, no pinning of the sha in the consumer project.
7. File modes: archive modes are ignored; files are created with `os.Create` (0666 & umask), directories 0755. No setuid/exec bits applied. REPRODUCED (Windows shows rw-rw-rw- for a 04777 entry; on Unix the mode would be 0666&^umask, SOURCE).
8. `Pack` (tarball.go:17-84): walks `dir` (= `rc.ConfigDir`) recursively with `WalkDir` and includes EVERY file whose extension is `.veld` or whose basename is `veld.config.json`, at any depth (including nested projects, `node_modules`, `generated/`). The `patterns` slice is dead code (20, 32). `WalkDir` does not follow directory symlinks, but a file symlink named `x.veld` pointing anywhere is included with the TARGET's content (d.IsDir() is false for the link; `os.Stat`/`os.Open` follow it) - SOURCE, not run. Dotfiles/dot-dirs are not excluded. `.veld-cache.json`/`.veld.lock.json` are NOT included (extension filter). Entry mode is fixed 0644, ModTime = file mtime (archive bytes not reproducible, so the sha differs between identical packs). Files that cannot be stat'd/opened are silently skipped; a file that grows between stat and copy yields a corrupt tar whose write error is ignored. `veld.config.json` (which may contain `hooks.postGenerate`, `baseUrl`, etc.) is always published.

Reproduction log (section 2) follows the same numbering.

---------------------------------------------------------------------------------------------------

## 2. Reproduction log (archive tests)

Method: `registrycopy/tarball.go` is byte-identical to `internal/registry/tarball.go` (`cmp` passed); `extract_test.go` crafts archives with archive/tar (no repo imports) and calls the real `Unpack`. Run: `go test -v -run '...' .` in `registrycopy`.

| Archive | Result |
|---|---|
| `package/../../escape.txt`, `../../escape2.txt`, `package/a/../../../escape3.txt` | refused silently (skipped); nothing outside dest; `ok.veld` extracted; err=nil |
| `/tmp/abs-escape.txt` | re-rooted to `dest\tmp\abs-escape.txt` (contained) |
| real absolute path, `C:\Windows\Temp\veld-drive-escape.txt` | Unpack returned error (invalid syntax); nothing written outside; earlier entries stay (`ok1.veld`) |
| `C:veld-driverel.txt`, `package/a.veld:hidden` | accepted; file `dest\C` / `dest\a.veld` size 0 with NTFS ADS (contained, hidden data) |
| `package\..\..\bs-escape.txt`, `..\..\bs-escape2.txt` | skipped; `package\sub\file.veld` extracted as `dest\package\sub\file.veld` |
| symlink `link -> <outside dir>` then `link/pwned.txt` | symlink became empty regular file; pwned.txt NOT written outside (Unpack error: path not found); err returned |
| hardlink `-> /etc/passwd`, char device, FIFO, PAX global header | all became empty regular files |
| 04777-mode entry | mode not applied |
| good1, good2, `C:\bad.txt`, never | error returned; good1+good2 left in dest (partial extraction) |
| truncated gzip (400 KB entry) | `unexpected EOF`; truncated 144,776-byte file left |
| 1 GiB zero entry, compressed 1,302,662 B (1.24 MiB) | extracted fully: 1,073,741,824 B, 2.86 s, err=nil (BOMB) |
| 100,000 one-byte entries, 1,484,475 B archive | 100,000 files created, 4 m 04 s, err=nil |

Cleanup: all test dirs are `t.TempDir()` (auto-removed); one leftover temp dir from a failed first run was deleted manually.

---------------------------------------------------------------------------------------------------

## 3. Authorization matrix

Auth column: R = requireAuth (401 if none), O = optionalAuth, N = none. Bearer accepts a JWT (anything with 2 dots) OR a `vtk_` token; cookie `veld_session` accepted when no Authorization header (middleware.go:70-100). "Scope" = API-token scope check. Body limits: NONE anywhere (no `http.MaxBytesReader`; only `ReadTimeout` 30 s / `WriteTimeout` 120 s at server.go:59-61). Server config has no TLS.

| Endpoint | Method | Auth | Membership/role | Ownership | Input validation | Scope check | Notes |
|---|---|---|---|---|---|---|---|
| /api/v1/auth/register | POST | N | - | - | non-empty, pw>=8; email/username not validated (HTML/any bytes accepted) | - | open registration; returns full 7-day JWT immediately (auth.go:78) |
| /auth/login | POST | N | - | - | none; bcrypt cost 10 | - | no rate limit (60 wrong pw in 4.5 s, all 401); partial JWT returned if TOTP (auth.go:107) |
| /auth/logout | POST | N | - | - | - | - | only clears cookie; JWT stays valid |
| /auth/me | GET | R | - | self | - | none | |
| /auth/totp-login | POST | N | - | - | 6-digit | - | no rate limit, no replay guard |
| /auth/setup-totp, confirm-totp | POST | R | - | self | confirm needs valid code | none | accepts the partial JWT (F-REG-01) |
| /auth/totp | DELETE | R | - | self | needs password | none | |
| /auth/verify-email | POST | N | - | token | token lookup | - | verification result is never used anywhere |
| /auth/resend-verification | POST | R | - | self | - | none | 503 without SMTP |
| /tokens | GET | R | - | own tokens | - | none | hash hidden (`json:"-"`) |
| /tokens | POST | R | none | self | name required; scopes/org_id/expires NOT validated | none | read-only token can mint admin token (F-REG-03); org_id never enforced |
| /tokens/{id} | DELETE | R | - | `user_id=$2` in SQL (OK) | - | none | |
| /stats | GET | N | - | - | - | - | public counts |
| /orgs | GET | O | - | - | - | - | anon: ALL orgs; authed: only own orgs (inconsistent) |
| /orgs | POST | R | any registered user | creator = owner | name charset a-z0-9- ; length/reserved names not checked; description/display_name unescaped | none | open org creation |
| /orgs/{org} | GET | O | none | - | - | - | members incl. email + user_id, and ALL packages incl. private, to anonymous (F-REG-06) |
| /orgs/{org}/members | POST | R | caller owner/admin | - | role enum; username lookup | none | admin can overwrite an owner's role (ON CONFLICT UPDATE, db.go:296) (F-REG-04) |
| /orgs/{org}/members/{u} | DELETE | R | caller owner/admin | - | last-owner guard only when caller is owner (orgs.go:180) | none | admin can remove sole owner |
| /packages | GET | O | none | - | `q` -> ILIKE param (wildcards unescaped) | - | any authed user sees every private package (packages.go:29-32) |
| /packages | POST | R | owner/admin of org | - | org/name/version only checked non-empty; NO charset/length/semver; multipart 32 MB is a memory threshold only | `write` | creates package record before tarball is validated; no archive validation |
| /packages/{o}/{n} | GET | O | private: member | - | - | none | 404 for non-visible |
| /packages/{o}/{n}/versions | GET | O | NONE | - | - | none | no visibility check at all (packages.go:80) |
| /packages/{o}/{n}/{v}/download | GET | O | private: any member | - | - | none (no `read`) | anon 401 vs non-member 403 vs absent 404 = existence oracle |
| .../{v}/deprecate | POST | R | owner/admin | - | message unbounded | none | |
| .../{v} | DELETE | R | owner only | - | - | none (read-only token works, REPRODUCED) | deletes DB row and file; version can then be re-published |
| / (catch-all) | any | N | - | - | - | - | serves embedded index.html for every path and method; no file server |

Auth package notes: JWT HS256 (auth/token.go:56-92): header is NOT parsed, algorithm is fixed to HMAC-SHA256 by construction (no alg confusion, no `none`); signature compared with `hmac.Equal` (77) (constant time); `exp` enforced (88); no `nbf`, no `aud`/`typ`/purpose claim; secret minimum 16 chars (server.go:38; CLAUDE.md says 32+); the secret is also the only key (no rotation, no `kid`). `vtk_` token = 32 bytes `crypto/rand` base64url (token.go:19-25), stored as sha256 hex (unsalted, fine for 256-bit random), revocable by DELETE /tokens/{id}; `expires_at` is stored but never checked (F-REG-05). `GenerateID` ignores `rand.Read` error (token.go:38). bcrypt `DefaultCost` (10) (auth.go:52). TOTP: HMAC-SHA1, 6 digits, +-1 step window (totp.go), secret 160-bit crypto/rand, secret stored plaintext in DB, no replay protection, no attempt limit. Email verification token = 2x16 random bytes hex (auth.go:348-350, 256-bit), expiry 24 h enforced (289), stored plaintext, single use; but `email_verified` is never consulted for any permission. Cookie: HttpOnly yes, SameSite=Lax, Secure NO (auth.go:372-381). CORS: `*` for all (server.go:152-154), no credentials flag, `PATCH` absent; no CSP/XFO/XCTO/HSTS (REPRODUCED empty). SQL: every query is parameterised; the only two `fmt.Sprintf` in db.go (408, 415) build `$N` placeholders only. Migrations: single `CREATE TABLE IF NOT EXISTS` + `ALTER ... ADD COLUMN IF NOT EXISTS` string executed at every start (db.go:43-128); no version table (REPRODUCED: `\dt` shows 7 tables, no schema_migrations). Storage: local FS; key = `org/name/version.tar.gz` built from unvalidated input (F-REG-02); `Put` writes to `.tmp-*` then renames (atomic per file). Publish ordering: pkg row created (packages.go:155) -> tarball stored (184) -> version row inserted (216); on insert failure `Storage.Delete(tarballKey)` (217) - which deletes the winner's file (F-REG-07). Web: `handlers/web.go` serves exactly one embedded file; no path-based file access, so no traversal (REPRODUCED: `/../../../etc/passwd` -> the SPA HTML). Logging: request line only (method, path, duration); no tokens in logs EXCEPT the email-verification URL with its token (auth.go:363) and SMTP username/host (email.go). Graceful shutdown: `signal.NotifyContext(os.Interrupt)` only (serve.go:138) - SIGTERM (docker stop, systemd) is not handled; 10 s Shutdown otherwise (server.go:~66). Health endpoint: none (any path returns 200 SPA, REPRODUCED). Backup/restore: no docs, no command (none expected); state = PostgreSQL + the storage dir, which must be backed up consistently; nothing reconciles DB rows and files.

Config precedence (serve.go:97-108): addr, dsn, storage, secret implement flag > env > file > default as documented. BaseURL: env/file only (no flag). SMTP: no flags; SMTP port: file only (no SMTP_PORT env); `server.Config` documents `JWT_SECRET`. `registry.config.example.json` has a placeholder secret "replace-this-with-openssl-rand-hex-32" (37 chars, so the example file starts without error if copied unchanged, i.e. a publicly known JWT secret); sslmode=disable DSN.

---------------------------------------------------------------------------------------------------

## 4. Live end-to-end log (Part D)

Environment: `docker run -d --name veld-audit-pg -p 55432:5432 -e POSTGRES_PASSWORD=audit -e POSTGRES_DB=veld postgres:16` (Docker Desktop had to be started first). Server: `veld.exe registry serve --addr :18080 --dsn "postgres://postgres:audit@localhost:55432/veld?sslmode=disable" --secret "audit-secret-0123456789-abcdefghij" --storage <scratch>\reg-audit\storage`. NOTE the command is `veld registry serve`; `veld serve`, `veld login`, `veld push`, `veld pull` as documented in CLAUDE.md do NOT exist (`Error: unknown command "login" for "veld"`, REPRODUCED).
SMTP is NOT required: registration returns a JWT immediately, the verification email is skipped with a log line, and `email_verified` is never enforced. API tokens are created via the API (the CLI's `login` needs a `vtk_` token; password login is not exposed by the CLI - `Client.Login` exists but no command calls it).

CLI sequence (PowerShell, USERPROFILE redirected):
```
veld registry login --registry http://localhost:18080 --token vtk_...   -> Logged in ... as cli5487   (plain http accepted; no warning)
credentials.json: {"registries":{"http://localhost:18080":{"token":"vtk_DYPN...","username":"cli5487"}}}  (plaintext)
veld registry push        -> Published @cliorgcli5487/hello@1.0.0  {"sha256":"b4faa0c5...","size":285}
veld registry push (again)-> Error: registry error (409): version 1.0.0 already published   (exit 1)
veld registry pull @cliorgcli5487/hello  -> Pulled ...@1.0.0 -> veld\packages\@cliorgcli5487\hello\{app.veld, veld.config.json}
veld registry pull ...@9.9.9  -> Error: download failed: registry error (404): version not found
veld registry pull @cliorgcli5487/nopkg -> Error: could not fetch versions: registry error (404): not found
tamper (append bytes to stored .tar.gz) then pull -> Error: integrity check failed: SHA-256 mismatch: expected b4faa0c5... got 26576dbc...  (exit 1, nothing extracted)
MITM proxy :18081 (replaces body, DELETES X-Veld-SHA256) -> "Pulled" OK; evil app.veld + veld.config.json (postGenerate hook) written
MITM proxy :18082 (replaces body AND sets matching sha) -> "Pulled" OK; same
```
Restart persistence: process killed (hard; Windows has no SIGTERM path), restarted with identical args: stats before/after identical (8 users, 5 orgs, 12 packages, 64 versions), earlier API token still valid, download of old tarball 200, migration re-ran silently with no error. Rows are in PostgreSQL, files in storage dir.

API harness output (`e2e-run1.log`, `e2e-run2.log`) is quoted in the findings below. Private packages: the API offers no way to make a package private (Publish hard-codes `Visibility: "public"`, packages.go:151; no update endpoint), so for the private-package tests I set `visibility='private'` with `UPDATE packages` in psql.

Cleanup done: server and proxies stopped, `docker rm -f veld-audit-pg` run and verified (0 containers named veld-audit-pg), real `~/.veld/credentials.json` verified not to contain 18080. Leftover scratch files (storage dir, `reg-audit\traversal-*` file written by the traversal test) are inside the scratchpad.

---------------------------------------------------------------------------------------------------

## 5. Findings

### F-REG-01: 2FA bypass - the pre-TOTP "partial" JWT is a fully valid session token
- Category: server-auth
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: high
- Exploitability: anyone who knows a victim's password (the exact attacker TOTP exists to stop); no other precondition. Also usable by a stolen password from a leaked DB of another service.
- Location: internal/server/handlers/auth.go:105-117 (partial issued with `IssueJWT`), internal/server/auth/token.go:56-92 (no purpose claim), internal/server/auth/middleware.go:70-100 (accepts any valid JWT as a session)
- Trigger/reproducer: `e2e-run2.log` E7. alice enables TOTP; `POST /auth/login` returns `{"partial":"eyJ...","totp_required":true}`; `GET /auth/me` with `Authorization: Bearer <partial>` -> `HTTP 200 {... "totp_enabled":true}`; `POST /tokens` with the partial -> HTTP 201 (admin API token minted). Never supplied a TOTP code.
- Expected vs actual: partial token should only be accepted by /auth/totp-login; actual: accepted everywhere for 5 minutes, and it can mint a permanent admin API token.
- Impact: two-factor authentication gives no protection at all against password compromise; attacker gets a durable `vtk_` token.
- Root cause: `JWTClaims` has only sub/exp/iat; the partial and the full token are indistinguishable.
- Recommendation: fix - add a `typ`/`purpose` claim (e.g. "2fa-pending") that `resolveAuth` rejects, and require it in totp-login.
- Regression check: test that a partial token gets 401 on /auth/me, /tokens, /packages (POST).
- Effort estimate: 1h-2h

### F-REG-02: Storage path traversal on publish - arbitrary `*.tar.gz` write, overwrite and delete
- Category: server-storage
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: high
- Exploitability: any registered user (registration and org creation are open and unverified); becomes owner of their own org, which is the only precondition for Publish.
- Location: internal/server/handlers/packages.go:122,177 (unvalidated `manifest.name`/`version` into the key); internal/server/storage/storage.go:33-36 (`Clean` then `Join` with no containment check); packages.go:217 and :334 (Delete of the same key)
- Trigger/reproducer: `e2e-run1.log` E4. Publish `{"org":"mal9041","name":"../../traversal-9041","version":"1.0.0"}` -> 201; file created at `<scratch>\reg-audit\traversal-9041\1.0.0.tar.gz`, i.e. outside the storage root. Then publish `name="../acme9041/auth"` -> victim's `acme9041/auth/1.0.0.tar.gz` overwritten (disk sha changed from 53da33... to d94982...; victim download shows header sha 53da33... but body sha d94982...). Then mallory `DELETE /packages/mal9041/..%2Facme9041%2Fauth/1.0.0` -> 204 and victim's tarball is gone (`HTTP 404 file not found in storage`).
- Expected vs actual: name/version must be constrained and the key must stay under the root; actual: arbitrary relative key. Content is attacker-controlled bytes; suffix is always `.tar.gz` (limits it to overwriting/creating `*.tar.gz` files anywhere the server user can write, and deleting such files). It breaks other orgs' packages (integrity-check failure at best; if the victim version's DB sha were also attacker-controlled it would be a substitution) and can destroy any `*.tar.gz` on the host.
- Impact: cross-tenant integrity/availability damage; arbitrary file write/delete of `.tar.gz` outside the data dir; combined with the missing sha anchoring on the client (F-REG-09) a path to serving wrong content.
- Root cause: no validation of package name/version (no charset, length, semver); trust in `filepath.Clean`.
- Recommendation: fix - validate org/name (`^[a-z0-9][a-z0-9._-]{0,63}$`) and strict semver at the handler, derive the storage key from the DB ids (content-addressed by sha), and verify `filepath.Rel(root, p)` stays inside.
- Regression check: publish with `..`, `/`, `%2F`, NUL, backslash, very long, and unicode names -> 400; assert nothing outside root.
- Effort estimate: 3h-6h

### F-REG-03: Token scope model is not enforced; a read-only token can mint admin tokens and delete versions
- Category: server-authz
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: high
- Exploitability: holder of any valid API token (e.g. a leaked read-only CI token).
- Location: internal/server/handlers/auth.go:393-439 (CreateToken: no scope check, scopes/org_id accepted verbatim); handlers/packages.go:276-338 (Deprecate/Delete never call `HasScope`); only packages.go:105 checks `write`; middleware.go:57-68 (`HasScope` returns true for JWT); `read` scope never checked; `org_id` of a token never enforced
- Trigger/reproducer: E2/E3. `ro` token (scopes [read]) -> `POST /tokens` with `scopes:[read,write,delete,admin]` -> token returned. Publish with `ro` -> 403 "missing 'write' scope", but with the minted token -> 201. `DELETE /packages/acme/auth/0.9.0` with the read-only token -> 204.
- Expected vs actual: scopes limit what a token can do; actual: trivially bypassed.
- Impact: least-privilege CI tokens give no protection; a leak of a "read" token is a full account compromise (publish, delete, rotate).
- Root cause: scopes bolted on at one handler; token management reachable with token auth.
- Recommendation: fix - require JWT session (or `admin` scope) for token management, validate scope enum, check scopes centrally in middleware per route, enforce `org_id`.
- Regression check: matrix test of (scope x endpoint).
- Effort estimate: 4h-8h

### F-REG-04: Org admins can demote or remove the sole owner (org takeover)
- Category: server-authz
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: medium
- Exploitability: org member with role `admin` (granted by an owner).
- Location: internal/server/handlers/orgs.go:108-155 (AddMember allows an admin to set another member's role; db.go:296-303 `ON CONFLICT DO UPDATE SET role`), orgs.go:157-193 (last-owner guard only inside `if caller.Role == "owner"`, line 180)
- Trigger/reproducer: E6. bob(admin) `POST /orgs/acme/members {"username":"alice","role":"member"}` -> 200, alice's role is now `member`; with alice restored, bob `DELETE /orgs/acme/members/alice` -> 204, `owners left: 0`.
- Expected vs actual: admins must not modify owners; org must keep >=1 owner; actual: orphaned org nobody can administer.
- Impact: privilege escalation/lock-out inside an org; orphaned orgs cannot publish new owners (all role changes need owner/admin).
- Root cause: role checks look only at caller role and requested role, not target's current role.
- Recommendation: fix - disallow admins touching owners and enforce last-owner invariant for every caller in a transaction.
- Regression check: unit tests per (caller role x target role x action).
- Effort estimate: 2h-3h

### F-REG-05: API token `expires_at` is never enforced
- Category: server-auth
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: holder of an expired token.
- Location: internal/server/db/db.go:535-557 (GetTokenByHash returns expired rows), middleware.go:89-98 (no check)
- Trigger/reproducer: E9. Token created with `expires_days:1`; DB row set to `now()-1 day`; `GET /auth/me` -> HTTP 200.
- Expected vs actual: 401; actual: valid forever.
- Impact: advertised expiry is a false guarantee.
- Root cause: field stored, never read.
- Recommendation: fix - add `expires_at IS NULL OR expires_at > now()` to the lookup.
- Regression check: test with a past `expires_at`.
- Effort estimate: 0.5h-1h

### F-REG-06: Private-package visibility is inconsistent across endpoints (latent leak) and unreachable from the API
- Category: server-authz
- Evidence status: reproduced defect (with `visibility` set by SQL; the API cannot set it)
- Severity (user impact): medium
- Security severity: medium
- Exploitability: anonymous (versions, org page) and any registered non-member (package list).
- Location: packages.go:25-43 (any authed user gets `public`+`private`), 80-96 (ListVersions no check), orgs.go:80-103 + db.go:428 (all packages of an org listed to anonymous), packages.go:57,242 (membership of any role suffices)
- Trigger/reproducer: E11 with acme/auth set private: anon `/packages` hides it; mallory (non-member) `/packages` LISTS it; anon `/orgs/acme` lists it and members' emails; anon `/packages/acme/auth/versions` -> 200 with full version rows including `tarball_key` and sha; anon download 401 vs non-member 403 vs absent 404 reveals existence.
- Expected vs actual: a private package must be invisible (404) to non-members on every endpoint; actual: 4 different leak paths. Because Publish always creates `public` packages and no endpoint changes visibility, the feature is currently unusable and the leaks are latent - until someone adds a toggle.
- Impact: the "private package" claim would be false; private contract names/versions/manifests exposed.
- Root cause: per-handler ad-hoc checks, no shared `canRead(pkg,user)`.
- Recommendation: refactor - one authorization helper used by every package/version/org read path, plus an explicit visibility API.
- Regression check: matrix test (anon/non-member/member x every GET).
- Effort estimate: 4h-8h

### F-REG-07: Concurrent/duplicate publish deletes the published tarball (DB row left pointing at nothing)
- Category: server-storage
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: low
- Exploitability: any org owner/admin (or two honest CI jobs racing).
- Location: packages.go:162-166 (check-then-act), 177-200 (Put renames over the existing key), 216-218 (on unique-violation: `Storage.Delete(tarballKey)` removes the winner's file)
- Trigger/reproducer: E5. Two concurrent publishes of the same new version: in 10/25 (run 1) and 16/25 (run 2) iterations one got 201, the other 500, and the subsequent download returned `HTTP 404 file not found in storage` although the DB row exists and `GET /versions` lists it.
- Expected vs actual: loser gets 409 and the winner's artifact is intact; actual: published version silently corrupted/unavailable.
- Impact: data loss of a "published, immutable" artifact; also leaves a pkg row created before the tarball is validated (name squatting with zero versions).
- Root cause: file written under its final key before the DB uniqueness decision; unconditional delete in the error path.
- Recommendation: fix - write to a unique temp key, insert row inside a transaction, then `link`/rename only on success (or content-address blobs by sha).
- Regression check: parallel-publish test (N=20) asserts exactly one 201 and a downloadable artifact.
- Effort estimate: 3h-5h

### F-REG-08: Versions are not immutable and "latest" is not the highest semver
- Category: server-db
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: org owner (delete + republish); a malicious/compromised owner or stolen write+delete token.
- Location: packages.go:311-338 (hard delete), 162 (only existing rows block); db.go:~470 (`ORDER BY published_at DESC`), cmd/veld/registry.go:199-209 (`versions[0]`), models.go:ParseSemver (unused by the CLI, tolerant parsing)
- Trigger/reproducer: E3. Deleted 0.9.0 and republished 0.9.0 with different content -> 201 (new sha). Published 2.0.0 then 1.0.1: list order `[1.0.1 2.0.0 0.9.0 1.0.0]`, so `veld registry pull @acme/auth` resolves to 1.0.1. Deprecated versions are also eligible for "latest".
- Expected vs actual: npm/crates semantics (a version, once published, is never reusable; latest = highest non-prerelease); actual: pinned versions can change content; "latest" is "most recently published".
- Impact: supply-chain integrity: consumers who pinned `@org/pkg@1.0.0` get different bytes after delete+republish, and no consumer-side lock/sha exists to notice. No semver ranges are supported (only exact or latest).
- Root cause: no tombstones, no semver validation, ordering by timestamp.
- Recommendation: fix - tombstone deleted versions (name+version permanently reserved), validate semver on publish, compute latest by semver, record sha in a consumer lockfile.
- Regression check: delete-then-republish must 409; ordering test.
- Effort estimate: 4h-8h

### F-REG-09: Client integrity check is optional and self-referential; http registries are accepted without warning
- Category: client
- Evidence status: reproduced defect
- Severity (user impact): high
- Security severity: high
- Exploitability: network attacker on any `http://` registry URL; malicious or compromised registry operator; anyone able to alter response headers (misconfigured proxy/CDN).
- Location: internal/registry/client.go:208-210 (sha taken from the same response; copy error ignored), cmd/veld/registry.go:232-236 (`if remoteSHA != ""`), 37-60 (no scheme check on login), credentials.go:230 (token sent over whatever scheme)
- Trigger/reproducer: MITM proxies in section 4: stripping `X-Veld-SHA256` -> `Pulled`, malicious `veld.config.json` with `postGenerate` hook written; replacing body and header consistently -> `Pulled`. `veld registry login --registry http://localhost:18080` accepted, token stored and sent in cleartext (CLAUDE.md and help text show http examples; `ftp://` is rejected only by Go's transport).
- Expected vs actual: fail closed when the sha is absent; authenticity anchored out of band (lockfile/signature) and https required for non-loopback hosts; actual: all optional.
- Impact: tampered contracts flow straight into code generation (`.veld` models drive generated code; a pulled `veld.config.json` can carry a `postGenerate` command that runs on the next `veld generate` if it ends up as the project config, see F-REG-20). Tokens and packages travel in plaintext if the operator uses http.
- Root cause: checksum is transport-corruption detection, not a trust root.
- Recommendation: fix - require header (fail closed), refuse non-https except loopback unless `--insecure`, add a `veld.lock` with pinned sha256, and plan package signing.
- Regression check: pull against a server omitting the header must fail; login to `http://example.com` must fail.
- Effort estimate: 6h-16h (signatures larger)

### F-REG-10: Unpack has no limits, writes in place, and drops I/O errors
- Category: tarball
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: malicious package author (any registered user can publish a public package; archive content is never validated server-side) or malicious registry; victim = anyone who runs `veld registry pull`.
- Location: internal/registry/tarball.go:98-143 (see section 1 items 3-5), client.go:209
- Trigger/reproducer: section 2 table: 1.24 MiB -> 1 GiB file in 2.9 s; 1.4 MB -> 100,000 files in 4 min; partial extraction left behind; truncated file left; `MkdirAll`/`io.Copy`/`Close` errors dropped (tarball.go:130,134,139,140).
- Expected vs actual: cap entries (count, per-file, total), reject non-regular entries explicitly, extract to a temp dir and rename atomically, check all errors; actual: none.
- Impact: disk-fill/CPU denial of service on developer or CI machines; corrupt half-extracted contracts used silently; symlink-safety only by accident (item 2); stale files persist across versions.
- Root cause: minimal implementation.
- Recommendation: rewrite - small hardened extractor (regular files and dirs only, `LimitReader`, count/size caps, containment via `filepath.Rel`, temp dir + rename) shared by CLI and server validation.
- Regression check: the section 2 corpus as a test table.
- Effort estimate: 4h-8h

### F-REG-11: Stored XSS sinks in the web UI (usernames, org text, versions, deprecation text)
- Category: server-web
- Evidence status: source-supported defect (hostile values stored and returned by the API - reproduced; rendering in a browser not executed)
- Severity (user impact): high
- Security severity: high
- Exploitability: any registered user (username, org description/display_name, token names) and any publisher (version string, package name; version string is not validated). Victim = any visitor/member viewing the page.
- Location: internal/server/web/index.html:335 (`${u.username}`), 377 (`${e.message}`), 420, 456-457, 486-492, 519-524 (`v.version`, `v.deprecated`), 543-544, 564-565, 585-587 (`m.username`, `m.email`, inline `onclick="removeMember('${org.name}','${m.username}')"`), 616, 633-634; only `escHtml` uses are lines 497, 503; no CSP.
- Trigger/reproducer: E1/E12: org description `<img src=x onerror=alert(1)>` stored (psql shows it verbatim); username `<img src=x onerror=alert(document.domain)>` registered with 201; version `<svg onload=alert(1)>` accepted by the handler (on this Windows host the storage layer then rejected it with a 500 because `<>` is invalid in NTFS filenames; on Linux it would succeed - SOURCE).
- Expected vs actual: output-encode every interpolation; actual: template literals into `innerHTML`.
- Impact: script runs same-origin as the victim; the HttpOnly cookie is not readable but the script can call the API as the victim (mint admin API tokens, publish, delete, change org roles) and read the minted token back.
- Root cause: SPA built with string templates, no escaping layer, no CSP, no input constraints.
- Recommendation: rewrite - render via `textContent`/a small escaping template tag or a framework, add a strict CSP, and validate username/org/package/version charsets server-side.
- Regression check: playwright test creating hostile names and asserting no script execution; server tests for charset.
- Effort estimate: 6h-12h

### F-REG-12: No rate limiting, lockout, or TOTP replay protection
- Category: server-auth
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: anonymous.
- Location: server.go:59 (no limiter in the handler chain); auth.go:88-159; totp.go:28-38
- Trigger/reproducer: E7/E8: 60 wrong passwords in 4.5 s, all 401, no 429; 300 consecutive wrong TOTP guesses, zero 429; the same valid TOTP code accepted twice (`first -> HTTP 200, second use of same code -> HTTP 200`); with no limiter, a 6-digit code (3 valid values per 10^6 across the +-1 window) is brute-forceable online; unmeasured, only the absence of any throttle was reproduced (and F-REG-01 makes the second factor moot anyway).
- Impact: online password/TOTP guessing and credential stuffing; registration spam (open, unverified).
- Root cause: absent middleware.
- Recommendation: fix - per-IP and per-account limiter with backoff, one-time TOTP counter storage.
- Regression check: burst test expects 429; replay test expects 401.
- Effort estimate: 4h-6h

### F-REG-13: No body-size limits and no server-side archive validation on publish
- Category: server-storage
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: any org owner/admin (publish); anonymous for JSON endpoints.
- Location: packages.go:111 (`ParseMultipartForm(32<<20)` is only the in-memory threshold, the comment says "32 MB max"), 169-200 (streams any bytes to disk, no size cap, no gzip/tar check); auth.go:39 etc. (`json.NewDecoder(r.Body)` unbounded); server.go:59-62
- Trigger/reproducer: E13: 200 MiB of non-archive bytes published as a version -> 201 (size 209,715,200 recorded), taking 2.5 s; 60 MiB JSON login body processed (401). Multipart spill also writes a temp copy in `$TMP`, doubling disk use. A client-side `Publish` buffers the full tarball in memory (client.go:148-159).
- Impact: disk exhaustion, memory pressure; registry can host arbitrary blobs; invalid archives are served to consumers who then hit F-REG-10.
- Root cause: `ParseMultipartForm` misread as a limit; no validation stage.
- Recommendation: fix - `http.MaxBytesReader` (global and per route), validate the tarball with the hardened extractor before accepting, per-user quotas.
- Regression check: oversize and non-tar uploads -> 413/400.
- Effort estimate: 3h-5h

### F-REG-14: Sessions cannot be revoked; no purpose/typ on JWTs; 7-day lifetime
- Category: server-auth
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: anyone holding a stolen JWT or cookie.
- Location: auth.go:162-171 (Logout only clears cookie), token.go:56-92, middleware.go:70-100
- Trigger/reproducer: E10: JWT used after `POST /auth/logout` -> `/auth/me` HTTP 200. No password-change/reset endpoint exists at all (no way to invalidate sessions or recover an account; no password reset flow).
- Impact: stolen session valid until expiry (7 days); no account recovery.
- Root cause: stateless JWTs only.
- Recommendation: refactor - server-side session table (or token-version column) checked in middleware; shorter TTL.
- Regression check: logout/password-change invalidates old tokens.
- Effort estimate: 4h-8h

### F-REG-15: Transport/browser hardening absent (no TLS, cookie not Secure, CORS `*`, no security headers)
- Category: server-web
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: network attacker where deployed without a TLS terminator; any website for CORS (no credentials, so limited).
- Location: server.go:56-66 (`ListenAndServe`, no TLS option), auth.go:372-381, server.go:150-163
- Trigger/reproducer: E14: `Set-Cookie veld_session ... HttpOnly=true Secure=false SameSite=Lax`; preflight `ACAO="*"`; `CSP="" XFO="" XCTO="" HSTS=""`. serve.go prints `http://localhost<addr>` as the URL.
- Impact: session cookie and tokens in cleartext on http deployments; clickjacking; the SPA can be framed.
- Root cause: server assumes a reverse proxy but nothing documents or enforces it.
- Recommendation: fix - document proxy requirement, add `Secure` when `base_url` is https, add headers + CSP, restrict CORS to configured origins.
- Regression check: header assertions in a handler test.
- Effort estimate: 2h-4h

### F-REG-16: Registry selection is nondeterministic and credentials are keyed by raw string (dependency-confusion exposure)
- Category: credentials
- Evidence status: reproduced defect
- Severity (user impact): medium
- Security severity: medium
- Exploitability: user who has logged in to more than one registry (or two spellings of one).
- Location: internal/registry/credentials.go:259-268 (`DefaultRegistry` returns the first key of a Go map), :230-236 (exact-string key), client.go:31-43; used by `pull` (registry.go:187-196), `versions`, `token create`
- Trigger/reproducer: three entries (`http://127.0.0.1:18080`, `http://localhost:18080`, `http://localhost:18080/`); 12 runs of `veld registry info` printed 9x/2x/1x different "default" registries.
- Expected vs actual: deterministic resolution from project config (`registry.url`) or an explicit default; actual: pull of `@org/name` without `--registry` may resolve against a random logged-in registry, and `push` uses `rc.Registry.URL` only if set, else the random default. `veld.config.json` has no per-scope registry mapping (as in `.npmrc` `@scope:registry=`), so a name in a public registry can shadow a private one; there is no way to pin a scope to a registry. The token for registry X is only sent to X (client.go builds the URL from the chosen BaseURL), so no cross-registry token leak was found.
- Impact: wrong-registry resolution; dependency confusion for private scopes.
- Root cause: credential store doubles as configuration; no scope->registry mapping.
- Recommendation: refactor - explicit `defaultRegistry` plus `scopes` mapping, normalised URL keys.
- Regression check: multi-registry test with a fixed order.
- Effort estimate: 3h-6h

### F-REG-17: Credentials stored in plaintext; file permissions are not enforced on Windows
- Category: credentials
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: low
- Exploitability: local user/process with read access to the profile; backup tools.
- Location: credentials.go:182-188 (`~/.veld/credentials.json`), 214-227 (`WriteFile(..., 0600)`, dir 0700)
- Trigger/reproducer: file content `{"registries":{"http://localhost:18080":{"token":"vtk_DYPN...","username":"cli5487"}}}`; `icacls` on Windows shows inherited ACEs for other principals (e.g. a sandbox user group) - mode bits are meaningless there. Write is not atomic (no temp+rename) and read-modify-write is unlocked (concurrent logins can lose entries). On Unix 0600 applies only on creation (an existing looser file keeps its mode).
- Impact: standard "plaintext dotfile" exposure; acceptable on Unix, weaker on Windows; no OS keychain integration; `VELD_TOKEN`-style env override does not exist for CI.
- Root cause: simple file store.
- Recommendation: fix - keep file but chmod after write, atomic write, optional env token, consider keychain later.
- Regression check: perms test on Unix.
- Effort estimate: 2h-4h

### F-REG-18: `Pack` publishes more than the package and ignores errors
- Category: tarball
- Evidence status: source-supported defect
- Severity (user impact): low
- Security severity: low
- Exploitability: the person running `push` (accidental disclosure); symlinks planted in a repo by a third party.
- Location: tarball.go:20,22-34,63-80
- Trigger/reproducer: SOURCE only (not executed): any `*.veld` / `veld.config.json` at any depth under the config dir is included (nested projects, vendored `veld/packages/**` pulled from other orgs, test fixtures); a file symlink `x.veld -> ~/.ssh/id_rsa` would upload the target's content; `veld.config.json` (hooks, baseUrl, internal hostnames) always uploaded; `WriteHeader`/`io.Copy`/`Close` errors dropped; unreadable files silently skipped so a partial package can be published; non-deterministic bytes (mtime).
- Impact: accidental leakage and incomplete/corrupt packages; reproducible-build property lost.
- Root cause: dead `patterns` slice; no package manifest listing files.
- Recommendation: fix - explicit file list from the package manifest, `Lstat` and skip non-regular files, zero ModTime, check errors.
- Regression check: pack test with symlink, nested and unreadable files.
- Effort estimate: 2h-4h

### F-REG-19: Email handling: SMTP proceeds without TLS, verification never enforced, token logged, HTML injection
- Category: server-auth
- Evidence status: source-supported defect (SMTP not exercised; not configured)
- Severity (user impact): low
- Security severity: low
- Exploitability: network attacker between server and SMTP host; log readers; any user (username in email).
- Location: email.go:97-106 (STARTTLS optional, then `loginAuth` has no TLS requirement: lines 33-35 vs stdlib PlainAuth), email.go:68 (To header from user input; `Rcpt` rejects CR/LF so injection is blocked in practice), auth.go:363 (full verify URL with token logged), auth.go:43-50 (email format unvalidated), email.go template `fmt.Sprintf` with raw username (HTML injection into the email), whole `email_verified` flag unused for authorization (REPRODUCED: unverified account publishes).
- Impact: SMTP credentials can be sent in cleartext after a stripped STARTTLS; account verification is cosmetic, so spam/squatting is unconstrained.
- Root cause: copy-paste mail code, verification is not a gate.
- Recommendation: fix - require TLS before AUTH (or implicit TLS), strip tokens from logs, escape template values, gate publishing/org creation on `email_verified` when SMTP is configured.
- Regression check: fake SMTP server that withholds STARTTLS.
- Effort estimate: 3h-5h

### F-REG-20: Pull writes untrusted files into the project and can overwrite existing files; pulled `veld.config.json` can carry a hook
- Category: client
- Evidence status: suspected risk (the write was reproduced via MITM; the hook executing was not run)
- Severity (user impact): medium
- Security severity: medium
- Exploitability: malicious package author/registry; requires the user to pass `--out` pointing at the project (e.g. `--out veld`) or otherwise to use the pulled config.
- Location: cmd/veld/registry.go:211-216,238 (default out `veld/packages/@org/name`, `--out` unrestricted, `os.Create` overwrite); `hooks.postGenerate` in config (CLAUDE.md config section)
- Trigger/reproducer: MITM proxy output wrote `veld.config.json {"input":"app.veld","hooks":{"postGenerate":"echo PWNED > pwned.txt"}}` into `veld\packages\@org\hello\`. With the default location the config is not auto-detected (CLAUDE.md lists only `./veld.config.json` and `./veld/veld.config.json`); with `--out veld` it would overwrite the real project config.
- Impact: code execution on next `veld generate` in that misuse; and any `.veld` content is imported into code generation.
- Root cause: packages ship config files; extractor overwrites silently.
- Recommendation: fix - exclude `veld.config.json` from packages (or ignore it), refuse overwrite without `--force`, refuse `--out` that equals/contains the project root.
- Regression check: pull test with a config entry.
- Effort estimate: 2h-3h

### F-REG-21: Operational gaps: no health endpoint, SIGTERM unhandled, no schema versioning, no TLS, no backups, zero tests
- Category: ops
- Evidence status: reproduced defect (health, migrations table, restart), source-supported (signals)
- Severity (user impact): medium
- Security severity: low
- Exploitability: operator.
- Location: server.go:84-140 (catch-all route serves SPA for `/healthz`: REPRODUCED 200 text/html 84 KB), serve.go:138 (`os.Interrupt` only), db.go:43-128 (no `schema_migrations`; REPRODUCED table list), storage.go (local FS only; DB row and file are not reconciled), `find internal/server internal/registry -name '*_test.go'` -> none, CLAUDE.md "Registry Server" section (no backup/restore, no HA)
- Trigger/reproducer: section 4. Restart re-ran the schema silently and data persisted (good); but a column change in the future cannot be versioned, and `ADD COLUMN IF NOT EXISTS` migrations run on every start by whichever instance starts first.
- Impact: orchestrators cannot probe readiness; `docker stop` kills in-flight uploads; upgrades are risky; no test net for any finding above.
- Root cause: prototype-level server.
- Recommendation: refactor - `/healthz` + `/readyz`, SIGTERM, versioned migrations, tests, documented backup of PG + blob store.
- Regression check: container smoke test with `docker stop`.
- Effort estimate: 6h-12h

### F-REG-22: Documentation and config mismatches
- Category: config
- Evidence status: documentation mismatch
- Severity (user impact): low
- Security severity: low
- Exploitability: n/a
- Location: CLAUDE.md lines 54-58, 392-399 (`veld login|push|pull|serve`, `--secret mysecret`); cmd/veld/main.go:99 (only `veld registry ...` exists); serve.go:97-108; server.go:38 vs CLAUDE.md (secret "32+ chars"); registry.config.example.json (`secret` is a guessable placeholder); `RegistryConfig.Enabled` in config.go:97 is never read (`grep` finds no use)
- Trigger/reproducer: `veld login` / `veld serve` -> `unknown command`; `--secret mysecret` (8 chars) is rejected by `server.New` (min 16); BaseURL has no flag and SMTP port no env var, contrary to "flags > env > file".
- Impact: operators copy the example secret or hit errors.
- Root cause: docs not synced with code.
- Recommendation: fix - align docs, make the example secret invalid (empty) so startup fails, enforce >=32.
- Regression check: doc test running the documented commands.
- Effort estimate: 1h-2h

### F-REG-23: Member emails and user IDs exposed to anonymous callers
- Category: server-authz
- Evidence status: reproduced defect
- Severity (user impact): low
- Security severity: low
- Exploitability: anonymous.
- Location: orgs.go:80-103, db.go:323-340 (`ListOrgMembers` returns email), models.go OrgMember json tags
- Trigger/reproducer: E11: `GET /api/v1/orgs/acme9075` unauthenticated -> `hasEmail=true`. Anonymous `GET /orgs` also lists every org, and `/stats` is public.
- Impact: PII harvesting; enumeration of orgs.
- Root cause: shared DTO.
- Recommendation: fix - drop `email` from public DTOs (show to org admins only).
- Regression check: anon response must not contain `@`.
- Effort estimate: 1h

---------------------------------------------------------------------------------------------------

## 6. Threat-model summary

- Malicious package author (any registered user): can publish arbitrary bytes under their org (F-REG-13), write/overwrite/delete `*.tar.gz` outside the store and break other orgs (F-REG-02), plant stored XSS (F-REG-11), and ship decompression-bomb or many-file archives and config-with-hook files that hit consumers (F-REG-10, F-REG-20). Symlink/traversal tricks in the archive itself are neutralised by the current extractor (section 2), but by accident.
- Compromised token: scopes are decorative (F-REG-03); a read token upgrades to admin; expiry ignored (F-REG-05); a stolen JWT cannot be revoked (F-REG-14); the partial-2FA token is a full session (F-REG-01). Token storage itself is good (256-bit random, stored as sha256).
- Malicious registry operator / compromised registry: can serve any bytes and any `X-Veld-SHA256` (or none); the client has no anchor (lockfile/signature) (F-REG-09); can re-publish a version after deletion (F-REG-08). Operator also holds all tokens in transit.
- Network attacker: http registries are accepted without warning, tokens and packages travel in the clear, no cookie `Secure` flag (F-REG-09, F-REG-15); SMTP may downgrade (F-REG-19).
- Decompression bomb / resource exhaustion: reproduced on pull (1 GiB from 1.2 MiB; 100k files) and on publish (200 MiB unvalidated blob; no body limits) (F-REG-10, F-REG-13).
- Secret leakage: verification tokens logged (F-REG-19); `--secret` on the command line is visible in process listings; example config contains a known secret (F-REG-22); credentials plaintext (F-REG-17). No token appears in request logs; error messages from the client do not echo tokens.
- Private package enumeration: latent only, because private packages cannot be created through the API; but 4 inconsistent read paths would leak them (F-REG-06), plus anonymous member email listing (F-REG-23) and existence oracle via 401/403/404.

## 7. Assessment

- Server: not production-ready. Two high-severity, trivially reproducible auth/storage defects (F-REG-01, -02, -03), no abuse controls, no tests. The architecture (PostgreSQL + blob store + embedded SPA) is reasonable, but the implementation is a prototype. Recommend refactor-with-hardening of the core (auth, authz helper, storage keyed by id/sha, migrations) rather than patching individual handlers; the SPA should be rewritten or at least retrofitted with escaping + CSP.
- Client: small and understandable. `Unpack` should be rewritten (F-REG-10), verification made mandatory and anchored (lockfile, F-REG-09), registry selection made explicit (F-REG-16).
- Same binary? The registry belongs in a separate binary (e.g. `veld-registry`) or repo: it brings `lib/pq`, `x/crypto`, an HTTP server, SMTP and a web UI into every developer's CLI, enlarging its attack surface and supply chain, while the CLI only needs the ~500-line client. CLAUDE.md already states the two-binary rule differently (it counts `veld serve` inside the CLI); the code matches the docs but the separation would be cleaner.
