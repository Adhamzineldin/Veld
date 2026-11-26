# Visibility goldens

What a server sends of a value to a caller (`spec/platform.md` section 2). The Node, Java and Rust servers must each reproduce `shown` and `hidden` from `value`, the caller and the clock. The reference is `veld_rules::apply_visibility_decl` (`crates/veld-rules/src/visibility.rs`); its test `crates/veld-rules/tests/visibility.rs` runs every case. Every `shown` and `hidden` in the file was printed by the reference itself, not calculated by hand, and read through.

| File | What it is |
|---|---|
| `contract.veld` | The contract, compiled as the package `guests`. Its principal is `User`. |
| `cases.json` | `{ "package", "contract", "cases": [ … ] }` |

One case:

```json
{ "name": "…", "type": "Guest", "note": "…",
  "caller": { "id": "…", "roles": [] } ,
  "now": "2026-06-10T12:00:00Z",
  "value": { … },
  "shown": "{\"city\":…}",
  "hidden": ["$.email", "$.past[0].door"] }
```

- `type` is a declaration of the contract (a model or an error) and `value` is a wire value of it, as the handler returned it.
- `caller` is the principal of the request, a wire value of `User`, or `null` for a request with no caller (`@auth(none)`). `now` is the clock of the request. A server gives the expression of a field `now` and `user` exactly these.
- `shown` is the value after visibility, as RFC 8785 canonical JSON. A generator compares **bytes**: its output, canonicalized, must equal this string.
- `hidden` are the wire paths of the fields that were dropped, sorted by their bytes (`[10]` comes before `[2]`) and without duplicates, written like the paths of violations: `$` is the root, `.wire` a member, `[2]` a list item. The `hidden` of an audit event is the same list without the `$` (`$.phone` is `phone`, `$.items[0].phone` is `items[0].phone`, see `tests/wire/audit`).

What the cases cover: a caller who is the field's subject, one who is not, one with a role, no caller at all; a field that is dropped whole with its own nested `@visible` (one path); an expression that cannot be evaluated (division by zero: the field is dropped for everyone); the clock (before, at the exact millisecond, after); a field the value lacks (not listed) and one that is `null` (dropped and listed); a guard on the field itself; a member the contract does not declare (kept); an inherited field and an own one; the arms of a union, the items of a page, a list and a map; a declared error; a value with nothing to drop; the order of the paths.

What they cannot state: a caller whose principal is another model than the one the field was checked against (the reference hides the field, as it does for no caller; a generator that serves two principals has to do the same), and a value that does not have the shape of its declaration (the reference refuses it with an error, and a generator should too).
