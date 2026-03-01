/**
 * Veld CLI — postinstall script
 *
 * Downloads the correct pre-built Veld binary for the current platform
 * from GitHub Releases. Falls back to `go install` if the download fails.
 *
 * Supported platforms:
 *   - linux-amd64, linux-arm64
 *   - darwin-amd64, darwin-arm64
 *   - windows-amd64
 */

"use strict";

const https = require("https");
const http = require("http");
const fs = require("fs");
const path = require("path");
const os = require("os");
const { execSync } = require("child_process");
const zlib = require("zlib");

const VERSION = "0.1.0";
const GITHUB_REPO = "Adhamzineldin/Veld";
const BASE_URL = `https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}`;

function getPlatformKey() {
  const platform = os.platform();
  const arch = os.arch();

  const platformMap = {
    linux: "linux",
    darwin: "darwin",
    win32: "windows",
  };

  const archMap = {
    x64: "amd64",
    arm64: "arm64",
  };

  const p = platformMap[platform];
  const a = archMap[arch];

  if (!p || !a) {
    return null;
  }

  return `${p}-${a}`;
}

function getBinaryName() {
  return os.platform() === "win32" ? "veld.exe" : "veld";
}

function getDownloadUrl(platformKey) {
  const ext = os.platform() === "win32" ? ".zip" : ".tar.gz";
  return `${BASE_URL}/veld-${platformKey}${ext}`;
}

function download(url) {
  return new Promise((resolve, reject) => {
    const get = url.startsWith("https") ? https.get : http.get;
    get(url, (res) => {
      // Follow redirects (GitHub sends 302)
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        return download(res.headers.location).then(resolve).catch(reject);
      }
      if (res.statusCode !== 200) {
        reject(new Error(`HTTP ${res.statusCode} downloading ${url}`));
        return;
      }
      const chunks = [];
      res.on("data", (chunk) => chunks.push(chunk));
      res.on("end", () => resolve(Buffer.concat(chunks)));
      res.on("error", reject);
    }).on("error", reject);
  });
}

async function extractTarGz(buffer, destDir) {
  // Write to temp file and use tar
  const tmpFile = path.join(os.tmpdir(), `veld-${Date.now()}.tar.gz`);
  fs.writeFileSync(tmpFile, buffer);
  fs.mkdirSync(destDir, { recursive: true });
  try {
    execSync(`tar -xzf "${tmpFile}" -C "${destDir}"`, { stdio: "pipe" });
  } finally {
    fs.unlinkSync(tmpFile);
  }
}

async function extractZip(buffer, destDir) {
  // Write to temp file and use PowerShell / unzip
  const tmpFile = path.join(os.tmpdir(), `veld-${Date.now()}.zip`);
  fs.writeFileSync(tmpFile, buffer);
  fs.mkdirSync(destDir, { recursive: true });
  try {
    if (os.platform() === "win32") {
      execSync(
        `powershell -Command "Expand-Archive -Force -Path '${tmpFile}' -DestinationPath '${destDir}'"`,
        { stdio: "pipe" }
      );
    } else {
      execSync(`unzip -o "${tmpFile}" -d "${destDir}"`, { stdio: "pipe" });
    }
  } finally {
    fs.unlinkSync(tmpFile);
  }
}

async function tryGoInstall() {
  console.log("Attempting fallback: go install github.com/Adhamzineldin/Veld/cmd/veld@latest");
  try {
    execSync("go install github.com/Adhamzineldin/Veld/cmd/veld@latest", {
      stdio: "inherit",
    });
    console.log("✓ Installed veld via go install");
    return true;
  } catch {
    return false;
  }
}

async function main() {
  const platformKey = getPlatformKey();

  if (!platformKey) {
    console.warn(
      `Warning: Unsupported platform ${os.platform()}-${os.arch()}.`
    );
    console.warn("Attempting go install fallback...");
    if (await tryGoInstall()) return;
    console.warn("Install veld manually: go install github.com/Adhamzineldin/Veld/cmd/veld@latest");
    return;
  }

  const url = getDownloadUrl(platformKey);
  const destDir = path.join(__dirname, "bin-platform");
  const binaryName = getBinaryName();
  const binaryPath = path.join(destDir, binaryName);

  // Skip if already downloaded
  if (fs.existsSync(binaryPath)) {
    console.log(`✓ veld binary already exists at ${binaryPath}`);
    return;
  }

  console.log(`Downloading veld ${VERSION} for ${platformKey}...`);
  console.log(`  ${url}`);

  try {
    const buffer = await download(url);

    if (os.platform() === "win32") {
      await extractZip(buffer, destDir);
    } else {
      await extractTarGz(buffer, destDir);
    }

    // Make binary executable on Unix
    if (os.platform() !== "win32") {
      fs.chmodSync(binaryPath, 0o755);
    }

    if (fs.existsSync(binaryPath)) {
      console.log(`✓ veld ${VERSION} installed successfully`);
    } else {
      throw new Error("Binary not found after extraction");
    }
  } catch (err) {
    console.warn(`Warning: Could not download pre-built binary: ${err.message}`);
    console.warn("");

    if (await tryGoInstall()) return;

    console.warn("Install veld manually:");
    console.warn("  go install github.com/Adhamzineldin/Veld/cmd/veld@latest");
    console.warn("");
    console.warn("Or download from: https://github.com/Adhamzineldin/Veld/releases");
  }
}

main();

