/**
 * Test of the npm wrapper (distribution/npm) against real release artifacts.
 *
 *   node distribution/tests/npm-wrapper.test.js <artifacts-dir>
 *
 * <artifacts-dir> holds the release assets: veld-<target>.tar.gz|zip and SHA256SUMS (for example
 * the artifacts of a release.yml dry run). The assets are served from a local HTTP server and
 * the wrapper is pointed at it with VELD_DOWNLOAD_BASE. Checks, for this platform:
 *   1. install.js downloads the right asset, verifies SHA256SUMS and installs a working binary;
 *   2. a tampered archive is rejected and nothing is installed;
 *   3. an asset that is missing from SHA256SUMS is rejected;
 *   4. `npm install <packed tarball>` followed by `npx veld --version` works.
 *
 * Set NPM_CLI to the path of npm-cli.js when `npm` cannot be spawned directly.
 */
"use strict";

const assert = require("assert");
const fs = require("fs");
const http = require("http");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");

const TARGETS = {
  "linux-x64": "x86_64-unknown-linux-musl",
  "linux-arm64": "aarch64-unknown-linux-musl",
  "darwin-x64": "x86_64-apple-darwin",
  "darwin-arm64": "aarch64-apple-darwin",
  "win32-x64": "x86_64-pc-windows-msvc",
};
const key = `${os.platform()}-${os.arch()}`;
const target = TARGETS[key];
const win = os.platform() === "win32";
const asset = `veld-${target}${win ? ".zip" : ".tar.gz"}`;
const binName = win ? "veld.exe" : "veld";
const pkgDir = path.resolve(__dirname, "..", "npm");
const version = require(path.join(pkgDir, "package.json")).version;

const artifacts = process.argv[2] && path.resolve(process.argv[2]);
if (!artifacts || !fs.existsSync(path.join(artifacts, "SHA256SUMS"))) {
  console.error("usage: node npm-wrapper.test.js <artifacts-dir containing SHA256SUMS>");
  process.exit(2);
}
if (!target) {
  console.error(`no release asset for ${key}`);
  process.exit(2);
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "veld-npm-test-"));

function serve(dir) {
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      const file = path.join(dir, path.basename(decodeURIComponent(req.url.split("?")[0])));
      if (!fs.existsSync(file) || !fs.statSync(file).isFile()) {
        res.writeHead(404).end();
        return;
      }
      res.writeHead(200).end(fs.readFileSync(file));
    });
    server.listen(0, "127.0.0.1", () =>
      resolve({ server, base: `http://127.0.0.1:${server.address().port}` }),
    );
  });
}

function copyPackage(name) {
  const dest = path.join(tmp, name);
  fs.cpSync(pkgDir, dest, {
    recursive: true,
    filter: (src) => !src.includes(`${path.sep}binaries`) && !src.includes("node_modules"),
  });
  return dest;
}

/**
 * Runs a command with its output captured (so a leaked child can never hold our pipes). It must
 * be asynchronous: the HTTP server serving the assets lives in this very process.
 */
function run(cmd, args, opts = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { windowsHide: true, stdio: ["ignore", "pipe", "pipe"], ...opts });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => (stdout += d));
    child.stderr.on("data", (d) => (stderr += d));
    const timer = setTimeout(() => child.kill("SIGKILL"), 180000);
    child.on("error", reject);
    child.on("close", (status) => {
      clearTimeout(timer);
      resolve({ status, stdout, stderr });
    });
  });
}

function npm(args, opts) {
  if (process.env.NPM_CLI) return run(process.execPath, [process.env.NPM_CLI, ...args], opts);
  return run("npm", args, { shell: win, ...opts });
}

function installJs(pkg, base) {
  return run(process.execPath, ["install.js"], {
    cwd: pkg,
    env: { ...process.env, VELD_DOWNLOAD_BASE: base },
  });
}

