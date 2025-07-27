import { useState, useEffect } from 'https://esm.sh/preact/hooks';
import init, { set_collision_mode, get_collision_mode } from '../../pkg/bresenham_lighting_engine.js';

export function useWasm() {
    const [wasmModule, setWasmModule] = useState(null);
    const [isLoading, setIsLoading] = useState(true);
    const [error, setError] = useState(null);
    const [initTime, setInitTime] = useState(0);

    useEffect(() => {
        async function loadWasmModule() {
            try {
                console.log("Loading WASM module...");

                // Check browser compatibility
                if (typeof WebAssembly === 'undefined') {
                    throw new Error("WebAssembly is not supported in this browser");
                }

                // Check if we're running in a secure context
                if (typeof window !== 'undefined' && !window.isSecureContext) {
                    console.warn("Not running in secure context - some features may be limited");
                }

                const initStart = performance.now();
                const module = await init();
                const initEnd = performance.now();

                if (!module) {
                    throw new Error("WASM module initialization returned null");
                }

                if (!module.memory) {
                    throw new Error("WASM module memory not available");
                }

                // Switch to pixel-based collision detection for better performance
                set_collision_mode(1); // 1 = Pixel mode
                console.log(`🚀 Switched to pixel-based collision detection for maximum performance`);
                console.log(`Current collision mode: ${get_collision_mode()}`);

                setWasmModule(module);
                setInitTime(initEnd - initStart);
                setIsLoading(false);

                console.log("WASM module loaded successfully");
                console.log("Memory buffer size:", module.memory.buffer.byteLength);
                console.log(`✅ WASM initialization completed in ${(initEnd - initStart).toFixed(2)}ms`);

            } catch (err) {
                console.error("Failed to load WASM module:", err);

                // Provide helpful error messages for common issues
                let errorMessage = err.message;
                if (err.message.includes("fetch")) {
                    errorMessage = "Failed to fetch WASM file. Make sure the server is running and files are accessible.";
                } else if (err.message.includes("compile")) {
                    errorMessage = "Failed to compile WASM module. The WASM file may be corrupted.";
                } else if (err.message.includes("instantiate")) {
                    errorMessage = "Failed to instantiate WASM module. Check browser compatibility.";
                }

                setError(new Error(errorMessage));
                setIsLoading(false);
            }
        }

        loadWasmModule();
    }, []);

    return { wasmModule, isLoading, error, initTime };
} 