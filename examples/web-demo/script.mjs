import init, {
  put,
  set_tile,
  get_tiles,
  get_blockmap,
} from "./pkg/bresenham_lighting_engine.js";

// Global state
let wasmModule = null;
let animationId = null;
let lastFrameTime = 0;
let frameCount = 0;
let fps = 0;

// Canvas elements
const canvas = document.getElementById("game");
const ctx = canvas.getContext("2d");
const wallsCanvas = document.getElementById("walls");
const wallsCtx = wallsCanvas.getContext("2d", { willReadFrequently: true });

// UI elements
const loadingDiv = document.getElementById("loading");
const errorDiv = document.getElementById("error");
const errorMessage = document.getElementById("error-message");
const controlsForm = document.getElementById("controls");
const canvasContainer = document.getElementById("canvas-container");
const instructions = document.getElementById("instructions");
const perfCounter = document.getElementById("performance");

// Performance tracking
const perfMetrics = {
  init: 0,
  update: 0,
  canvas: 0,
  fps: 0,
};

// Wall data storage (simple pixel-based for demo)
const wallData = new Uint8Array(180 * 180);

// IsBlocked function that WASM will call
globalThis.IsBlocked = function (x0, y0, x1, y1) {
  // Simple line-walking algorithm to check if path is blocked
  const dx = Math.abs(x1 - x0);
  const dy = Math.abs(y1 - y0);
  const sx = x0 < x1 ? 1 : -1;
  const sy = y0 < y1 ? 1 : -1;
  let err = dx - dy;

  let x = x0;
  let y = y0;

  while (true) {
    // Check bounds
    if (x < 0 || x >= 180 || y < 0 || y >= 180) {
      return false;
    }

    // Check if this pixel is a wall
    const pixelData = wallsCtx.getImageData(x, y, 1, 1).data;
    if (pixelData[3] > 128) {
      // If alpha > 128, consider it a wall
      return true;
    }

    // If we've reached the destination, no blocking found
    if (x === x1 && y === y1) {
      break;
    }

    const e2 = 2 * err;
    if (e2 > -dy) {
      err -= dy;
      x += sx;
    }
    if (e2 < dx) {
      err += dx;
      y += sy;
    }
  }

  return false;
};

// Log function for debugging
globalThis.Log = function (message) {
  console.log("[WASM]", message);
};

function showError(message) {
  errorMessage.textContent = message;
  errorDiv.style.display = "block";
  loadingDiv.style.display = "none";
}

function hideLoading() {
  loadingDiv.style.display = "none";
  controlsForm.style.display = "grid";
  canvasContainer.style.display = "block";
  instructions.style.display = "block";
  perfCounter.style.display = "block";
}

function updatePerformanceDisplay() {
  document.getElementById("perf-init").textContent =
    `Initialization: ${perfMetrics.init.toFixed(2)}ms`;
  document.getElementById("perf-update").textContent =
    `Light Update: ${perfMetrics.update.toFixed(2)}ms`;
  document.getElementById("perf-canvas").textContent =
    `Canvas Render: ${perfMetrics.canvas.toFixed(2)}ms`;
  document.getElementById("perf-fps").textContent = `FPS: ${fps}`;
}

function updateControlLabels() {
  const formData = new FormData(controlsForm);
  document.getElementById("x-value").textContent = formData.get("x");
  document.getElementById("y-value").textContent = formData.get("y");
  document.getElementById("radius-value").textContent = formData.get("radius");
}

