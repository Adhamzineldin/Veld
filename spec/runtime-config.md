# Typed runtime config

Status: **DRAFT. Not implemented.** Proposal: `docs/proposals/rules-and-events.md` sections 4 and 13. It is built after the workspace layout (`workspace.md`), as its own change.

## 1. Why

Some values differ per deployment and not per generation: the address of a service a target calls, how strictly a server checks its own output, and later the brokers of a messaging transport. Today they reach generated code in three different ways:

- a client reads its base URL from a constructor option, else from the environment variable `VELD_<SERVICE>_URL`, else from `base-url` in `veld.toml`, which is compiled into the client;
- a server reads its rule output mode from a field of its hooks object (`rules: { output }`);
- nothing lists them in one place, so a missing value is found by the first request that needs it.

This spec gives every target one **runtime config**: a generated type that holds exactly those values, a check that reports every problem at once, and an optional helper that fills it from environment variables whose names the application chooses. Generated code reads no environment variable on its own.

## 2. What is a setting

A setting is plain data that a deployment chooses. Functions are not settings: the fetch implementation, token providers, fact loaders, the clock and the other hooks stay where they are.

| Path | Type | Required | In the runtime config of |
|---|---|---|---|
| `services.<service>.url` | absolute `http` or `https` URL | yes | a client target, for each service it covers; a server target with `clients = true`, for each service in `consumes` |
| `rules.output` | `strict`, `audit` or `off` | no (default `strict`) | a server target whose contract has rules (`rules.md` 5.3) |
| `messaging` | defined by `events.md` with phase E2 | | a server target whose service publishes or subscribes |

- `<service>` is the service's name as the workspace spells it (`card-service`).
- A target whose runtime config would have no path has none: nothing in this spec is generated for it, and its output is the bytes it is today. A server without rules that consumes nothing is such a target.
- The paths are ordered by their text. Every generated list of them (types, checks, schema, messages) uses that order.

A trailing `/` of a URL is dropped, as today.

## 3. What is generated

One runtime config per target, named `<Target>Runtime`: the target's name in Pascal case, then `Runtime` (`bookings` gives `BookingsRuntime`, `web` gives `WebRuntime`).

| | TypeScript, Node | Java | Rust |
|---|---|---|---|
| The type | an interface in `runtime.ts` | an immutable class with a builder | a struct that derives `serde::Deserialize` |
| The check | `checkRuntime(value: unknown): <Target>Runtime` | `build()` of the builder, and `fromMap` | `check(&self)` and the deserializer |
| From a flat map of paths | `fromEnv(names, env?)` | `fromMap(Map<String, String>)`, `fromEnv(Map<String, String> names)` | `from_env(names)`, `from_map(map)` |
| Every client of the target | `createClients(runtime, options?)` | `<Target>Clients.create(runtime, options)` | `clients(&runtime, options)` |

The generated type has no dependency beyond what the target already has.

### 3.1 TypeScript and Node

```ts
// runtime.ts of the target `bookings` (a server that consumes iam and payments, with rules)
export interface BookingsRuntime {
  services: {
    iam: { url: string };
    payments: { url: string };
  };
  rules?: { output?: RuleOutputMode };
}

export function checkRuntime(value: unknown): BookingsRuntime;
export function fromEnv(names: BookingsRuntimeEnv, env?: Record<string, string | undefined>): BookingsRuntime;
export function createClients(runtime: BookingsRuntime, options?: ClientOptions): { iam: IamClient; payments: PaymentsClient };
```

- The application builds the object however it likes: its own settings module, a JSON or YAML file, a secrets manager.
- `fromEnv` maps paths to names the application chooses: `fromEnv({ "services.iam.url": "IAM_URL", "services.payments.url": "PAYMENTS_URL" })`. `BookingsRuntimeEnv` requires a name for every required path and accepts one for every optional path. `env` defaults to `process.env` where it exists and to an empty object elsewhere; a browser build passes its own (`import.meta.env`).
- The generated server entry takes the runtime config as its last argument, after the hooks: `createBookingsHandler(impl, hooks, runtime)`, and each framework adapter does the same. The argument is optional exactly when the type has no required path.
- `createClients` gives each client its URL from `services` and passes `options` to every client.

### 3.2 Java

```java
BookingsRuntime runtime = BookingsRuntime.builder()
    .iamUrl("http://iam:3001")
    .paymentsUrl("http://payments:3002")
    .rulesOutput(RuleOutputMode.AUDIT)
    .build();
```

- The builder has one method per path, named after it in camel case without `services` (`iamUrl`, `rulesOutput`). The class has a getter of the same name.
- `fromMap` takes the paths of section 2 as keys, so a `Properties` file or any other flat source works. `fromEnv(names)` reads `System.getenv` through a map from path to variable name.
- With `framework = "spring"` the adapter also generates `<Target>RuntimeProperties`, a `@ConfigurationProperties` class with a `toRuntime()` method, so the settings bind from `application.yml`, profiles or a config server. Its prefix is the target option `runtime-prefix` (default `veld`): `veld.services.iam.url`.
- The server entry and `<Target>Clients.create` take the runtime config as in section 3.1.

