//! Visual output mechanisms for testing the Bresenham lighting engine.
//!
//! This module provides comprehensive testing with image output capabilities,
//! allowing AI agents and developers to see the actual visual results of
//! lighting calculations. This creates a tighter feedback loop for debugging
//! and validation.
//!
//! # Features
//!
//! - **Image Generation**: Creates PNG files showing light canvases
//! - **Composite Scenes**: Combines multiple lights into single images
//! - **Obstacle Visualization**: Shows how obstacles affect lighting
//! - **Comparison Images**: Side-by-side before/after comparisons
//! - **Mock Environment**: Provides test-friendly obstacle detection
//!
//! # Output Directory
//!
//! All images are saved to `test_output/` directory in the project root.
//! The directory is created automatically if it doesn't exist.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use image::{ImageBuffer, Rgb, RgbImage};

use bresenham_lighting_engine::*;

/// Simple mock obstacle detection for testing.
///
/// This replaces the JavaScript-based obstacle detection with a pure Rust
/// implementation that can be used in test environments.
use std::sync::Mutex;
static MOCK_OBSTACLES: Mutex<Vec<(i16, i16, i16, i16)>> = Mutex::new(Vec::new());

/// Mock implementation of the is_blocked function for testing.
///
/// This function checks if a line segment intersects with any of the
/// mock obstacles that have been set up for testing.
///
/// # Arguments
/// * `x0`, `y0` - Starting point of the line segment
/// * `x1`, `y1` - Ending point of the line segment
///
/// # Returns
/// `true` if the line segment intersects with any mock obstacle
pub fn mock_is_blocked(x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
    if let Ok(obstacles) = MOCK_OBSTACLES.lock() {
        for &(ox0, oy0, ox1, oy1) in obstacles.iter() {
            if line_segments_intersect(x0, y0, x1, y1, ox0, oy0, ox1, oy1) {
                return true;
            }
        }
    }
    false
}

/// Add a mock obstacle for testing.
///
/// # Arguments
/// * `x0`, `y0` - Starting point of the obstacle line
/// * `x1`, `y1` - Ending point of the obstacle line
pub fn add_mock_obstacle(x0: i16, y0: i16, x1: i16, y1: i16) {
    if let Ok(mut obstacles) = MOCK_OBSTACLES.lock() {
        obstacles.push((x0, y0, x1, y1));
    }
}

/// Clear all mock obstacles.
pub fn clear_mock_obstacles() {
    if let Ok(mut obstacles) = MOCK_OBSTACLES.lock() {
        obstacles.clear();
    }
}

/// Check if two line segments intersect using the cross product method.
fn line_segments_intersect(
    x1: i16,
    y1: i16,
    x2: i16,
    y2: i16,
    x3: i16,
    y3: i16,
    x4: i16,
    y4: i16,
) -> bool {
    fn ccw(ax: i16, ay: i16, bx: i16, by: i16, cx: i16, cy: i16) -> bool {
        (cy - ay) * (bx - ax) > (by - ay) * (cx - ax)
    }

    ccw(x1, y1, x3, y3, x4, y4) != ccw(x2, y2, x3, y3, x4, y4)
        && ccw(x1, y1, x2, y2, x3, y3) != ccw(x1, y1, x2, y2, x4, y4)
}

/// Initialize the test environment.
fn init_test_environment() {
    // Set up the mock obstacle detection function
    set_is_blocked_fn(mock_is_blocked);

    // Initialize the lighting system
    lighting::init();
    block_map::init();

    // Clear any existing mock obstacles
    clear_mock_obstacles();
}

/// Create the output directory if it doesn't exist.
fn ensure_output_dir() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("test_output");
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }

    let output_gitignore = Path::new("test_output/.gitignore");
    if !output_gitignore.exists() {
        let mut gitignore = File::create(output_gitignore)?;
        gitignore.write_all(b"*")?;
    }

    Ok(())
}

