import { render } from 'https://esm.sh/preact';
import { html } from 'https://esm.sh/htm/preact';
import TestApp from './components/TestApp.js';

// Set up logging function BEFORE importing WASM to avoid initialization errors
globalThis.log_from_js = function (message) {
    console.log("[WASM]", message);
};

// Simplified console.log_from_js for compatibility  
globalThis.console = globalThis.console || {};
globalThis.console.log_from_js = globalThis.log_from_js;

render(html`<${TestApp} />`, document.getElementById('app')); 