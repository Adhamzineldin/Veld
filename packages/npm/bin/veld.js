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
  console.error("Or install the binary manually:");
  console.error("  go install github.com/Adhamzineldin/Veld/cmd/veld@latest");
  console.error("  pip install maayn-veld");
  console.error("  brew install maayn-veld/tap/maayn-veld");
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
