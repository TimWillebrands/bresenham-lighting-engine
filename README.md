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

- ✨ **Zero GPU dependency** - Your integrated graphics can take a nap
- 🚀 **WASM-ready** - Runs in browsers without breaking a sweat
- 📦 **Minimalistic AF** - No bloated dependencies, just pure algorithmic goodness
- 🎯 **Portable** - Will perhaps work in native game engines
- ⚡ **Performant** - Surprisingly fast for CPU-based lighting

## Basic Web Usage 

This requires a bundler like Vite to wire up the wasm and stuff. Package it with `wasm-pack build --target web` to get a version that doesn't need bundlers. 

The engine has two ways to block light, and they exist for different things:

- **`set_pixel(x, y, blocked)`** marks a single cell of the 180×180 grid as
  an in-world object — a chair, a barrel, a character. Use this for stuff
  that moves or gets placed at runtime.
- **`set_tile(tx, ty, type)`** / **`set_map_data(types, size)`** define the
  coarse 30×30 tile layout. Boundaries between tiles of different types
  become walls automatically, and contiguous same-type tiles form rooms
  used to skip occluded rays cheaply. Use this for architecture.

```typescript
import { memory, put, set_pixel, set_tile } from 'bresenham-lighting-engine';

// Architecture: tile (5,3) is type 1, surrounded by type 0 → walls on all
// four sides of that tile.
set_tile(5, 3, 1);

// A runtime object: mark the cell at (120, 90) as blocking.
set_pixel(120, 90, 1);

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

## Visual feedback & scenarios

Scenarios live in [`src/scenarios/`](src/scenarios/mod.rs) as plain Rust
functions taking `&mut LightingEngine`. They are shared by:

- **Exploration loop** — `cargo run --example scenario -- --name single_light`
  prints an ASCII matrix of the resulting canvas to stdout. Pass
  `--output-format png --out path.png` to render a PNG, or `--list` to see
  what's defined.
- **Regression loop** — `cargo test --test scenarios` runs invariant-based
  assertions. Failures embed the ASCII matrix in the panic message so the
  output is self-explanatory.

See [`CONTEXT.md`](CONTEXT.md) for the canonical vocabulary
(Tile/Cell/Wall/Object/Room/LightingEngine/Light/Canvas/Ray) and
[`docs/decisions/`](docs/decisions/) for ADRs.

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
