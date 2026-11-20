# Veld for VS Code

A thin client for the [Veld](https://github.com/Adhamzineldin/Veld) contract language. All language
features come from the language server that ships inside the `veld` binary (`veld lsp`):

- diagnostics for the whole project, with codes and related locations
- hover (docs, kind, signature, resolved type; method, path, statuses and errors for endpoints)
- go to definition (types, `use` imports, errors) and find references
- completion (keywords, HTTP methods, types with auto-import, annotations with argument snippets, enum variants after `= .`)
- outline, rename across files, formatting, semantic tokens

The TextMate grammar (`syntaxes/veld.tmLanguage.json`) is generated from the compiler's language
table with `cargo xtask codegen`. Do not edit it by hand.

## Requirements

Install Veld 2 and make sure `veld` is on your `PATH`, or set `veld.path`.

## Settings

| Setting | Default | Meaning |
|---|---|---|
| `veld.path` | `veld` | The executable started as `veld lsp` |
| `veld.trace.server` | `off` | Trace the language server protocol |

## Development

```bash
npm install
npm run compile
```

The v1 extension re-implemented the language in TypeScript. That code was removed in 2.0 and
is preserved in git history.
