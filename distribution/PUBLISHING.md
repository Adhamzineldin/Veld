# Publishing Guide

This guide explains how to publish Veld to npm and PyPI.

## Prerequisites

### npm
- ✅ You have the `@maayn` organization on npm
- ✅ You're logged in: `npm login`
- ✅ You have publish access to `@maayn` scope

### PyPI
- ✅ You have a PyPI account: https://pypi.org/account/register/
- ✅ You have an API token: https://pypi.org/manage/account/token/
- ✅ Package name `maayn-veld` is available (confirmed ✅)

## Publishing to npm

```bash
cd packages/npm

# 1. Verify you're logged in
npm whoami

# 2. Verify package.json looks correct
cat package.json

# 3. Publish (will publish as @maayn/veld)
npm publish --access public
```

**Note:** The first time publishing a scoped package, you need `--access public` to make it publicly available.

## Publishing to PyPI

```bash
cd packages/pip

# 1. Install build tools
pip install build twine

# 2. Build the package
python -m build

# 3. Check the build (optional)
twine check dist/*

# 4. Upload to PyPI (will prompt for credentials)
twine upload dist/*

# Or use API token:
twine upload dist/* --username __token__ --password pypi-YourAPITokenHere
```

## Version Updates

When you release a new version:

1. **Update version in both packages:**
   - `packages/npm/package.json`: `"version": "0.2.0"`
   - `packages/pip/pyproject.toml`: `version = "0.2.0"`
   - `packages/pip/veld/__init__.py`: `__version__ = "0.2.0"`
   - `packages/npm/install.js`: `const VERSION = "0.2.0";`
   - `packages/pip/veld/__main__.py`: `VERSION = "0.2.0"`

2. **Create GitHub Release:**
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

3. **Publish both packages** (follow steps above)

## Testing Before Publishing

### Test npm package locally:
```bash
cd packages/npm
npm pack
# Creates a .tgz file you can test
npm install -g ./maayn-veld-0.1.0.tgz
veld --version
```

### Test PyPI package locally:
```bash
cd packages/pip
python -m build
pip install dist/maayn_veld-0.1.0-py3-none-any.whl
veld --version
```

## Troubleshooting

### npm: "You do not have permission to publish"
- Make sure you're a member of the `@maayn` organization
- Check: `npm org ls maayn`

### PyPI: "Package already exists"
- The version number is already published
- Increment the version number and try again

### PyPI: "Invalid distribution"
- Run `twine check dist/*` to see errors
- Make sure `pyproject.toml` is valid

## Package URLs After Publishing

- **npm**: https://www.npmjs.com/package/@maayn/veld
- **PyPI**: https://pypi.org/project/maayn-veld/
