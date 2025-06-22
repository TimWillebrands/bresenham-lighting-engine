# Visual Testing System for Bresenham Lighting Engine

This document describes the comprehensive visual testing system that generates image outputs for AI-assisted development and debugging of the Bresenham Lighting Engine.

## Purpose

The visual testing system addresses a critical need in graphics engine development: **immediate visual feedback**. Instead of relying solely on numerical test assertions, developers and AI agents can see actual rendered output, making it easier to:

- Detect visual regressions
- Understand lighting behavior
- Debug rendering issues
- Validate algorithm correctness
- Document engine capabilities

## Architecture

### Core Components

1. **Mock Obstacle System** (`tests/output_mechanisms.rs`)
   - Thread-safe obstacle storage using `Mutex<Vec<(i16, i16, i16, i16)>>`
   - Line intersection detection for ray-obstacle collision
   - Replaces WASM JavaScript bindings for pure Rust testing

2. **Overridable is_blocked Function** (`src/lib.rs`)
   - Function pointer system allowing test-time override
   - `set_is_blocked_fn()` and `reset_is_blocked_fn()` for test control
   - Seamless integration with existing lighting calculations

3. **Image Generation** (`tests/output_mechanisms.rs`)
   - Canvas-to-RGB conversion for light data visualization
   - Composite scene rendering with additive blending
   - Obstacle visualization with line drawing
   - PNG output using the `image` crate

### Test Categories

#### Basic Functionality Tests
- **Single Light**: Verifies basic light rendering with color gradients
- **Multiple Lights**: Tests light composition and blending
- **Different Sizes**: Validates radius scaling behavior

#### Advanced Scenarios
- **Obstacle Interaction**: Tests shadow casting and ray blocking
- **Movement Sequences**: Animation frame generation
- **Complex Scenes**: Multi-light environments with obstacles
- **Edge Cases**: Boundary conditions and extreme parameters

### Generated Outputs

All images are saved as PNG files in `test_output/` directory:

```
test_output/
├── single_light.png                    # Basic light demonstration
├── light_size_X.png                   # Various radius sizes
├── lightN_individual.png              # Individual light renders
├── multiple_lights_composite.png      # Combined multi-light scene
├── light_with_obstacles.png           # Shadow casting demo
├── light_no_obstacles.png             # Comparison without obstacles
├── movement_frame_XX.png              # Animation sequence
├── complex_scene.png                  # Realistic multi-light scene
├── minimum_light.png                  # Edge case: radius 1
├── maximum_light.png                  # Edge case: max test radius
└── heavily_blocked_light.png          # Dense obstacle scenario
```

## Usage

```bash
# Set required stack size for lighting engine
export RUST_MIN_STACK=8388608

# Run specific test
cargo test --test output_mechanisms test_single_light_output

# Run all visual tests
cargo test --test output_mechanisms
```

## Technical Implementation

### Stack Size Requirements

The lighting engine pre-computes large ray lookup tables (`ALL_RAYS`) that require significant stack space:

```rust
// Production: 60 distances × 360 angles × Vec<PtI> per combination
// Test mode: 10 distances × 36 angles (reduced for stack safety)
static ALL_RAYS: Lazy<[[Vec<PtI>; ANGLES]; MAX_DIST]>
```

**Required**: `RUST_MIN_STACK=8388608` (8MB stack)

### Mock Obstacle System

The test system implements a complete obstacle detection replacement:

```rust
// Thread-safe obstacle storage
static MOCK_OBSTACLES: Mutex<Vec<(i16, i16, i16, i16)>> = Mutex::new(Vec::new());

// Line intersection detection
fn line_segments_intersect(x1, y1, x2, y2, x3, y3, x4, y4) -> bool {
    // Cross product method for line-line intersection
}
```

### Function Override Mechanism

The lighting engine uses a function pointer system for testability:

```rust
// Global function pointer with RwLock protection
static IS_BLOCKED_FN: Lazy<RwLock<IsBlockedFn>> =
    Lazy::new(|| RwLock::new(default_is_blocked_impl));

// Test-time override
pub fn set_is_blocked_fn(func: IsBlockedFn) {
    if let Ok(mut current_func) = IS_BLOCKED_FN.write() {
        *current_func = func;
    }
}
```

