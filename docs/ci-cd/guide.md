# Veld CI/CD Guide

## Overview — Two Workflows

You have **two** GitHub Actions workflows:

| Workflow | File | When it runs |
|----------|------|-------------|
| **CI** | `.github/workflows/ci.yml` | Every push to `master`/`main` and every pull request |
| **Release** | `.github/workflows/release.yml` | Only when you push a **version tag** like `v0.1.0` |

---

## When does each workflow run?

### CI (`ci.yml`) — Automatic on every push

Runs automatically when you:
```bash
git push origin master          # push to master
git push origin my-feature      # push a PR branch
```

What it does:
1. Builds all Go packages (`go build ./...`)
2. Runs static analysis (`go vet ./...`)
3. Runs all tests (`go test ./... -race`)
4. Cross-compiles for all 5 platforms (just to verify it compiles)

**No secrets needed.** This workflow is read-only.

---

### Release (`release.yml`) — Only on version tags

Runs **only** when you push a tag starting with `v`:

```bash
# Step 1: Commit your work
git add -A
git commit -m "feat: new feature"

# Step 2: Tag it with a version
git tag v0.2.0

# Step 3: Push the tag (this triggers the release)
git push origin v0.2.0
```

What it does (6 jobs, in order):

```
1. Test           → runs go test, go vet (fails fast if broken)
       ↓
2. Build (×5)     → cross-compiles for linux/darwin/windows × amd64/arm64
       ↓
3. Release        → creates GitHub Release with archives + checksums + changelog
       ↓  ↓  ↓
4. npm  5. pip  6. Homebrew   → publishes to package managers (parallel)
```

---

## The 3 Secrets — Where to get them

### Secret 1: `GITHUB_TOKEN` — **You already have this. Do nothing.**

This is automatically provided by GitHub Actions. It's used to create the
GitHub Release and upload the binary archives. No setup needed whatsoever.

---

### Secret 2: `NPM_TOKEN` — For publishing to npmjs.com

**Only needed if you want `npm install veld` to work.**  
If you don't plan to publish to npm yet, the release will still work — the
npm publish step will just fail silently (it won't block the release).

**How to get it:**

1. Go to [npmjs.com](https://www.npmjs.com/) → Sign up or log in
2. Click your avatar (top right) → **Access Tokens**
3. Click **Generate New Token** → choose **Automation** type
4. Copy the token (starts with `npm_...`)
5. Go to your GitHub repo → **Settings** → **Secrets and variables** → **Actions**
6. Click **New repository secret**
   - Name: `NPM_TOKEN`
   - Value: paste the token
7. Click **Add secret**

---

### Secret 3: `PYPI_TOKEN` — For publishing to pypi.org

**Only needed if you want `pip install veld` to work.**  
Same as npm — if you skip this, the pip publish step just fails, release still works.

**How to get it:**

1. Go to [pypi.org](https://pypi.org/) → Register or log in
2. Go to **Account settings** → scroll to **API tokens**
3. Click **Add API token**
   - Token name: `veld-github-actions`
   - Scope: **Entire account** (or project-scoped after first publish)
4. Copy the token (starts with `pypi-...`)
5. Go to your GitHub repo → **Settings** → **Secrets and variables** → **Actions**
6. Click **New repository secret**
   - Name: `PYPI_TOKEN`
   - Value: paste the token
7. Click **Add secret**

---

### Secret 4 (Optional): `HOMEBREW_TAP_TOKEN` — For Homebrew tap

**Only needed if you set up a separate `homebrew-tap` repository.**  
Currently commented out in the workflow. The Homebrew formula update step
just prints the updated formula — it doesn't push anywhere yet.

To enable it later:
1. Create a repo called `veld-dev/homebrew-tap`
2. Create a GitHub Personal Access Token (PAT) with `repo` scope
3. Add it as `HOMEBREW_TAP_TOKEN` secret

---

## Adding secrets to your GitHub repo (step-by-step)

```
GitHub repo page
  → Settings (top tab)
    → Secrets and variables (left sidebar)
      → Actions
        → New repository secret
```

That's it. You add the name and value, click save. The workflow reads them
automatically via `${{ secrets.NPM_TOKEN }}` etc.

---

## What happens when you push a tag — full timeline

```
You run:  git tag v0.3.0 && git push origin v0.3.0

  0:00  GitHub sees the v* tag push → triggers release.yml
  0:05  Job 1 (Test): go test, go vet
  0:30  Job 2 (Build): 5 parallel jobs compile for each platform
        → linux-amd64, linux-arm64, darwin-amd64, darwin-arm64, windows-amd64
        → each produces a .tar.gz or .zip archive
  1:30  Job 3 (Release):
        → downloads all 5 archives
        → generates SHA256 checksums
        → auto-generates changelog from git commits since last tag
        → creates GitHub Release at github.com/veld-dev/veld/releases
        → uploads: veld-linux-amd64.tar.gz, veld-darwin-arm64.tar.gz, etc.
  2:00  Jobs 4-6 (parallel):
        → npm publish (if NPM_TOKEN is set)
        → pip publish (if PYPI_TOKEN is set)
        → Homebrew formula update
  ~3:00 Done ✓
```

Total time: ~2-3 minutes.

---

## Pre-release versions

If your tag has a hyphen, it's treated as a pre-release:

```bash
git tag v0.3.0-beta.1
git push origin v0.3.0-beta.1
```

Pre-releases:
- ✅ Still build binaries and create a GitHub Release
- ✅ Marked as "Pre-release" on the releases page
- ❌ Do NOT publish to npm, pip, or Homebrew

---

## What if I don't have the secrets yet?

**That's fine.** The release workflow is designed to be resilient:

- `GITHUB_TOKEN` — automatic, always works
- `NPM_TOKEN` missing → Job 4 fails, but Jobs 1-3 succeed (you still get the GitHub Release)
- `PYPI_TOKEN` missing → Job 5 fails, but everything else works
- `HOMEBREW_TAP_TOKEN` missing → Job 6 just prints the formula (no push)

You can add secrets later and re-run the release, or just push a new tag.

---

## Quick start — your first release

```bash
# 1. Make sure everything is committed
git add -A
git commit -m "release: v0.1.0"

# 2. Tag it
git tag v0.1.0

# 3. Push everything
git push origin master
git push origin v0.1.0

# 4. Watch it at: https://github.com/veld-dev/veld/actions
```

After ~3 minutes, check `https://github.com/veld-dev/veld/releases` — you'll
see your release with all binaries attached.

