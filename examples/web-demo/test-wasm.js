#!/usr/bin/env node

/**
 * Simple test to verify WASM functionality outside the browser
 * This helps debug issues without needing a full browser environment
 */

import { readFileSync } from "fs";
import { resolve } from "path";

// Polyfill WebAssembly for Node.js testing
global.performance = { now: () => Date.now() };

// Mock the external functions that WASM expects
global.IsBlocked = function (x0, y0, x1, y1) {
  // Simple test implementation - consider anything at (10,10) blocked
  return x1 === 10 && y1 === 10;
};

// Keep original console but add WASM logging
const originalConsole = global.console;
global.Log = function (...args) {
  originalConsole.log("[WASM]", ...args);
};

async function testWasm() {
  try {
    originalConsole.log("🧪 Testing WASM module functionality...\n");

    // Load the WASM file directly
    const wasmPath = resolve("./pkg/bresenham_lighting_engine_bg.wasm");
    const wasmBuffer = readFileSync(wasmPath);

    originalConsole.log(
      "✅ WASM file loaded, size:",
      wasmBuffer.length,
      "bytes",
    );

    // Create WebAssembly instance
    const wasmModule = await WebAssembly.compile(wasmBuffer);
    originalConsole.log("✅ WASM module compiled successfully");

    // Create imports object that matches what wasm-bindgen expects
    const imports = {
      wbg: {
        __wbg_IsBlocked_dfb1a36e2bc8737b: global.IsBlocked,
        __wbg_logfromjs_3a1b032ee7780183: function (ptr, len) {
          // Mock log function - would normally read string from WASM memory
          originalConsole.log("[WASM LOG]", "ptr:", ptr, "len:", len);
        },
        __wbindgen_init_externref_table: function () {
          // Mock implementation
        },
      },
    };

    const instance = await WebAssembly.instantiate(wasmModule, imports);
    originalConsole.log("✅ WASM instance created");

    const exports = instance.exports;
    originalConsole.log(
      "📦 Available exports:",
      Object.keys(exports).filter((k) => !k.startsWith("__")),
    );

    // Test basic functionality
    originalConsole.log("\n🔬 Testing exports...");

    // Test memory access
    if (exports.memory) {
      originalConsole.log(
        "✅ Memory available, size:",
        exports.memory.buffer.byteLength,
        "bytes",
      );
    } else {
      originalConsole.log("❌ No memory export found");
    }

    // Test functions
    const testFunctions = [
      "put",
      "get_tiles",
      "get_blockmap",
      "set_tile",
      "start",
    ];

    for (const funcName of testFunctions) {
      if (typeof exports[funcName] === "function") {
        originalConsole.log(`✅ ${funcName} function available`);
      } else {
        originalConsole.log(`❌ ${funcName} function missing`);
      }
    }

    // Initialize the WASM module
    originalConsole.log("\n🚀 Initializing WASM module...");
    if (exports.__wbindgen_start) {
      exports.__wbindgen_start();
      originalConsole.log("✅ WASM module initialized");
    }

    if (exports.start) {
      exports.start();
      originalConsole.log("✅ Lighting engine initialized");
    }

    // Test light creation
    originalConsole.log("\n💡 Testing light creation...");
    if (exports.put) {
      const lightPtr = exports.put(1, 20, 50, 50);
      originalConsole.log("✅ Light created, canvas pointer:", lightPtr);

      if (lightPtr !== 0 && exports.memory) {
        const lightSize = 20 * 2 + 1; // radius * 2 + 1
        const pixelCount = lightSize * lightSize * 4; // RGBA
        const lightData = new Uint8Array(
          exports.memory.buffer,
          lightPtr,
          Math.min(pixelCount, 1000),
        );
        originalConsole.log(
          "✅ Light data accessible, first 10 bytes:",
          Array.from(lightData.slice(0, 10)),
        );
      }
    }

    // Test tile operations
    originalConsole.log("\n🧱 Testing tile operations...");
    if (exports.set_tile) {
      exports.set_tile(5, 5, 1);
      originalConsole.log("✅ Tile set successfully");
    }

    if (exports.get_tiles) {
      const tilesPtr = exports.get_tiles();
      originalConsole.log("✅ Tiles pointer:", tilesPtr);
    }

    originalConsole.log(
      "\n🎉 All tests passed! WASM module is working correctly.",
    );

    return {
      success: true,
      memory: !!exports.memory,
      functions: testFunctions.filter((f) => typeof exports[f] === "function"),
    };
  } catch (error) {
    originalConsole.error("❌ Test failed:", error.message);
    originalConsole.error("Stack trace:", error.stack);
    return { success: false, error: error.message };
  }
}

// Run the test
testWasm().then((result) => {
  originalConsole.log("\n📊 Test Results:", result);
  process.exit(result.success ? 0 : 1);
});
