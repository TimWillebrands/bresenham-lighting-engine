//! Core lighting engine implementation using Bresenham-style ray casting.
//!
//! This module provides the main lighting calculations for the engine. It uses
//! CPU-based ray casting algorithms inspired by Bresenham's line algorithm to
//! calculate lighting effects without requiring GPU acceleration.
//!
//! # Key Concepts
//!
//! - **Ray Casting**: Light rays are cast from light sources at 360 different angles
//! - **Shadow Calculation**: When rays hit obstacles, shadow boundaries are calculated
//! - **Light Falloff**: Light intensity decreases with distance from the source
//! - **Color Rendering**: Uses HSV color space for smooth color transitions
//!
//! # Performance Considerations
//!
//! The lighting calculations are CPU-intensive but designed to be deterministic
//! and portable across all platforms. For multiple lights, consider using
//! parallel processing with libraries like `rayon`.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::{arctan, ray};

/// Color mode configuration for light sources
#[derive(Clone, Debug, PartialEq)]
pub enum ColorMode {
    /// Solid color using specified hue (0-255)
    Solid(u8),
    /// Custom HSV color with specified hue and saturation
    Custom { hue: u8, saturation: u8 },
}

/// Maximum distance for light ray casting
#[cfg(all(test, not(target_arch = "wasm32")))]
const MAX_DIST: usize = 10; // Smaller for tests to avoid stack overflow
#[cfg(not(all(test, not(target_arch = "wasm32"))))]
const MAX_DIST: usize = 60;

/// Number of discrete angles for ray casting (360 degrees)  
#[cfg(all(test, not(target_arch = "wasm32")))]
const ANGLES: usize = 36; // Smaller for tests to avoid stack overflow
#[cfg(not(all(test, not(target_arch = "wasm32"))))]
const ANGLES: usize = 360;

/// 2D point represented as (x, y) coordinates using 16-bit signed integers
type PtI = (i16, i16);

/// RGBA color representation with each channel as an 8-bit unsigned integer
/// Layout: R, G, B, A (matches HTML5 Canvas ImageData format)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

/// Pre-computed ray data structure using a HashMap for memory efficiency.
/// This avoids the massive stack allocation that was causing WASM memory issues.
static ALL_RAYS: Lazy<HashMap<(usize, usize), Vec<PtI>>> = Lazy::new(|| {
    let mut rays: HashMap<(usize, usize), Vec<PtI>> = HashMap::new();

    // Calculate all possible points within the maximum light radius
    let center = (0, 0);
    let radius = MAX_DIST as i16;
    let top = center.1 - radius;
    let bottom = center.1 + radius;
    let left = center.0 - radius;
    let right = center.0 + radius;

    // For each point in the bounding box, determine if it's within the light radius
    // and calculate which ray (angle/distance combination) it belongs to
    for y in top..=bottom {
        for x in left..=right {
            let pt = (x, y);
            let dist = arctan::distance(pt);

            if dist <= radius as u16 {
                let raw_angle = arctan::rad_to_deg(arctan::atan2_int(y as i32, x as i32));
                let angle = (raw_angle as usize) % ANGLES; // Ensure angle is always < ANGLES
                let distance = dist as usize;
                
                // Bounds checks
                if angle >= ANGLES || distance >= MAX_DIST {
                    continue;
                }
                
                // Use HashMap to store ray points - much more memory efficient
                rays.entry((distance, angle)).or_insert_with(Vec::new).push(pt);
            }
        }
    }
    
    rays
});