### 3.3 Rust

```rust
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BookingsRuntime {
    pub services: BookingsServices,
    #[serde(default)]
    pub rules: RulesRuntime,
}
```

- Any `serde` source fills it: a TOML or JSON file, `figment`, `config`, `envy`. A field whose path segment is not a Rust identifier is renamed for `serde` (`#[serde(rename = "card-service")]`).
- `check` runs the checks `serde` cannot (section 4). `from_env` and `from_map` take pairs of path and name, and run `check`.
- The server entry and `clients` take `&BookingsRuntime`.

## 4. The check

Every way of building a runtime config ends in the same check, and every generated function that receives one (`createClients`, the server entry) runs it first. A problem is found when the application starts, not at the first request.

The check collects every problem and fails once, with an error that carries the list (`VeldConfigError` in TypeScript, `VeldConfigException` in Java, `ConfigError` in Rust). Each problem has the path and a message:

| Problem | Message |
|---|---|
| a required path is absent or empty | `<path> is missing` |
| a URL is not an absolute `http` or `https` URL | `<path> must be an http or https URL` |
| a value outside its list | `<path> must be "strict", "audit" or "off"` |
| a key the type does not have | `<path> is not a setting` |
| in `fromEnv`: the named variable is not set (required paths only) | `<path> is missing (environment variable <NAME> is not set)` |

The error's text is `invalid runtime config: ` followed by the problems in path order, separated by `; `. The texts are the same in every language.

An unknown key is a problem and is not ignored, so that a misspelt `rules.ouput` cannot leave a server in a mode nobody chose.

## 5. What is removed

- **The environment fallback.** A generated client no longer reads `VELD_<SERVICE>_URL`.
- **The compiled-in URL.** `base-url` no longer reaches generated code.
- **A client without a URL.** The `baseUrl` option of a client is required: a TypeScript client constructor requires the property, the Java client has no constructor without options, and the Rust client constructor takes the options. A client built directly, without the runtime config, keeps working as long as it passes `baseUrl`, which every client that passes one today already does.
- **`rules: { output }` in the hooks object.** The mode is `rules.output` of the runtime config, and there is one place to set it. `rules.md` 5.3 already says the hooks field is temporary.

`base-url` stays a key of `[service]` in `veld.toml` with one meaning left: the `servers` entry of the OpenAPI export. The IR keeps the field, so no IR snapshot and no lock file changes.

## 6. `veld export runtime-schema`

`veld export runtime-schema [<target>] [-o FILE]` writes the JSON Schema (draft 2020-12) of one target's runtime config, so that Helm values, a Kubernetes ConfigMap or a CI step can be validated before a deploy.

- The schema has `title` (`<Target>Runtime`), `type`, `properties`, `required` and `additionalProperties: false` at every level, `format: "uri"` on URLs and `enum` on lists. Keys are sorted; the output is deterministic.
- Without `<target>` the workspace MUST have exactly one target with a runtime config (`E0803` otherwise, and the message lists them). A target without one is `E0803`.
- The file is written as `veld export openapi` writes its file (`config.md`, "Generated output").

## 7. What changes in generated output

This change breaks the rule that a contract without rules generates the bytes it did before, on purpose and only here:

- every generated **client** changes: its constructor, its base URL resolution and its documentation, in TypeScript, Java and Rust;
- every **server that consumes a service** gains the runtime module and the last argument;
- every **server with rules** moves `rules.output`;
- a server without rules that consumes nothing does not change.

The snapshots of generated clients from before the branch therefore change. They are listed for the owner with the commit that changes them. The hand-written code of the examples, the toolchain tests and the docs that rely on the fallback move in the same change: `examples/microservices` sets `VELD_<SERVICE>_URL` in its deployment files today.

## 8. Other generators

Every built-in generator follows this spec. The plugin protocol does not change: a plugin receives the same IR and target options, and its guide tells it to generate the same paths and messages.

## 9. Open points

The owner confirmed point 1 on 25 November 2026 (the client keeps `baseUrl`, and `createClients` fills it). Points 2 to 6 are built as drafted unless the owner says otherwise:

1. **`baseUrl` stays a client option and becomes required** (section 5), and `createClients` fills it from the runtime config. The alternative, a client constructor that takes the runtime config, would change every place that builds a client today.
2. **The runtime config is the last argument of the server entry** (section 3.1), after the hooks, as the proposal sketches. The alternative is a field of the hooks object.
3. **An unknown key fails the check** (section 4).
4. **`base-url` stays in `veld.toml` for the OpenAPI export only** (section 5). The alternative is to rename it (`openapi-server`), which moves a key that exists today.
5. **`sample`** (checking only a share of responses, in the proposal's sketch) is not in this spec: `rules.md` does not define it.
6. **`validateResponses` and `validateRequests`** stay fields of the hooks object. They are switches a deployment might choose too, but they exist since before the branch and moving them changes every server.
