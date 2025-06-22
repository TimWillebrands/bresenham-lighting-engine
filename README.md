# bresenham-lighting-engine

> When your GPU is too mainstream and you want to do lighting the hard way™️

## What is this madness?

This is a lighting engine that said "nah fam" to GPU shaders and decided to use **Bresenham line algorithms** for ray casting instead. Why? Because sometimes you gotta touch grass (or in this case, touch CPU cycles).

The core idea is simple but kinda genius:
- Cast rays using classic line-drawing algorithms
- Calculate shadows by checking when rays get yeeted by obstacles  
- Apply light falloff based on distance
- Render everything with HSV color space because we're fancy like that

## Features that actually slap

✨ **Zero GPU dependency** - Your integrated graphics can take a nap  
🚀 **WASM-ready** - Runs in browsers without breaking a sweat  
📦 **Minimalistic AF** - No bloated dependencies, just pure algorithmic goodness  
🎯 **Portable** - Will eventually work in native game engines (Godot gang rise up)  
⚡ **Performant** - Surprisingly fast for CPU-based lighting (trust the math)  

## Quick Start (for the impatient)

Install `wasm-pack`:
```bash
cargo install wasm-pack
```

Build the WASM module:
```bash
wasm-pack build --target web
```

Run the demo:
```bash
bun run index.js
```

Then open `index.html` in your browser and watch those sweet, sweet rays do their thing.

## The Vibe

This project is basically asking: "What if we did 2D lighting but made it work everywhere without needing a GPU?" The answer involves a lot of trigonometry, some clever ray casting, and the realization that sometimes the old ways hit different.

Perfect for:
- Indie games that need to run on a potato 🥔
- Retro-style projects with modern performance
- When you want lighting but your target platform thinks shaders are spicy food
- Flexing that you can do graphics programming without graphics hardware

## Roadmap (aka "Things We'll Probably Do")

- [x] Basic WASM implementation that doesn't crash
- [x] Ray casting with proper shadow calculation  
- [ ] Native library bindings (C/C++/whatever)
- [ ] Godot plugin (because indie devs deserve nice things)
- [ ] Performance optimizations (SIMD goes brrr)
- [ ] Better color blending modes
- [ ] Multi-light support that doesn't melt your CPU

## Technical Deep Dive

The engine uses a grid-based approach where:
1. Light sources cast rays in 360° using integer arithmetic
2. Each ray checks for obstacles using line-walking algorithms
3. Shadow boundaries are calculated using geometric projections
4. Light falloff follows distance-based attenuation
5. Final colors are rendered using HSV→RGB conversion

It's like raytracing but your CPU is doing all the heavy lifting, and honestly? It's kinda beautiful.

## Contributing

Found a bug? Performance issue? Want to add more mathematical wizardry? PRs welcome! Just keep it clean and document your dark magic.

## License

ISC - Do whatever you want, just don't sue us if your computer catches fire from all the trigonometry.

---

*Built with Rust and questionable life choices. Powered by the ancient art of drawing lines on computers.*