function updateLighting() {
  if (!wasmModule) return;

  const formData = new FormData(controlsForm);
  const x = parseInt(formData.get("x"));
  const y = parseInt(formData.get("y"));
  const radius = parseInt(formData.get("radius"));

  // Time the light update
  const updateStart = performance.now();
  const canvasPtr = put(0, radius, x, y);
  const updateEnd = performance.now();
  perfMetrics.update = updateEnd - updateStart;

  if (canvasPtr === 0) {
    console.warn("Light update returned null pointer");
    return;
  }

  // Time the canvas rendering
  const canvasStart = performance.now();

  // Clear the canvas
  ctx.clearRect(0, 0, 180, 180);

  // Get the light canvas data from WASM memory
  const lightSize = radius * 2 + 1;

  if (canvasPtr === 0) {
    console.warn("Light update returned null pointer");
    return;
  }

  // Access WASM memory through the initialized module
  if (!wasmModule || !wasmModule.memory) {
    console.warn("WASM memory not available");
    return;
  }

  const lightData = new Uint8ClampedArray(
    wasmModule.memory.buffer,
    canvasPtr,
    lightSize * lightSize * 4,
  );

  // Create and draw the light image
  const imageData = new ImageData(lightData, lightSize, lightSize);
  ctx.putImageData(
    imageData,
    x - Math.floor(lightSize / 2),
    y - Math.floor(lightSize / 2),
  );

  // Draw black background behind the light
  ctx.globalCompositeOperation = "destination-over";
  ctx.fillStyle = "black";
  ctx.fillRect(0, 0, 180, 180);
  ctx.globalCompositeOperation = "source-over";

  const canvasEnd = performance.now();
  perfMetrics.canvas = canvasEnd - canvasStart;

  updatePerformanceDisplay();
}

function drawWall(ev) {
  ev.preventDefault();
  ev.stopPropagation();

  const rect = wallsCanvas.getBoundingClientRect();
  const scaleX = 180 / rect.width;
  const scaleY = 180 / rect.height;

  if (ev.buttons === 1 || ev.buttons === 2) {
    const x = Math.floor(ev.offsetX * scaleX);
    const y = Math.floor(ev.offsetY * scaleY);

    if (x < 0 || x >= 180 || y < 0 || y >= 180) return;

    const isErasing = ev.buttons === 2;
    const color = isErasing ? "rgba(0,0,0,0)" : "rgba(255,255,255,255)";

    // Draw a small brush
    wallsCtx.fillStyle = color;
    wallsCtx.fillRect(x - 1, y - 1, 3, 3);

    // Update lighting
    updateLighting();
  }

  if (ev.buttons === 4) {
    // Middle mouse button
    const x = Math.floor(ev.offsetX * scaleX);
    const y = Math.floor(ev.offsetY * scaleY);

    if (x < 0 || x >= 180 || y < 0 || y >= 180) return;

    // Update light position
    document.getElementById("x").value = x;
    document.getElementById("y").value = y;
    updateControlLabels();
    updateLighting();
  }
}

function calculateFPS() {
  const now = performance.now();
  frameCount++;

  if (now - lastFrameTime >= 1000) {
    fps = Math.round((frameCount * 1000) / (now - lastFrameTime));
    frameCount = 0;
    lastFrameTime = now;
    updatePerformanceDisplay();
  }

  animationId = requestAnimationFrame(calculateFPS);
}

async function initializeDemo() {
  try {
    // Initialize WASM module
    const initStart = performance.now();
    wasmModule = await init();
    const initEnd = performance.now();
    perfMetrics.init = initEnd - initStart;

    console.log("WASM module initialized successfully");
    console.log("WASM module has memory:", !!wasmModule.memory);

    // Set up event listeners
    controlsForm.addEventListener("input", function (ev) {
      updateControlLabels();
      updateLighting();
    });

    wallsCanvas.addEventListener("pointermove", drawWall);
    wallsCanvas.addEventListener("pointerdown", drawWall);

    // Prevent context menu on right click
    wallsCanvas.addEventListener("contextmenu", (e) => e.preventDefault());

    // Initial setup
    updateControlLabels();
    updateLighting();

    // Hide loading and show interface
    hideLoading();

    // Start FPS counter
    lastFrameTime = performance.now();
    calculateFPS();

    console.log("Demo initialized successfully");
  } catch (error) {
    console.error("Failed to initialize demo:", error);
    showError(`Failed to initialize: ${error.message}`);
  }
}

// Start the demo when the page loads
initializeDemo();
