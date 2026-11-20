# Veld examples

Each example is a complete Veld 2.0 project: contracts, `veld.toml`, the generated output
(committed) and **hand-written servers and frontends that use it**. The hand-written code imports
the generated code as packages (`@veld/<target>`), wired by `veld setup`, and the examples are
tested end to end: the servers are booted and called over HTTP, and the React apps are rendered
against them.

| Example | Shows |
|---|---|
| [`hello/`](hello) | The minimum, end to end: an Express server implementing `HelloService`, and a Vite + React app using the generated hooks, with a README walk-through |
| [`microservices/`](microservices) | Five services (four Node, one Java), mutual `consumes` between Node and Java, typed errors across both, a React app, Docker compose |
| [`library-package/`](library-package) | A shared `[libraries.common]` package whose types and errors are used by two services (Express and Fastify) and a typed client script |

In an example directory (build the CLI once with `cargo build -p veld --release`):

```bash
veld generate            # write the generated code
veld setup               # add @veld/<target> to the package.json files (Maven/Gradle for Java)
npm install              # in each app folder
```

then follow the example's README. Every app has `start`, `dev` and `build` scripts and a committed
`package-lock.json`; `node_modules` and build output are ignored (`examples/.gitignore`).

## Tests

- `cargo test -p veld --test examples`: every example compiles, its committed output is up to date,
  and regenerating from scratch gives the same bytes.
- `cargo test -p veld --test examples_e2e`: copies each example to a temporary directory,
  regenerates it (nothing may change), runs `npm ci`, `tsc --noEmit` on the hand-written code (which
  proves the `@veld/...` imports), boots the servers on free ports (Node through `tsx`, Java through
  `javac` and `java`) and runs a scripted flow over HTTP, then renders the React app with
  `react-test-renderer`. It needs Node with npm (and a JDK for `microservices`) and skips with a
  message when they are missing; `VELD_REQUIRE_TOOLCHAINS=1` makes that a failure.

After you change a contract in an example, run `veld generate` and commit the result.

The older v1 examples are in [`archive/v1/examples`](../archive/v1/examples). They are reference only.
