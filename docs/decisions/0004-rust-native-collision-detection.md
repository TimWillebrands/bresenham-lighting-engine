# Rust-Native Collision Detection for Ray Casting Performance

- Status: accepted

Technical Story: Performance bottleneck identified in ray casting where JavaScript-based collision detection causes ~250ms per light update due to expensive WASM bridge calls.

## Context and Problem Statement

The current lighting engine uses a JavaScript-based `IsBlocked` function for collision detection during ray casting. Each ray segment requires a collision check, resulting in hundreds of WASM bridge calls per light update. Performance measurements show:

- ~1ms per `IsBlocked` call (JavaScript → WASM bridge overhead)
- ~360 rays × multiple distance steps = hundreds of calls per light
- Total impact: ~250ms per light update (unacceptable for real-time rendering)

The current architecture calls from Rust → WASM bridge → JavaScript for every ray segment, making the bridge the primary performance bottleneck. How can we eliminate this bottleneck while maintaining flexibility for different collision detection needs?

## Decision Drivers

- **Performance**: Sub-5ms light updates required for real-time rendering
- **Flexibility**: Support both pixel-perfect and tile-based collision detection
- **Maintainability**: Clean abstractions that don't compromise performance
- **Compatibility**: Preserve existing tile-based world system
- **Developer Experience**: Efficient APIs for updating collision data from JavaScript

## Considered Options

1. **Rust-native collision detection with pixel bitmap**
2. **Pre-compute all collision data in JavaScript and transfer once**
3. **Reduce bridge calls by batching collision queries**
4. **Hybrid approach with both pixel and tile-based systems**

## Decision Outcome

Chosen option: **"Hybrid approach with both pixel and tile-based systems"**, because it provides maximum performance while maintaining flexibility and backwards compatibility.

### Implementation Plan

1. **Create collision detection abstraction**:
   ```rust
   pub trait CollisionDetector {
       fn is_blocked(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool;
   }
   ```

2. **Add pixel-based collision system**:
   - Fast bitmap storage for pixel-level blocking data
   - Bresenham line algorithm in Rust for collision testing
   - Efficient JavaScript APIs for updating pixel data

3. **Enhance existing tile-based system**:
   - Use existing `block_map` for structured worlds
   - Fast cell edge collision detection

4. **WASM API extensions**:
   ```rust
   // Batch pixel updates for efficiency
   pub fn set_pixel_batch(pixels: &[u32], blocked: bool);
   pub fn clear_pixels();
   pub fn set_collision_mode(mode: CollisionMode);
   ```

### Positive Consequences

- **Massive performance improvement**: 250ms → ~1-5ms per light update
- **Flexibility**: Support both pixel-perfect and tile-based collision
- **Native speed**: All collision detection runs at native Rust performance
- **Clean architecture**: Abstracted collision interface supports future extensions
- **Backwards compatibility**: Existing tile-based worlds continue to work

### Negative Consequences

- **Memory usage**: Pixel bitmap requires additional memory (manageable for 180×180)
- **Complexity**: Two collision systems to maintain
- **API surface**: Additional WASM exports for pixel management
- **Implementation effort**: Requires significant refactoring

## Pros and Cons of the Options

### Rust-native collision detection with pixel bitmap

- Good, because eliminates all WASM bridge calls during ray casting
- Good, because provides pixel-perfect collision detection
- Good, because uses fast native Bresenham implementation
- Bad, because requires additional memory for bitmap storage
- Bad, because single collision detection approach lacks flexibility

### Pre-compute all collision data in JavaScript and transfer once

- Good, because reduces bridge calls to once per frame
- Good, because allows complex JavaScript-based collision logic
- Bad, because still requires expensive computation in JavaScript
- Bad, because large data transfers for complex collision data
- Bad, because doesn't solve the fundamental performance problem

### Reduce bridge calls by batching collision queries

- Good, because reduces bridge call overhead
- Good, because minimal changes to existing architecture
- Bad, because still requires JavaScript collision computation
- Bad, because complex batching logic and state management
- Bad, because doesn't achieve native performance

### Hybrid approach with both pixel and tile-based systems

- Good, because maximum performance for both use cases
- Good, because backwards compatibility with existing tile system
- Good, because clean abstraction supports future collision types
- Good, because developers can choose appropriate collision method
- Bad, because higher implementation complexity
- Bad, because two systems to maintain and test

## Technical Implementation Details

### Pixel Collision System
```rust
pub struct PixelCollisionMap {
    width: u16,
    height: u16,
    pixels: Vec<u64>, // Bitpack 64 pixels per u64 for efficiency
}

impl CollisionDetector for PixelCollisionMap {
    fn is_blocked(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
        // Fast Bresenham line algorithm with bitpacked lookup
    }
}
```

### JavaScript API
```javascript
// High-performance pixel updates
wasm.setPixelBatch(coordinates, isBlocked);
wasm.clearAllPixels();

// Switch collision detection modes
wasm.setCollisionMode('pixel'); // or 'tile'
```

### Performance Targets
- Light update: < 5ms (down from ~250ms)
- Pixel update batch: < 1ms for typical drawing operations
- Memory overhead: < 10KB for 180×180 pixel map

## Links

- Builds on [ADR-0001](0001-bresenham-based-lighting-engine.md) - Core ray casting architecture
- Builds on [ADR-0002](0002-use-rust-for-core-engine.md) - Rust performance advantages
- Enhances existing `block_map` module in `src/block_map.rs` 