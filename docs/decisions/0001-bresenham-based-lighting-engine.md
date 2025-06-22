# Use Bresenham Line Algorithms for CPU-Based Lighting Engine

- Status: accepted
- Deciders: Development Team
- Date: 2024-12-19

Technical Story: Need for a portable, GPU-independent lighting engine that can work across multiple platforms and devices.

## Context and Problem Statement

Traditional 2D lighting engines rely heavily on GPU shaders and graphics hardware acceleration. This creates several challenges:
- Platform dependency on specific graphics APIs (OpenGL, DirectX, WebGL)
- Performance bottlenecks on devices with weak or integrated GPUs
- Complexity in deployment across diverse hardware configurations
- Limited portability to embedded systems or constrained environments

How can we create a lighting engine that is performant, portable, and doesn't require specialized graphics hardware?

## Decision Drivers

- **Platform Independence**: Need to run on any device with a CPU, regardless of GPU capabilities
- **Web Compatibility**: Must work in browsers via WebAssembly without WebGL dependencies
- **Minimalistic Design**: Keep dependencies and complexity low
- **Performance**: Achieve acceptable performance using only CPU resources
- **Portability**: Enable easy integration into various game engines and platforms
- **Deterministic Behavior**: Ensure consistent results across different hardware

## Considered Options

- **GPU Shader-Based Lighting**: Traditional approach using fragment shaders
- **Software Rasterization**: Full software rendering pipeline
- **Bresenham Ray Casting**: Use line-drawing algorithms for light ray calculation
- **Hybrid CPU/GPU Approach**: Fallback system with GPU when available

## Decision Outcome

Chosen option: "Bresenham Ray Casting", because it provides the best balance of performance, portability, and simplicity while meeting all our core requirements.

### Positive Consequences

- **Zero GPU Dependency**: Runs on any system with a CPU
- **Predictable Performance**: CPU-based calculation provides consistent timing
- **Small Footprint**: Minimal memory usage and no shader compilation overhead
- **Easy Integration**: Simple C-style API that works everywhere
- **Deterministic**: Same input always produces identical output across platforms
- **Educational Value**: Demonstrates classical computer graphics algorithms

### Negative Consequences

- **CPU Intensive**: Uses more CPU cycles than GPU-accelerated alternatives
- **Limited Visual Effects**: Cannot easily implement complex lighting effects like volumetric lighting
- **Scaling Challenges**: Performance may degrade with very large numbers of lights
- **Modern Expectations**: May seem "retro" compared to modern shader-based engines

## Pros and Cons of the Options

### GPU Shader-Based Lighting

- Good, because extremely fast on modern hardware
- Good, because supports complex visual effects
- Good, because industry standard approach
- Bad, because requires specific GPU capabilities
- Bad, because platform-dependent (WebGL, OpenGL, DirectX)
- Bad, because fails on systems without adequate GPU support

### Software Rasterization

- Good, because completely software-based
- Good, because maximum portability
- Bad, because extremely slow for complex scenes
- Bad, because requires implementing entire rendering pipeline
- Bad, because memory intensive

### Bresenham Ray Casting

- Good, because leverages proven line-drawing algorithms
- Good, because excellent performance-to-complexity ratio
- Good, because works on minimal hardware
- Good, because deterministic and debuggable
- Good, because small codebase and memory footprint
- Bad, because limited to relatively simple lighting models
- Bad, because CPU-bound performance scaling

### Hybrid CPU/GPU Approach

- Good, because best of both worlds when GPU available
- Good, because graceful degradation
- Bad, because significantly more complex to implement
- Bad, because requires maintaining two separate codepaths
- Bad, because testing complexity across different hardware configurations

## Technical Implementation Notes

The Bresenham-based approach works by:

1. **Ray Generation**: Cast rays from light sources using integer arithmetic
2. **Line Walking**: Use Bresenham-style algorithms to walk along each ray
3. **Occlusion Testing**: Check for obstacles at each step along the ray
4. **Shadow Calculation**: Determine blocked angle ranges when obstacles are hit
5. **Light Falloff**: Apply distance-based attenuation using simple math
6. **Color Rendering**: Convert HSV color space to RGB for final output

This approach is particularly well-suited for:
- Tile-based games and 2D environments
- Retro-style graphics with pixel-perfect lighting
- Embedded systems and resource-constrained devices
- Educational demonstrations of classical algorithms

## Links

- [Bresenham's Line Algorithm - Wikipedia](https://en.wikipedia.org/wiki/Bresenham%27s_line_algorithm)
- [2D Visibility/Line of Sight Tutorial](http://www.redblobgames.com/articles/visibility/)
- [WebAssembly Performance Considerations](https://hacks.mozilla.org/2018/01/making-webassembly-even-faster-firefoxs-new-streaming-and-tiering-compiler/)