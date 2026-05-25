//! Core lighting calculations and ray-casting.
//!
//! [`Light`] is the per-light renderer; the engine ([`crate::engine::LightingEngine`])
//! owns a registry of them and a [`crate::collision::HybridCollisionMap`] that they
//! consult during ray traversal.
//!
//! The precomputed Bresenham ray table [`ALL_RAYS`] is a pure function of the
//! `MAX_DIST` and `ANGLES` compile-time constants and is shared across all engines.
//!
//! Free functions in this module are back-compat shims that operate on the
//! process-wide [`crate::engine::DEFAULT_ENGINE`]. New Rust code should
//! construct its own [`crate::engine::LightingEngine`] and call methods on it.

use once_cell::sync::Lazy;
use std::collections::HashMap;

use crate::collision::{CollisionDetector, HybridCollisionMap};
use crate::engine::DEFAULT_ENGINE;
use crate::{arctan, ray};

/// Color mode configuration for light sources.
#[derive(Clone, Debug, PartialEq)]
pub enum ColorMode {
    /// Solid color using specified hue (0-255).
    Solid(u8),
    /// Custom HSV color with specified hue and saturation.
    Custom { hue: u8, saturation: u8 },
}

/// Maximum ray distance from a light's centre, in cells.
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) const MAX_DIST: usize = 10;
#[cfg(not(all(test, not(target_arch = "wasm32"))))]
pub(crate) const MAX_DIST: usize = 60;

/// Number of discrete ray angles per light (full revolution).
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) const ANGLES: usize = 36;
#[cfg(not(all(test, not(target_arch = "wasm32"))))]
pub(crate) const ANGLES: usize = 360;

/// Accessor for [`MAX_DIST`], usable from other modules without `pub` exposure.
pub fn max_dist() -> usize {
    MAX_DIST
}

/// Accessor for [`ANGLES`], usable from other modules without `pub` exposure.
pub fn angles() -> usize {
    ANGLES
}

type PtI = (i16, i16);

/// RGBA color (matches HTML5 Canvas `ImageData` byte layout).
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

/// Process-wide precomputed Bresenham ray table.
///
/// Keyed by `(distance, angle)`, each entry lists the cell offsets at that
/// distance/angle relative to a light at the origin. Because this is a pure
/// function of the compile-time constants `MAX_DIST` and `ANGLES`, it is
/// safely shared across every [`crate::engine::LightingEngine`].
pub(crate) static ALL_RAYS: Lazy<HashMap<(usize, usize), Vec<PtI>>> = Lazy::new(|| {
    let mut rays: HashMap<(usize, usize), Vec<PtI>> = HashMap::new();

    let center = (0, 0);
    let radius = MAX_DIST as i16;
    let top = center.1 - radius;
    let bottom = center.1 + radius;
    let left = center.0 - radius;
    let right = center.0 + radius;

    for y in top..=bottom {
        for x in left..=right {
            let pt = (x, y);
            let dist = arctan::distance(pt);

            if dist <= radius as u16 {
                let raw_angle = arctan::rad_to_deg(arctan::atan2_int(y as i32, x as i32));
                let angle = (raw_angle as usize) % ANGLES;
                let distance = dist as usize;

                if angle >= ANGLES || distance >= MAX_DIST {
                    continue;
                }

                rays.entry((distance, angle)).or_insert_with(Vec::new).push(pt);
            }
        }
    }

    rays
});

/// A single point light's per-instance state and render output.
///
/// Owned by [`crate::engine::LightingEngine`]; not constructed directly by
/// callers.
pub struct Light {
    pos: PtI,
    r: i16,
    color_mode: Option<ColorMode>,
    canvas: Vec<Color>,
    canvas_size: usize,
    blocked_angles: [u8; ANGLES],
}

impl Light {
    pub(crate) fn new(pos: PtI, r: i16, color_mode: Option<ColorMode>) -> Self {
        let canvas_size = (r * 2 + 1) as usize;
        let canvas_pixels = canvas_size * canvas_size;
        Light {
            pos,
            r,
            color_mode,
            canvas: vec![Color::default(); canvas_pixels],
            canvas_size,
            blocked_angles: [255; ANGLES],
        }
    }

    pub(crate) fn pos(&self) -> PtI {
        self.pos
    }

    pub(crate) fn radius(&self) -> i16 {
        self.r
    }

    pub(crate) fn color_mode(&self) -> &Option<ColorMode> {
        &self.color_mode
    }

    pub(crate) fn canvas(&self) -> &[Color] {
        &self.canvas
    }

    pub(crate) fn canvas_size(&self) -> usize {
        self.canvas_size
    }

    pub(crate) fn set_state(&mut self, pos: PtI, r: i16, color_mode: Option<ColorMode>) {
        self.pos = pos;
        self.r = r;
        self.color_mode = color_mode;
    }

