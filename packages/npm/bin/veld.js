#!/usr/bin/env node

/**
 * Veld CLI — npm wrapper
 *
 * This wrapper uses the bundled binary for the current platform.
 * Binaries are included in the package for all supported platforms.
 */

"use strict";

const { execFileSync } = require("child_process");
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
  // Use bundled binary for this platform
  const platformKey = getPlatformKey();
  if (platformKey) {
    const bundledBin = path.join(__dirname, "..", "binaries", platformKey, getBinaryName());
    if (fs.existsSync(bundledBin)) {
      return bundledBin;
    }
  }

  // Fallback: check if veld is on PATH
  const envPath = os.platform() === "win32" ? "veld.exe" : "veld";
  return envPath;
}

try {
  const binary = getBinaryPath();
  const args = process.argv.slice(2);

  const result = execFileSync(binary, args, {
    stdio: "inherit",
    env: process.env,
  });

  process.exit(0);
} catch (err) {
  if (err.status !== undefined) {
    process.exit(err.status);
  }
  console.error("Error: Could not run veld binary.");
  console.error("Try reinstalling: npm install @maayn/veld");
  console.error("");
  console.error("Or install manually:");
  console.error("  go install github.com/Adhamzineldin/Veld/cmd/veld@latest");
  console.error("");
  console.error(err.message);
  process.exit(1);
}

