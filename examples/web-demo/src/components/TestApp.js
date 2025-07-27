import { html } from 'https://esm.sh/htm/preact';
import { useWasm } from '../hooks/useWasm.js';
import { useTests } from '../hooks/useTests.js';
import LoadingScreen from './LoadingScreen.js';
import ErrorScreen from './ErrorScreen.js';
import TestStep from './TestStep.js';

export default function TestApp() {
    const { wasmModule, isLoading, error, initTime } = useWasm();
    const tests = useTests(wasmModule);

    if (error) {
        return html`<${ErrorScreen} error=${error} />`;
    }

    if (isLoading) {
        return html`<${LoadingScreen} />`;
    }

    return html`
        <div>
            <h1>🚀 Bresenham Lighting Engine - Debug Test</h1>
            <p>This page tests the WASM module functionality step by step.</p>

            <${TestStep}
                id="init"
                title="1. WASM Module Initialization"
                status="✅ Initialization successful"
                className="success"
                details=${`✅ WASM module loaded in ${initTime.toFixed(2)}ms\nMemory buffer size: ${wasmModule.memory.buffer.byteLength} bytes`}
            />

            <${TestStep}
                id="collision"
                title="2. Collision System Test"
                status=${tests.testStates.collision.status}
                className=${tests.testStates.collision.className}
                details=${tests.testStates.collision.details}
                showButton=${tests.testStates.collision.status === 'ready'}
                buttonText="Test Collision Detection"
                onButtonClick=${tests.testCollision}
            />

            <${TestStep}
                id="lighting"
                title="3. Lighting System Test"
                status=${tests.testStates.lighting.status}
                className=${tests.testStates.lighting.className}
                details=${tests.testStates.lighting.details}
                showButton=${tests.testStates.lighting.status === 'ready'}
                buttonText="Test Light Update"
                onButtonClick=${tests.testLighting}
            />

            <${TestStep}
                id="visual"
                title="4. Visual Test"
                status=${tests.testStates.visual.status}
                className=${tests.testStates.visual.className}
                details=${tests.testStates.visual.details}
                showButton=${tests.testStates.visual.status === 'ready'}
                buttonText="Render Light"
                onButtonClick=${tests.testVisual}
                extraButtons=${[
                    { text: 'Add Test Obstacles', onClick: tests.addTestObstacles, enabled: tests.testStates.visual.status === 'ready' },
                    { text: 'Clear Obstacles', onClick: tests.clearObstacles, enabled: tests.testStates.visual.status === 'ready' }
                ]}
            >
                <canvas
                    ref=${tests.canvasRef}
                    width="180"
                    height="180"
                />
            </${TestStep}>
        </div>
    `;
} 