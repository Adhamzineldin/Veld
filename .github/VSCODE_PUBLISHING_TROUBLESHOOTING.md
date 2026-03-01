# VS Code Extension Publishing Troubleshooting

## Issue: CI Passes But Extension Not in Marketplace

### Common Causes

#### 1. **Publisher Doesn't Exist** ⚠️ Most Common
The publisher `adhamzineldin` must exist in Azure DevOps before publishing.

**Fix:**
1. Go to https://marketplace.visualstudio.com/manage
2. Sign in with your Microsoft account
3. Click **Create Publisher** (if you don't have one)
4. Publisher ID: `adhamzineldin` (must match `package.json`)
5. Complete the publisher profile

**Verify:**
- Check if publisher exists: https://marketplace.visualstudio.com/publishers/adhamzineldin
- If 404, the publisher doesn't exist yet

#### 2. **Secret Not Set or Wrong**
The `VSCE_PAT` secret might not be set or has wrong value.

**Check:**
1. Go to your GitHub repo → **Settings** → **Secrets and variables** → **Actions**
2. Look for `VSCE_PAT` secret
3. If missing, add it

**Create Token:**
1. Go to https://dev.azure.com
2. Sign in (use the same account as your publisher)
3. Click your profile → **Personal Access Tokens**
4. **New Token**:
   - Name: "VS Code Extension Publishing"
   - Organization: **All accessible organizations**
   - Expiration: Your choice
   - Scopes: **Marketplace (Manage)** ✅
5. Copy the token
6. Add to GitHub as `VSCE_PAT` secret

#### 3. **Token Doesn't Match Publisher**
The token must be from the same Microsoft account that owns the publisher.

**Fix:**
- Make sure you're signed into Azure DevOps with the same account
- The publisher `adhamzineldin` must be owned by that account

#### 4. **Version Already Published**
You can't publish the same version twice.

**Fix:**
- Increment version in `editors/vscode/package.json`
- Create a new tag with the new version

#### 5. **First Publication Needs Manual Approval**
First-time extensions may need manual review (1-3 days).

**Check Status:**
- Go to https://marketplace.visualstudio.com/manage
- Click on your publisher
- Check extension status

### Debugging Steps

1. **Check CI Logs:**
   - Go to GitHub Actions → Latest workflow run
   - Look for "Publish to VS Code Marketplace" step
   - Check for error messages

2. **Test Locally:**
   ```bash
   cd editors/vscode
   npm install
   npm run compile
   npx @vscode/vsce publish -p YOUR_TOKEN
   ```

3. **Verify Publisher:**
   ```bash
   # Check if publisher exists
   curl https://marketplace.visualstudio.com/publishers/adhamzineldin
   ```

4. **Check Extension Status:**
   - Visit: https://marketplace.visualstudio.com/manage
   - Look for "veld-vscode" extension
   - Check if it's pending review or published

### Quick Fix Checklist

- [ ] Publisher `adhamzineldin` exists at https://marketplace.visualstudio.com/manage
- [ ] `VSCE_PAT` secret is set in GitHub (Settings → Secrets → Actions)
- [ ] Token has **Marketplace (Manage)** scope
- [ ] Token is from the same Microsoft account as publisher
- [ ] Version in `package.json` is unique (not already published)
- [ ] CI workflow shows "Publish to VS Code Marketplace" step completed

### Manual Publishing (If CI Fails)

```bash
cd editors/vscode
npm install
npm run compile

# Get token from Azure DevOps
export VSCE_PAT="your-token-here"

# Publish
npx @vscode/vsce publish -p "$VSCE_PAT"
```

### After Publishing

- Extension URL: `https://marketplace.visualstudio.com/items?itemName=adhamzineldin.veld-vscode`
- Install command: `code --install-extension adhamzineldin.veld-vscode`
- May take 5-10 minutes to appear in search
