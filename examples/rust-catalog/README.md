# Rust book catalog

The Axum server serves two books by ISBN. The command-line client uses the generated blocking
ureq transport and decodes the contract's typed `NotFound` error when a book is missing.

With Veld 2 and Cargo on your PATH, run these commands from this directory in one terminal:

```sh
veld generate
cargo run --locked --bin server
```

From the same directory in a second terminal:

```sh
cargo run --locked --bin lookup -- 9780141439518
# Pride and Prejudice (9780141439518)
cargo run --locked --bin lookup -- 000
# No book found for ISBN 000 (exit code 1)
```

The server listens on `127.0.0.1:3000`. Set `PORT` on the server and `CATALOG_URL` on the client
to use a different address. Both applications import the generated crates through Cargo path
dependencies. Edit `contracts/books.veld` and rerun `veld generate` to change the contract.

- [Server implementation](src/bin/server.rs)
- [Client implementation](src/bin/lookup.rs)
- [Generator targets](veld.toml)
