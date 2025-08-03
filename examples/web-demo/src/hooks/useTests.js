import { useState, useCallback, useRef } from 'https://esm.sh/preact/hooks';
import { put, set_pixel, clear_pixel_collisions } from '../../pkg/bresenham_lighting_engine.js';

export function useTests(wasmModule) {
    const [testStates, setTestStates] = useState({
        init: { status: 'complete', className: 'success', details: '' },
        collision: { status: 'ready', className: '', details: '' },
        lighting: { status: 'waiting', className: '', details: '' },
        visual: { status: 'waiting', className: '', details: '' }
    });

    const canvasRef = useRef(null);

    const updateTestState = useCallback((testId, updates) => {
        setTestStates(prev => ({
            ...prev,
            [testId]: { ...prev[testId], ...updates }
        }));
    }, []);

    const logToTest = useCallback((testId, message) => {
        setTestStates(prev => ({
            ...prev,
            [testId]: {
                ...prev[testId],
                details: prev[testId].details + message + '\n'
            }
        }));
    }, []);

    const clearTestLog = useCallback((testId) => {
        updateTestState(testId, { details: '' });
    }, [updateTestState]);

    const testCollision = useCallback(async () => {
        if (!wasmModule) return;

        try {
            updateTestState('collision', { status: '⏳ Testing collision system...', className: '' });
            clearTestLog('collision');
            
            // The unified collision system is always active - no mode switching needed
            logToTest('collision', 'Unified collision system active - pixel + room optimization');
            
            // Collision system is unified - no mode validation needed
            
            // Test pixel setting
            set_pixel(50, 50, 1); // Add obstacle
            logToTest('collision', '✅ Set pixel obstacle at (50, 50)');
            
            // Test clearing
            clear_pixel_collisions();
            logToTest('collision', '✅ Cleared pixel collisions');
            
            updateTestState('collision', { 
                status: '✅ Collision system working', 
                className: 'success' 
            });
            updateTestState('lighting', { status: 'ready' });
            
        } catch (error) {
            logToTest('collision', `❌ Error: ${error.message}`);
            updateTestState('collision', { 
                status: '❌ Collision test failed', 
                className: 'error' 
            });
        }
    }, [wasmModule, updateTestState, logToTest, clearTestLog]);

    const testLighting = useCallback(async () => {
        if (!wasmModule) return;

        try {
            updateTestState('lighting', { status: '⏳ Testing lighting system...', className: '' });
            clearTestLog('lighting');
            
            const startTime = performance.now();
            const canvasPtr = put(0, 30, 90, 90);
            const endTime = performance.now();
            
            logToTest('lighting', `Light update time: ${(endTime - startTime).toFixed(3)}ms`);
            logToTest('lighting', `Canvas pointer: ${canvasPtr}`);
            
            if (canvasPtr === 0) {
                throw new Error('Light update returned null pointer');
            }
            
            updateTestState('lighting', { 
                status: '✅ Lighting system working', 
                className: 'success' 
            });
            updateTestState('visual', { status: 'ready' });
            
        } catch (error) {
            logToTest('lighting', `❌ Error: ${error.message}`);
            logToTest('lighting', `Stack trace: ${error.stack}`);
            updateTestState('lighting', { 
                status: '❌ Lighting test failed', 
                className: 'error' 
            });
        }
    }, [wasmModule, updateTestState, logToTest, clearTestLog]);

    const testVisual = useCallback(async () => {
        if (!wasmModule || !canvasRef.current) return;

        try {
            updateTestState('visual', { status: '⏳ Rendering light...', className: '' });
            
            const radius = 40;
            const x = 90;
            const y = 90;
            
            const startTime = performance.now();
            const canvasPtr = put(0, radius, x, y);
            const endTime = performance.now();
            
            if (canvasPtr === 0) {
                throw new Error('Light update returned null pointer');
            }
            
            const ctx = canvasRef.current.getContext('2d');
            
            // Clear canvas
            ctx.fillStyle = 'black';
            ctx.fillRect(0, 0, 180, 180);
            
            // Get light data
            const lightSize = radius * 2 + 1;
            const lightData = new Uint8ClampedArray(
                wasmModule.memory.buffer,
                canvasPtr,
                lightSize * lightSize * 4
            );
            
            // Render light
            const imageData = new ImageData(lightData, lightSize, lightSize);
            ctx.putImageData(
                imageData,
                x - Math.floor(lightSize / 2),
                y - Math.floor(lightSize / 2)
            );
            
            logToTest('visual', `✅ Light rendered in ${(endTime - startTime).toFixed(3)}ms`);
            logToTest('visual', `Light size: ${lightSize}x${lightSize} pixels`);
            updateTestState('visual', { 
                status: '✅ Visual rendering successful', 
                className: 'success' 
            });
            
        } catch (error) {
            logToTest('visual', `❌ Error: ${error.message}`);
            updateTestState('visual', { 
                status: '❌ Visual test failed', 
                className: 'error' 
            });
        }
    }, [wasmModule, updateTestState, logToTest, canvasRef]);

    const addTestObstacles = useCallback(async () => {
        if (!wasmModule) return;

        try {
            logToTest('visual', 'Adding test obstacles...');
            
            // Add some obstacles
            for (let x = 60; x < 120; x += 2) {
                set_pixel(x, 70, 1);
                set_pixel(x, 110, 1);
            }
            for (let y = 70; y < 110; y += 2) {
                set_pixel(60, y, 1);
                set_pixel(120, y, 1);
            }
            
            // Re-render light
            await testVisual();
            
        } catch (error) {
            logToTest('visual', `❌ Error adding obstacles: ${error.message}`);
        }
    }, [wasmModule, logToTest, testVisual]);

    const clearObstacles = useCallback(async () => {
        if (!wasmModule) return;

        try {
            logToTest('visual', 'Clearing obstacles...');
            clear_pixel_collisions();
            await testVisual();
        } catch (error) {
            logToTest('visual', `❌ Error clearing obstacles: ${error.message}`);
        }
    }, [wasmModule, logToTest, testVisual]);

    return {
        testStates,
        canvasRef,
        testCollision,
        testLighting, 
        testVisual,
        addTestObstacles,
        clearObstacles
    };
} 