# Message catalog goldens

Which language an answer is written in and what each message says (`spec/platform.md` section 22). The Node, Java and Rust generators must each give the same results for the same contract and catalogs. The reference is `veld_rules::messages` (`crates/veld-rules/src/messages.rs`) and `veld_ir::message_keys`; the test `crates/veld-rules/tests/messages.rs` runs everything here.

| File | What it is |
|---|---|
| `workspace/` | A one-member workspace: `veld.toml`, `contracts/bookings.veld`, and the catalogs `messages/ar.toml` and `messages/pt-br.toml`. The package is `bookings`. |
| `cases.json` | The tables below. |

`cases.json`:

- `languages`: the languages the member has catalogs for (`ar`, `pt-br`).
- `headers`: `{ "header", "language" }`. The `Accept-Language` value (`null` is no header) and the catalog chosen for it (`null` is none: the contract's text). It covers the order of preference by quality and by position, `q=0`, a malformed quality (the entry is skipped, not the header), a tag with a region (`ar-EG` is `ar`), `pt` not matching `pt-br`, `*` matching nothing, whitespace, and a tag that is not a language range.
- `messages`: `{ "header", "key", "default", "text", "language" }`. `default` is the contract's text, `text` what the answer says, `language` the catalog it came from or `null` (then the answer's `Content-Language` is not set on this account). A key the chosen catalog lacks keeps `default` and `language` is `null`.
- `keys`: the `messageKey` of each kind of rule and of each fixed text. A generator reads it as: a violation of this rule carries this `messageKey`.
- `catalogKeys`: the keys a catalog of this member may have, with the contract's text (`null` for a rule that has no message). The fixed keys (`fixedKeys`) are valid too and are not listed. A key in a catalog that is in neither list is `W1301`; a key with a text in this list that a catalog lacks counts toward `W1302`.

What they cannot state: the bytes of a whole answer (the wire goldens of `tests/wire/rules` do that, and a server with catalogs adds `messageKey` to each violation and `Content-Language` to the answer when some message came from a catalog), and what a client does with its own catalogs (it picks by its own rule; the table is the same).
