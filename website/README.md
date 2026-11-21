# Veld website

Vite + React 19 + TypeScript. Docs live in `src/pages/docs`; the live playground is `src/pages/PlaygroundPage.tsx`.

## The playground engine (WebAssembly)

The playground runs the real Veld compiler and generators, built from `crates/veld-wasm`.

**Prerequisites:** Rust (rustup + cargo). `npm run build:wasm` (run automatically by `npm run build`, and by `npm run dev` when missing) adds the `wasm32-unknown-unknown` target and installs the exact `wasm-bindgen-cli` version the crate pins if they are absent (`rustup target add wasm32-unknown-unknown` by hand also works). `wasm-opt` is used when installed.

The generated `src/wasm/` is git-ignored: it is rebuilt from the Rust sources on every deploy build, so the deploy environment needs Rust. Set `VELD_WASM_SKIP=1` to use an already built `src/wasm/`.

Parity with the CLI: `cargo test -p veld-wasm --test parity` (CLI vs engine, byte for byte, for every example and preset), then `npm run test:wasm` replays the same cases through the built `.wasm`.

For the browser acceptance test, run the native parity test, then run the following from `website/`:

```sh
npm install
npx playwright-core install chromium
npm run build
npm run test:playground
```

The test opens the production site in isolated Chromium, compares each preset's downloaded ZIP with CLI output, and exercises streaming targets, diagnostics, formatting, and copying. Set `CHROMIUM_PATH` to use an existing Chromium executable.

Other scripts: `npm run lint`, `npm run check:docs`, `npm run gen:cli-help`.
