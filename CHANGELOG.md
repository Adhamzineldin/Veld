# Changelog

All notable changes to Veld are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Changed

- `veld generate`, `veld watch` and `veld ci` stage generated files and commit them only when the whole run succeeds. A contract that fails to parse or validate leaves the previous output, the cache and `.veld.lock.json` untouched. Veld never deletes or overwrites files it did not write. Each output directory records what Veld wrote in `.veld-manifest.json`. See [docs/guides/generated-output.md](docs/guides/generated-output.md). (B-006, ADR-007)
- `veld clean` deletes only the files listed in the manifest and keeps `.veld.lock.json` unless `--lock` is given. A non-empty output directory without a manifest is left alone and the command fails. (B-009)
- `veld generate --dry-run` lists the files it would create, update or delete per resolved output directory, and writes nothing, including `.veld-cache.json`. (B-009)
- Output directories that are or contain the config directory, a contract directory, the working directory, the home directory or a filesystem root, or that already contain `.veld` files, are refused for every command and workspace entry. The old rule in config, which only looked at the last path segment, is gone. (B-008)
- The generated `README.md` is written only into the entry's own output directories and says that files you add are never touched.
- Hand-edited generated files that are no longer produced are kept with a warning instead of being deleted.

### Added

- `veld generate --overwrite-unmanaged` replaces files in the output directory that the manifest does not list.
- `veld clean --lock` also removes the breaking-change baseline.

### Fixed

- Breaking-change detection (`--strict`, `--force`, the interactive prompt) works again: the lock is no longer deleted before the comparison. (F-NODE-04)
- `veld init` no longer overwrites an existing `README.md` or contract files; it exits 1 and names them. (F-CLI-05)
- `types/index.ts` lists exports in a stable order, so the same contract always produces byte-identical output. (B-012, F-LEAD-01)
- `veld watch` and `veld ci` use the configured framework, validation, services and server SDK settings, like `veld generate`.
