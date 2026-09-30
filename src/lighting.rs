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

use crate::collision::HybridCollisionMap;
use crate::engine::DEFAULT_ENGINE;
use crate::arctan;

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
/// mask pixel per visited cell) and [`crate::engine::LightingEngine::compute_fov`]
/// (which marks a visibility mask).
///
/// `visit` receives `(offset, angle, d, weight)`, where `offset` is the visited
/// cell's position relative to `pos`, `angle`/`d` identify the ray and `weight`
/// is the Object transmittance *before* the cell (deposit then attenuate: an
/// opaque cell is seen, not seen through). Cells in `pos`'s own tile never
/// occlude. World (cell) coords are `pos + offset`; distance caps at `max_dist`.
pub(crate) fn trace_visible_cells<F>(
    pos: PtI,
    collision: &HybridCollisionMap,
    rays: &RayTable,
    max_dist: usize,
    mut visit: F,
) where
    F: FnMut(PtI, usize, u8, f32),
{
    let mut blocked_angles = [255u8; ANGLES];

    let block = |blocked_angles: &mut [u8; ANGLES], angle: usize, d: usize| {
        blocked_angles[angle] = blocked_angles[angle].min(d as u8);
        if d < 3 {
            let left_angle = if angle > 0 { angle - 1 } else { ANGLES - 1 };
            let right_angle = (angle + 1) % ANGLES;
            blocked_angles[left_angle] = blocked_angles[left_angle].min(d as u8);
            blocked_angles[right_angle] = blocked_angles[right_angle].min(d as u8);
        }
    };

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
                    let (before, after) = if collision.room_blocked(pos.0, pos.1, curr.0, curr.1) {
                        (0.0, 0.0)
                    } else {
                        collision.transmittance(pos.0, pos.1, curr.0, curr.1)
                    };
                    if before == 0.0 {
                        block(&mut blocked_angles, angle, d);
                        break;
                    }

                    visit(*cell, angle, d as u8, before);
                    if after == 0.0 {
                        block(&mut blocked_angles, angle, d);
                    }
                }
            }
        }
    }
}

/// A single point light's per-instance transport state and mask output.
///
/// Per ADR-0010 the engine emits **transport only**: the canvas is a white
/// radial-falloff mask (alpha = attenuation). Colour/intensity are renderer
/// concerns. Owned by [`crate::engine::LightingEngine`].
pub struct Light {
    pos: PtI,
    r: i16,
    canvas: Vec<Color>,
    canvas_size: usize,
}

impl Light {
    pub(crate) fn new(pos: PtI, r: i16) -> Self {
        let canvas_size = (r * 2 + 1) as usize;
        let canvas_pixels = canvas_size * canvas_size;
        Light {
            pos,
            r,
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

    pub(crate) fn canvas(&self) -> &[Color] {
        &self.canvas
    }

    pub(crate) fn canvas_size(&self) -> usize {
        self.canvas_size
    }

    pub(crate) fn set_pos(&mut self, pos: PtI) {
        self.pos = pos;
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
        trace_visible_cells(pos, collision, rays, effective_max, |offset, _angle, d, weight| {
            self.render_light_pixel(offset, d, weight);
        });

        self.canvas.as_ptr()
    }

    fn render_light_pixel(&mut self, cell: PtI, distance: u8, weight: f32) {
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
            // Raw linear attenuation — the falloff *curve* is a renderer-side
            // option (`<lighting><falloff>`, ADR-0010): shaping happens there.
            self.canvas[cell_idx] = Color(255, 255, 255, (falloff as f32 * weight).round() as u8);
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

    /// Cell `(x, y)`; out-of-bounds reads as transparent.
    fn get(&self, x: i16, y: i16) -> Color {
        if x < 0 || y < 0 || x >= self.size as i16 || y >= self.size as i16 {
            return Color::default();
        }
        self.cells[x as usize + y as usize * self.size]
    }

    /// Write `color` to cell `(x, y)`; out-of-bounds writes are ignored.
    fn set(&mut self, x: i16, y: i16, color: Color) {
        if x < 0 || y < 0 || x >= self.size as i16 || y >= self.size as i16 {
            return;
        }
        self.cells[x as usize + y as usize * self.size] = color;
    }
}

/// A room-bounded flat ambient fill mask.
///
/// Unlike a [`Light`] (a point source with radial falloff), an `Ambient` has
/// no radius or falloff: every cell of a single same-type tile **Room** is
/// opaque white; everything else transparent. Colour is a renderer concern
/// (ADR-0010) — alpha is the in-room/out-of-room mask.
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

    /// Mark the `cells_per_tile²` block of cells belonging to tile
    /// `(tile_x, tile_y)` as in-room (opaque white).
    pub(crate) fn fill_tile(&mut self, tile_x: usize, tile_y: usize, cells_per_tile: usize) {
        let cx0 = tile_x * cells_per_tile;
        let cy0 = tile_y * cells_per_tile;
        for dy in 0..cells_per_tile {
            for dx in 0..cells_per_tile {
                self.canvas
                    .set((cx0 + dx) as i16, (cy0 + dy) as i16, Color(255, 255, 255, 255));
            }
        }
    }
}

/// A full-map **FOV canvas**.
///
/// Shaped like an [`Ambient`]'s output (full-map, `cells_per_row²` RGBA cells,
/// blitted at origin `(0,0)`) rather than a [`Light`]'s bounding square. Each
/// cell is white with alpha = the best Object transmittance any viewer's rays
/// reach it with (255 clear sight, partial through translucent Objects, 0
/// unseen) — no distance falloff. Per [ADR-0006](../../docs/adr/0006-fog-of-war-in-renderer.md) the
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

    /// Mark the cell at `(cx, cy)` (world cell coords) visible with `weight`
    /// (0…1). Max-merged, so unioning viewers is just repeated marking.
    /// Out-of-bounds coordinates are ignored.
    pub(crate) fn mark(&mut self, cx: i16, cy: i16, weight: f32) {
        let alpha = (weight * 255.0).round() as u8;
        if alpha > self.canvas.get(cx, cy).3 {
            self.canvas.set(cx, cy, Color(255, 255, 255, alpha));
        }
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
