# Publishing

Publish only after the release workflow has uploaded the assets for the tag, because the wrappers
download them. The version in `npm/package.json`, `pip/pyproject.toml`, `pip/veld/__init__.py` and
`homebrew/Formula/maayn-veld.rb` must equal the tag without the `v`.

## npm

```bash
cd distribution/npm
npm publish --access public      # --tag next for a pre-release
```

## PyPI

```bash
cd distribution/pip
pip install build twine
python -m build
twine upload dist/*
```

## Homebrew

```bash
cd distribution/homebrew
./update-formula.sh /path/to/SHA256SUMS
# copy Formula/maayn-veld.rb into the homebrew-tap repository and push
```