function mirror(name, edit) {
  const dir = path.join(tmp, name);
  fs.mkdirSync(dir);
  for (const f of fs.readdirSync(artifacts)) {
    if (fs.statSync(path.join(artifacts, f)).isFile()) fs.copyFileSync(path.join(artifacts, f), path.join(dir, f));
  }
  edit(dir);
  return dir;
}

async function main() {
  const good = await serve(artifacts);

  // 1. Happy path: the right asset, verified, installed, runs.
  const pkg = copyPackage("good");
  let r = await installJs(pkg, good.base);
  assert.strictEqual(r.status, 0, `install failed:\n${r.stdout}\n${r.stderr}`);
  assert.ok(r.stdout.includes(asset), `expected download of ${asset}:\n${r.stdout}`);
  const bin = path.join(pkg, "binaries", target, binName);
  assert.ok(fs.existsSync(bin), "binary not installed");
  r = await run(process.execPath, [path.join(pkg, "bin", "veld.js"), "--version"]);
  assert.strictEqual(r.status, 0, r.stderr);
  assert.ok(r.stdout.includes(version.replace(/-.*/, "")), `--version said ${r.stdout}`);
  console.log(`ok: installed ${asset}, \`veld --version\` -> ${r.stdout.trim()}`);

  // 2. A tampered archive is rejected and nothing is installed.
  const tampered = mirror("tampered", (dir) => {
    fs.appendFileSync(path.join(dir, asset), Buffer.from([0]));
  });
  const bad = await serve(tampered);
  const pkgBad = copyPackage("bad");
  r = await installJs(pkgBad, bad.base);
  assert.notStrictEqual(r.status, 0, "a tampered archive was accepted");
  assert.ok(/checksum mismatch/.test(r.stderr), r.stderr);
  assert.ok(!fs.existsSync(path.join(pkgBad, "binaries")), "something was installed from a tampered archive");
  console.log("ok: tampered archive rejected");

  // 3. An asset missing from SHA256SUMS is rejected.
  const unlisted = mirror("unlisted", (dir) => {
    const sums = fs.readFileSync(path.join(dir, "SHA256SUMS"), "utf8").split(/\r?\n/);
    fs.writeFileSync(path.join(dir, "SHA256SUMS"), sums.filter((l) => !l.includes(asset)).join("\n"));
  });
  const un = await serve(unlisted);
  const pkgUn = copyPackage("unlisted");
  r = await installJs(pkgUn, un.base);
  assert.notStrictEqual(r.status, 0, "an unlisted asset was accepted");
  assert.ok(/not listed in SHA256SUMS/.test(r.stderr), r.stderr);
  console.log("ok: asset missing from SHA256SUMS rejected");

  // 4. npm install of the packed tarball, then `npx veld --version`.
  const packed = await npm(["pack", "--pack-destination", tmp, pkgDir]);
  assert.strictEqual(packed.status, 0, packed.stderr);
  const tarball = path.join(tmp, packed.stdout.trim().split(/\r?\n/).pop());
  const project = path.join(tmp, "project");
  fs.mkdirSync(project);
  fs.writeFileSync(path.join(project, "package.json"), '{"name":"veld-npx-test","private":true}');
  const env = { ...process.env, VELD_DOWNLOAD_BASE: good.base };
  r = await npm(["install", tarball], { cwd: project, env });
  assert.strictEqual(r.status, 0, `npm install failed:\n${r.stdout}\n${r.stderr}`);
  r = await npm(["exec", "--no", "--", "veld", "--version"], { cwd: project, env });
  assert.strictEqual(r.status, 0, `npx veld --version failed:\n${r.stdout}\n${r.stderr}`);
  assert.ok(r.stdout.includes(version.replace(/-.*/, "")), r.stdout);
  console.log(`ok: npx veld --version -> ${r.stdout.trim()}`);

  for (const s of [good, bad, un]) s.server.close();
}

main()
  .then(() => {
    fs.rmSync(tmp, { recursive: true, force: true });
    console.log("npm wrapper: all checks passed");
  })
  .catch((e) => {
    console.error(e);
    fs.rmSync(tmp, { recursive: true, force: true });
    process.exit(1);
  });
