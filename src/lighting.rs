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

use crate::{arctan, is_blocked, ray};

/// Maximum distance for light ray casting
#[cfg(not(test))]
const DIST: usize = 60;
#[cfg(test)]
const DIST: usize = 10; // Smaller for tests to avoid stack overflow

/// Number of discrete angles for ray casting (360 degrees)
#[cfg(not(test))]
const ANGLES: usize = 360;
#[cfg(test)]
const ANGLES: usize = 36; // Smaller for tests to avoid stack overflow

/// Size of one side of the light canvas (diameter of the light circle)
const LIGHT_ROW: usize = DIST * 2 + 1;

/// Total number of pixels in the light canvas
const LIGHT_SIZE: usize = LIGHT_ROW * LIGHT_ROW;

/// 2D point represented as (x, y) coordinates using 16-bit signed integers
type PtI = (i16, i16);

/// RGBA color representation with each channel as an 8-bit unsigned integer
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

/// Pre-computed ray data structure containing all possible ray points
/// organized by [distance][angle] for efficient lookup during lighting calculations.
///
/// This lazy static is computed once at startup and contains all the points
/// that each ray will traverse for every possible angle and distance combination.
static ALL_RAYS: Lazy<[[Vec<PtI>; ANGLES]; DIST]> = Lazy::new(|| {
    let mut rays: [[Vec<PtI>; ANGLES]; DIST] =
        std::array::from_fn(|_| std::array::from_fn(|_| Vec::new()));

    // Calculate all possible points within the light radius
    let center = (0, 0);
    let radius = DIST as i16;
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
                let angle = arctan::rad_to_deg(arctan::atan2_int(y as i32, x as i32)) as usize;
                if angle < ANGLES && (dist as usize) < DIST {
                    rays[dist as usize][angle].push(pt);
                }
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
    /// Rendered pixel data for this light (RGBA format)
    canvas: [Color; LIGHT_SIZE],
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
    ///
    /// # Returns
    /// A new Light instance with cleared canvas and unblocked angles
    fn new(pos: PtI, r: i16) -> Self {
        Light {
            pos,
            r,
            canvas: [Color::default(); LIGHT_SIZE],
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
        // Reset state for fresh calculation
        self.blocked_angles.fill(255);
        self.canvas.iter_mut().for_each(|p| *p = Color::default());

        // Process each distance ring from the light source
        for d in 0..self.r as u8 {
            // Process each angle (360 degrees)
            'angle_loop: for angle in 0..ANGLES {
                // Skip this angle if it's already blocked at a closer distance
                if self.blocked_angles[angle] < d {
                    continue;
                }

                // Process all cells at this specific distance and angle
                for cell in &ALL_RAYS[d as usize][angle] {
                    // Special case: only process cardinal directions at distance 0
                    if d == 0 && angle % 90 != 0 {
                        continue;
                    }

                    // Transform cell coordinates to world coordinates
                    let curr = (cell.0 + self.pos.0, cell.1 + self.pos.1);
                    let prev = ray::step(curr, self.pos);

                    // Check if this ray is blocked by an obstacle
                    if is_blocked(prev.0, prev.1, curr.0, curr.1) {
                        // Calculate shadow projection when ray hits an obstacle
                        let (p, n) = self.get_shadow_directions(angle, *cell);

                        let prev_angle =
                            arctan::rad_to_deg(arctan::atan2_int(p.1 as i32, p.0 as i32)) as usize;
                        let next_angle =
                            arctan::rad_to_deg(arctan::atan2_int(n.1 as i32, n.0 as i32)) as usize;

                        // Mark the shadow range as blocked
                        self.mark_shadow_range(prev_angle, next_angle, d);

                        // Skip to next angle since this ray is blocked
                        continue 'angle_loop;
                    }

                    // Ray is not blocked, so render the light at this position
                    self.render_light_pixel(*cell, angle, d);
                }
            }
        }

        self.canvas.as_ptr()
    }

    /// Determines the shadow projection directions based on the angle and obstacle position
    ///
    /// # Arguments
    /// * `angle` - The angle of the ray that hit the obstacle (0-359°)
    /// * `cell` - The local coordinates of the obstacle relative to the light
    ///
    /// # Returns
    /// A tuple of two points representing the shadow boundaries (previous, next)
    fn get_shadow_directions(&self, angle: usize, cell: PtI) -> (PtI, PtI) {
        match angle {
            315 => ((cell.0 - 1, cell.1), (cell.0, cell.1 - 1)),
            a if !(45..=315).contains(&a) => ((cell.0, cell.1 + 1), (cell.0, cell.1 - 1)),
            45 => ((cell.0, cell.1 + 1), (cell.0 + 1, cell.1)),
            a if a > 45 && a < 135 => ((cell.0 - 1, cell.1), (cell.0 + 1, cell.1)),
            135 => ((cell.0 - 1, cell.1), (cell.0, cell.1 + 1)),
            a if a > 135 && a < 225 => ((cell.0, cell.1 - 1), (cell.0, cell.1 + 1)),
            225 => ((cell.0, cell.1 - 1), (cell.0 - 1, cell.1)),
            _ => ((cell.0 + 1, cell.1), (cell.0 - 1, cell.1)),
        }
    }

    /// Marks a range of angles as blocked for shadow casting
    ///
    /// # Arguments
    /// * `prev_angle` - Starting angle of the shadow range
    /// * `next_angle` - Ending angle of the shadow range
    /// * `distance` - Distance at which the blocking occurs
    fn mark_shadow_range(&mut self, prev_angle: usize, next_angle: usize, distance: u8) {
        if prev_angle < next_angle {
            // Normal case: shadow range doesn't cross 0°/360° boundary
            for a in prev_angle..=next_angle {
                if a < ANGLES {
                    self.blocked_angles[a] = distance;
                }
            }
        } else {
            // Special case: shadow range crosses the 0°/360° boundary
            for a in 0..=prev_angle {
                self.blocked_angles[a] = distance;
            }
            for a in next_angle..ANGLES {
                self.blocked_angles[a] = distance;
            }
        }
    }

    /// Renders a single pixel of light onto the canvas
    ///
    /// # Arguments
    /// * `cell` - Local coordinates relative to the light center
    /// * `angle` - The angle of the ray (used for hue calculation)
    /// * `distance` - Distance from light source (used for brightness falloff)
    fn render_light_pixel(&mut self, cell: PtI, angle: usize, distance: u8) {
        // Transform local coordinates to canvas coordinates
        let c = (cell.0 + LIGHT_ROW as i16 / 2, cell.1 + LIGHT_ROW as i16 / 2);
        let cell_idx = c.0 as usize + c.1 as usize * LIGHT_ROW;

        // Calculate brightness falloff based on distance
        let falloff = 255 - (255 * distance as u16) / (self.r as u16);

        // Ensure we don't write outside the canvas bounds
        if cell_idx < self.canvas.len() {
            // Use angle for hue, full saturation, and distance-based brightness
            self.canvas[cell_idx] = hsv2rgb(angle as u8, 255, falloff as u8);
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

/// Updates an existing light or creates a new one with the specified parameters
///
/// This is the main entry point for the lighting system from WASM. It manages
/// the light storage and triggers recalculation when light properties change.
///
/// # Arguments
/// * `id` - Unique identifier for the light (0-255)
/// * `r` - Light radius/range
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
    // Attempt to get write access to the light map
    if let Ok(mut light_map) = LIGHT_MAP.write() {
        // Get existing light or create a new one
        let light = light_map.entry(id).or_insert_with(|| Light::new((x, y), r));

        // Update light properties
        light.pos = (x, y);
        light.r = r;

        // Recalculate and return the updated canvas
        light.update()
    } else {
        // Return null pointer if we can't acquire the lock
        std::ptr::null()
    }
}

/// Initializes the lighting system
///
/// This function must be called before any lighting calculations can be performed.
/// It forces the lazy initialization of the ray lookup tables, which is an expensive
/// one-time calculation that pre-computes all possible ray trajectories.
///
/// # Performance Note
/// The initialization involves computing ray trajectories for 60 distances × 360 angles,
/// which can take a noticeable amount of time on slower devices. Consider calling
/// this during a loading screen or startup phase.
pub fn init() {
    // Force initialization of the ray lookup table
    Lazy::force(&ALL_RAYS);

    // The LIGHT_MAP is already initialized via Lazy::new(), so no additional setup needed
}
