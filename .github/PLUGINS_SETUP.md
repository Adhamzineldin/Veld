# Plugin Publishing Setup Guide

This guide explains how to set up automatic publishing for VS Code and JetBrains plugins.

## Overview

When you push a version tag (e.g., `v0.2.0`), the CI/CD pipeline will automatically:
1. ✅ Build and publish the VS Code extension to the Marketplace
2. ✅ Build and publish the JetBrains plugin to the Plugin Repository

## Prerequisites

### VS Code Extension

1. **Create Azure DevOps Account** (if you don't have one):
   - Go to https://dev.azure.com
   - Sign in with your Microsoft account (or create one)
   - Create an organization (e.g., `maayn` or your name)

2. **Create Personal Access Token**:
   - Go to https://dev.azure.com/{your-org}
   - Click your profile → **Personal Access Tokens**
   - Click **New Token**
   - Name: "VS Code Extension Publishing"
   - Organization: **All accessible organizations**
   - Expiration: Set to your preference (or never expire)
   - Scopes: **Marketplace (Manage)** ✅
   - Click **Create**
   - **Copy the token immediately** (you won't see it again!)

3. **Add Secret to GitHub**:
   - Go to your GitHub repo → **Settings** → **Secrets and variables** → **Actions**
   - Click **New repository secret**
   - Name: `VSCE_PAT`
   - Value: Paste your Azure DevOps token
   - Click **Add secret**

4. **Verify Publisher**:
   - The publisher in `editors/vscode/package.json` should match your Azure DevOps organization
   - Current: `"publisher": "veld-dev"`
   - If you want to change it, update `package.json` and create the publisher at:
     https://marketplace.visualstudio.com/manage

### JetBrains Plugin

1. **Create JetBrains Account** (if you don't have one):
   - Go to https://account.jetbrains.com/
   - Sign up or sign in

2. **Generate Marketplace Token**:
   - Go to https://plugins.jetbrains.com/author/me/tokens
   - Click **Generate New Token**
   - Name: "Plugin Publishing"
   - Scope: **Plugin Repository** ✅
   - Click **Generate**
   - **Copy the token immediately** (you won't see it again!)

3. **Add Secret to GitHub**:
   - Go to your GitHub repo → **Settings** → **Secrets and variables** → **Actions**
   - Click **New repository secret**
   - Name: `JETBRAINS_PUBLISH_TOKEN`
   - Value: Paste your JetBrains token
   - Click **Add secret**

4. **Plugin Signing (Optional but Recommended)**:
   - Signing verifies the plugin's authenticity
   - If you want to sign your plugin, generate a certificate:
     ```bash
     # Generate private key
     openssl genrsa -out private.key 4096
     
     # Generate certificate
     openssl req -new -x509 -key private.key -out certificate.crt -days 365
     ```
   - Add these as GitHub secrets:
     - `JETBRAINS_CERT_CHAIN` - Contents of `certificate.crt`
     - `JETBRAINS_PRIVATE_KEY` - Contents of `private.key`
     - `JETBRAINS_KEY_PASSWORD` - Password you used (if any)

## How It Works

### Triggering a Release

```bash
# 1. Make your changes and commit
git add .
git commit -m "feat: new features"

# 2. Create and push a version tag
git tag v0.2.0
git push origin v0.2.0
```

The workflow will automatically:
1. Extract version from tag (`v0.2.0` → `0.2.0`)
2. Update version in plugin files
3. Build the plugins
4. Publish to both marketplaces

### Version Numbering

- Use semantic versioning: `MAJOR.MINOR.PATCH`
- Examples: `v0.1.0`, `v0.2.0`, `v1.0.0`
- Pre-releases (e.g., `v0.2.0-beta.1`) will **skip** publishing

### What Gets Published

**VS Code Extension:**
- Package: `veld-vscode-{version}.vsix`
- Published to: https://marketplace.visualstudio.com/
- Install command: `code --install-extension veld-dev.veld-vscode`

**JetBrains Plugin:**
- Package: `veld-jetbrains-{version}.zip`
- Published to: https://plugins.jetbrains.com/
- Install via: IDE → Settings → Plugins → Marketplace

## Testing Before Publishing

### Test VS Code Extension Locally

```bash
cd editors/vscode
npm install
npm run compile
npm run package
code --install-extension veld-vscode-0.2.0.vsix
```

### Test JetBrains Plugin Locally

```bash
cd editors/jetbrains
./gradlew buildPlugin
./gradlew runIde  # Opens IDE with plugin installed
```

## Troubleshooting

### VS Code: "Authentication failed"
- Verify `VSCE_PAT` secret is set correctly
- Make sure token has **Marketplace (Manage)** scope
- Token might have expired - generate a new one

### VS Code: "Extension already exists"
- The version number is already published
- Increment the version in the tag (e.g., `v0.2.1`)

### JetBrains: "Invalid token"
- Verify `JETBRAINS_PUBLISH_TOKEN` secret is set correctly
- Make sure token has **Plugin Repository** scope
- Token might have expired - generate a new one

### JetBrains: "Plugin verification failed"
- Check the workflow logs for specific errors
- Run `./gradlew verifyPlugin` locally to debug
- Make sure `plugin.xml` is valid

### Plugin not appearing in marketplace
- **VS Code**: Usually appears within minutes
- **JetBrains**: First publication requires manual review (1-3 days)
- Check your publisher dashboard:
  - VS Code: https://marketplace.visualstudio.com/manage
  - JetBrains: https://plugins.jetbrains.com/author/me

## Manual Publishing (Fallback)

If CI/CD fails, you can publish manually:

### VS Code
```bash
cd editors/vscode
npm install
npm run compile
npx @vscode/vsce publish -p $VSCE_PAT
```

### JetBrains
```bash
cd editors/jetbrains
export ORG_GRADLE_PROJECT_intellijPublishToken=$JETBRAINS_TOKEN
./gradlew publishPlugin
```

## Secrets Summary

Add these secrets to GitHub (Settings → Secrets → Actions):

| Secret Name | Description | Where to Get It |
|------------|-------------|-----------------|
| `VSCE_PAT` | Azure DevOps token for VS Code Marketplace | https://dev.azure.com → Personal Access Tokens |
| `JETBRAINS_PUBLISH_TOKEN` | JetBrains Marketplace token | https://plugins.jetbrains.com/author/me/tokens |
| `JETBRAINS_CERT_CHAIN` | Certificate chain (optional) | Generated via OpenSSL |
| `JETBRAINS_PRIVATE_KEY` | Private key (optional) | Generated via OpenSSL |
| `JETBRAINS_KEY_PASSWORD` | Key password (optional) | Your chosen password |

## Next Steps

1. ✅ Set up Azure DevOps account and get `VSCE_PAT`
2. ✅ Set up JetBrains account and get `JETBRAINS_PUBLISH_TOKEN`
3. ✅ Add secrets to GitHub
4. ✅ (Optional) Set up plugin signing
5. ✅ Test locally
6. ✅ Push a version tag: `git tag v0.2.0 && git push origin v0.2.0`

**That's it!** The plugins will automatically publish to both marketplaces. 🚀