/// Thread-safe storage for all active lights in the scene
///
/// Uses RwLock to allow multiple concurrent reads while ensuring exclusive access
/// for writes, making it safe to use from multiple threads.
static LIGHT_MAP: Lazy<RwLock<HashMap<u8, Light>>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// Represents a single light source with its properties and rendered output
///
/// Each light maintains its own canvas for rendering and blocked angle data
/// for shadow calculations. The light can be updated independently and
/// returns a pointer to its rendered pixel data.
struct Light {
    /// World position of the light source
    pos: PtI,
    /// Radius/range of the light (maximum distance it can illuminate)
    r: i16,
    /// Color mode configuration for this light (None = default rainbow effect)
    color_mode: Option<ColorMode>,
    /// Rendered pixel data for this light (RGBA format)
    canvas: Vec<Color>,
    /// Canvas dimensions (width and height)
    canvas_size: usize,
    /// For each angle (0-359°), stores the distance at which the ray is blocked
    /// A value of 255 means the ray is not blocked within the light's range
    blocked_angles: [u8; ANGLES],
}

impl Light {
    /// Creates a new light source at the specified position with the given radius
    ///
    /// # Arguments
    /// * `pos` - World coordinates (x, y) where the light is positioned
    /// * `r` - Maximum distance the light can illuminate
    /// * `color_mode` - Color configuration for this light (None for default rainbow)
    ///
    /// # Returns
    /// A new Light instance with cleared canvas and unblocked angles
    fn new(pos: PtI, r: i16, color_mode: Option<ColorMode>) -> Self {
        let canvas_size = (r * 2 + 1) as usize;
        let canvas_pixels = canvas_size * canvas_size;
        Light {
            pos,
            r,
            color_mode,
            canvas: vec![Color::default(); canvas_pixels],
            canvas_size,
            blocked_angles: [255; ANGLES], // 255 = not blocked
        }
    }

    /// Updates the light's rendering by recalculating all rays and shadows
    ///
    /// This is the core lighting calculation that:
    /// 1. Resets all blocked angles and canvas pixels
    /// 2. Casts rays at all angles for each distance
    /// 3. Checks for obstacles and calculates shadows
    /// 4. Applies light falloff based on distance
    /// 5. Renders colored pixels to the canvas
    ///
    /// # Returns
    /// Pointer to the beginning of the canvas pixel data for WASM interop
    fn update(&mut self) -> *const Color {
        // Resize canvas if radius changed
        let new_canvas_size = (self.r * 2 + 1) as usize;
        let new_canvas_pixels = new_canvas_size * new_canvas_size;
        if self.canvas.len() != new_canvas_pixels {
            self.canvas = vec![Color::default(); new_canvas_pixels];
            self.canvas_size = new_canvas_size;
        }

        // Reset state for fresh calculation
        self.blocked_angles.fill(255);
        self.canvas.iter_mut().for_each(|p| *p = Color::default());

        // Process each distance ring from the light source
        for d in 0..self.r as usize {
            if d >= MAX_DIST {
                break;
            }

            // Process each angle (360 degrees)
            for angle in 0..ANGLES {
                // Skip this angle if it's already blocked at a closer distance
                if self.blocked_angles[angle] < d as u8 {
                    continue;
                }

                // Process all cells at this specific distance and angle
                if let Some(cells) = ALL_RAYS.get(&(d, angle)) {
                    for cell in cells {
                        // Special case: only process cardinal directions at distance 0
                        if d == 0 && angle % 90 != 0 {
                            continue;
                        }

                        // Transform cell coordinates to world coordinates
                        let curr = (cell.0 + self.pos.0, cell.1 + self.pos.1);
                        let _prev = ray::step(curr, self.pos);

                        // Check if this ray is blocked by an obstacle
                        // CRITICAL: Check the FULL ray from light source to current cell
                        if crate::collision::is_blocked(self.pos.0, self.pos.1, curr.0, curr.1) {
                            // Block only this specific ray and maybe 1 adjacent ray
                            self.blocked_angles[angle] = d as u8;

                            // Optionally block 1 adjacent ray on each side for very close obstacles
                            if d < 3 {
                                let left_angle = if angle > 0 { angle - 1 } else { ANGLES - 1 };
                                let right_angle = (angle + 1) % ANGLES;

                                if self.blocked_angles[left_angle] > d as u8 {
                                    self.blocked_angles[left_angle] = d as u8;
                                }
                                if self.blocked_angles[right_angle] > d as u8 {
                                    self.blocked_angles[right_angle] = d as u8;
                                }
                            }

                            // Skip to next angle since this ray is blocked
                            break;
                        }

                        // Ray is not blocked, so render the light at this position
                        self.render_light_pixel(*cell, angle, d as u8);
                    }
                }
                // Note: If no cells exist for this (distance, angle) combination,
                // we simply skip processing (no error needed with HashMap approach)
            }
        }

        self.canvas.as_ptr()
    }

