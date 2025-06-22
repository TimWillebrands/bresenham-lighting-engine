# Bresenham Lighting Engine - Web Demo

A real-time interactive demonstration of the Bresenham-based CPU lighting engine running in the browser via WebAssembly.

![Demo Screenshot](https://via.placeholder.com/600x400/1a1a1a/00ff88?text=Interactive+Lighting+Demo)

## 🚀 Quick Start

### Prerequisites

- [Bun](https://bun.sh) (recommended) or Node.js 18+
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) for building WASM

```bash
# Install wasm-pack if you haven't already
cargo install wasm-pack
```

### Running the Demo

1. **Navigate to the demo directory:**
   ```bash
   cd examples/web-demo
   ```

2. **Install dependencies:**
   ```bash
   bun install  # or npm install
   ```

3. **Start the development server:**
   ```bash
   bun run dev  # or npm run dev
   ```

4. **Open your browser:**
   Navigate to http://localhost:3000

The development server includes:
- 🔄 **Auto-reload** on file changes
- 🔧 **WASM rebuilding** when Rust code changes
- 📊 **Performance metrics** display
- 🐛 **Error handling** and debugging tools

## 🎮 How to Use

### Controls

- **🖱️ Left Click + Drag**: Draw walls (obstacles that block light)
- **🖱️ Right Click + Drag**: Erase walls
- **🖱️ Middle Click**: Move light to cursor position
- **🎚️ Sliders**: Adjust light position and radius in real-time

### Interactive Features

- **Real-time lighting**: Watch shadows update as you draw walls
- **Performance monitoring**: See CPU timing for each operation
- **Responsive design**: Works on desktop and mobile browsers
- **Pixel-perfect rendering**: Crisp pixel art aesthetic

## 🛠️ Development Commands

```bash
# Start development server with hot reload
bun run dev

# Build WASM module only
bun run build

# Serve without watching (production mode)
bun run serve

# Rebuild WASM and restart dev server
bun run rebuild
```

## 📊 Performance Metrics

The demo displays real-time performance metrics:

- **Initialization**: Time to load and initialize the WASM module
- **Light Update**: Time for ray casting and shadow calculation
- **Canvas Render**: Time to draw the lighting result
- **FPS**: Current frame rate

Typical performance on modern hardware:
- Light update: 1-5ms
- Canvas render: 0.5-2ms
- 60 FPS smooth interaction

## 🏗️ Architecture

### Frontend (JavaScript)
- **Canvas Rendering**: Dual-canvas setup for lighting and walls
- **Event Handling**: Mouse/touch input for drawing and light control
- **WASM Integration**: Direct memory access for pixel data
- **Live Reload**: Development server with WebSocket updates

### Backend (Rust/WASM)
- **Ray Casting**: Bresenham-style line algorithms
- **Shadow Calculation**: Geometric projection for blocked light
- **Memory Management**: Efficient pixel buffer allocation
- **Thread Safety**: Concurrent light processing support

### Communication Bridge
- **IsBlocked**: JavaScript → WASM collision detection
- **Log**: WASM → JavaScript debugging output
- **Memory Sharing**: Direct access to WASM linear memory

## 🎯 Features Demonstrated

- ✅ **CPU-based lighting** without GPU dependencies
- ✅ **Real-time ray casting** at 60 FPS
- ✅ **Dynamic shadow calculation** with proper projection
- ✅ **Interactive world editing** with immediate feedback
- ✅ **Cross-platform compatibility** (works everywhere)
- ✅ **WebAssembly performance** close to native speeds

## 🐛 Troubleshooting

### Common Issues

**WASM module fails to load:**
- Ensure your browser supports WebAssembly
- Check browser console for detailed error messages
- Try refreshing the page

**Poor performance:**
- Reduce light radius if frame rate drops
- Check if other tabs are consuming CPU
- Try closing browser dev tools

**Walls not blocking light:**
- Ensure you're left-clicking and dragging
- Check that the walls canvas is properly layered
- Try drawing larger/thicker walls

### Browser Compatibility

- ✅ Chrome 57+
- ✅ Firefox 52+
- ✅ Safari 11+
- ✅ Edge 16+

### Development Issues

**Auto-reload not working:**
- Check that port 3000 is available
- Verify WebSocket connection in browser console
- Try restarting the development server

**WASM build fails:**
- Ensure `wasm-pack` is installed and updated
- Check that Rust toolchain includes `wasm32-unknown-unknown` target
- Run `rustup target add wasm32-unknown-unknown` if needed

## 📚 Learning Resources

- [WebAssembly Documentation](https://webassembly.org/)
- [wasm-bindgen Guide](https://rustwasm.github.io/wasm-bindgen/)
- [Bresenham's Line Algorithm](https://en.wikipedia.org/wiki/Bresenham%27s_line_algorithm)
- [2D Visibility Algorithms](http://www.redblobgames.com/articles/visibility/)

## 🤝 Contributing

Found a bug or want to add a feature?

1. Fork the repository
2. Make your changes in the web demo
3. Test with `bun run dev`
4. Submit a pull request

Ideas for improvements:
- Multiple colored lights
- Different obstacle types
- Save/load world presets
- Touch controls optimization
- Performance profiling tools

---

*Built with Rust 🦀 + WebAssembly 🕸️ + Modern Web APIs*