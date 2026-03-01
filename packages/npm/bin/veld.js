#!/usr/bin/env node

/**
 * Veld CLI — npm wrapper
 *
 * This thin wrapper spawns the platform-specific Veld binary that was
 * downloaded during `npm install` (postinstall). All CLI arguments are
 * forwarded as-is.
 */

"use strict";

const { execFileSync } = require("child_process");
const path = require("path");
const fs = require("fs");
const os = require("os");

function getBinaryName() {
  return os.platform() === "win32" ? "veld.exe" : "veld";
}

function getBinaryPath() {
  // Check local node_modules/.veld-bin first (postinstall puts it here)
  const localBin = path.join(__dirname, "..", "bin-platform", getBinaryName());
  if (fs.existsSync(localBin)) {
    return localBin;
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
  console.error("Try reinstalling: npm install veld");
  console.error("");
  console.error("Or install manually:");
  console.error("  go install github.com/Adhamzineldin/Veld/cmd/veld@latest");
  console.error("");
  console.error(err.message);
  process.exit(1);
}