    /// Renders a single pixel of light onto the canvas
    ///
    /// # Arguments
    /// * `cell` - Local coordinates relative to the light center
    /// * `angle` - The angle of the ray (used for hue calculation when in rainbow mode)
    /// * `distance` - Distance from light source (used for brightness falloff)
    fn render_light_pixel(&mut self, cell: PtI, angle: usize, distance: u8) {
        // Transform local coordinates to canvas coordinates
        let c = (
            cell.0 + self.canvas_size as i16 / 2,
            cell.1 + self.canvas_size as i16 / 2,
        );

        // Check bounds
        if c.0 < 0 || c.1 < 0 || c.0 >= self.canvas_size as i16 || c.1 >= self.canvas_size as i16 {
            return;
        }

        let cell_idx = c.0 as usize + c.1 as usize * self.canvas_size;

        // Calculate brightness falloff based on distance
        let falloff = 255 - (255 * distance as u16) / (self.r as u16);

        // Ensure we don't write outside the canvas bounds
        if cell_idx < self.canvas.len() {
            let color = match &self.color_mode {
                // Default rainbow effect - hue varies by angle, full saturation
                None => {
                    let scaled_hue = (angle * 255) / (ANGLES - 1);
                    hsv2rgb(scaled_hue as u8, 255, falloff as u8)
                }
                // Solid color - fixed hue, full saturation
                Some(ColorMode::Solid(hue)) => {
                    hsv2rgb(*hue, 255, falloff as u8)
                }
                // Custom color - specified hue and saturation
                Some(ColorMode::Custom { hue, saturation }) => {
                    hsv2rgb(*hue, *saturation, falloff as u8)
                }
            };
            
            self.canvas[cell_idx] = color;
        }
    }
}

/// Converts HSV (Hue, Saturation, Value) color to RGB format
///
/// This function provides smooth color transitions by using the HSV color space,
/// which is more intuitive for lighting effects than direct RGB manipulation.
///
/// # Arguments
/// * `h` - Hue (0-255, representing 0-360°)
/// * `s` - Saturation (0-255, 0=grayscale, 255=full color)
/// * `v` - Value/Brightness (0-255, 0=black, 255=full brightness)
///
/// # Returns
/// RGBA color with alpha channel set to 255 (fully opaque)
fn hsv2rgb(h: u8, s: u8, v: u8) -> Color {
    // Handle grayscale case (no saturation)
    if s == 0 {
        return Color(v, v, v, 255);
    }

    // Divide hue into 6 sectors (each 60° of the color wheel)
    let sector = h / 43; // 255/6 ≈ 43
    let remainder = (h - (sector * 43)) * 6;

    // Calculate intermediate color values
    let p = (v as u16 * (255 - s) as u16 / 255) as u8;
    let q = (v as u16 * (255 - (s as u16 * remainder as u16 / 255)) / 255) as u8;
    let t = (v as u16 * (255 - (s as u16 * (255 - remainder) as u16 / 255)) / 255) as u8;

    // Return RGB values based on which sector of the color wheel we're in
    match sector {
        0 => Color(v, t, p, 255), // Red to Yellow
        1 => Color(q, v, p, 255), // Yellow to Green
        2 => Color(p, v, t, 255), // Green to Cyan
        3 => Color(p, q, v, 255), // Cyan to Blue
        4 => Color(t, p, v, 255), // Blue to Magenta
        _ => Color(v, p, q, 255), // Magenta to Red
    }
}

