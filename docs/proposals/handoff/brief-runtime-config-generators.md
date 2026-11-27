# Brief: the typed runtime config in the generators

Three agents get this brief, one per target (Node with TypeScript, Java, Rust). The message that started you says which is yours. Read `common.md` first (the law of this machine), then `CLAUDE.md`, then `brief-in-the-generators.md` (the three targets, how to work next to the other agents, the report), then this file.

This task differs from the usual generator task in one way: **it changes existing generated output on purpose.** There is no capability to declare and nothing to refuse.

## Read first

- `spec/runtime-config.md`, all of it, including section 9.1 (what the shared model chose). It is the specification.
- `crates/veld-codegen/src/runtime_config/`: the shared model. `RuntimeModel::for_request(req, profile)` is what a generator calls. `None` means the target has no setting: **emit nothing for it and keep today's bytes.** The model gives the type name, the settings in path order, which are required, their defaults, and every message as a function. **Never type a message or a path yourself**: embed what the module gives.
- `tests/wire/runtime-config/cases.json` and its README: the goldens. Your generated check, `fromMap`/`from_map` and `fromEnv`/`from_env` must give the same values, the same problems in the same order and the same error text.
- In your emitter, before you write anything: how a client gets its base URL today (the constructor option, the `VELD_<SERVICE>_URL` fallback, the compiled-in `base-url`), where a server reads its rule output mode (`rules: { output }` in the hooks), how a server with `clients = true` builds the clients of the services it consumes, and the tests and test programs that rely on each.

## What to build (your target's column of section 3)

1. **The runtime module** of a target that has settings: the `<Target>Runtime` type, the check that collects every problem and fails once (section 4), building from a nested value, from a flat map of paths and from environment variable names the application chooses, and the function that builds every client of the target from it. Java with `framework = "spring"` also gets `<Target>RuntimeProperties` (section 3.2; the target option `runtime-prefix`).
2. **The server entry** takes the runtime config as its last argument, after the hooks, in every adapter of your target; optional exactly when the model has no required path (`has_required()`). The check runs first in every generated function that receives a runtime config.
3. **What section 5 removes**, in your target: the environment fallback, the compiled-in URL, a client without a URL (`baseUrl` becomes required), and `rules: { output }` in the hooks (the mode is `rules.output` of the runtime config; a server whose model has no `rules.output` keeps what it does today).
4. **"Has rules"** is decided by the shared `contract_has_rules`, through the model. If your emitter decides it differently today (the Java and Rust emitters also count consumed services, policies and `@rollout`), say what changes for which contracts, and keep every other use of your own predicate as it is.
5. **Everything hand-written that relied on what is removed moves in the same change**, as far as it is your target's:
   - the test programs under `tests/toolchain/<your target>/` and the Rust tests of your emitter that build a client or a server;
   - the examples under `examples/` that have a target of yours: their hand-written code, their deployment files (`examples/microservices` sets `VELD_<SERVICE>_URL` today), and their **committed generated output**, which you regenerate with the debug binary (`veld generate` in the example; never edit generated files by hand). An example with targets of several emitters is edited by several agents: touch only your target's folders and the files that start your target's programs;
   - `apps/veld` tests and the `veld init` templates that write code for your target, if they build a client without a URL or set the output mode in the hooks.
6. Tests that call a generator directly must pass their options through `veld_codegen::with_target_name(&opts, "<name>")`: a target with settings and no name is `E0802`.

## Gates

- A target without settings (a server without rules that consumes nothing) keeps its bytes: no existing snapshot of such a target may change.
- Every other existing snapshot of your target **will** change, and that is the point. Do not store them. In your report, list each changed snapshot with one line saying what changes in it (the constructor, the URL resolution, the runtime module, the moved output mode), grouped, so the main session can check each against that line.
- No new runtime dependency (Rust: `serde` is already there; the Spring properties class uses Spring, which the adapter already depends on). Deterministic output: settings in the model's order.
- The docs (`docs/`, `website/`) are not yours: say in your report which pages mention what you removed.

## Tests that must exist when you hand back (written and compiled, not run)

- The goldens of `cases.json` through your generated code: a test program that reads the file, runs each case through the generated check, the flat-map reader and the environment reader of a generated target, and compares values, problems and error text. One generated target per model of the file (the README says how the models map to contracts; if the file's models cannot be produced by generating a contract, say so and use the check with the model's settings the way the README describes).
- The client: it requires a URL; `createClients` (or your target's name for it) gives each client its URL and passes the options; a trailing `/` is dropped.
- The server: the entry with and without required paths; the output mode comes from the runtime config; a bad runtime config fails when the handler is created, with the whole list.
- Snapshots of the runtime module of a consuming server with rules, of a client target, and for Spring of the properties class.
- The examples still build: compile each example you touched with your target's compiler (compile only).

## Your report

As `brief-in-the-generators.md` says, plus the list of changed snapshots described under "Gates", and every example and test program you changed.
