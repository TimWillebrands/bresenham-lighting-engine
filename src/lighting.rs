//! Core lighting calculations and ray-casting.
//!
//! [`Light`] is the per-light renderer; the engine ([`crate::engine::LightingEngine`])
//! owns a registry of them, a [`crate::collision::HybridCollisionMap`] that they
//! consult during ray traversal, and the precomputed Bresenham ray table
//! ([`RayTable`]) they sample to walk those rays.
//!
//! Per [ADR-0008](../../docs/decisions/0008-per-engine-all-rays-and-runtime-resolution.md),
//! the ray table is per-engine — each engine builds its own at construction
//! time. The previous process-wide `ALL_RAYS` is gone.
//!
//! Free functions in this module are back-compat shims that operate on the
//! process-wide [`crate::engine::DEFAULT_ENGINE`]. New Rust code should
//! construct its own [`crate::engine::LightingEngine`] and call methods on it.

use std::collections::HashMap;

use crate::collision::{CollisionDetector, HybridCollisionMap};
use crate::engine::DEFAULT_ENGINE;
use crate::arctan;

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

/// Per-engine precomputed Bresenham ray table.
///
/// Keyed by `(distance, angle)`, each entry lists the cell offsets at that
/// distance/angle relative to a light at the origin. Built once at engine
/// construction time from the engine's `max_dist` (per ADR-0008).
pub type RayTable = HashMap<(usize, usize), Vec<PtI>>;

/// Build a Bresenham ray table for the given maximum ray length.
///
/// Used by [`crate::engine::LightingEngine::new`] to populate its per-instance
/// `all_rays` field.
pub(crate) fn build_ray_table(max_dist: usize) -> RayTable {
    let mut rays: RayTable = HashMap::new();

    let center = (0i16, 0i16);
    let radius = max_dist as i16;
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

                if angle >= ANGLES || distance >= max_dist {
                    continue;
                }

                rays.entry((distance, angle)).or_insert_with(Vec::new).push(pt);
            }
        }
    }

    rays
}

