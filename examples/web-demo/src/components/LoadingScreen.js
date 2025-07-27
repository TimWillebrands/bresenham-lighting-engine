import { html } from 'https://esm.sh/htm/preact';

export default function LoadingScreen() {
    return html`
        <div class="loading">
            <div class="loading-spinner"></div>
            <div>Loading WebAssembly module...</div>
            <div style="font-size: 0.8rem; margin-top: 8px; opacity: 0.7">
                This may take a moment on the first load
            </div>
        </div>
    `;
} 