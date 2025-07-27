# bresenham-lighting-engine

[![Live Demo](https://img.shields.io/badge/demo-live-brightgreen)](https://TimWillebrands.github.io/bresenham-lighting-engine/)

## What is this madness?

This is a lighting engine that said "nah fam" to GPU shaders and decided to use **Bresenham line algorithms** for ray casting instead.

The core idea is simple:
- Cast rays using classic line-drawing algorithms
- Calculate shadows by checking when rays get yeeted by obstacles
- Apply light falloff based on distance
- Render everything with HSV color space because we're fancy like that

## Features that actually slap

✨ **Zero GPU dependency** - Your integrated graphics can take a nap
🚀 **WASM-ready** - Runs in browsers without breaking a sweat
📦 **Minimalistic AF** - No bloated dependencies, just pure algorithmic goodness
🎯 **Portable** - Will perhaps work in native game engines
⚡ **Performant** - Surprisingly fast for CPU-based lighting

## Quick Start (for the impatient)

Install `wasm-pack`:
```bash
cargo install wasm-pack
```

Build the WASM module:
```bash
wasm-pack build --target web
```

## Roadmap (aka "Things I'll Probably Never Do")

- [x] Basic WASM implementation that doesn't crash
- [x] Ray casting with proper shadow calculation
- [ ] Native library bindings (C/C++/whatever)
- [ ] Godot plugin (because indie devs deserve nice things)
- [ ] Performance optimizations (SIMD goes brrr)
- [ ] Better color blending modes
- [ ] Multi-light support that doesn't melt your CPU