/// Convert a light canvas to an RGB image.
///
/// # Arguments
/// * `canvas_ptr` - Pointer to the light canvas data
/// * `canvas_size` - Size of the canvas (width/height)
///
/// # Returns
/// RGB image buffer containing the light data
fn canvas_to_image(canvas_ptr: *const Color, canvas_size: usize) -> RgbImage {
    let mut img = ImageBuffer::new(canvas_size as u32, canvas_size as u32);

    if canvas_ptr.is_null() {
        return img;
    }

    unsafe {
        let canvas_slice = std::slice::from_raw_parts(canvas_ptr, canvas_size * canvas_size);

        for (i, &Color(r, g, b, _a)) in canvas_slice.iter().enumerate() {
            let x = (i % canvas_size) as u32;
            let y = (i / canvas_size) as u32;
            img.put_pixel(x, y, Rgb([r, g, b]));
        }
    }

    img
}

/// Create a composite image showing multiple light sources.
///
/// # Arguments
/// * `lights` - Vector of (canvas_ptr, canvas_size, x, y, label) tuples
/// * `world_width` - Total width of the world to render
/// * `world_height` - Total height of the world to render
///
/// # Returns
/// RGB image showing the composite lighting scene
fn create_composite_image(
    lights: Vec<(*const Color, usize, i16, i16, &str)>,
    world_width: u32,
    world_height: u32,
) -> RgbImage {
    let mut composite = ImageBuffer::new(world_width, world_height);

    // Render each light onto the composite
    for &(canvas_ptr, canvas_size, light_x, light_y, _label) in &lights {
        if canvas_ptr.is_null() {
            continue;
        }

        let light_img = canvas_to_image(canvas_ptr, canvas_size);
        let half_size = (canvas_size / 2) as i32;

        // Calculate the position to place this light on the composite
        let start_x = (light_x as i32 - half_size).max(0) as u32;
        let start_y = (light_y as i32 - half_size).max(0) as u32;

        // Blend the light onto the composite
        for y in 0..canvas_size as u32 {
            for x in 0..canvas_size as u32 {
                let composite_x = start_x + x;
                let composite_y = start_y + y;

                if composite_x < world_width && composite_y < world_height {
                    let light_pixel = light_img.get_pixel(x, y);
                    let composite_pixel: Rgb<u8> = *composite.get_pixel(composite_x, composite_y);

                    // Simple additive blending
                    let r = (light_pixel[0] as u16 + composite_pixel[0] as u16).min(255) as u8;
                    let g = (light_pixel[1] as u16 + composite_pixel[1] as u16).min(255) as u8;
                    let b = (light_pixel[2] as u16 + composite_pixel[2] as u16).min(255) as u8;

                    composite.put_pixel(composite_x, composite_y, Rgb([r, g, b]));
                }
            }
        }
    }

    composite
}

