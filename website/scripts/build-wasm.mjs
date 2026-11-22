// Builds the Veld engine (crates/veld-wasm) to WebAssembly and writes the JS bindings to
// website/src/wasm/ (generated, git-ignored). Run by `npm run build:wasm`, and automatically
// before `npm run dev` and `npm run build`.
//
// Prerequisites: a Rust toolchain (rustup + cargo). This script adds the wasm32 target and
// installs the exact wasm-bindgen CLI the crate pins, if they are missing.
//
//   node scripts/build-wasm.mjs               build
//   node scripts/build-wasm.mjs --if-missing  build only when src/wasm is absent (dev servers)
//   VELD_WASM_SKIP=1                          do nothing (use an already built src/wasm)
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, statSync, mkdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';

const site = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repo = resolve(site, '..');
const outDir = join(site, 'src', 'wasm');
const wasmFile = join(outDir, 'veld_wasm_bg.wasm');

if (process.env.VELD_WASM_SKIP) {
  console.log('build-wasm: VELD_WASM_SKIP is set, skipping');
  process.exit(0);
}
if (process.argv.includes('--if-missing') && existsSync(wasmFile)) {
  process.exit(0);
}

function run(cmd, args) {
  const r = spawnSync(cmd, args, { stdio: 'inherit', cwd: repo });
  if (r.error || r.status !== 0) {
    console.error(`build-wasm: \`${cmd} ${args.join(' ')}\` failed${r.error ? `: ${r.error.message}` : ''}`);
    process.exit(1);
  }
}

function capture(cmd, args) {
  const r = spawnSync(cmd, args, { encoding: 'utf8', cwd: repo, maxBuffer: 64 * 1024 * 1024 });
  return r.error || r.status !== 0 ? null : r.stdout.trim();
}

// The wasm-bindgen CLI must be the exact version the crate pins.
const cargoToml = readFileSync(join(repo, 'crates', 'veld-wasm', 'Cargo.toml'), 'utf8');
const pinned = /wasm-bindgen\s*=\s*"=([^"]+)"/.exec(cargoToml)?.[1];
if (!pinned) {
  console.error('build-wasm: cannot find the pinned wasm-bindgen version in crates/veld-wasm/Cargo.toml');
  process.exit(1);
}

if (capture('cargo', ['--version']) === null) {
  console.error('build-wasm: `cargo` was not found. Install Rust from https://rustup.rs, then run `npm run build:wasm`.');
  process.exit(1);
}
const targets = capture('rustup', ['target', 'list', '--installed']);
if (targets !== null && !targets.split(/\s+/).includes('wasm32-unknown-unknown')) {
  console.log('build-wasm: adding the wasm32-unknown-unknown target');
  run('rustup', ['target', 'add', 'wasm32-unknown-unknown']);
}
const have = capture('wasm-bindgen', ['--version'])?.split(' ')[1];
if (have !== pinned) {
  console.log(`build-wasm: installing wasm-bindgen-cli ${pinned} (found ${have ?? 'none'})`);
  run('cargo', ['install', 'wasm-bindgen-cli', '--version', `=${pinned}`, '--locked']);
}

console.log('build-wasm: compiling crates/veld-wasm');
run('cargo', ['build', '-p', 'veld-wasm', '--target', 'wasm32-unknown-unknown', '--profile', 'wasm']);

// The target directory can be redirected; ask cargo where it is.
const meta = capture('cargo', ['metadata', '--format-version', '1', '--no-deps']);
const targetDir = meta ? JSON.parse(meta).target_directory : join(repo, 'target');
const built = join(targetDir, 'wasm32-unknown-unknown', 'wasm', 'veld_wasm.wasm');

mkdirSync(outDir, { recursive: true });
run('wasm-bindgen', [built, '--target', 'web', '--out-dir', outDir, '--out-name', 'veld_wasm']);

// Optional: wasm-opt (binaryen) shrinks the module further when it is installed.
if (capture('wasm-opt', ['--version']) !== null) {
  console.log('build-wasm: running wasm-opt -Oz');
  run('wasm-opt', ['-Oz', '--all-features', wasmFile, '-o', wasmFile]);
}

const raw = statSync(wasmFile).size;
const gz = gzipSync(readFileSync(wasmFile), { level: 9 }).length;
const kb = (n) => `${(n / 1024).toFixed(0)} KiB`;
console.log(`build-wasm: veld_wasm_bg.wasm ${kb(raw)} raw, ${kb(gz)} gzip -> ${outDir}`);
