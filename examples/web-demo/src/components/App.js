import { html } from 'https://esm.sh/htm/preact';
import { useEffect } from 'https://esm.sh/preact/hooks';
import { useWasm } from '../hooks/useWasm.js';
import { useLighting } from '../hooks/useLighting.js';
import LoadingScreen from './LoadingScreen.js';
import ErrorScreen from './ErrorScreen.js';
import LightingDemo from './LightingDemo.js';
import Footer from './Footer.js';

export default function App() {
    const { wasmModule, isLoading, error, initTime } = useWasm();
    const lighting = useLighting(wasmModule);

    // Add keyboard shortcuts
    useEffect(() => {
        function handleKeyDown(e) {
            if (e.key === "c" && e.ctrlKey) {
                // Ctrl+C: Clear walls
                lighting.clearWalls();
                console.log("🧹 Cleared all walls and collision data");
                e.preventDefault();
            }
        }

        document.addEventListener("keydown", handleKeyDown);
        return () => document.removeEventListener("keydown", handleKeyDown);
    }, [lighting.clearWalls]);

    // Log success message when initialization is complete
    useEffect(() => {
        if (wasmModule && !isLoading) {
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
        }
    }, [wasmModule, isLoading]);

    if (error) {
        return html`<${ErrorScreen} error=${error} />`;
    }

    if (isLoading) {
        return html`<${LoadingScreen} />`;
    }

    return html`
        <div class="container">
            <h1>🚀 Bresenham Lighting Engine</h1>
            
            <div class="header-info">
                <p>
                    Interactive WebAssembly demo of CPU-based 2D lighting using
                    Bresenham's line algorithm
                </p>
                <p>
                    <a
                        href="https://github.com/timwillebrands/bresenham-lighting-engine"
                        target="_blank"
                        rel="noopener"
                    >View on GitHub</a>
                </p>
            </div>

            <${LightingDemo} 
                lighting=${lighting} 
                initTime=${initTime}
            />

            <${Footer} />
        </div>
    `;
} 
