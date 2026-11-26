# Redaction goldens

What a capture or an audit event may keep of a value (`spec/rules.md` section 19.3). The Node, Java and Rust generators that emit captures or audit events must each reproduce `redacted` and `paths` from `value`. The reference is `veld_rules::redact` (`crates/veld-rules/src/redact.rs`); its test `crates/veld-rules/tests/redaction.rs` runs every case.

| File | What it is |
|---|---|
| `contract.veld` | The contract, compiled as the package `guests`. |
| `cases.json` | `{ "package", "contract", "cases": [ … ] }` |

One case:

```json
{ "name": "…", "type": "Guest", "note": "…",
  "value": { … },
  "redacted": "{\"city\":…}",
  "paths": ["$.name", "$.home.street"] }
```

- `type` is a declaration of the contract (a model, an error or an event) and `value` is a wire value of it.
- `redacted` is the value with every `@pii` and `@secret` field left out, as RFC 8785 canonical JSON. A generator compares **bytes**: its output, canonicalized, must equal this string.
- `paths` are the wire paths of what was left out, sorted by their bytes (`[10]` comes before `[2]`) and without duplicates.

What the cases cover: a nested model, an optional field that is absent and one that is `null`, a list, a map, a model that is itself marked (the whole value goes, one path), a field with both annotations, a member the contract does not know (kept), an inherited field and an own one, a union (two arms, chosen by `kind`), a page (its other members stay), an error, an event, a value with nothing to leave out, and the order of the paths.

What they cannot state: a value that does not have the shape of its declaration (the reference refuses it with an error, and a generator should too: it cannot know such a value is clean), and what a capture does with the left-out values (`veld test --from-violation` fills them in, section 12.2).
