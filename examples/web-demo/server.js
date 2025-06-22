#!/usr/bin/env bun

/**
 * Simple development server for the Bresenham Lighting Engine web demo
 *
 * Features:
 * - Static file serving
 * - Auto-reload on file changes
 * - CORS headers for WASM
 * - Proper MIME types
 */

import { file, serve } from "bun";
import { watch } from "fs";
import path from "path";

const PORT = process.env.PORT || 3000;
const DEV_MODE = process.argv.includes("--hot") || process.env.NODE_ENV === "development";

// WebSocket connections for live reload
const wsConnections = new Set();

// MIME types for proper file serving
const mimeTypes = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.mjs': 'text/javascript',
  '.css': 'text/css',
  '.wasm': 'application/wasm',
  '.json': 'application/json',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.gif': 'image/gif',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon'
};

function getMimeType(filename) {
  const ext = path.extname(filename).toLowerCase();
  return mimeTypes[ext] || 'text/plain';
}

// Live reload client script
const liveReloadScript = `
<script>
if (window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1') {
  const ws = new WebSocket('ws://localhost:${PORT}/ws');
  ws.onmessage = (event) => {
    if (event.data === 'reload') {
      console.log('[DEV] Reloading page...');
      window.location.reload();
    }
  };
  ws.onopen = () => console.log('[DEV] Live reload connected');
  ws.onclose = () => console.log('[DEV] Live reload disconnected');
}
</script>
`;

function broadcastReload() {
  console.log('[DEV] Broadcasting reload to', wsConnections.size, 'clients');
  wsConnections.forEach(ws => {
    try {
      ws.send('reload');
    } catch (e) {
      wsConnections.delete(ws);
    }
  });
}

// Watch for file changes in development mode
if (DEV_MODE) {
  console.log('[DEV] Watching for file changes...');

  // Watch current directory and WASM pkg
  watch('.', { recursive: true }, (eventType, filename) => {
    if (filename && (
      filename.endsWith('.html') ||
      filename.endsWith('.js') ||
      filename.endsWith('.mjs') ||
      filename.endsWith('.css') ||
      filename.endsWith('.wasm')
    )) {
      console.log(`[DEV] File changed: ${filename}`);
      setTimeout(broadcastReload, 100); // Small debounce
    }
  });
}

const server = serve({
  port: PORT,
  async fetch(req, server) {
    const url = new URL(req.url);

    // Handle WebSocket upgrade for live reload
    if (url.pathname === '/ws' && DEV_MODE) {
      if (server.upgrade(req)) {
        return; // do not return a Response
      }
      return new Response("Upgrade failed", { status: 400 });
    }

    // Default to index.html for root
    let pathname = url.pathname === '/' ? '/index.html' : url.pathname;

    try {
      // Security: prevent directory traversal
      const safePath = path.normalize(pathname).replace(/^(\.\.[\/\\])+/, '');
      const filePath = path.join(process.cwd(), safePath);

      const fileHandle = file(filePath);
      const exists = await fileHandle.exists();

      if (!exists) {
        return new Response("Not Found", { status: 404 });
      }

      let content = await fileHandle.arrayBuffer();
      const mimeType = getMimeType(pathname);

      // Inject live reload script into HTML files in dev mode
      if (DEV_MODE && mimeType === 'text/html') {
        const htmlContent = new TextDecoder().decode(content);
        const modifiedHtml = htmlContent.replace('</body>', `${liveReloadScript}</body>`);
        content = new TextEncoder().encode(modifiedHtml);
      }

      const headers = {
        'Content-Type': mimeType,
        'Cross-Origin-Embedder-Policy': 'require-corp',
        'Cross-Origin-Opener-Policy': 'same-origin',
      };

      // Add CORS headers for WASM files
      if (mimeType === 'application/wasm') {
        headers['Cross-Origin-Resource-Policy'] = 'cross-origin';
      }

      return new Response(content, { headers });

    } catch (error) {
      console.error('Server error:', error);
      return new Response("Internal Server Error", { status: 500 });
    }
  },

  websocket: DEV_MODE ? {
    message(ws, message) {
      // Echo messages for debugging
      console.log('[WS] Received:', message);
    },
    open(ws) {
      wsConnections.add(ws);
      console.log('[WS] Client connected, total:', wsConnections.size);
    },
    close(ws) {
      wsConnections.delete(ws);
      console.log('[WS] Client disconnected, total:', wsConnections.size);
    }
  } : undefined
});

console.log(`
🚀 Bresenham Lighting Engine Demo Server

📍 Local:   http://localhost:${PORT}
🔧 Mode:    ${DEV_MODE ? 'Development (with live reload)' : 'Production'}
📁 Serving: ${process.cwd()}

${DEV_MODE ? '👀 Watching for changes...' : ''}
`);

// Graceful shutdown
process.on('SIGINT', () => {
  console.log('\n👋 Shutting down server...');
  server.stop();
  process.exit(0);
});