### Image Processing

Canvas data conversion with proper color handling:

```rust
fn canvas_to_image(canvas_ptr: *const Color, canvas_size: usize) -> RgbImage {
    unsafe {
        let canvas_slice = std::slice::from_raw_parts(canvas_ptr, canvas_size * canvas_size);
        for (i, &Color(r, g, b, _a)) in canvas_slice.iter().enumerate() {
            let x = (i % canvas_size) as u32;
            let y = (i / canvas_size) as u32;
            img.put_pixel(x, y, Rgb([r, g, b]));
        }
    }
}
```

## Expected Visual Patterns

### Normal Light Behavior
- **Center**: Bright white/yellow core
- **Gradient**: Smooth HSV-based color transitions
- **Shape**: Circular light distribution
- **Colors**: Rainbow effect based on ray angles

### With Obstacles
- **Hard Shadows**: Sharp shadow boundaries
- **Selective Blocking**: Individual rays blocked
- **Shadow Spread**: Adjacent angle blocking for close obstacles

### Multiple Lights
- **Additive Blending**: Color combination at overlaps
- **Independent Shadows**: Each light casts separate shadows
- **Color Mixing**: Interesting intersection patterns

## Debugging Guide

### Common Issues

**Stack Overflow Errors**
```bash
# Always use increased stack size
RUST_MIN_STACK=8388608 cargo test --test output_mechanisms
```

**Black/Empty Images**
- Verify `lighting::init()` completed
- Check light coordinates and radius values
- Ensure canvas size calculations are correct

**Missing Shadow Effects**
- Confirm obstacles were added with `add_mock_obstacle()`
- Verify obstacle coordinates intersect light rays
- Check that `set_is_blocked_fn(mock_is_blocked)` was called

**Unexpected Visual Artifacts**
- Test mode uses reduced resolution (36 angles vs 360)
- Small canvas sizes may cause pixelation
- Light positions near edges may be clipped

### Performance Characteristics

- **Image Generation**: ~10-50ms per image
- **Lighting Calculation**: ~1-5ms per light in test mode
- **Total Test Suite**: ~200-500ms for all tests
- **Memory Usage**: ~10MB peak (mostly ray lookup tables)

## Extending the System

### Adding New Test Scenarios

1. Create new test function in `tests/output_mechanisms.rs`
2. Set up lighting configuration and obstacles
3. Generate and save images with descriptive names
4. Update documentation with new test descriptions

Example:
```rust
#[test]
fn test_my_new_scenario() -> Result<(), Box<dyn std::error::Error>> {
    ensure_output_dir()?;
    init_test_environment();

    // Configure obstacles
    add_mock_obstacle(5, 5, 10, 10);

    // Create lights
    let light_ptr = lighting::update_or_add_light(1, 4, 7, 7);

    // Generate image
    let canvas_size = 4 * 2 + 1;
    let img = canvas_to_image(light_ptr, canvas_size);
    img.save("test_output/my_new_scenario.png")?;

    Ok(())
}
```

## Benefits for AI Development

The visual testing system provides several advantages for AI-assisted development:

1. **Immediate Feedback**: AI agents can see actual results instead of parsing numerical data
2. **Pattern Recognition**: Visual patterns are easier to analyze than raw pixel data
3. **Regression Detection**: Image comparison can identify subtle changes
4. **Documentation**: Generated images serve as visual documentation
5. **Debugging Context**: Visual output helps understand lighting behavior

## Conclusion

This visual testing system transforms the development experience for the Bresenham Lighting Engine by providing immediate, visual feedback on lighting calculations. The combination of mock obstacle systems, overridable functions, and comprehensive image generation creates a powerful tool for both human and AI-assisted development.

The system is designed to be:
- **Reliable**: Deterministic output for consistent testing
- **Comprehensive**: Covers all major lighting scenarios
- **Extensible**: Easy to add new test cases and output formats
- **Portable**: Works across platforms with minimal dependencies
- **Fast**: Quick feedback loop for iterative development

For AI agents, this system provides the visual context needed to make informed decisions about lighting engine behavior, significantly improving the development feedback loop.
