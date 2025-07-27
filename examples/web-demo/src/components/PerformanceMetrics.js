import { html } from 'https://esm.sh/htm/preact';

export default function PerformanceMetrics({ perfMetrics, initTime }) {
    const { init, update, canvas, fps } = perfMetrics;

    return html`
        <div class="performance">
            <h4>⚡ Performance Metrics</h4>
            <div class="perf-metric">
                <span>Initialization:</span>
                <span class="perf-value">${initTime.toFixed(2)}ms</span>
            </div>
            <div class="perf-metric">
                <span>Light Update:</span>
                <span class="perf-value">${update.toFixed(2)}ms</span>
            </div>
            <div class="perf-metric">
                <span>Canvas Render:</span>
                <span class="perf-value">${canvas.toFixed(2)}ms</span>
            </div>
            <div class="perf-metric">
                <span>FPS:</span>
                <span class="perf-value">${fps}</span>
            </div>
        </div>
    `;
} 