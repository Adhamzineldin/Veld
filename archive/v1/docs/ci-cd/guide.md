# Veld CI/CD Guide

## Overview — Two Workflows

| Workflow | File | When it runs |
|----------|------|-------------|
| **CI** | `.github/workflows/ci.yml` | Every push to `master`/`main` and every pull request |
| **Release** | `.github/workflows/release.yml` | Push a **version tag** (`v0.1.0`) **OR** manual dispatch from Actions UI |

---

## How does the Release workflow decide what to publish?

### Automatic mode (tag push)

When you push a tag, the workflow **diffs the files changed** since the last
tag and only publishes what was actually touched:

| Files changed | What gets published |
|---|---|
| `cmd/` or `internal/` or `go.mod` | Binary + npm + pip + Homebrew *(cascading)* |
| `packages/npm/` | npm |
| `packages/pip/` | pip |
| `packages/homebrew/` | Homebrew |
| `packages/composer/` | Composer |
| `editors/vscode/` | VS Code extension |
| `editors/jetbrains/` | JetBrains plugin |

**Cascading rule:** If the Go binary source changes, npm/pip/Homebrew are also
published because they bundle the binary.

**First release:** If there's no previous tag, everything is published.

### Manual mode (workflow dispatch)

Go to **Actions → Release → Run workflow** and you get checkboxes:

- ☑ **Publish everything** — overrides all checkboxes
- ☑ **Build & release binary** — GitHub Release with cross-compiled archives
- ☐ **Publish npm package**
- ☐ **Publish pip package**
- ☐ **Update Homebrew formula**
- ☐ **Publish VS Code extension**
- ☐ **Publish JetBrains plugin**

Plus a **version** field (e.g. `v0.3.0`).

### Pre-release tags

Tags with a hyphen (e.g. `v0.3.0-beta.1`) only create a GitHub Release.
All package/plugin publishing is skipped. Manual dispatch can override this.

---

## Pipeline overview

```
Tag push: git tag v0.3.0 && git push origin v0.3.0
                    ↓
  ┌──── Resolve ────────────────────────────────────────────────┐
  │  Compare files changed since last tag                       │
  │  Output: which targets to publish (binary, npm, vscode...)  │
  └──────────────┬──────────────────────────────────────────────┘
                 ↓
  ┌──── Test ───────┐
  │  go test, go vet │
  └───────┬─────────┘
          ↓
  ┌──── Build (×5) ───────────────────────────────┐  (if binary=true)
  │  linux/amd64, linux/arm64, darwin/amd64,       │
  │  darwin/arm64, windows/amd64                    │
  └───────┬────────────────────────────────────────┘
          ↓
  ┌──── Release ────────────────────────────────────┐
  │  GitHub Release with archives + checksums       │
  └───┬──────┬──────┬───────────────────────────────┘
      ↓      ↓      ↓
   npm    pip    Homebrew    (if their flag is true)
                 
  ┌──── VS Code ────┐  ┌──── JetBrains ────┐
  │  (if vscode=true)│  │  (if jetbrains=true)│
  └─────────────────┘  └────────────────────┘
```

---

## Quick start — your first release

```bash
# 1. Commit everything
git add -A && git commit -m "release: v0.1.0"

# 2. Tag it
git tag v0.1.0

# 3. Push
git push origin master && git push origin v0.1.0

# 4. Watch: https://github.com/<you>/veld/actions
```

The workflow auto-detects what changed and publishes only those targets.

---

## Manual publish (e.g. only update the VS Code plugin)

1. Go to **Actions → Release → Run workflow**
2. Enter version: `v0.3.0`
3. Uncheck **Build & release binary** (already done)
4. Check **Publish VS Code extension**
5. Click **Run workflow**

---

## The Secrets — Where to get them

| Secret | For | How to get it |
|--------|-----|--------------|
| `GITHUB_TOKEN` | GitHub Release | **Automatic** — already provided |
| `NPM_TOKEN` | npm publish | [npmjs.com](https://npmjs.com) → Access Tokens → Automation |
| `PYPI_TOKEN` | pip publish | [pypi.org](https://pypi.org) → Account → API tokens |
| `VSCE_PAT` | VS Code Marketplace | [dev.azure.com](https://dev.azure.com) → Personal Access Tokens (Marketplace scope) |
| `JETBRAINS_PUBLISH_TOKEN` | JetBrains Marketplace | [plugins.jetbrains.com/author/me/tokens](https://plugins.jetbrains.com/author/me/tokens) |

Missing secrets are fine — those jobs just fail, the rest still succeeds.

---

## CI (`ci.yml`) — Automatic on every push

Runs on every push to master/main and every PR:
1. `go build ./...`
2. `go vet ./...`
3. `go test ./... -race`
4. Cross-compiles for 5 platforms (just to verify)

No secrets needed. Read-only.