/// Updates an existing light or creates a new one with a solid color
///
/// # Arguments
/// * `id` - Unique identifier for the light (0-255)
/// * `r` - Light radius/range (clamped to MAX_DIST)
/// * `x` - World X coordinate
/// * `y` - World Y coordinate
/// * `hue` - Color hue (0-255, representing 0-360°)
///
/// # Returns
/// Pointer to the light's canvas data for rendering, or null pointer on error
pub fn update_or_add_light_with_solid_color(id: u8, r: i16, x: i16, y: i16, hue: u8) -> *const Color {
    update_light_with_color_mode(id, r, x, y, Some(ColorMode::Solid(hue)))
}

/// Updates an existing light or creates a new one with custom HSV color
///
/// # Arguments
/// * `id` - Unique identifier for the light (0-255)
/// * `r` - Light radius/range (clamped to MAX_DIST)
/// * `x` - World X coordinate
/// * `y` - World Y coordinate
/// * `hue` - Color hue (0-255, representing 0-360°)
/// * `saturation` - Color saturation (0-255, 0=grayscale, 255=full color)
///
/// # Returns
/// Pointer to the light's canvas data for rendering, or null pointer on error
pub fn update_or_add_light_with_custom_color(id: u8, r: i16, x: i16, y: i16, hue: u8, saturation: u8) -> *const Color {
    update_light_with_color_mode(id, r, x, y, Some(ColorMode::Custom { hue, saturation }))
}

/// Internal helper function to update lights with any color mode
fn update_light_with_color_mode(id: u8, r: i16, x: i16, y: i16, color_mode: Option<ColorMode>) -> *const Color {
    // Clamp radius to maximum supported distance
    let clamped_r = r.min(MAX_DIST as i16).max(1);

    // Attempt to get write access to the light map
    if let Ok(mut light_map) = LIGHT_MAP.write() {
        // Check if we need to create a new light or update existing
        let needs_new_light = if let Some(existing_light) = light_map.get(&id) {
            existing_light.r != clamped_r || existing_light.color_mode != color_mode
        } else {
            true
        };

        if needs_new_light {
            // Create new light with correct radius and color mode
            let new_light = Light::new((x, y), clamped_r, color_mode.clone());
            light_map.insert(id, new_light);
        }

        // Get the light and update its properties
        if let Some(light) = light_map.get_mut(&id) {
            light.pos = (x, y);
            light.r = clamped_r;
            light.color_mode = color_mode;
            light.update()
        } else {
            std::ptr::null()
        }
    } else {
        // Return null pointer if we can't acquire the lock
        std::ptr::null()
    }
}

/// Updates an existing light or creates a new one with the specified parameters
///
/// This function maintains backward compatibility by using the default rainbow color mode.
/// For custom colors, use `update_or_add_light_with_solid_color` or 
/// `update_or_add_light_with_custom_color`.
///
/// # Arguments
/// * `id` - Unique identifier for the light (0-255)
/// * `r` - Light radius/range (clamped to MAX_DIST)
/// * `x` - World X coordinate
/// * `y` - World Y coordinate
///
/// # Returns
/// Pointer to the light's canvas data for rendering, or null pointer on error
///
/// # Thread Safety
/// This function is thread-safe thanks to the RwLock protecting the light map.
/// Multiple lights can be updated concurrently from different threads.
pub fn update_or_add_light(id: u8, r: i16, x: i16, y: i16) -> *const Color {
    update_light_with_color_mode(id, r, x, y, None)
}

/// Initializes the lighting system
///
/// This function must be called before any lighting calculations can be performed.
/// It forces the lazy initialization of the ray lookup tables, which is an expensive
/// one-time calculation that pre-computes all possible ray trajectories.
///
/// # Performance Note
/// The initialization involves computing ray trajectories for MAX_DIST distances × ANGLES angles,
/// which can take a noticeable amount of time on slower devices. Consider calling
/// this during a loading screen or startup phase.
pub fn init() {
    // Force initialization of the ray lookup table
    Lazy::force(&ALL_RAYS);

    // The LIGHT_MAP is already initialized via Lazy::new(), so no additional setup needed
}
