#!/usr/bin/env bun

/**
 * Development entry point for the Bresenham Lighting Engine web demo
 *
 * This script provides a convenient way to:
 * - Build the WASM module
 * - Start the development server
 * - Watch for changes and rebuild as needed
 */

import { spawn } from "child_process";
import { existsSync } from "fs";
import path from "path";

const PROJECT_ROOT = path.resolve("../..");
const DEMO_ROOT = process.cwd();

console.log("🚀 Bresenham Lighting Engine - Development Setup");
console.log("=".repeat(50));

function runCommand(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    console.log(`\n📦 Running: ${command} ${args.join(" ")}`);

    const child = spawn(command, args, {
      stdio: "inherit",
      shell: true,
      ...options,
    });

    child.on("close", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`Command failed with exit code ${code}`));
      }
    });

    child.on("error", reject);
  });
}

async function checkPrerequisites() {
  console.log("\n🔍 Checking prerequisites...");

  // Check if wasm-pack is installed
  try {
    await runCommand("wasm-pack", ["--version"]);
    console.log("✅ wasm-pack is installed");
  } catch (error) {
    console.error("❌ wasm-pack is not installed");
    console.log("📥 Install it with: cargo install wasm-pack");
    process.exit(1);
  }

  // Check if Cargo.toml exists
  const cargoPath = path.join(PROJECT_ROOT, "Cargo.toml");
  if (!existsSync(cargoPath)) {
    console.error("❌ Cargo.toml not found in project root");
    console.log("📁 Expected at:", cargoPath);
    process.exit(1);
  }
  console.log("✅ Cargo.toml found");
}

async function buildWasm() {
  console.log("\n🔨 Building WASM module...");

  try {
    await runCommand("wasm-pack", [
      "build",
      "--target",
      "web",
      "--out-dir",
      path.join(DEMO_ROOT, "pkg"),
      "--release",
      PROJECT_ROOT,
    ]);
    console.log("✅ WASM build completed");
  } catch (error) {
    console.error("❌ WASM build failed:", error.message);
    process.exit(1);
  }
}

async function startDevServer() {
  console.log("\n🌐 Starting development server...");

  try {
    // Start the server with hot reload
    await runCommand("bun", ["run", "server.js", "--hot"]);
  } catch (error) {
    console.error("❌ Server failed:", error.message);
    process.exit(1);
  }
}

async function main() {
  try {
    await checkPrerequisites();
    await buildWasm();

    console.log("\n🎉 Setup complete!");
    console.log("🔧 Starting development server with auto-reload...");
    console.log("🌍 Open http://localhost:3000 in your browser");
    console.log("⚡ Edit files and watch them reload automatically!");

    await startDevServer();
  } catch (error) {
    console.error("\n💥 Setup failed:", error.message);
    process.exit(1);
  }
}

// Handle graceful shutdown
process.on("SIGINT", () => {
  console.log("\n👋 Shutting down...");
  process.exit(0);
});

process.on("SIGTERM", () => {
  console.log("\n👋 Shutting down...");
  process.exit(0);
});

main();
