#!/usr/bin/env node

/**
 * Veld CLI — npm wrapper
 *
 * This wrapper uses the bundled binary for the current platform.
 * The binary is downloaded by the postinstall script (install.js).
 */

"use strict";

const { spawnSync } = require("child_process");
const path = require("path");
const fs = require("fs");
const os = require("os");

function getPlatformKey() {
  const targets = {
    "linux-x64": "x86_64-unknown-linux-musl",
    "linux-arm64": "aarch64-unknown-linux-musl",
    "darwin-x64": "x86_64-apple-darwin",
    "darwin-arm64": "aarch64-apple-darwin",
    "win32-x64": "x86_64-pc-windows-msvc",
  };
  return targets[os.platform() + "-" + os.arch()] || null;
}

function getBinaryName() {
  return os.platform() === "win32" ? "veld.exe" : "veld";
}

function getBinaryPath() {
  // 1. Use bundled binary for this platform (downloaded by postinstall)
  const platformKey = getPlatformKey();
  if (platformKey) {
    const bundledBin = path.join(__dirname, "..", "binaries", platformKey, getBinaryName());
    if (fs.existsSync(bundledBin)) {
      return bundledBin;
    }
  }

  // 2. Check if veld binary was placed at package root (single-binary layout)
  const rootBin = path.join(__dirname, "..", getBinaryName());
  if (fs.existsSync(rootBin)) {
    return rootBin;
  }

  // 3. Not found — return null so we can show a helpful error
  return null;
}

const binary = getBinaryPath();

if (!binary) {
  console.error("Error: Veld binary not found for your platform.");
  console.error("");
  console.error("The postinstall script may have failed. Try:");
  console.error("  npm rebuild @maayn/veld");
  console.error("");
  console.error("Or install another way:");
  console.error("  brew install maayn-veld/tap/maayn-veld");
  console.error("  cargo install --git https://github.com/Adhamzineldin/Veld veld");
  process.exit(1);
}

const args = process.argv.slice(2);

// Use spawnSync with shell:false to avoid opening a new terminal window on Windows.
// stdio:"inherit" ensures stdin is passed through for interactive commands (e.g. veld init).
const result = spawnSync(binary, args, {
  stdio: "inherit",
  env: process.env,
  shell: false,
  windowsHide: true,
});

if (result.error) {
  if (result.error.code === "ENOENT") {
    console.error("Error: Veld binary not found at: " + binary);
    console.error("Try reinstalling: npm install @maayn/veld");
  } else {
    console.error("Error running veld:", result.error.message);
  }
  process.exit(1);
}

process.exit(result.status || 0);