/// Draw obstacles on an image.
fn draw_obstacles_on_image(img: &mut RgbImage) {
    if let Ok(obstacles) = MOCK_OBSTACLES.lock() {
        for &(x0, y0, x1, y1) in obstacles.iter() {
            // Draw a thick line representing the obstacle
            let points = bresenham_line(x0, y0, x1, y1);
            for (x, y) in points {
                if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
                    // Draw obstacle in white
                    img.put_pixel(x as u32, y as u32, Rgb([255, 255, 255]));
                    // Draw a thicker line by drawing adjacent pixels
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            let nx = x + dx;
                            let ny = y + dy;
                            if nx >= 0
                                && ny >= 0
                                && (nx as u32) < img.width()
                                && (ny as u32) < img.height()
                            {
                                img.put_pixel(nx as u32, ny as u32, Rgb([255, 255, 255]));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Simple Bresenham line algorithm for drawing obstacles.
fn bresenham_line(x0: i16, y0: i16, x1: i16, y1: i16) -> Vec<(i16, i16)> {
    let mut points = Vec::new();
    let mut x0 = x0;
    let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    loop {
        points.push((x0, y0));

        if x0 == x1 && y0 == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x0 += sx;
        }
        if e2 < dx {
            err += dx;
            y0 += sy;
        }
    }

    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_light_output() -> Result<(), Box<dyn std::error::Error>> {
        ensure_output_dir()?;
        init_test_environment();

        // Create a simple light - using smaller radius for test mode
        let canvas_ptr = lighting::update_or_add_light(1, 3, 0, 0);

        // In test mode, the function might return null due to WASM limitations
        // but we should still generate what we can
        let canvas_size = 3 * 2 + 1; // radius * 2 + 1
        let img = canvas_to_image(canvas_ptr, canvas_size);
        img.save("test_output/single_light.png")?;

        println!("✓ Generated single_light.png");
        Ok(())
    }

    #[test]
    fn test_multiple_lights() -> Result<(), Box<dyn std::error::Error>> {
        ensure_output_dir()?;
        init_test_environment();

        // Create multiple lights - using smaller radii for test mode
        let light1 = lighting::update_or_add_light(1, 2, 5, 5);
        let light2 = lighting::update_or_add_light(2, 3, 12, 8);
        let light3 = lighting::update_or_add_light(3, 2, 8, 12);

        // Save individual light images
        let img1 = canvas_to_image(light1, 2 * 2 + 1);
        img1.save("test_output/light1_individual.png")?;

        let img2 = canvas_to_image(light2, 3 * 2 + 1);
        img2.save("test_output/light2_individual.png")?;

        let img3 = canvas_to_image(light3, 2 * 2 + 1);
        img3.save("test_output/light3_individual.png")?;

        // Create composite scene
        let lights = vec![
            (light1, 2 * 2 + 1, 5, 5, "Light 1"),
            (light2, 3 * 2 + 1, 12, 8, "Light 2"),
            (light3, 2 * 2 + 1, 8, 12, "Light 3"),
        ];

        let composite = create_composite_image(lights, 25, 25);
        composite.save("test_output/multiple_lights_composite.png")?;

        println!("✓ Generated multiple lights images");
        Ok(())
    }

    #[test]
    fn test_lights_with_obstacles() -> Result<(), Box<dyn std::error::Error>> {
        ensure_output_dir()?;
        init_test_environment();

        // Add some obstacles
        add_mock_obstacle(2, 2, 6, 2); // Horizontal wall
        add_mock_obstacle(6, 2, 6, 6); // Vertical wall
        add_mock_obstacle(3, 3, 5, 5); // Diagonal wall

        // Create a light that will be affected by obstacles
        let light_ptr = lighting::update_or_add_light(1, 3, 4, 4);

        let canvas_size = 3 * 2 + 1;
        let mut img = canvas_to_image(light_ptr, canvas_size);

        // Draw obstacles on the image for visualization
        draw_obstacles_on_image(&mut img);

        img.save("test_output/light_with_obstacles.png")?;

        // Create a comparison without obstacles
        clear_mock_obstacles();
        let light_no_obstacles = lighting::update_or_add_light(2, 3, 4, 4);

        let canvas_size = 3 * 2 + 1;
        let img = canvas_to_image(light_no_obstacles, canvas_size);
        img.save("test_output/light_no_obstacles.png")?;

        println!("✓ Generated obstacle comparison images");
        Ok(())
    }

    #[test]
    fn test_different_light_sizes() -> Result<(), Box<dyn std::error::Error>> {
        ensure_output_dir()?;
        init_test_environment();

        let sizes = vec![1, 2, 3, 4]; // Smaller sizes for test mode

        for (i, &size) in sizes.iter().enumerate() {
            let light_ptr = lighting::update_or_add_light(i as u8, size, 0, 0);

            let canvas_size = size * 2 + 1;
            let img = canvas_to_image(light_ptr, canvas_size as usize);
            img.save(format!("test_output/light_size_{}.png", size))?;
        }

        println!("✓ Generated different light size images");
        Ok(())
    }

    #[test]
    fn test_light_movement_sequence() -> Result<(), Box<dyn std::error::Error>> {
        ensure_output_dir()?;
        init_test_environment();

        // Create a sequence showing light movement
        let positions = vec![(2, 2), (4, 2), (6, 4), (4, 6), (2, 4)];

        for (frame, &(x, y)) in positions.iter().enumerate() {
            let light_ptr = lighting::update_or_add_light(1, 2, x, y);

            let canvas_size = 2 * 2 + 1;
            let img = canvas_to_image(light_ptr, canvas_size);
            img.save(format!("test_output/movement_frame_{:02}.png", frame))?;
        }

        println!("✓ Generated light movement sequence");
        Ok(())
    }

    #[test]
    fn test_complex_scene() -> Result<(), Box<dyn std::error::Error>> {
        ensure_output_dir()?;
        init_test_environment();

        // Create a complex scene with multiple lights and obstacles
        add_mock_obstacle(8, 4, 12, 4); // Horizontal wall
        add_mock_obstacle(12, 4, 12, 8); // Vertical wall
        add_mock_obstacle(4, 10, 8, 10); // Another horizontal wall
        add_mock_obstacle(6, 6, 10, 8); // Diagonal wall

        // Add multiple lights
        let lights = vec![
            (
                lighting::update_or_add_light(1, 3, 6, 2),
                3 * 2 + 1,
                6,
                2,
                "Main Light",
            ),
            (
                lighting::update_or_add_light(2, 2, 14, 6),
                2 * 2 + 1,
                14,
                6,
                "Side Light",
            ),
            (
                lighting::update_or_add_light(3, 2, 10, 12),
                2 * 2 + 1,
                10,
                12,
                "Bottom Light",
            ),
            (
                lighting::update_or_add_light(4, 2, 2, 8),
                2 * 2 + 1,
                2,
                8,
                "Corner Light",
            ),
        ];

        // Create composite image
        let mut composite = create_composite_image(lights, 20, 20);

        // Draw obstacles
        draw_obstacles_on_image(&mut composite);

        composite.save("test_output/complex_scene.png")?;

        println!("✓ Generated complex scene image");
        Ok(())
    }

    #[test]
    fn test_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
        ensure_output_dir()?;
        init_test_environment();

        // Test minimum size light
        let min_light = lighting::update_or_add_light(1, 1, 0, 0);
        let img = canvas_to_image(min_light, 1 * 2 + 1);
        img.save("test_output/minimum_light.png")?;

        // Test light at edge of test limits
        let max_light = lighting::update_or_add_light(2, 5, 0, 0); // Close to MAX_DIST in test mode (10)
        let img = canvas_to_image(max_light, 5 * 2 + 1);
        img.save("test_output/maximum_light.png")?;

        // Test light with obstacles very close
        add_mock_obstacle(1, 0, 1, 2);
        add_mock_obstacle(0, 1, 2, 1);

        let blocked_light = lighting::update_or_add_light(3, 3, 1, 1);
        let mut img = canvas_to_image(blocked_light, 3 * 2 + 1);
        draw_obstacles_on_image(&mut img);
        img.save("test_output/heavily_blocked_light.png")?;

        println!("✓ Generated edge case images");
        Ok(())
    }

    #[test]
    fn test_realistic_large_light_with_walls() -> Result<(), Box<dyn std::error::Error>> {
        ensure_output_dir()?;
        init_test_environment();

        // Create a realistic scene using the maximum test radius (10) for true wall interaction
        // Room dimensions are scaled to work well with radius-10 lights

        // Create a medium-sized room with proper proportions for radius-10 lights
        // Room center at (15,15), walls at distance ~12-15 from center so light actually hits them
        add_mock_obstacle(5, 5, 25, 5); // Top wall
        add_mock_obstacle(5, 5, 5, 25); // Left wall
        add_mock_obstacle(5, 25, 25, 25); // Bottom wall
        add_mock_obstacle(25, 5, 25, 15); // Right wall (partial - doorway)

        // Interior furniture at distances where radius-10 light will interact
        add_mock_obstacle(10, 10, 18, 10); // Table/counter - 5-8 units from center
        add_mock_obstacle(10, 10, 10, 13); // Table leg
        add_mock_obstacle(18, 10, 18, 13); // Table leg
        add_mock_obstacle(15, 18, 15, 22); // Column/pillar - 8 units from center

        // Position light at room center for optimal wall interaction
        let main_light_x = 15;
        let main_light_y = 15;

        // Use maximum test radius (10) to actually reach the walls
        let main_light = lighting::update_or_add_light(1, 10, main_light_x, main_light_y);

        // Add secondary lights at corners for enhanced lighting
        let corner_light1 = lighting::update_or_add_light(2, 8, 8, 8); // Top-left corner
        let corner_light2 = lighting::update_or_add_light(3, 6, 22, 8); // Top-right corner
        let corner_light3 = lighting::update_or_add_light(4, 7, 8, 22); // Bottom-left corner

        let lights = vec![
            (
                main_light,
                10 * 2 + 1,
                main_light_x,
                main_light_y,
                "Central Light (R10)",
            ),
            (corner_light1, 8 * 2 + 1, 8, 8, "Corner Light 1 (R8)"),
            (corner_light2, 6 * 2 + 1, 22, 8, "Corner Light 2 (R6)"),
            (corner_light3, 7 * 2 + 1, 8, 22, "Corner Light 3 (R7)"),
        ];

        // Create appropriately sized composite (35x35) for the room
        let mut composite = create_composite_image(lights, 35, 35);

        // Draw the walls/obstacles on the image for visualization
        draw_obstacles_on_image(&mut composite);

        composite.save("test_output/realistic_large_light_with_walls.png")?;

        // Also create a comparison version without walls
        clear_mock_obstacles();
        let light_no_walls = lighting::update_or_add_light(10, 10, main_light_x, main_light_y);
        let lights_no_walls = vec![(
            light_no_walls,
            10 * 2 + 1,
            main_light_x,
            main_light_y,
            "Light Without Walls (R10)",
        )];
        let composite_no_walls = create_composite_image(lights_no_walls, 35, 35);
        composite_no_walls.save("test_output/realistic_light_no_walls.png")?;

        println!("✓ Generated realistic large light scene with architectural walls");
        println!(
            "  - realistic_large_light_with_walls.png: Shows radius-10 light interaction with room walls"
        );
        println!("  - realistic_light_no_walls.png: Same light without obstacles for comparison");
        println!(
            "  - Uses maximum test radius (10) for actual wall interaction and shadow casting"
        );

        Ok(())
    }

    #[test]
    #[ignore] // This test requires production build to run properly
    fn test_production_scale_lighting() -> Result<(), Box<dyn std::error::Error>> {
        ensure_output_dir()?;

        // This test is designed to demonstrate what the engine can do in production mode
        // Run with: cargo test --release test_production_scale_lighting -- --ignored

        println!("🏭 Production Scale Lighting Test");
        println!("This test demonstrates the engine's full capabilities with:");
        println!("  • Radius 60 lights (vs test mode limit of 5)");
        println!("  • 360 ray angles (vs test mode limit of 36)");
        println!("  • Complex architectural scenes");
        println!("  • Large-scale room and building lighting");

        // Note: In test mode, this will be limited, but serves as documentation
        // of what the production system can achieve

        init_test_environment();

        // Create a realistic office environment scaled for test mode (radius 10 max)
        // Office dimensions: 50x40 - reasonable for radius-10 lights
        add_mock_obstacle(5, 5, 45, 5); // North wall
        add_mock_obstacle(5, 5, 5, 35); // West wall
        add_mock_obstacle(45, 5, 45, 35); // East wall
        add_mock_obstacle(5, 35, 45, 35); // South wall

        // Interior architectural elements positioned for light interaction
        add_mock_obstacle(15, 10, 15, 30); // Support column 1
        add_mock_obstacle(25, 10, 25, 30); // Support column 2
        add_mock_obstacle(35, 10, 35, 30); // Support column 3

        // Work areas / furniture within light range
        add_mock_obstacle(10, 15, 20, 15); // Desk row 1
        add_mock_obstacle(10, 25, 20, 25); // Desk row 2
        add_mock_obstacle(30, 15, 40, 15); // Desk row 3
        add_mock_obstacle(30, 25, 40, 25); // Desk row 4

        // Meeting room in corner
        add_mock_obstacle(35, 20, 42, 20); // Meeting room north
        add_mock_obstacle(35, 20, 35, 32); // Meeting room west
        add_mock_obstacle(42, 20, 42, 32); // Meeting room east

        // Position lights strategically for maximum wall/obstacle interaction
        let main_x = 25; // Center of office
        let main_y = 20;

        // Use maximum radius (10) for main lighting that will actually reach walls and obstacles
        let central_light = lighting::update_or_add_light(1, 10, main_x, main_y);

        // Additional accent lighting with substantial radii
        let accent1 = lighting::update_or_add_light(2, 8, 12, 12); // Work area 1
        let accent2 = lighting::update_or_add_light(3, 8, 38, 12); // Work area 2
        let accent3 = lighting::update_or_add_light(4, 7, 38, 26); // Meeting room
        let accent4 = lighting::update_or_add_light(5, 6, 10, 28); // Corner area

        let lights = vec![
            (
                central_light,
                10 * 2 + 1,
                main_x,
                main_y,
                "Main Overhead (R10)",
            ),
            (accent1, 8 * 2 + 1, 12, 12, "Work Area 1 (R8)"),
            (accent2, 8 * 2 + 1, 38, 12, "Work Area 2 (R8)"),
            (accent3, 7 * 2 + 1, 38, 26, "Meeting Room (R7)"),
            (accent4, 6 * 2 + 1, 10, 28, "Corner (R6)"),
        ];

        // Create appropriately sized composite (50x40) for test mode
        let mut composite = create_composite_image(lights, 50, 40);
        draw_obstacles_on_image(&mut composite);

        composite.save("test_output/production_scale_office_lighting.png")?;

        // Also create a night scene version with different light intensities
        clear_mock_obstacles();

        // Same obstacles but different lighting mood
        add_mock_obstacle(5, 5, 45, 5);
        add_mock_obstacle(5, 5, 5, 35);
        add_mock_obstacle(45, 5, 45, 35);
        add_mock_obstacle(5, 35, 45, 35);
        add_mock_obstacle(15, 10, 15, 30);
        add_mock_obstacle(25, 10, 25, 30);
        add_mock_obstacle(35, 10, 35, 30);

        // Night lighting - more focused, less ambient but still large enough to interact with walls
        let night_main = lighting::update_or_add_light(10, 8, main_x, main_y); // Reduced but still substantial
        let night_accent1 = lighting::update_or_add_light(11, 5, 38, 26); // Meeting room
        let night_accent2 = lighting::update_or_add_light(12, 4, 12, 12); // Security corner

        let night_lights = vec![
            (night_main, 8 * 2 + 1, main_x, main_y, "Night Main (R8)"),
            (night_accent1, 5 * 2 + 1, 38, 26, "Night Meeting Room (R5)"),
            (night_accent2, 4 * 2 + 1, 12, 12, "Night Security (R4)"),
        ];

        let mut night_composite = create_composite_image(night_lights, 50, 40);
        draw_obstacles_on_image(&mut night_composite);

        night_composite.save("test_output/production_scale_night_lighting.png")?;

        println!("✓ Generated production-scale lighting demonstrations");
        println!("  - production_scale_office_lighting.png: Full office lighting with radius 6-10 lights");
        println!("  - production_scale_night_lighting.png: Night/security lighting mode with radius 4-8 lights");
        println!("  - Showcases architectural lighting with actual wall and obstacle shadow interactions");
        println!("  - Room scaled appropriately for test mode MAX_DIST (10) to show realistic lighting behavior");

        Ok(())
    }

    #[test]
    fn test_radius_comparison_realistic_scenarios() -> Result<(), Box<dyn std::error::Error>> {
        ensure_output_dir()?;
        init_test_environment();

        // Create a scenario that clearly shows the difference between small and large radii
        // This demonstrates why the original complaint about "radius too small" was valid

        // Create a medium room where the difference will be obvious
        add_mock_obstacle(8, 8, 22, 8); // Top wall
        add_mock_obstacle(8, 8, 8, 22); // Left wall
        add_mock_obstacle(8, 22, 22, 22); // Bottom wall
        add_mock_obstacle(22, 8, 22, 22); // Right wall

        // Add furniture that's positioned to show shadow differences
        add_mock_obstacle(12, 12, 18, 12); // Table
        add_mock_obstacle(15, 15, 15, 18); // Chair

        let center_x = 15;
        let center_y = 15;

        // Test 1: Small radius (3) - the old approach
        let small_light = lighting::update_or_add_light(1, 3, center_x, center_y);
        let small_img = canvas_to_image(small_light, 3 * 2 + 1);
        // Extend canvas to show room context
        let mut small_room_img = image::ImageBuffer::new(30, 30);
        // Place the small light canvas in the center of the larger room image
        for y in 0..(3 * 2 + 1) {
            for x in 0..(3 * 2 + 1) {
                let light_pixel = small_img.get_pixel(x as u32, y as u32);
                let room_x = (center_x - 3) + x as i16;
                let room_y = (center_y - 3) + y as i16;
                if room_x >= 0 && room_y >= 0 && (room_x as u32) < 30 && (room_y as u32) < 30 {
                    small_room_img.put_pixel(room_x as u32, room_y as u32, *light_pixel);
                }
            }
        }
        draw_obstacles_on_image(&mut small_room_img);
        small_room_img.save("test_output/radius_comparison_small_r3.png")?;

        // Test 2: Large radius (10) - the improved approach
        clear_mock_obstacles();
        add_mock_obstacle(8, 8, 22, 8); // Top wall
        add_mock_obstacle(8, 8, 8, 22); // Left wall
        add_mock_obstacle(8, 22, 22, 22); // Bottom wall
        add_mock_obstacle(22, 8, 22, 22); // Right wall
        add_mock_obstacle(12, 12, 18, 12); // Table
        add_mock_obstacle(15, 15, 15, 18); // Chair

        let large_light = lighting::update_or_add_light(2, 10, center_x, center_y);
        let mut large_room_img = canvas_to_image(large_light, 10 * 2 + 1);
        draw_obstacles_on_image(&mut large_room_img);
        large_room_img.save("test_output/radius_comparison_large_r10.png")?;

        // Test 3: Side-by-side comparison without obstacles to show light coverage
        clear_mock_obstacles();
        let small_no_walls = lighting::update_or_add_light(3, 3, 10, 15);
        let large_no_walls = lighting::update_or_add_light(4, 10, 20, 15);

        let comparison_lights = vec![
            (small_no_walls, 3 * 2 + 1, 10, 15, "Small R3"),
            (large_no_walls, 10 * 2 + 1, 20, 15, "Large R10"),
        ];

        let comparison_img = create_composite_image(comparison_lights, 35, 30);
        comparison_img.save("test_output/radius_comparison_side_by_side.png")?;

        println!(
            "✓ Generated radius comparison images demonstrating the importance of larger radii"
        );
        println!("  - radius_comparison_small_r3.png: Shows how small radius barely reaches walls");
        println!(
            "  - radius_comparison_large_r10.png: Shows proper wall interaction with large radius"
        );
        println!(
            "  - radius_comparison_side_by_side.png: Side-by-side comparison of coverage areas"
        );

        Ok(())
    }

    #[test]
    fn test_output_summary() {
        println!("\n=== Bresenham Lighting Engine Test Output Summary ===");
        println!("All test images have been generated in the 'test_output/' directory.");
        println!("\nGenerated images:");
        println!("• single_light.png - Basic single light source");
        println!("• light1_individual.png, light2_individual.png, light3_individual.png - Individual lights");
        println!("• multiple_lights_composite.png - Multiple lights combined");
        println!("• light_with_obstacles.png - Light affected by obstacles");
        println!("• light_no_obstacles.png - Same light without obstacles");
        println!("• light_size_X.png - Lights of different sizes");
        println!("• movement_frame_XX.png - Light movement animation frames");
        println!("• complex_scene.png - Complex scene with multiple lights and obstacles");
        println!("• minimum_light.png - Smallest possible light");
        println!("• maximum_light.png - Largest light within test limits");
        println!("• heavily_blocked_light.png - Light with nearby obstacles");
        println!("• realistic_large_light_with_walls.png - Properly scaled room lighting with radius-10 lights");
        println!("• realistic_light_no_walls.png - Same scene without walls for comparison");
        println!("• production_scale_office_lighting.png - Office lighting with appropriate radius scaling");
        println!("• production_scale_night_lighting.png - Night mode lighting scenario");
        println!("• radius_comparison_small_r3.png - Shows inadequate coverage with small radius");
        println!(
            "• radius_comparison_large_r10.png - Shows proper wall interaction with large radius"
        );
        println!(
            "• radius_comparison_side_by_side.png - Direct comparison of light coverage areas"
        );
        println!("\nThese images provide visual feedback for:");
        println!("✓ Individual light rendering");
        println!("✓ Multi-light composition");
        println!("✓ Obstacle shadow casting");
        println!("✓ Light falloff and color gradients");
        println!("✓ Edge cases and boundary conditions");
        println!("✓ Animation and movement effects");
        println!("✓ Realistic architectural lighting scenarios with proper radius scaling");
        println!("✓ Large-scale room and building interior lighting");
        println!("✓ Production-scale office and commercial lighting");
        println!("✓ Day/night lighting mode comparisons");
        println!("✓ Radius size impact on realistic lighting scenarios");
        println!("\nUse these images to verify lighting behavior and debug issues!");
    }
}
