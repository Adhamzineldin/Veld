# Veld examples

Each example is a complete Veld 2.0 project with its generated output committed, so you can read
the generated code without installing anything.

| Example | Shows |
|---|---|
| [`hello/`](hello) | The minimum: one contract, a Node server and a TypeScript client |
| [`microservices/`](microservices) | Five services, mutual `consumes` (accounts and transactions), Node servers and one Java server, one TypeScript client for the frontend |
| [`library-package/`](library-package) | A shared `[libraries.common]` package imported by two services with `use common:...` |

Build the CLI once (`cargo build -p veld --release`, binary in `target/release/`), then in an example directory:

```bash
veld check              # compile the contracts
veld generate           # write the generated code
veld generate --dry-run # show what would change; nothing is written
```

`cargo test -p veld --test examples` runs `veld check` on every example and verifies that the
committed output is exactly what `veld generate` produces. After you change a contract in an
example, run `veld generate` and commit the result.

The older v1 examples are in [`archive/v1/examples`](../archive/v1/examples). They are reference only.
