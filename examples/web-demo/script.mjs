import init, {
    put,
    set_tile,
    get_tiles,
    get_blockmap,
    set_collision_mode,
    get_collision_mode,
    set_pixel,
    set_pixel_batch,
    clear_pixel_collisions,
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

// Wall data storage - keep for compatibility with existing drawing logic
const wallPixels = new Set();

// Note: The old IsBlocked function is no longer needed since we use native Rust collision detection

// Set up logging function BEFORE importing WASM to avoid initialization errors
globalThis.log_from_js = function (message) {
    console.log("[WASM]", message);
};

// Simplified console.log_from_js for compatibility  
globalThis.console = globalThis.console || {};
globalThis.console.log_from_js = globalThis.log_from_js;

function showError(message) {
    errorMessage.textContent = message;
    errorDiv.style.display = "block";
    loadingDiv.style.display = "none";
    console.error("Demo Error:", message);
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
    if (!wasmModule) {
        console.warn("WASM module not initialized");
        return;
    }

    const formData = new FormData(controlsForm);
    const x = parseInt(formData.get("x"));
    const y = parseInt(formData.get("y"));
    const radius = parseInt(formData.get("radius"));

    // Validate inputs
    if (isNaN(x) || isNaN(y) || isNaN(radius)) {
        console.warn("Invalid input values");
        return;
    }

    // Time the light update
    const updateStart = performance.now();
    let canvasPtr;

    try {
        canvasPtr = put(0, radius, x, y);
    } catch (error) {
        console.error("Error calling put():", error);
        console.error("Error details:", {
            radius: radius,
            x: x,
            y: y,
            wasmModule: !!wasmModule,
            collisionMode: get_collision_mode && get_collision_mode()
        });
        return;
    }

    const updateEnd = performance.now();
    perfMetrics.update = updateEnd - updateStart;

    if (canvasPtr === 0) {
        console.warn("Light update returned null pointer");
        return;
    }

    // Time the canvas rendering
    const canvasStart = performance.now();

    try {
        // Clear the canvas
        ctx.clearRect(0, 0, 180, 180);

        // Get the light canvas data from WASM memory
        const lightSize = radius * 2 + 1;

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

    } catch (error) {
        console.error("Error rendering canvas:", error);
    }

    const canvasEnd = performance.now();
    perfMetrics.canvas = canvasEnd - canvasStart;

    updatePerformanceDisplay();
}

function updateWallPixel(x, y, isWall) {
    const pixelKey = `${x},${y}`;
    if (isWall) {
        wallPixels.add(pixelKey);
    } else {
        wallPixels.delete(pixelKey);
    }
    
    // Update the native Rust collision system
    set_pixel(x, y, isWall ? 1 : 0);
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

        // Draw a small brush (3x3 pixels for better visibility)
        wallsCtx.fillStyle = color;
        for (let dx = -1; dx <= 1; dx++) {
            for (let dy = -1; dy <= 1; dy++) {
                const brushX = x + dx;
                const brushY = y + dy;
                if (brushX >= 0 && brushX < 180 && brushY >= 0 && brushY < 180) {
                    wallsCtx.fillRect(brushX, brushY, 1, 1);
                    updateWallPixel(brushX, brushY, !isErasing);
                }
            }
        }

        // Update lighting
        updateLighting();
    }

    if (ev.buttons === 4) {
        // Middle mouse button - move light
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

// Enhanced error handling for WASM loading
async function loadWasmModule() {
    try {
        // Try to load the WASM module with enhanced error handling
        console.log("Loading WASM module...");

        // Check if we're running in a secure context (required for some WASM features)
        if (typeof window !== 'undefined' && !window.isSecureContext) {
            console.warn("Not running in secure context - some features may be limited");
        }

        wasmModule = await init();

        if (!wasmModule) {
            throw new Error("WASM module initialization returned null");
        }

        if (!wasmModule.memory) {
            throw new Error("WASM module memory not available");
        }

        console.log("WASM module loaded successfully");
        console.log("Memory buffer size:", wasmModule.memory.buffer.byteLength);

        return wasmModule;

    } catch (error) {
        console.error("Failed to load WASM module:", error);

        // Provide helpful error messages for common issues
        if (error.message.includes("fetch")) {
            throw new Error("Failed to fetch WASM file. Make sure the server is running and files are accessible.");
        } else if (error.message.includes("compile")) {
            throw new Error("Failed to compile WASM module. The WASM file may be corrupted.");
        } else if (error.message.includes("instantiate")) {
            throw new Error("Failed to instantiate WASM module. Check browser compatibility.");
        }

        throw error;
    }
}

async function initializeDemo() {
    try {
        console.log("🚀 Initializing Bresenham Lighting Engine Demo");

        // Check browser compatibility
        if (typeof WebAssembly === 'undefined') {
            throw new Error("WebAssembly is not supported in this browser");
        }

        // Initialize WASM module with timing
        const initStart = performance.now();
        await loadWasmModule();
        const initEnd = performance.now();
        perfMetrics.init = initEnd - initStart;

        console.log(`✅ WASM initialization completed in ${perfMetrics.init.toFixed(2)}ms`);

        // Switch to pixel-based collision detection for better performance
        try {
            set_collision_mode(1); // 1 = Pixel mode
            console.log(`🚀 Switched to pixel-based collision detection for maximum performance`);
            console.log(`Current collision mode: ${get_collision_mode()}`);
        } catch (error) {
            console.error("Failed to set collision mode:", error);
            throw error;
        }

        // Set up event listeners
        controlsForm.addEventListener("input", function (ev) {
            updateControlLabels();
            updateLighting();
        });

        // Enhanced pointer events for better touch support
        wallsCanvas.addEventListener("pointermove", drawWall);
        wallsCanvas.addEventListener("pointerdown", drawWall);
        wallsCanvas.addEventListener("touchstart", (e) => e.preventDefault());
        wallsCanvas.addEventListener("touchmove", (e) => e.preventDefault());

        // Prevent context menu on right click
        wallsCanvas.addEventListener("contextmenu", (e) => e.preventDefault());

        // Add keyboard shortcuts
        document.addEventListener("keydown", (e) => {
            if (e.key === "c" && e.ctrlKey) {
                // Ctrl+C: Clear walls
                wallsCtx.clearRect(0, 0, 180, 180);
                wallPixels.clear();
                clear_pixel_collisions(); // Clear native collision system
                updateLighting();
                console.log("🧹 Cleared all walls and collision data");
                e.preventDefault();
            }
        });

        // Initial setup
        updateControlLabels();
        updateLighting();

        // Hide loading and show interface
        hideLoading();

        // Start FPS counter
        lastFrameTime = performance.now();
        calculateFPS();

        console.log("🎉 Demo initialized successfully!");
        console.log("💡 Tips:");
        console.log("  - Left click + drag: Draw walls");
        console.log("  - Right click + drag: Erase walls");
        console.log("  - Middle click: Move light");
        console.log("  - Ctrl+C: Clear all walls");
        console.log("");
        console.log("⚡ Performance Notes:");
        console.log("  - Using native Rust collision detection (pixel mode)");
        console.log("  - Expect ~1-5ms light updates vs ~250ms with old JavaScript bridge");
        console.log("  - 50x+ performance improvement for real-time lighting!");

    } catch (error) {
        console.error("Failed to initialize demo:", error);
        showError(`Failed to initialize: ${error.message}`);

        // Add some troubleshooting info
        console.log("🔧 Troubleshooting info:");
        console.log("  - Browser:", navigator.userAgent);
        console.log("  - WebAssembly support:", typeof WebAssembly !== 'undefined');
        console.log("  - Secure context:", window.isSecureContext);
        console.log("  - Location:", window.location.href);
    }
}

// Enhanced startup with better error handling
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initializeDemo);
} else {
    // DOM is already loaded
    initializeDemo();
}

// Cleanup on page unload
window.addEventListener('beforeunload', () => {
    if (animationId) {
        cancelAnimationFrame(animationId);
    }
});

// Export for debugging
window.demoDebug = {
    wasmModule,
    perfMetrics,
    wallPixels,
    updateLighting,
    fps: () => fps
};
