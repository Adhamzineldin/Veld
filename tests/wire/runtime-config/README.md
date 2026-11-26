# Runtime config goldens

The settings of a target's typed runtime config, the check that every generator emits, and the exact text of its errors (`spec/runtime-config.md` sections 2 to 4). The Node, Java and Rust generators must each reproduce them from their own generated check. The reference is `veld_codegen::runtime_config` (`RuntimeModel::check`, `check_map`, `check_env`, `normalize_url`, `runtime_type_name`); its test `crates/veld-codegen/tests/runtime_config_wire.rs` runs every case here.

One file, `cases.json`, with five top-level keys.

| Key | What it is |
|---|---|
| `version` | `1`. |
| `models` | The runtime configs the cases use, by id. |
| `type_names` | `target` and the `type` name (`<Target>Runtime`) it gets. |
| `urls` | `input` and `normalized`: the URL with every trailing `/` removed, or `null` when it is not an absolute `http` or `https` URL. |
| `cases` | The checks. |

## `models`

```json
"server": { "target": "bookings", "type": "BookingsRuntime", "settings": [
  { "path": "rules.output", "kind": "list", "values": ["strict", "audit", "off"], "required": false, "default": "strict" },
  { "path": "services.iam.url", "kind": "url", "required": true } ] }
```

The settings are in the order of the spec: byte order of the path text, so `services.a-b.url` comes before `services.a.url`. A generated type, check and message list MUST use this order. `kind` is `url` or `list` (`values` are the allowed strings, in the order the message prints them).

## `cases`

```json
{ "name": "…", "model": "server", "from": "value", "input": { "services": { … } },
  "expect": { "values": { "rules.output": "strict", "services.iam.url": "http://iam:3001" } } }
```

- `from: "value"`: the generated check (`checkRuntime`, `build()`, `check`) is given `input`, a nested value. `null` is an absent value.
- `from: "map"`: the flat form (`fromMap`, `from_map`): `input` is an object from path to string.
- `from: "env"`: `fromEnv`: `names` maps paths to variable names, `env` is the environment. A variable that is not set and one that is empty are the same.

`expect` is either:

- `{ "values": { path: text, … } }`: the check passes. `values` holds every setting that has a value, with the default filled in and every trailing `/` removed from a URL. A generated type holds the same.
- `{ "problems": [ { "path": …, "message": … }, … ], "error": "invalid runtime config: …" }`: the check fails with all of them. `problems` are sorted by path (byte order), then by message. `error` is the text of the whole error: `invalid runtime config: ` and the messages in that order, separated by `; `. A test compares the messages and the text byte for byte.

What the cases cover: defaults, the trailing `/`, `null` and empty values (absent for a required path), a wrong JSON type, an unknown key at every level and a dotted key, a value where an object belongs (`<path> must be an object`, `runtime config must be an object` for the value itself), the ordering with `a` and `a-b`, a client (no `rules` section), the flat map, and `fromEnv` with unset, empty and invalid variables, a name that is not a path, and a required path with no name.

A language that cannot report some of these as one error (the `serde` deserializer of the Rust type reports a wrong type its own way) MUST say so in its target spec and report the same text through `check`, `from_map` and `from_env`.
