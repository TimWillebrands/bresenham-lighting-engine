#!/usr/bin/env bun

/**
 * Simple file watcher for WASM rebuilding during development
 *
 * Watches Rust source files and automatically rebuilds the WASM module
 * when changes are detected. No external dependencies required.
 */

import { spawn } from "child_process";
import { watch } from "fs";
import path from "path";

const PROJECT_ROOT = path.resolve("../..");
const DEMO_ROOT = process.cwd();
const SRC_DIR = path.join(PROJECT_ROOT, "src");

console.log("👀 WASM Watch Mode");
console.log("=".repeat(40));
console.log("📁 Watching:", SRC_DIR);
console.log("📦 Output:", path.join(DEMO_ROOT, "pkg"));
console.log("🔄 Auto-rebuild enabled");
console.log("");

let isBuilding = false;
let buildQueued = false;

function runBuild() {
  if (isBuilding) {
    buildQueued = true;
    return;
  }

  isBuilding = true;
  const startTime = Date.now();

  console.log("🔨 Building WASM module...");

  const child = spawn("wasm-pack", [
    "build",
    "--target", "web",
    "--out-dir", path.join(DEMO_ROOT, "pkg"),
    "--release",
    PROJECT_ROOT
  ], {
    stdio: "pipe"
  });

  let output = "";
  let errorOutput = "";

  child.stdout.on("data", (data) => {
    output += data.toString();
  });

  child.stderr.on("data", (data) => {
    errorOutput += data.toString();
  });

  child.on("close", (code) => {
    const duration = Date.now() - startTime;

    if (code === 0) {
      console.log(`✅ Build completed in ${duration}ms`);
      if (output.includes("warning")) {
        console.log("⚠️  Build completed with warnings");
      }
    } else {
      console.log(`❌ Build failed (exit code: ${code})`);
      if (errorOutput) {
        console.log("Error output:");
        console.log(errorOutput);
      }
    }

    isBuilding = false;

    // If another build was queued while we were building, start it now
    if (buildQueued) {
      buildQueued = false;
      setTimeout(runBuild, 100); // Small delay to avoid rapid rebuilds
    }
  });

  child.on("error", (error) => {
    console.error("❌ Failed to start build process:", error.message);
    isBuilding = false;
  });
}

// Initial build
console.log("🚀 Starting initial build...");
runBuild();

// Watch for changes
let debounceTimer = null;

watch(SRC_DIR, { recursive: true }, (eventType, filename) => {
  if (filename && filename.endsWith('.rs')) {
    console.log(`📝 File changed: ${filename}`);

    // Debounce rapid file changes
    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }

    debounceTimer = setTimeout(() => {
      runBuild();
    }, 500); // Wait 500ms after last change
  }
});

console.log("🎯 Watching for changes... Press Ctrl+C to stop");

// Graceful shutdown
process.on('SIGINT', () => {
  console.log('\n👋 Stopping watch mode...');
  process.exit(0);
});

process.on('SIGTERM', () => {
  console.log('\n👋 Stopping watch mode...');
  process.exit(0);
});
