import { html } from 'https://esm.sh/htm/preact';

export default function Instructions() {
    return html`
        <div class="instructions">
            <h3>🎮 How to Use</h3>
            <ul>
                <li>
                    <strong>Left click + drag</strong> on the canvas to draw
                    walls (obstacles that block light)
                </li>
                <li><strong>Right click + drag</strong> to erase walls</li>
                <li>
                    <strong>Middle click</strong> to move the light to that
                    position instantly
                </li>
                <li>
                    Use the <strong>sliders above</strong> to adjust light
                    properties in real-time
                </li>
                <li><strong>Ctrl+C</strong> to clear all walls</li>
            </ul>

            <div class="tip">
                <strong>💡 Pro Tip:</strong> Try creating complex shapes and
                watch how the CPU-based ray casting creates realistic
                lighting and shadows without any GPU acceleration!
            </div>
        </div>
    `;
} 