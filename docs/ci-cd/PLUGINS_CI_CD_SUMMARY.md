# ✅ Plugins CI/CD Setup Complete!

## What's Been Done

### 1. ✅ LICENSE Files Added
- `editors/vscode/LICENSE` - MIT License for VS Code extension
- `editors/jetbrains/LICENSE` - MIT License for JetBrains plugin

### 2. ✅ CI/CD Workflow Created
- `.github/workflows/publish-plugins.yml` - Automatically publishes both plugins on version tag push

### 3. ✅ Configuration Updates
- Updated VS Code `package.json` repository URL to match your GitHub repo
- Both plugins are ready for automated publishing

## How It Works

When you push a version tag:
```bash
git tag v0.2.0
git push origin v0.2.0
```

The workflow automatically:
1. ✅ Extracts version from tag
2. ✅ Updates version in plugin files
3. ✅ Builds VS Code extension
4. ✅ Builds JetBrains plugin
5. ✅ Publishes to VS Code Marketplace
6. ✅ Publishes to JetBrains Plugin Repository

## Setup Required

### Step 1: VS Code Marketplace

1. **Get Azure DevOps Token:**
   - Go to https://dev.azure.com
   - Create/Login to account
   - Create organization (e.g., `maayn`)
   - Go to Personal Access Tokens
   - Create token with **Marketplace (Manage)** scope
   - Copy the token

2. **Add to GitHub Secrets:**
   - Repo → Settings → Secrets → Actions
   - New secret: `VSCE_PAT`
   - Paste your Azure DevOps token

3. **Verify Publisher:**
   - Current publisher in `package.json`: `veld-dev`
   - If you want to change it, update `editors/vscode/package.json`
   - Create publisher at: https://marketplace.visualstudio.com/manage

### Step 2: JetBrains Marketplace

1. **Get JetBrains Token:**
   - Go to https://account.jetbrains.com/
   - Sign in/up
   - Go to https://plugins.jetbrains.com/author/me/tokens
   - Generate token with **Plugin Repository** scope
   - Copy the token

2. **Add to GitHub Secrets:**
   - Repo → Settings → Secrets → Actions
   - New secret: `JETBRAINS_PUBLISH_TOKEN`
   - Paste your JetBrains token

### Step 3: (Optional) Plugin Signing

If you want to sign your JetBrains plugin:

```bash
# Generate certificate
openssl genrsa -out private.key 4096
openssl req -new -x509 -key private.key -out certificate.crt -days 365
```

Add these GitHub secrets:
- `JETBRAINS_CERT_CHAIN` - Contents of `certificate.crt`
- `JETBRAINS_PRIVATE_KEY` - Contents of `private.key`
- `JETBRAINS_KEY_PASSWORD` - Password (if you set one)

## Testing

### Test VS Code Extension Locally:
```bash
cd editors/vscode
npm install
npm run compile
npm run package
code --install-extension veld-vscode-0.2.0.vsix
```

### Test JetBrains Plugin Locally:
```bash
cd editors/jetbrains
./gradlew buildPlugin
./gradlew runIde  # Opens IDE with plugin
```

## Publishing Your First Release

Once secrets are set up:

```bash
# 1. Make your changes
git add .
git commit -m "feat: new features"

# 2. Create and push version tag
git tag v0.2.0
git push origin v0.2.0
```

The CI/CD will automatically:
- ✅ Build both plugins
- ✅ Publish to VS Code Marketplace
- ✅ Publish to JetBrains Plugin Repository

## What Gets Published

**VS Code Extension:**
- Package: `veld-vscode-{version}.vsix`
- Marketplace: https://marketplace.visualstudio.com/
- Install: `code --install-extension veld-dev.veld-vscode`

**JetBrains Plugin:**
- Package: `veld-jetbrains-{version}.zip`
- Marketplace: https://plugins.jetbrains.com/
- Install: IDE → Settings → Plugins → Marketplace → Search "Veld"

## Important Notes

1. **Pre-releases are skipped**: Tags like `v0.2.0-beta.1` won't publish
2. **First JetBrains publication**: Requires manual review (1-3 days)
3. **VS Code**: Usually appears within minutes
4. **Version conflicts**: If version already exists, increment the version

## Troubleshooting

See `.github/PLUGINS_SETUP.md` for detailed troubleshooting guide.

## Summary

✅ LICENSE files added to both plugins
✅ CI/CD workflow created
✅ Configuration updated
⏳ **Next**: Set up secrets and push a version tag!

**You're all set!** 🚀