/// Walk the precomputed ray table outward from `pos`, invoking `visit` for
/// every cell a ray reaches before it is occluded by the [`HybridCollisionMap`]
/// (Room + Object collision). Shared by [`Light::update`] (which renders a
/// coloured pixel per visited cell) and [`crate::engine::LightingEngine::compute_fov`]
/// (which marks a binary visibility mask).
///
/// `visit` receives `(offset, angle, d)`, where `offset` is the visited cell's
/// position relative to `pos` and `angle`/`d` identify the ray. World (cell)
/// coords are just `pos + offset`. Distance is capped at `max_dist`; the same
/// occlusion rules as `Light::update` apply, minus colour and falloff.
pub(crate) fn trace_visible_cells<F>(
    pos: PtI,
    collision: &HybridCollisionMap,
    rays: &RayTable,
    max_dist: usize,
    mut visit: F,
) where
    F: FnMut(PtI, usize, u8),
{
    let mut blocked_angles = [255u8; ANGLES];

    for d in 0..max_dist {
        for angle in 0..ANGLES {
            if blocked_angles[angle] < d as u8 {
                continue;
            }

            if let Some(cells) = rays.get(&(d, angle)) {
                for cell in cells {
                    if d == 0 && angle % 90 != 0 {
                        continue;
                    }

                    let curr = (cell.0 + pos.0, cell.1 + pos.1);

                    // Full-ray occlusion check from the viewer origin to cell.
                    if collision.is_blocked(pos.0, pos.1, curr.0, curr.1) {
                        blocked_angles[angle] = d as u8;

                        if d < 3 {
                            let left_angle = if angle > 0 { angle - 1 } else { ANGLES - 1 };
                            let right_angle = (angle + 1) % ANGLES;

                            if blocked_angles[left_angle] > d as u8 {
                                blocked_angles[left_angle] = d as u8;
                            }
                            if blocked_angles[right_angle] > d as u8 {
                                blocked_angles[right_angle] = d as u8;
                            }
                        }

                        break;
                    }

                    visit(*cell, angle, d as u8);
                }
            }
        }
    }
}

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

    /// Recalculate this light's canvas, consulting `collision` for occlusion
    /// and `rays` for precomputed Bresenham geometry. `max_dist` caps the
    /// effective light radius for this pass.
    pub(crate) fn update(
        &mut self,
        collision: &HybridCollisionMap,
        rays: &RayTable,
        max_dist: usize,
    ) -> *const Color {
        let new_canvas_size = (self.r * 2 + 1) as usize;
        let new_canvas_pixels = new_canvas_size * new_canvas_size;
        if self.canvas.len() != new_canvas_pixels {
            self.canvas = vec![Color::default(); new_canvas_pixels];
            self.canvas_size = new_canvas_size;
        }

        self.canvas.iter_mut().for_each(|p| *p = Color::default());

        let pos = self.pos;
        let effective_max = (self.r as usize).min(max_dist);
        trace_visible_cells(pos, collision, rays, effective_max, |offset, angle, d| {
            self.render_light_pixel(offset, angle, d);
        });

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

/// A full-map RGBA canvas: `size²` cells in row-major order, blitted at origin
/// `(0,0)` by the JS compositor. Shared storage behind the engine's full-map
/// effects ([`Ambient`], [`Fov`]); each wraps one and adds its own write
/// primitive. The persistent allocation keeps the pointer handed to JS valid
/// between frames.
struct FullMapCanvas {
    cells: Vec<Color>,
    size: usize,
}

impl FullMapCanvas {
    /// Allocate a fully-transparent `size²` canvas.
    fn new(size: usize) -> Self {
        FullMapCanvas {
            cells: vec![Color::default(); size * size],
            size,
        }
    }

    fn cells(&self) -> &[Color] {
        &self.cells
    }

    /// Reset every cell to transparent.
    fn clear(&mut self) {
        self.cells.iter_mut().for_each(|p| *p = Color::default());
    }

    /// Write `color` to cell `(x, y)`; out-of-bounds writes are ignored.
    fn set(&mut self, x: i16, y: i16, color: Color) {
        if x < 0 || y < 0 || x >= self.size as i16 || y >= self.size as i16 {
            return;
        }
        self.cells[x as usize + y as usize * self.size] = color;
    }
}

/// A room-bounded flat ambient fill.
///
/// Unlike a [`Light`] (a point source with radial falloff), an `Ambient` has
/// no radius, intensity, or falloff: every cell of a single same-type tile
/// **Room** is filled with one flat RGB colour. Alpha is the in-room/out-of-room
/// mask (`255` inside the room, `0` everywhere else).
///
/// Owned by [`crate::engine::LightingEngine`], which floods it via
/// `update_or_add_ambient`.
pub struct Ambient {
    canvas: FullMapCanvas,
}

impl Ambient {
    /// Allocate a transparent full-map canvas of `canvas_size²` cells.
    pub(crate) fn new(canvas_size: usize) -> Self {
        Ambient {
            canvas: FullMapCanvas::new(canvas_size),
        }
    }

    pub(crate) fn canvas(&self) -> &[Color] {
        self.canvas.cells()
    }

    /// Reset every cell to transparent.
    pub(crate) fn clear(&mut self) {
        self.canvas.clear();
    }

    /// Fill the `cells_per_tile²` block of cells belonging to tile
    /// `(tile_x, tile_y)` with `color`.
    pub(crate) fn fill_tile(
        &mut self,
        tile_x: usize,
        tile_y: usize,
        cells_per_tile: usize,
        color: Color,
    ) {
        let cx0 = tile_x * cells_per_tile;
        let cy0 = tile_y * cells_per_tile;
        for dy in 0..cells_per_tile {
            for dx in 0..cells_per_tile {
                self.canvas.set((cx0 + dx) as i16, (cy0 + dy) as i16, color);
            }
        }
    }
}

/// A full-map binary **FOV canvas**.
///
/// Shaped like an [`Ambient`]'s output (full-map, `cells_per_row²` RGBA cells,
/// blitted at origin `(0,0)`) rather than a [`Light`]'s bounding square. Each
/// cell is either opaque white `(255, 255, 255, 255)` where some viewer's rays
/// reach it, or fully transparent `(0, 0, 0, 0)` where none do — binary alpha,
/// no falloff. Per [ADR-0006](../../docs/adr/0006-fog-of-war-in-renderer.md) the
/// engine holds no explored/fog state; this is the live mask only, recomputed
/// from scratch on every [`crate::engine::LightingEngine::compute_fov`] call.
pub struct Fov {
    canvas: FullMapCanvas,
}

impl Fov {
    /// Allocate a fully-transparent full-map canvas of `canvas_size²` cells.
    pub(crate) fn new(canvas_size: usize) -> Self {
        Fov {
            canvas: FullMapCanvas::new(canvas_size),
        }
    }

    pub(crate) fn canvas(&self) -> &[Color] {
        self.canvas.cells()
    }

    /// Reset every cell to transparent.
    pub(crate) fn clear(&mut self) {
        self.canvas.clear();
    }

    /// Mark the cell at `(cx, cy)` (world cell coords) as visible — opaque
    /// white. Out-of-bounds coordinates are ignored. Idempotent, so unioning
    /// multiple viewers is just repeated marking.
    pub(crate) fn mark(&mut self, cx: i16, cy: i16) {
        self.canvas.set(cx, cy, Color(255, 255, 255, 255));
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

/// Force initialization of the default engine (which builds its own ray
/// geometry cache during construction). Cheap to call repeatedly.
pub fn init() {
    once_cell::sync::Lazy::force(&DEFAULT_ENGINE);
}