    /// Recalculate this light's canvas, consulting `collision` for occlusion.
    pub(crate) fn update(&mut self, collision: &HybridCollisionMap) -> *const Color {
        let new_canvas_size = (self.r * 2 + 1) as usize;
        let new_canvas_pixels = new_canvas_size * new_canvas_size;
        if self.canvas.len() != new_canvas_pixels {
            self.canvas = vec![Color::default(); new_canvas_pixels];
            self.canvas_size = new_canvas_size;
        }

        self.blocked_angles.fill(255);
        self.canvas.iter_mut().for_each(|p| *p = Color::default());

        for d in 0..self.r as usize {
            if d >= MAX_DIST {
                break;
            }

            for angle in 0..ANGLES {
                if self.blocked_angles[angle] < d as u8 {
                    continue;
                }

                if let Some(cells) = ALL_RAYS.get(&(d, angle)) {
                    for cell in cells {
                        if d == 0 && angle % 90 != 0 {
                            continue;
                        }

                        let curr = (cell.0 + self.pos.0, cell.1 + self.pos.1);
                        let _prev = ray::step(curr, self.pos);

                        // Full-ray occlusion check from light origin to cell.
                        if collision.is_blocked(self.pos.0, self.pos.1, curr.0, curr.1) {
                            self.blocked_angles[angle] = d as u8;

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

                            break;
                        }

                        self.render_light_pixel(*cell, angle, d as u8);
                    }
                }
            }
        }

        self.canvas.as_ptr()
    }

    fn render_light_pixel(&mut self, cell: PtI, angle: usize, distance: u8) {
        let c = (
            cell.0 + self.canvas_size as i16 / 2,
            cell.1 + self.canvas_size as i16 / 2,
        );

        if c.0 < 0 || c.1 < 0 || c.0 >= self.canvas_size as i16 || c.1 >= self.canvas_size as i16 {
            return;
        }

        let cell_idx = c.0 as usize + c.1 as usize * self.canvas_size;
        let falloff = 255 - (255 * distance as u16) / (self.r as u16);

        if cell_idx < self.canvas.len() {
            let color = match &self.color_mode {
                None => {
                    let scaled_hue = (angle * 255) / (ANGLES - 1);
                    hsv2rgb(scaled_hue as u8, 255, falloff as u8)
                }
                Some(ColorMode::Solid(hue)) => hsv2rgb(*hue, 255, falloff as u8),
                Some(ColorMode::Custom { hue, saturation }) => {
                    hsv2rgb(*hue, *saturation, falloff as u8)
                }
            };

            self.canvas[cell_idx] = color;
        }
    }
}

/// HSV-to-RGB conversion. Alpha is always 255.
pub(crate) fn hsv2rgb(h: u8, s: u8, v: u8) -> Color {
    if s == 0 {
        return Color(v, v, v, 255);
    }

    let sector = h / 43;
    let remainder = (h - (sector * 43)) * 6;

    let p = (v as u16 * (255 - s) as u16 / 255) as u8;
    let q = (v as u16 * (255 - (s as u16 * remainder as u16 / 255)) / 255) as u8;
    let t = (v as u16 * (255 - (s as u16 * (255 - remainder) as u16 / 255)) / 255) as u8;

    match sector {
        0 => Color(v, t, p, 255),
        1 => Color(q, v, p, 255),
        2 => Color(p, v, t, 255),
        3 => Color(p, q, v, 255),
        4 => Color(t, p, v, 255),
        _ => Color(v, p, q, 255),
    }
}

// ------------------------------- shims ----------------------------------

/// WASM/back-compat shim. Forwards to [`crate::engine::DEFAULT_ENGINE`].
pub fn update_or_add_light(id: u8, r: i16, x: i16, y: i16) -> *const Color {
    DEFAULT_ENGINE
        .write()
        .map(|mut e| e.update_or_add_light(id, r, x, y))
        .unwrap_or(std::ptr::null())
}

/// WASM/back-compat shim. Forwards to [`crate::engine::DEFAULT_ENGINE`].
pub fn update_or_add_light_with_solid_color(
    id: u8,
    r: i16,
    x: i16,
    y: i16,
    hue: u8,
) -> *const Color {
    DEFAULT_ENGINE
        .write()
        .map(|mut e| e.update_or_add_light_with_solid_color(id, r, x, y, hue))
        .unwrap_or(std::ptr::null())
}

/// WASM/back-compat shim. Forwards to [`crate::engine::DEFAULT_ENGINE`].
pub fn update_or_add_light_with_custom_color(
    id: u8,
    r: i16,
    x: i16,
    y: i16,
    hue: u8,
    saturation: u8,
) -> *const Color {
    DEFAULT_ENGINE
        .write()
        .map(|mut e| e.update_or_add_light_with_custom_color(id, r, x, y, hue, saturation))
        .unwrap_or(std::ptr::null())
}

/// WASM/back-compat shim. Forwards to [`crate::engine::DEFAULT_ENGINE`].
pub fn update_collision_map(map_data: Vec<i32>, map_size: usize) {
    if let Ok(mut e) = DEFAULT_ENGINE.write() {
        e.update_map_data(map_data, map_size);
    }
}

/// Force initialization of process-wide ray geometry caches. Cheap to call
/// repeatedly; no-op after the first invocation.
pub fn init() {
    Lazy::force(&ALL_RAYS);
}
