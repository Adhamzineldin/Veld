// Replays the native parity cases against the built WebAssembly module.
//
// `cargo test -p veld-wasm --test parity` generates every example and playground preset with the
// real `veld` CLI, asserts the wasm engine matches it byte for byte, and writes the inputs plus
// the SHA-256 of every expected file to target/veld-wasm-parity/cases.json. This script runs the
// same inputs through the compiled .wasm (the exact module the website ships) and compares the
// hashes, so the wasm32 build is held to the CLI's bytes too.
//
//   npm run test:wasm      (after `npm run build:wasm` and the cargo parity test)
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const site = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repo = resolve(site, '..');
const targetDir = process.env.CARGO_TARGET_DIR ?? join(repo, 'target');
const casesFile = join(targetDir, 'veld-wasm-parity', 'cases.json');
const wasmFile = join(site, 'src', 'wasm', 'veld_wasm_bg.wasm');

for (const [what, file, fix] of [
  ['the wasm module', wasmFile, 'npm run build:wasm'],
  ['the parity cases', casesFile, 'cargo test -p veld-wasm --test parity'],
]) {
  if (!existsSync(file)) {
    console.error(`test-wasm: ${what} is missing (${file}). Run \`${fix}\` first.`);
    process.exit(1);
  }
}

const engine = await import(pathToFileURL(join(site, 'src', 'wasm', 'veld_wasm.js')).href);
engine.initSync({ module: readFileSync(wasmFile) });

const sha = (text) => createHash('sha256').update(text, 'utf8').digest('hex');
const cases = JSON.parse(readFileSync(casesFile, 'utf8'));
let failures = 0;
let compared = 0;
for (const c of cases) {
  const r = JSON.parse(engine.generate(JSON.stringify(c.files), JSON.stringify(c.target), JSON.stringify(c.project)));
  if (!c.ok) {
    if (r.ok) {
      console.error(`FAIL ${c.name}: the CLI refused the target but wasm generated it`);
      failures++;
    }
    continue;
  }
  if (!r.ok) {
    console.error(`FAIL ${c.name}: wasm failed: ${r.diagnostics.map((d) => d.rendered).join('\n')}`);
    failures++;
    continue;
  }
  const got = Object.fromEntries(r.files.map((f) => [f.path, sha(f.text)]));
  for (const p of new Set([...Object.keys(got), ...Object.keys(c.expected)])) {
    if (got[p] !== c.expected[p]) {
      console.error(`FAIL ${c.name}: ${p} ${got[p] ? (c.expected[p] ? 'differs' : 'is extra') : 'is missing'}`);
      failures++;
    }
  }
  compared++;
}
if (failures > 0) {
  console.error(`test-wasm: ${failures} mismatches`);
  process.exit(1);
}
console.log(`test-wasm: ${compared} targets from ${cases.length} cases byte-identical to the CLI, through the built .wasm`);
