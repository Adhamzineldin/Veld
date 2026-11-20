/**
 * Veld CLI postinstall script.
 *
 * Downloads the pre-built `veld` binary that matches this package's version from
 * the GitHub release, verifies it against the release's SHA256SUMS, and unpacks it
 * to ./binaries/<target>/. A checksum mismatch is a hard error: nothing is installed.
 *
 * Release assets (see .github/workflows/release.yml):
 *   veld-<target-triple>.tar.gz   (veld-x86_64-pc-windows-msvc.zip on Windows)
 *   SHA256SUMS
 */

"use strict";

const https = require("https");
const fs = require("fs");
const path = require("path");
const os = require("os");
const crypto = require("crypto");
const { execFileSync } = require("child_process");

const GITHUB_REPO = "Adhamzineldin/Veld";
const VERSION = process.env.VELD_VERSION
  ? process.env.VELD_VERSION.replace(/^v/, "")
  : require("./package.json").version;

const TARGETS = {
  "linux-x64": "x86_64-unknown-linux-musl",
  "linux-arm64": "aarch64-unknown-linux-musl",
  "darwin-x64": "x86_64-apple-darwin",
  "darwin-arm64": "aarch64-apple-darwin",
  "win32-x64": "x86_64-pc-windows-msvc",
};

function download(url, redirects = 5) {
  return new Promise((resolve, reject) => {
    https
      .get(url, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          res.resume();
          if (redirects === 0) return reject(new Error(`too many redirects for ${url}`));
          return download(res.headers.location, redirects - 1).then(resolve, reject);
        }
        if (res.statusCode !== 200) {
          res.resume();
          return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
        }
        const chunks = [];
        res.on("data", (c) => chunks.push(c));
        res.on("end", () => resolve(Buffer.concat(chunks)));
        res.on("error", reject);
      })
      .on("error", reject);
  });
}

function expectedSha(sums, asset) {
  for (const line of sums.split(/\r?\n/)) {
    const m = line.match(/^([0-9a-fA-F]{64})\s+\*?(.+)$/);
    if (m && m[2].trim() === asset) return m[1].toLowerCase();
  }
  return null;
}

function extract(buffer, asset, destDir) {
  const tmp = path.join(os.tmpdir(), `veld-${process.pid}-${asset}`);
  fs.writeFileSync(tmp, buffer);
  fs.mkdirSync(destDir, { recursive: true });
  try {
    if (asset.endsWith(".zip")) {
      if (os.platform() === "win32") {
        execFileSync("powershell", [
          "-NoProfile", "-Command",
          `Expand-Archive -Force -Path '${tmp}' -DestinationPath '${destDir}'`,
        ], { stdio: "pipe" });
      } else {
        execFileSync("unzip", ["-o", tmp, "-d", destDir], { stdio: "pipe" });
      }
    } else {
      execFileSync("tar", ["-xzf", tmp, "-C", destDir], { stdio: "pipe" });
    }
  } finally {
    fs.unlinkSync(tmp);
  }
}

async function main() {
  const key = `${os.platform()}-${os.arch()}`;
  const target = TARGETS[key];
  if (!target) {
    console.error(`veld: no pre-built binary for ${key}. Build from source: cargo install --git https://github.com/${GITHUB_REPO} veld`);
    process.exit(1);
  }
  const win = os.platform() === "win32";
  const asset = `veld-${target}${win ? ".zip" : ".tar.gz"}`;
  const base = `https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}`;
  const destDir = path.join(__dirname, "binaries", target);
  const binary = path.join(destDir, win ? "veld.exe" : "veld");
  if (fs.existsSync(binary)) return;

  console.log(`veld: downloading ${asset} (v${VERSION})`);
  const [archive, sums] = await Promise.all([
    download(`${base}/${asset}`),
    download(`${base}/SHA256SUMS`),
  ]);
  const want = expectedSha(sums.toString("utf8"), asset);
  if (!want) throw new Error(`${asset} is not listed in SHA256SUMS`);
  const got = crypto.createHash("sha256").update(archive).digest("hex");
  if (got !== want) throw new Error(`checksum mismatch for ${asset}: expected ${want}, got ${got}`);

  extract(archive, asset, destDir);
  if (!fs.existsSync(binary)) throw new Error("binary not found after extraction");
  if (!win) fs.chmodSync(binary, 0o755);
  console.log(`veld: v${VERSION} installed`);
}

main().catch((err) => {
  console.error(`veld: install failed: ${err.message}`);
  console.error(`Download a release manually: https://github.com/${GITHUB_REPO}/releases`);
  process.exit(1);
});
