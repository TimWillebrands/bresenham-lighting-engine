import { useState, useCallback, useEffect, useRef } from 'https://esm.sh/preact/hooks';
import { put, set_pixel, clear_pixel_collisions, set_map_data } from '../../pkg/bresenham_lighting_engine.js';

export function useLighting(wasmModule) {
    const [lightConfig, setLightConfig] = useState({
        x: 80,
        y: 80,
        radius: 40
    });

    // Room layout setup - collision detection is now unified
    const [roomsConfigured, setRoomsConfigured] = useState(false);

    const createSimpleRoomLayout = useCallback(() => {
        const mapSize = 180;
        const mapData = new Int32Array(mapSize * mapSize);
        
        // Create a simple room layout with 3 rooms and walls
        for (let y = 0; y < mapSize; y++) {
            for (let x = 0; x < mapSize; x++) {
                const idx = y * mapSize + x;
                
                // Create border walls
                if (x === 0 || x === mapSize - 1 || y === 0 || y === mapSize - 1) {
                    mapData[idx] = 0; // Wall
                }
                // Vertical wall divider at x = 60
                else if (x === 60 && y > 20 && y < mapSize - 20) {
                    mapData[idx] = 0; // Wall
                }
                // Horizontal wall divider at y = 90
                else if (y === 90 && x > 60 && x < mapSize - 20) {
                    mapData[idx] = 0; // Wall
                }
                // Room 1: left side
                else if (x < 60) {
                    mapData[idx] = 1; // Room 1
                }
                // Room 2: top right
                else if (x > 60 && y < 90) {
                    mapData[idx] = 2; // Room 2
                }
                // Room 3: bottom right
                else if (x > 60 && y > 90) {
                    mapData[idx] = 3; // Room 3
                }
                else {
                    mapData[idx] = 1; // Default to room 1
                }
            }
        }
        
        // Add some doorways
        // Door between room 1 and room 2
        for (let y = 40; y < 50; y++) {
            mapData[y * mapSize + 60] = 1; // Open the wall
        }
        
        // Door between room 2 and room 3
        for (let x = 100; x < 110; x++) {
            mapData[90 * mapSize + x] = 2; // Open the wall
        }
        
        set_map_data(Array.from(mapData), mapSize);
        setRoomsConfigured(true);
        console.log("Created simple room layout for collision optimization");
    }, []);



    const [perfMetrics, setPerfMetrics] = useState({
        init: 0,
        update: 0,
        canvas: 0,
        fps: 0
    });

    const [fps, setFps] = useState(0);
    const wallPixels = useRef(new Set());
    const canvasRef = useRef(null);
    const wallsCanvasRef = useRef(null);
    const animationIdRef = useRef(null);
    const lastFrameTimeRef = useRef(0);
    const frameCountRef = useRef(0);

    const updateLightConfig = useCallback((updates) => {
        setLightConfig(prev => ({ ...prev, ...updates }));
    }, []);

    const updateWallPixel = useCallback((x, y, isWall) => {
        const pixelKey = `${x},${y}`;
        if (isWall) {
            wallPixels.current.add(pixelKey);
        } else {
            wallPixels.current.delete(pixelKey);
        }
        
        // Update the native Rust collision system
        set_pixel(x, y, isWall ? 1 : 0);
    }, []);

    const clearWalls = useCallback(() => {
        wallPixels.current.clear();
        clear_pixel_collisions();
        
        // Clear the walls canvas
        if (wallsCanvasRef.current) {
            const ctx = wallsCanvasRef.current.getContext('2d');
            ctx.clearRect(0, 0, 180, 180);
        }
    }, []);

    const updateLighting = useCallback(() => {
        if (!wasmModule || !canvasRef.current) {
            console.warn("WASM module or canvas not available");
            return;
        }

        const { x, y, radius } = lightConfig;

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
            return;
        }

        const updateEnd = performance.now();
        const updateTime = updateEnd - updateStart;

        if (canvasPtr === 0) {
            console.warn("Light update returned null pointer");
            return;
        }

        // Time the canvas rendering
        const canvasStart = performance.now();

        try {
            const ctx = canvasRef.current.getContext('2d');
            
            // Clear the canvas
            ctx.clearRect(0, 0, 180, 180);

            // Get the light canvas data from WASM memory
            const lightSize = radius * 2 + 1;

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
        const canvasTime = canvasEnd - canvasStart;

        setPerfMetrics(prev => ({
            ...prev,
            update: updateTime,
            canvas: canvasTime
        }));

    }, [wasmModule, lightConfig]);

    // FPS calculation
    useEffect(() => {
        function calculateFPS() {
            const now = performance.now();
            frameCountRef.current++;

            if (now - lastFrameTimeRef.current >= 1000) {
                const newFps = Math.round((frameCountRef.current * 1000) / (now - lastFrameTimeRef.current));
                setFps(newFps);
                setPerfMetrics(prev => ({ ...prev, fps: newFps }));
                frameCountRef.current = 0;
                lastFrameTimeRef.current = now;
            }

            animationIdRef.current = requestAnimationFrame(calculateFPS);
        }

        if (wasmModule) {
            lastFrameTimeRef.current = performance.now();
            calculateFPS();
        }

        return () => {
            if (animationIdRef.current) {
                cancelAnimationFrame(animationIdRef.current);
            }
        };
    }, [wasmModule]);

    // Update lighting when config changes (only for slider/user changes, not manual calls)
    useEffect(() => {
        if (wasmModule) {
            updateLighting();
        }
    }, [wasmModule, lightConfig]); // Removed updateLighting from deps to prevent cycles

    // Initialize room layout when WASM module is available
    useEffect(() => {
        if (wasmModule) {
            createSimpleRoomLayout(); // Initialize room layout for optimization
        }
    }, [wasmModule, createSimpleRoomLayout]);

    return {
        lightConfig,
        updateLightConfig,
        perfMetrics,
        fps,
        updateWallPixel,
        clearWalls,
        updateLighting,
        canvasRef,
        wallsCanvasRef,
        roomsConfigured,
        createSimpleRoomLayout
    };
} 