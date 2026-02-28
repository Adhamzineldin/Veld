# Publishing the Veld VS Code Extension

## Prerequisites

1. **Install vsce** (VS Code Extension Manager):
   ```bash
   npm install -g @vscode/vsce
   ```

2. **Create a Personal Access Token**:
   - Go to https://dev.azure.com/veld-dev
   - User Settings → Personal Access Tokens → New Token
   - Name: "VS Code Extension Publishing"
   - Organization: All accessible organizations
   - Scopes: **Marketplace (Manage)**
   - Copy the token (you won't see it again!)

3. **Login to vsce**:
   ```bash
   vsce login veld-dev
   # Paste your PAT when prompted
   ```

## Build & Package

### 1. Install Dependencies
```bash
cd editors/vscode
npm install
```

### 2. Compile TypeScript
```bash
npm run compile
# or watch mode:
npm run watch
```

### 3. Package Extension
```bash
npm run package
# Creates: veld-vscode-0.1.0.vsix
```

### 4. Test Locally
```bash
code --install-extension veld-vscode-0.1.0.vsix
# Test in a new VS Code window
```

## Publish to Marketplace

### Option 1: Publish with vsce
```bash
vsce publish
# Automatically increments version and publishes
```

### Option 2: Manual Version Control
```bash
# Update version in package.json first
vsce publish 0.1.1
```

### Option 3: Publish Minor/Major
```bash
vsce publish minor  # 0.1.0 → 0.2.0
vsce publish major  # 0.1.0 → 1.0.0
vsce publish patch  # 0.1.0 → 0.1.1
```

## Pre-Publish Checklist

- [ ] All TypeScript compiles without errors
- [ ] Extension works in local testing
- [ ] README.md is complete and accurate
- [ ] package.json has correct version, description, keywords
- [ ] icon.png added (128x128px, optional but recommended)
- [ ] LICENSE file included
- [ ] Repository URL is correct
- [ ] Screenshots added to README (optional but recommended)

## Post-Publish

1. **Verify on Marketplace**:
   - Visit: https://marketplace.visualstudio.com/items?itemName=veld-dev.veld-vscode
   - Check that description, screenshots, and install button work

2. **Test Installation**:
   ```bash
   code --install-extension veld-dev.veld-vscode
   ```

3. **Announce**:
   - Update main Veld README with marketplace link
   - Tweet/blog about the release
   - Update documentation

## Continuous Publishing (GitHub Actions)

Create `.github/workflows/publish-vscode.yml`:

```yaml
name: Publish VS Code Extension

on:
  push:
    tags:
      - 'vscode-v*'

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: 20
      
      - name: Install dependencies
        run: |
          cd editors/vscode
          npm install
      
      - name: Compile
        run: |
          cd editors/vscode
          npm run compile
      
      - name: Publish
        env:
          VSCE_PAT: ${{ secrets.VSCE_PAT }}
        run: |
          cd editors/vscode
          npx vsce publish -p $VSCE_PAT
```

Add secret `VSCE_PAT` to GitHub repository settings.

## Troubleshooting

### Error: "Missing publisher name"
- Make sure `publisher` is set in `package.json`
- Create publisher at: https://marketplace.visualstudio.com/manage

### Error: "Extension already exists"
- Make sure you increment the version number
- Or use `vsce publish patch/minor/major`

### Error: "Authentication failed"
- Regenerate your Personal Access Token
- Make sure it has **Marketplace (Manage)** scope
- Login again with `vsce login veld-dev`

### Extension not activating
- Check `activationEvents` in package.json
- Check for TypeScript compilation errors
- Look at VS Code dev console: Help → Toggle Developer Tools

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 0.1.0 | 2026-02-28 | Initial release |

## Resources

- [VS Code Extension API](https://code.visualstudio.com/api)
- [Publishing Extensions](https://code.visualstudio.com/api/working-with-extensions/publishing-extension)
- [Extension Marketplace](https://marketplace.visualstudio.com/)

