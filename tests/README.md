# Visual Test Output System

This directory contains comprehensive visual testing for the Bresenham Lighting Engine, providing image-based feedback for AI agents and developers to verify lighting behavior.

## Overview

The visual test system generates PNG images showing various lighting scenarios, allowing for:
- **Immediate Visual Feedback**: See actual lighting results instead of just numerical data
- **Regression Testing**: Compare images between versions to detect changes
- **Debug Assistance**: Identify lighting artifacts or unexpected behavior
- **Documentation**: Visual examples of engine capabilities

## Running the Tests

### Standard Test Run
```bash
# Run with increased stack size (required for lighting engine)
RUST_MIN_STACK=8388608 cargo test --test output_mechanisms
```

### Individual Test Categories
```bash
# Test single light rendering
RUST_MIN_STACK=8388608 cargo test --test output_mechanisms test_single_light_output

# Test multiple lights
RUST_MIN_STACK=8388608 cargo test --test output_mechanisms test_multiple_lights

# Test obstacle interactions
RUST_MIN_STACK=8388608 cargo test --test output_mechanisms test_lights_with_obstacles

# Test different light sizes
RUST_MIN_STACK=8388608 cargo test --test output_mechanisms test_different_light_sizes

# Test light movement sequences
RUST_MIN_STACK=8388608 cargo test --test output_mechanisms test_light_movement_sequence

# Test complex scenes
RUST_MIN_STACK=8388608 cargo test --test output_mechanisms test_complex_scene

# Test edge cases
RUST_MIN_STACK=8388608 cargo test --test output_mechanisms test_edge_cases

# Test realistic large-scale lighting with walls
RUST_MIN_STACK=8388608 cargo test --test output_mechanisms test_realistic_large_light_with_walls

# Test production-scale lighting demonstrations (may be limited by test mode)
RUST_MIN_STACK=8388608 cargo test --test output_mechanisms test_production_scale_lighting -- --ignored
```

## Generated Images

All images are saved to `test_output/` directory:

### Basic Lighting Tests
- `single_light.png` - Basic single light source with color gradients
- `light_size_X.png` - Lights of various radii (1-4 in test mode)
- `minimum_light.png` - Smallest possible light (radius 1)
- `maximum_light.png` - Largest light within test constraints

### Multi-Light Scenarios  
- `light1_individual.png`, `light2_individual.png`, `light3_individual.png` - Individual lights
- `multiple_lights_composite.png` - Multiple lights combined with additive blending

### Obstacle Testing
- `light_with_obstacles.png` - Light affected by mock obstacles (white lines)
- `light_no_obstacles.png` - Same light without obstacles for comparison
- `heavily_blocked_light.png` - Light with obstacles placed very close

### Animation Sequences
- `movement_frame_XX.png` - Series showing light movement (5 frames)

### Complex Scenes
- `complex_scene.png` - Multiple lights with various obstacles in a realistic layout

### Realistic Lighting Scenarios
- `realistic_large_light_with_walls.png` - Large-scale room lighting with architectural walls
- `realistic_light_no_walls.png` - Same scene without walls for comparison

### Production-Scale Demonstrations
- `production_scale_office_lighting.png` - Large office/warehouse lighting simulation
- `production_scale_night_lighting.png` - Night/security lighting mode

## Technical Details

### Mock Obstacle System
The test system includes a mock obstacle detection system that replaces the WASM JavaScript bindings:
- `add_mock_obstacle(x0, y0, x1, y1)` - Add line segment obstacle
- `clear_mock_obstacles()` - Remove all obstacles
- Line intersection detection using cross product method

### Test Constraints
Due to test mode limitations:
- Maximum light radius: ~5 (vs 60 in production)
- Ray angle resolution: 36 angles (vs 360 in production)
- Small canvas sizes to prevent stack overflow

### Color Representation
- **Hue**: Determined by ray angle (creates rainbow effect)
- **Saturation**: Always full (255) for vivid colors
- **Brightness**: Decreases with distance from light source
- **Obstacles**: Shown as white lines when drawn on images

## Expected Visual Patterns

### Normal Light Behavior
- **Center**: Bright white/yellow center
- **Gradient**: Smooth color transition from center to edge
- **Circular Shape**: Even light distribution in all directions
- **Rainbow Effect**: Different colors at different angles

### With Obstacles
- **Hard Shadows**: Sharp shadow boundaries behind obstacles
- **Partial Blocking**: Some rays blocked while adjacent rays continue
- **Shadow Spread**: Shadows may affect 1-2 adjacent angles for close obstacles

### Multiple Lights
- **Additive Blending**: Colors combine where light circles overlap
- **Color Mixing**: Interesting color combinations at intersection points
- **Independent Shadows**: Each light casts its own shadows

### Realistic Scenarios
- **Architectural Walls**: Sharp shadow boundaries from room walls and furniture
- **Large-Scale Lighting**: Simulated radius-60 lighting effects using overlapping smaller lights
- **Production Environments**: Office, warehouse, and commercial lighting setups
- **Day/Night Modes**: Different lighting intensities for various scenarios

## Troubleshooting

### Stack Overflow Errors
The lighting engine uses large static arrays. Always run tests with:
```bash
RUST_MIN_STACK=8388608 cargo test --test output_mechanisms
```

### Empty/Black Images
- Check that `lighting::init()` completed successfully
- Verify light coordinates are within reasonable bounds
- Ensure light radius is > 0

### Missing Obstacle Effects
- Verify obstacles were added with `add_mock_obstacle()`
- Check obstacle coordinates are within the light's range
- Ensure obstacles intersect the light rays

### Image Artifacts
- Small canvas sizes in test mode may cause pixelation
- Very close obstacles may cause unexpected shadow patterns
- Light positions near canvas edges may clip

## Integration with CI/CD

For automated testing:
1. Run tests as part of build process
2. Compare generated images with reference images
3. Flag significant visual changes for review
4. Archive images for version comparison

## Running All Tests at Once

Use the comprehensive test runner script:
```bash
./run_all_visual_tests.sh
```

This script runs all visual tests and provides a detailed report of generated images.

## Extending the Test Suite

To add new test scenarios:
1. Create new test function in `output_mechanisms.rs`
2. Set up lighting and obstacles as needed
3. Generate and save images with descriptive names
4. Update this README with new image descriptions
5. Add the test to `run_all_visual_tests.sh` if desired

## Performance Notes

- Image generation adds ~100-200ms per test
- PNG compression is fast for the small test images
- Most time is spent in lighting calculations, not image generation
- Tests are CPU-bound, suitable for parallel execution