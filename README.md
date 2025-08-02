# bresenham-lighting-engine

[![Live Demo](https://img.shields.io/badge/demo-live-brightgreen)](https://TimWillebrands.github.io/bresenham-lighting-engine/)
[![NPM Version](https://img.shields.io/npm/v/bresenham-lighting-engine)](https://www.npmjs.com/package/bresenham-lighting-engine)

## What is this madness?

This is an experimental lighting engine that said "nah fam" to GPU shaders and decided to use **Bresenham line algorithms** for ray casting instead.

The core idea is simple:
- Cast rays using classic line-drawing algorithms
- Calculate shadows by checking when rays get yeeted by obstacles
- Apply light falloff based on distance
- Render everything with HSV color space because we're fancy like that

## Features that might possibly slap

✨ **Zero GPU dependency** - Your integrated graphics can take a nap
🚀 **WASM-ready** - Runs in browsers without breaking a sweat
📦 **Minimalistic AF** - No bloated dependencies, just pure algorithmic goodness
🎯 **Portable** - Will perhaps work in native game engines
⚡ **Performant** - Surprisingly fast for CPU-based lighting

## Installation

### Option 1: NPM Package (Recommended)

```bash
npm install bresenham-lighting-engine
```

```javascript
import init, { LightingEngine } from 'bresenham-lighting-engine';

async function main() {
  await init();
  const engine = new LightingEngine(800, 600);
  // Start casting rays like it's 1962!
}
```

### Option 2: Build from Source

## Installation

```bash
npm install bresenham-lighting-engine
```

## Basic Web Usage 

This requires a bundler like Vite to wire up the wasm and stuff. Package it with `wasm-pack build --target web` to get a version that doesn't need bundlers. 

```typescript
import { memory, put, set_collision_mode } from 'bresenham-lighting-engine';

// Initialize collision detection
set_collision_mode(1); // 0=tile-based, 1=pixel-based

// Create a light: id=0, radius=50, x=200, y=100
const lightPtr = put(0, 50, 200, 100);

// Extract pixel data from WASM memory
const lightSize = 50 * 2 + 1; // radius * 2 + 1
const pixelData = new Uint8ClampedArray(
  memory.buffer,
  lightPtr,
  lightSize * lightSize * 4 // RGBA
);

// Render to canvas
const canvas = document.querySelector('canvas');
const ctx = canvas.getContext('2d');
const imageData = new ImageData(pixelData, lightSize, lightSize);
ctx.putImageData(imageData, 0, 0);
```

## Development

Install `wasm-pack`:
```bash
cargo install wasm-pack
```

Build the WASM module:
```bash
wasm-pack build --target bundler
```

## Roadmap (aka "Things I'll Probably Never Do")

- [x] Basic WASM implementation that doesn't crash
- [x] Ray casting with proper shadow calculation
- [ ] Native library bindings (C/C++/whatever)
- [ ] Godot plugin (because indie devs deserve nice things)
- [ ] Performance optimizations (SIMD goes brrr)
- [ ] Better color blending modes
- [ ] Multi-light support that doesn't melt your CPU
