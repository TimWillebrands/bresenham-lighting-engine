//! [`LightingEngine`] — owned instance of all per-scenario mutable state.
//!
//! Per [ADR-0007](../../docs/decisions/0007-extract-lighting-engine-type.md), all
//! mutable runtime state used to live in process-wide statics. That made parallel
//! tests racy and prevented embedders from holding more than one scene at a time.
//!
//! A [`LightingEngine`] owns:
//!
//! - the tile map (`Vec<u8>`)
//! - the derived cell block map (`Vec<CellDetails>`)
//! - the collision system ([`HybridCollisionMap`] — rooms + objects)
//! - the registry of active [`Light`]s
//!
//! Process-wide caches that are pure functions of compile-time constants — most
//! notably the precomputed Bresenham ray table `ALL_RAYS` in [`crate::lighting`]
//! — stay shared across all engines.
//!
//! # Back-compat shim
//!
//! A default singleton [`DEFAULT_ENGINE`] backs every `#[wasm_bindgen]` free
//! function exposed to JavaScript. New Rust callers (tests, examples, embedders)
//! should construct their own instance with [`LightingEngine::new`] and call
//! methods on it directly — that's what makes parallel test execution safe.

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use once_cell::sync::Lazy;

use crate::block_map::{compute_cell_details_for_tile, CellDetails};
use crate::collision::HybridCollisionMap;
use crate::lighting::{
    build_ray_table, trace_visible_cells, Ambient, Color, Fov, Light, RayTable,
};
use crate::map_grid::UnionFind;

/// Default cell-grid subdivision per tile, used by [`LightingEngine::default`]
/// and the WASM back-compat [`DEFAULT_ENGINE`].
pub const DEFAULT_CELLS_PER_TILE: usize = 6;

/// Default tile-grid side length, used by [`LightingEngine::default`] and the
/// WASM back-compat [`DEFAULT_ENGINE`].
pub const DEFAULT_TILES_PER_ROW: usize = 30;

/// Owned instance of the lighting engine's mutable runtime state.
///
/// Construct one per scenario. Multiple instances coexist freely — they share
/// only immutable, process-wide ray geometry caches.
pub struct LightingEngine {
    cells_per_tile: usize,
    tiles_per_row: usize,
    max_dist: usize,
    all_rays: RayTable,
    tiles: Vec<u8>,
    cells: Vec<CellDetails>,
    collision: HybridCollisionMap,
    lights: HashMap<u8, Light>,
    /// Registry of active room-bounded ambient emitters, parallel to `lights`.
    /// Each entry owns a full-map canvas flooded by `update_or_add_ambient`.
    ambients: HashMap<u8, Ambient>,
    /// Lazily-allocated full-map FOV canvas, reused across `compute_fov` calls
    /// so the pointer handed to JS stays valid between frames. The engine holds
    /// no fog/explored memory (ADR-0006) — this is the live mask only.
    fov: Option<Fov>,
    /// Edge overrides as canonical `(lo, hi)` tile-index pairs → permission
    /// mask (`EDGE_LIGHT`/`EDGE_MOVE`). An entry overrides the room graph on
    /// its edge: set bits grant crossing (open door = both, window = light),
    /// cleared bits deny even within a room (closed door = 0). Absence =
    /// room membership decides. See ADR-0003.
    edges: HashMap<(usize, usize), u8>,
    /// Tile-resolution room graph, kept in sync with `tiles`. Edge masks
    /// never enter it (a union would dissolve the whole boundary); they are
    /// consulted per-edge in `neighbours` / `cast_ray`.
    tile_uf: UnionFind,
    /// Solid tiles (ADR-0016): tile index → authored opacity + collides.
    /// Opacity is also spread over the tile's cells in the Object map.
    solids: HashMap<usize, Solid>,
}

/// A solid tile as authored: `opacity` 0 clear … 255 opaque per crossing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Solid {
    pub opacity: u8,
    pub collides: bool,
}

/// `cast_ray` passes while the product of crossed tiles' transparency is at
/// least this (translucent smoke never reaches 0).
pub const LOS_MIN: f32 = 0.1;

/// Edge-permission bit: light rays and line-of-sight cross this edge.
pub const EDGE_LIGHT: u8 = 1;
/// Edge-permission bit: pathfinding (`path`/`neighbours`) crosses this edge.
pub const EDGE_MOVE: u8 = 2;

impl Default for LightingEngine {
    fn default() -> Self {
        Self::new(DEFAULT_CELLS_PER_TILE, DEFAULT_TILES_PER_ROW)
    }
}

impl LightingEngine {
    /// Construct an engine with empty tile map (all tiles type 0, one big
    /// room, no objects, no lights) at the given resolution.
    ///
    /// `cells_per_tile` is the cell-grid subdivision per tile (collision /
    /// lighting resolution); `tiles_per_row` is the world's tile-grid side.
    pub fn new(cells_per_tile: usize, tiles_per_row: usize) -> Self {
        assert!(cells_per_tile > 0, "cells_per_tile must be > 0");
        assert!(tiles_per_row > 0, "tiles_per_row must be > 0");
        let tiles_total = tiles_per_row * tiles_per_row;
        let cells_per_row = cells_per_tile * tiles_per_row;
        let cells_total = cells_per_row * cells_per_row;
        let tiles = vec![0u8; tiles_total];
        let cells = vec![CellDetails::default(); cells_total];
        // Default: one big room covering all cells, no walls, no objects.
        let map_data = vec![1i32; cells_per_row * cells_per_row];
        let collision = HybridCollisionMap::new(map_data, cells_per_row, cells_per_tile);
        let max_dist = crate::lighting::max_dist();
        let all_rays = build_ray_table(max_dist);
        let tile_uf = UnionFind::new(vec![0i32; tiles_total], tiles_per_row);
        Self {
            cells_per_tile,
            tiles_per_row,
            max_dist,
            all_rays,
            tiles,
            cells,
            collision,
            lights: HashMap::new(),
            ambients: HashMap::new(),
            fov: None,
            edges: HashMap::new(),
            tile_uf,
            solids: HashMap::new(),
        }
    }

    /// Cell-grid subdivision per tile (was the module-level `CELLS_PER_TILE`
    /// constant; now per-instance per ADR-0008).
    pub fn cells_per_tile(&self) -> usize {
        self.cells_per_tile
    }

    /// World's tile-grid side length (was the module-level `TILES_PER_ROW`).
    pub fn tiles_per_row(&self) -> usize {
        self.tiles_per_row
    }

    /// Cell-grid side length (`cells_per_tile * tiles_per_row`).
    pub fn cells_per_row(&self) -> usize {
        self.cells_per_tile * self.tiles_per_row
    }

    /// Read-only view of the tile-type array (row-major, `TILES_TOTAL` long).
    pub fn tiles(&self) -> &[u8] {
        &self.tiles
    }

    /// Read-only view of the derived block-map cell array
    /// (row-major, `CELLS_TOTAL` long).
    pub fn block_map(&self) -> &[CellDetails] {
        &self.cells
    }

    /// Reference to the underlying collision system. Useful for diagnostics.
    pub fn collision(&self) -> &HybridCollisionMap {
        &self.collision
    }

    /// Check whether the segment `(x0,y0)→(x1,y1)` (cell coords) is blocked by
    /// a wall or object.
    pub fn is_blocked(&self, x0: i16, y0: i16, x1: i16, y1: i16) -> bool {
        use crate::collision::CollisionDetector;
        self.collision.is_blocked(x0, y0, x1, y1)
    }

    /// Set a single tile type. Out-of-range coordinates are ignored.
    /// Recomputes the affected cell edge flags and refreshes the room
    /// union-find from the new tile map.
    pub fn set_tile(&mut self, x: u32, y: u32, tile: u8) {
        let tiles_per_row = self.tiles_per_row;
        let index = (x as usize) + (y as usize * tiles_per_row);
        if index >= self.tiles.len() {
            return;
        }
        self.tiles[index] = tile;
        self.recompute_block_map();
        self.refresh_collision_from_tiles();
        self.refresh_tile_uf_from_tiles();
    }

    /// Overwrite the entire tile map. Length must match `tiles_per_row²`;
    /// otherwise the call is a no-op.
    pub fn set_tile_map(&mut self, tiles: Vec<u8>) {
        if tiles.len() != self.tiles.len() {
            return;
        }
        self.tiles = tiles;
        self.recompute_block_map();
        self.refresh_collision_from_tiles();
        self.refresh_tile_uf_from_tiles();
    }

    /// Replace the room map data of the broad-phase collision detector.
    /// Used by external callers (e.g. WASM) that want to push a precomputed
    /// room layout without going through the tile setter.
    pub fn update_map_data(&mut self, map_data: Vec<i32>, map_size: usize) {
        self.collision.update_map_data(map_data, map_size);
    }

    /// Mark a single cell as blocking (an Object cell) or not.
    pub fn set_pixel(&mut self, x: u16, y: u16, blocked: bool) {
        self.collision.pixel_map_mut().set_pixel(x, y, blocked);
    }

    /// Batched form of [`set_pixel`].
    pub fn set_pixel_batch<I>(&mut self, pixels: I)
    where
        I: IntoIterator<Item = (u16, u16, bool)>,
    {
        self.collision.pixel_map_mut().set_pixel_batch(pixels);
    }

    /// Record a permission override for the edge between two tiles (mask 0
    /// = fully sealed, even within a room). Order-insensitive; only the
    /// cell-edge overlay is republished — masks never touch the room graph.
    pub fn set_edge(&mut self, t1_idx: usize, t2_idx: usize, mask: u8) {
        let pair = canonical_edge(t1_idx, t2_idx);
        if self.edges.insert(pair, mask) != Some(mask) {
            self.publish_edge_cell_overrides();
        }
    }

    /// Replace the whole edge set from a flat `[t1, t2, mask, …]` array in
    /// one call (single overlay republish). JS replays edges from scratch on
    /// every token snapshot, so replace-semantics need no separate clear.
    pub fn replace_edges(&mut self, flat: &[u32]) {
        self.edges.clear();
        for chunk in flat.chunks_exact(3) {
            self.edges
                .insert(canonical_edge(chunk[0] as usize, chunk[1] as usize), chunk[2] as u8);
        }
        self.publish_edge_cell_overrides();
    }

    /// Recorded override between two tiles: `Some(0)` = sealed, `None` = no
    /// override (room membership decides). Order-insensitive.
    pub fn edge_mask(&self, t1_idx: usize, t2_idx: usize) -> Option<u8> {
        self.edges.get(&canonical_edge(t1_idx, t2_idx)).copied()
    }

    /// Replace the whole solid set from a flat `[tile, opacity255, collides,
    /// …]` array. Each tile's cells get `t^(1/cells_per_tile)` so one straight
    /// crossing attenuates by exactly the authored transparency `t`.
    pub fn replace_solids(&mut self, flat: &[u32]) {
        let total = self.tiles_per_row * self.tiles_per_row;
        let mut next: HashMap<usize, Solid> = HashMap::new();
        for chunk in flat.chunks_exact(3) {
            let tile = chunk[0] as usize;
            if tile >= total {
                continue;
            }
            next.insert(tile, Solid { opacity: chunk[1].min(255) as u8, collides: chunk[2] != 0 });
        }
        let stale: Vec<usize> = self.solids.keys().filter(|t| !next.contains_key(t)).copied().collect();
        for tile in stale {
            self.fill_tile_opacity(tile, 0);
        }
        for (&tile, solid) in &next {
            let t = 1.0 - solid.opacity as f64 / 255.0;
            let cell_t = t.powf(1.0 / self.cells_per_tile as f64);
            let cell_opacity = (255.0 * (1.0 - cell_t)).round() as u8;
            self.fill_tile_opacity(tile, cell_opacity);
        }
        self.solids = next;
    }

    /// Recorded solid on a tile, if any.
    pub fn solid_at(&self, tile_idx: usize) -> Option<Solid> {
        self.solids.get(&tile_idx).copied()
    }

    fn fill_tile_opacity(&mut self, tile: usize, opacity: u8) {
        let cpt = self.cells_per_tile;
        let (cx0, cy0) = ((tile % self.tiles_per_row) * cpt, (tile / self.tiles_per_row) * cpt);
        let map = self.collision.pixel_map_mut();
        for dy in 0..cpt {
            for dx in 0..cpt {
                map.set_opacity((cx0 + dx) as u16, (cy0 + dy) as u16, opacity);
            }
        }
    }

    fn collides(&self, tile_idx: usize) -> bool {
        self.solids.get(&tile_idx).is_some_and(|s| s.collides)
    }

    /// Clear all object cells and solids (does not touch the tile map).
    pub fn clear_pixel_collisions(&mut self) {
        use crate::collision::CollisionDetector;
        self.collision.clear();
        self.solids.clear();
    }

    /// Create or update a light and return a pointer to its transport mask —
    /// a white radial-falloff canvas (alpha = attenuation, ADR-0010). Rust
    /// callers should prefer [`Self::light_canvas`] after this call.
    pub fn update_or_add_light(&mut self, id: u8, r: i16, x: i16, y: i16) -> *const Color {
        let clamped_r = r.min(self.max_dist as i16).max(1);

        // Disjoint borrows: `lights` mutably, `collision`+`all_rays` immutably.
        let collision = &self.collision;
        let all_rays = &self.all_rays;
        let max_dist = self.max_dist;
        let light = self
            .lights
            .entry(id)
            .or_insert_with(|| Light::new((x, y), clamped_r));
        if light.radius() != clamped_r {
            // Radius change resizes the canvas — rebuild.
            *light = Light::new((x, y), clamped_r);
        } else {
            light.set_pos((x, y));
        }
        light.update(collision, all_rays, max_dist)
    }

    /// Create or update a room-bounded ambient emitter and return a pointer to
    /// its full-map mask (`cells_per_row²` RGBA cells in wasm linear memory).
    ///
    /// The emitter floods the same-type [`UnionFind`] room of the tile at
    /// `(tile_x, tile_y)` — every cell of every tile sharing that tile's room
    /// is opaque white; everything else stays transparent `(0, 0, 0, 0)`.
    /// Colour is applied renderer-side (ADR-0010). Because the room is the
    /// `tile_uf` partition (which is door-agnostic, per ADR-0003), the fill
    /// never crosses a Door, open or closed. An emitter on a non-floor tile
    /// (`tile <= 0`) or out of range emits an empty (fully transparent) canvas.
    pub fn update_or_add_ambient(&mut self, id: u8, tile_x: i16, tile_y: i16) -> *const Color {
        let tiles_per_row = self.tiles_per_row;
        let cells_per_tile = self.cells_per_tile;
        let cells_per_row = self.cells_per_row();

        // Resolve the emitter tile's room first (needs `&mut tile_uf` for
        // find()), then collect every tile in that room. Done before borrowing
        // `ambients` so the two mutable borrows of `self` don't overlap.
        let in_range = tile_x >= 0
            && tile_y >= 0
            && (tile_x as usize) < tiles_per_row
            && (tile_y as usize) < tiles_per_row;
        let mut room_tiles: Vec<usize> = Vec::new();
        if in_range {
            let index = (tile_x as usize) + (tile_y as usize) * tiles_per_row;
            if self.tile_at(index) > 0 {
                let room = self.tile_uf.find(index);
                for i in 0..tiles_per_row * tiles_per_row {
                    if self.tile_at(i) > 0 && self.tile_uf.find(i) == room {
                        room_tiles.push(i);
                    }
                }
            }
        }

        let ambient = self
            .ambients
            .entry(id)
            .or_insert_with(|| Ambient::new(cells_per_row));
        ambient.clear();
        for &ti in &room_tiles {
            ambient.fill_tile(ti % tiles_per_row, ti / tiles_per_row, cells_per_tile);
        }
        ambient.canvas().as_ptr()
    }

    /// Borrow an ambient emitter's full-map canvas if one with the given id
    /// exists.
    pub fn ambient_canvas(&self, id: u8) -> Option<&[Color]> {
        self.ambients.get(&id).map(|a| a.canvas())
    }

    /// Compute the live field-of-view mask for a set of viewer points and
    /// return a pointer to the resulting full-map **FOV canvas**
    /// (`cells_per_row²` RGBA cells in wasm linear memory).
    ///
    /// `viewers` is a flat array of viewer positions in cell coords
    /// `[x0, y0, x1, y1, …]`; a trailing odd element (if any) is ignored. Each
    /// viewer casts rays out to the ray table's max distance through the same
    /// Room + Object collision as [`Light::update`] (minus falloff); every
    /// cell a ray reaches is marked white with alpha = its Object
    /// transmittance, everything else stays transparent `(0, 0, 0, 0)`.
    /// Results max-merge across viewers. An empty `viewers` array yields a
    /// fully-transparent canvas.
    ///
    /// Pure compute: the engine stores no explored/fog memory (ADR-0006). The
    /// returned canvas is overwritten on the next call.
    pub fn compute_fov(&mut self, viewers: &[i16]) -> *const Color {
        let cells_per_row = self.cells_per_row();
        let max_dist = self.max_dist;

        // Disjoint field borrows: `collision` + `all_rays` immutably, `fov`
        // mutably. Bind each field directly so the borrow checker sees them as
        // non-overlapping.
        let collision = &self.collision;
        let rays = &self.all_rays;
        let fov = self.fov.get_or_insert_with(|| Fov::new(cells_per_row));

        fov.clear();
        for pair in viewers.chunks_exact(2) {
            let pos = (pair[0], pair[1]);
            trace_visible_cells(pos, collision, rays, max_dist, |offset, _angle, _d, weight| {
                fov.mark(pos.0 + offset.0, pos.1 + offset.1, weight);
            });
        }
        fov.canvas().as_ptr()
    }

    /// Borrow the most recently computed FOV canvas, if [`Self::compute_fov`]
    /// has been called at least once.
    pub fn fov_canvas(&self) -> Option<&[Color]> {
        self.fov.as_ref().map(|f| f.canvas())
    }

    /// Borrow a light's canvas if one with the given id exists.
    pub fn light_canvas(&self, id: u8) -> Option<&[Color]> {
        self.lights.get(&id).map(|l| l.canvas())
    }

    /// Side length (in cells) of a light's square canvas.
    pub fn light_canvas_size(&self, id: u8) -> Option<usize> {
        self.lights.get(&id).map(|l| l.canvas_size())
    }

    /// Light position in cell coords.
    pub fn light_position(&self, id: u8) -> Option<(i16, i16)> {
        self.lights.get(&id).map(|l| l.pos())
    }

    /// Light radius in cells.
    pub fn light_radius(&self, id: u8) -> Option<i16> {
        self.lights.get(&id).map(|l| l.radius())
    }

    fn recompute_block_map(&mut self) {
        let tiles_total = self.tiles.len();
        for tile_index in 0..tiles_total {
            compute_cell_details_for_tile(
                tile_index,
                &self.tiles,
                &mut self.cells,
                self.cells_per_tile,
                self.tiles_per_row,
            );
        }
    }

    fn refresh_collision_from_tiles(&mut self) {
        // The broad-phase room graph is queried in **cell coordinates** by
        // [`crate::lighting::Light::update`] (ray walks are at cell resolution),
        // so the union-find must be built at cell resolution as well. Expand
        // the tile map: every cell inherits the type of its containing tile.
        // Two cells share a room iff their tiles share a type AND are
        // 4-connected via same-type tiles — the union-find's adjacency union
        // takes care of that automatically.
        let cells_per_tile = self.cells_per_tile;
        let tiles_per_row = self.tiles_per_row;
        let cells_per_row = cells_per_tile * tiles_per_row;
        let mut cell_map = vec![0i32; cells_per_row * cells_per_row];
        for tile_y in 0..tiles_per_row {
            for tile_x in 0..tiles_per_row {
                let tile = self.tiles[tile_y * tiles_per_row + tile_x] as i32;
                let cy0 = tile_y * cells_per_tile;
                let cx0 = tile_x * cells_per_tile;
                for dy in 0..cells_per_tile {
                    let row = (cy0 + dy) * cells_per_row;
                    for dx in 0..cells_per_tile {
                        cell_map[row + cx0 + dx] = tile;
                    }
                }
            }
        }
        self.collision.update_map_data(cell_map, cells_per_row);
        self.publish_edge_cell_overrides();
    }

    /// Rebuild the tile-resolution room graph from the current tile map.
    /// Doors are *not* unioned into the UF — a UF union closes transitively
    /// and would dissolve the entire room boundary the moment one door
    /// opened. Door connectivity is consulted edge-by-edge in `neighbours`
    /// / `cast_ray` instead.
    fn refresh_tile_uf_from_tiles(&mut self) {
        let tiles: Vec<i32> = self.tiles.iter().map(|&t| t as i32).collect();
        self.tile_uf = UnionFind::new(tiles, self.tiles_per_row);
    }

    /// Publish the edge overrides into the collision detector's cell-edge
    /// overlay: `EDGE_LIGHT` edges into the pass set (holes in a room
    /// boundary), lightless edges into the block set (seals within a room).
    /// Each tile-edge becomes `cells_per_tile` cell-pair entries along the
    /// shared boundary.
    fn publish_edge_cell_overrides(&mut self) {
        let cells_per_tile = self.cells_per_tile;
        let tiles_per_row = self.tiles_per_row;
        let cells_per_row = cells_per_tile * tiles_per_row;
        let mut pass: HashSet<(usize, usize)> = HashSet::new();
        let mut block: HashSet<(usize, usize)> = HashSet::new();
        for (&(a, b), &mask) in &self.edges {
            let target = if mask & EDGE_LIGHT != 0 { &mut pass } else { &mut block };
            let (a_x, a_y) = (a % tiles_per_row, a / tiles_per_row);
            let (b_x, b_y) = (b % tiles_per_row, b / tiles_per_row);
            if a_y == b_y && a_x.abs_diff(b_x) == 1 {
                let left_tx = a_x.min(b_x);
                let cy0 = a_y * cells_per_tile;
                let cx_left = left_tx * cells_per_tile + cells_per_tile - 1;
                let cx_right = cx_left + 1;
                for dy in 0..cells_per_tile {
                    let li = (cy0 + dy) * cells_per_row + cx_left;
                    let ri = (cy0 + dy) * cells_per_row + cx_right;
                    target.insert(canonical_edge(li, ri));
                }
            } else if a_x == b_x && a_y.abs_diff(b_y) == 1 {
                let top_ty = a_y.min(b_y);
                let cx0 = a_x * cells_per_tile;
                let cy_top = top_ty * cells_per_tile + cells_per_tile - 1;
                let cy_bot = cy_top + 1;
                for dx in 0..cells_per_tile {
                    let ti = cy_top * cells_per_row + cx0 + dx;
                    let bi = cy_bot * cells_per_row + cx0 + dx;
                    target.insert(canonical_edge(ti, bi));
                }
            }
        }
        self.collision.set_edge_cell_overrides(pass, block);
    }

    /// Tile-coord BFS pathfinder. Returns the chain of tile indices from
    /// `(x1,y1)` to `(x2,y2)` inclusive, or empty if no route exists or
    /// either endpoint is a wall tile. Walks `neighbours()` so it respects
    /// the room graph (and door overlays that join rooms across boundaries).
    pub fn path(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) -> Vec<usize> {
        let tpr = self.tiles_per_row;
        let total = tpr * tpr;
        if x1 < 0 || y1 < 0 || x2 < 0 || y2 < 0 {
            return Vec::new();
        }
        let start = (x1 as usize) + (y1 as usize) * tpr;
        let goal = (x2 as usize) + (y2 as usize) * tpr;
        if start >= total || goal >= total {
            return Vec::new();
        }
        if self.tile_at(start) <= 0 || self.tile_at(goal) <= 0 || self.collides(goal) {
            return Vec::new();
        }

        let mut came_from: HashMap<usize, Option<usize>> = HashMap::new();
        let mut frontier: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        frontier.push_back(start);
        came_from.insert(start, None);
        let mut found = start == goal;

        while let Some(current) = frontier.pop_front() {
            if current == goal {
                found = true;
                break;
            }
            for next in self.neighbours(current, false) {
                if !came_from.contains_key(&next) && self.tile_at(next) > 0 {
                    came_from.insert(next, Some(current));
                    frontier.push_back(next);
                }
            }
        }

        if !found {
            return Vec::new();
        }

        let mut points = Vec::new();
        let mut current = goal;
        while current != start {
            points.push(current);
            match came_from.get(&current) {
                Some(Some(prev)) => current = *prev,
                _ => return Vec::new(),
            }
        }
        points.push(start);
        points.reverse();
        points
    }

    /// Tile-coord line-of-sight check. Per step of the Bresenham walk, an
    /// edge override decides via its `EDGE_LIGHT` bit (door/window grants
    /// across a boundary, a sealed edge denies even within a room); without
    /// one, staying inside the same room decides. Overrides are checked
    /// per-step, never via union-find merges, so opening one door does
    /// not silently dissolve the rest of the wall. Solid tiles crossed
    /// (not the endpoints) multiply transparency; fails below [`LOS_MIN`].
    pub fn cast_ray(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
        let tpr = self.tiles_per_row as i32;
        let in_bounds = |x: i32, y: i32| x >= 0 && y >= 0 && x < tpr && y < tpr;
        if !in_bounds(x1, y1) || !in_bounds(x2, y2) {
            return true;
        }
        let dx = x2 - x1;
        let dy = y2 - y1;
        let nx = dx.abs();
        let ny = dy.abs();
        let sx = if dx > 0 { 1 } else { -1 };
        let sy = if dy > 0 { 1 } else { -1 };
        let mut px = x1;
        let mut py = y1;
        let mut ix = 0;
        let mut iy = 0;
        let origin_idx = (py * tpr + px) as usize;
        let mut current_idx = origin_idx;
        let mut current_room = self.tile_uf.find(current_idx);
        let mut transparency = 1.0f32;

        while ix < nx || iy < ny {
            let prev_idx = current_idx;
            if prev_idx != origin_idx {
                if let Some(solid) = self.solids.get(&prev_idx) {
                    transparency *= 1.0 - solid.opacity as f32 / 255.0;
                    if transparency < LOS_MIN {
                        return false;
                    }
                }
            }
            if (ix as f32 + 0.5) / (nx as f32) < (iy as f32 + 0.5) / (ny as f32) {
                px += sx;
                ix += 1;
            } else {
                py += sy;
                iy += 1;
            }
            if !in_bounds(px, py) {
                return true;
            }
            let next_idx = (py * tpr + px) as usize;
            let next_room = self.tile_uf.find(next_idx);
            // Empty-map fast path: no overrides means room membership decides.
            let pass = if self.edges.is_empty() {
                next_room == current_room
            } else {
                match self.edges.get(&canonical_edge(prev_idx, next_idx)) {
                    Some(&m) => m & EDGE_LIGHT != 0,
                    None => next_room == current_room,
                }
            };
            if !pass {
                return false;
            }
            current_idx = next_idx;
            current_room = next_room;
        }
        true
    }

    /// Read-only access to the tile-resolution room id of `tile_idx`.
    pub fn tile_find(&mut self, tile_idx: usize) -> usize {
        self.tile_uf.find(tile_idx)
    }

    /// Tile type at `tile_idx` (or `-1` for out-of-range).
    pub fn tile_at(&self, tile_idx: usize) -> i32 {
        if tile_idx < self.tiles.len() {
            self.tiles[tile_idx] as i32
        } else {
            -1
        }
    }

    /// 4- or 8-connected tile neighbours of `tile_idx` reachable in one
    /// step: an edge override decides via its `EDGE_MOVE` bit (open door
    /// grants across a boundary, closed door denies even within a room);
    /// without one, sharing a room (same `tile_uf` root) decides. Diagonals
    /// are reachable only if at least one of the two cardinal steps that
    /// lead to the diagonal is itself reachable (no corner cutting).
    /// Colliding solid tiles are never reachable; leaving one is fine.
    pub fn neighbours(&mut self, tile_idx: usize, include_diagonal: bool) -> Vec<usize> {
        let tiles_per_row = self.tiles_per_row;
        let total = tiles_per_row * tiles_per_row;
        if tile_idx >= total {
            return Vec::new();
        }
        let row = tile_idx / tiles_per_row;
        let col = tile_idx % tiles_per_row;
        let room = self.tile_uf.find(tile_idx);
        let mut out = Vec::with_capacity(if include_diagonal { 8 } else { 4 });

        let north = if row > 0 { Some(tile_idx - tiles_per_row) } else { None };
        let south = if row + 1 < tiles_per_row { Some(tile_idx + tiles_per_row) } else { None };
        let west = if col > 0 { Some(tile_idx - 1) } else { None };
        let east = if col + 1 < tiles_per_row { Some(tile_idx + 1) } else { None };

        let solids = &self.solids;
        let collides = |i: usize| solids.get(&i).is_some_and(|s| s.collides);
        let reachable = |ni: Option<usize>, uf: &mut UnionFind| -> Option<usize> {
            let ni = ni?;
            if collides(ni) {
                return None;
            }
            // Empty-map fast path: no overrides means room membership decides.
            let ok = if self.edges.is_empty() {
                uf.find(ni) == room
            } else {
                match self.edges.get(&canonical_edge(tile_idx, ni)) {
                    Some(&m) => m & EDGE_MOVE != 0,
                    None => uf.find(ni) == room,
                }
            };
            if ok {
                Some(ni)
            } else {
                None
            }
        };

        let n_ok = reachable(north, &mut self.tile_uf);
        let s_ok = reachable(south, &mut self.tile_uf);
        let w_ok = reachable(west, &mut self.tile_uf);
        let e_ok = reachable(east, &mut self.tile_uf);

        if let Some(i) = n_ok { out.push(i); }
        if let Some(i) = w_ok { out.push(i); }
        if let Some(i) = s_ok { out.push(i); }
        if let Some(i) = e_ok { out.push(i); }

        if include_diagonal {
            let diag = |dr: i32, dc: i32| -> Option<usize> {
                let nr = row as i32 + dr;
                let nc = col as i32 + dc;
                if nr < 0 || nc < 0 || nr >= tiles_per_row as i32 || nc >= tiles_per_row as i32 {
                    return None;
                }
                Some(nr as usize * tiles_per_row + nc as usize)
            };
            let push_diag = |out: &mut Vec<usize>, idx: Option<usize>, gate_a: bool, gate_b: bool| {
                if !(gate_a || gate_b) {
                    return;
                }
                if let Some(d) = idx.filter(|&d| !collides(d)) {
                    out.push(d);
                }
            };
            push_diag(&mut out, diag(-1, -1), n_ok.is_some(), w_ok.is_some());
            push_diag(&mut out, diag(-1, 1), n_ok.is_some(), e_ok.is_some());
            push_diag(&mut out, diag(1, -1), s_ok.is_some(), w_ok.is_some());
            push_diag(&mut out, diag(1, 1), s_ok.is_some(), e_ok.is_some());
        }

        out
    }
}

/// Canonicalise an unordered tile-index pair so `(a, b)` and `(b, a)` map
/// to the same `HashSet` entry.
pub(crate) fn canonical_edge(a: usize, b: usize) -> (usize, usize) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Default process-wide engine instance, used as a back-compat shim for the
/// WASM/JS API. Rust callers should prefer constructing their own
/// [`LightingEngine`] via [`LightingEngine::new`].
pub static DEFAULT_ENGINE: Lazy<RwLock<LightingEngine>> =
    Lazy::new(|| RwLock::new(LightingEngine::default()));

/// Gradient used by [`LightingEngine::render_canvas_text`], from darkest to
/// brightest. Ten characters → integer division by 255 maps cleanly.
const ASCII_GRADIENT: &[u8; 10] = b" .:-=+*#%@";

impl LightingEngine {
    /// Render a light's canvas as a multiline ASCII matrix.
    ///
    /// One character per cell, one line per row, trailing `\n` after each
    /// row. Returns `None` if no light with the given id exists.
    ///
    /// This is the agent-readable debug primitive used by the exploration
    /// loop (printed unconditionally) and the regression loop (embedded in
    /// panic messages on assertion failure) so a failing test produces
    /// self-explanatory output.
    pub fn render_canvas_text(&self, id: u8) -> Option<String> {
        let light = self.lights.get(&id)?;
        let canvas = light.canvas();
        let size = light.canvas_size();
        let mut out = String::with_capacity((size + 1) * size);
        for row in 0..size {
            for col in 0..size {
                let pixel = canvas[row * size + col];
                // Transport mask: attenuation lives in alpha (ADR-0010).
                let brightness = pixel.3;
                let idx = (brightness as usize * (ASCII_GRADIENT.len() - 1)) / 255;
                out.push(ASCII_GRADIENT[idx] as char);
            }
            out.push('\n');
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_canvas_text_is_none_for_missing_light() {
        let engine = LightingEngine::new(6, 30);
        assert!(engine.render_canvas_text(0).is_none());
    }

    #[test]
    fn render_canvas_text_dimensions_match_canvas() {
        let mut engine = LightingEngine::new(6, 30);
        engine.update_or_add_light(1, 3, 5, 5);
        let s = engine.render_canvas_text(1).expect("light exists");
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 7, "radius-3 light renders 7 rows, got: {}\n{}", lines.len(), s);
        for (i, line) in lines.iter().enumerate() {
            assert_eq!(
                line.chars().count(),
                7,
                "row {} expected 7 cols, got {}: {:?}",
                i,
                line.chars().count(),
                line
            );
        }
    }

    #[test]
    fn render_canvas_text_center_brighter_than_corner() {
        let mut engine = LightingEngine::new(6, 30);
        engine.update_or_add_light(1, 3, 5, 5);
        let s = engine.render_canvas_text(1).unwrap();
        let lines: Vec<&str> = s.lines().collect();
        let center = lines[3].chars().nth(3).unwrap();
        let corner = lines[0].chars().nth(0).unwrap();
        let g = std::str::from_utf8(ASCII_GRADIENT).unwrap();
        let center_idx = g.find(center).unwrap_or_else(|| panic!("center {:?} not in gradient\n{}", center, s));
        let corner_idx = g.find(corner).unwrap_or_else(|| panic!("corner {:?} not in gradient\n{}", corner, s));
        assert!(
            center_idx >= corner_idx,
            "center {:?} (idx {}) should be at least as bright as corner {:?} (idx {}). Canvas:\n{}",
            center, center_idx, corner, corner_idx, s
        );
    }

    #[test]
    fn light_emits_white_mask_with_falloff_alpha() {
        // ADR-0010: transport only — white pixels, attenuation in alpha,
        // full-strength at the centre, dimmer toward the rim.
        let mut e = LightingEngine::new(6, 30);
        e.update_or_add_light(1, 3, 5, 5);
        let canvas = e.light_canvas(1).expect("light exists");
        let centre = canvas[3 + 3 * 7]; // centre of the radius-3 light's 7×7 canvas
        assert_eq!((centre.0, centre.1, centre.2, centre.3), (255, 255, 255, 255));
        let rim = canvas[3 + 1 * 7]; // two cells up from centre
        assert_eq!((rim.0, rim.1, rim.2), (255, 255, 255), "mask stays white");
        assert!(rim.3 > 0 && rim.3 < 255, "rim alpha attenuated, got {}", rim.3);
    }

    #[test]
    fn independent_engines_do_not_share_lights() {
        let mut a = LightingEngine::new(6, 30);
        let b = LightingEngine::new(6, 30);
        a.update_or_add_light(7, 2, 4, 4);
        assert!(a.light_canvas(7).is_some());
        assert!(b.light_canvas(7).is_none());
    }

    #[test]
    fn set_tile_at_non_default_resolution_marks_cell_edges() {
        // Engine with explicit non-default resolution (4 cells per tile, 5x5 tiles).
        let mut engine = LightingEngine::new(4, 5);
        // Empty world: all tiles type 0 → no internal boundaries.
        // Flip tile (2,2) to type 1, leaving its neighbours type 0.
        engine.set_tile(2, 2, 1);
        let cells = engine.block_map();
        let cells_per_row = engine.cells_per_row();
        // Top-left cell of tile (2,2) sits at cell (8,8). With cells_per_tile=4
        // its edges are at cell rows/cols 8..=11. The north edge should be
        // blocked (different from tile above), as should the west edge.
        let nw = cells[8 * cells_per_row + 8];
        assert!(nw.n_blocked, "NW cell should have north edge blocked");
        assert!(nw.w_blocked, "NW cell should have west edge blocked");
        // Inner cell (9,9) sits inside tile (2,2) — no edges blocked.
        let inner = cells[9 * cells_per_row + 9];
        assert!(!inner.n_blocked && !inner.s_blocked && !inner.e_blocked && !inner.w_blocked);
    }

    #[test]
    fn engine_exposes_resolution() {
        let e = LightingEngine::new(6, 30);
        assert_eq!(e.cells_per_tile(), 6);
        assert_eq!(e.tiles_per_row(), 30);

        let e2 = LightingEngine::new(9, 20);
        assert_eq!(e2.cells_per_tile(), 9);
        assert_eq!(e2.tiles_per_row(), 20);
    }

    #[test]
    fn path_returns_indices_between_two_passable_tiles() {
        let mut e = LightingEngine::new(2, 5);
        e.set_tile_map(vec![1u8; 25]);
        let p = e.path(0, 0, 4, 0);
        assert!(!p.is_empty(), "expected non-empty path, got {:?}", p);
        assert_eq!(p[0], 0, "path should start at start index");
        assert_eq!(p[p.len() - 1], 4, "path should end at goal index");
    }

    #[test]
    fn set_edge_records_canonical_pair() {
        let mut e = LightingEngine::default();
        assert_eq!(e.edge_mask(5, 7), None);

        // A non-zero mask records the edge; the pair is order-insensitive.
        e.set_edge(5, 7, EDGE_LIGHT | EDGE_MOVE);
        assert_eq!(e.edge_mask(5, 7), Some(EDGE_LIGHT | EDGE_MOVE));
        assert_eq!(e.edge_mask(7, 5), Some(EDGE_LIGHT | EDGE_MOVE));

        // Mask 0 seals it — a stored override, distinct from no override.
        e.set_edge(7, 5, 0);
        assert_eq!(e.edge_mask(5, 7), Some(0));
    }

    #[test]
    fn sealed_edge_blocks_movement_and_los_within_a_room() {
        let mut e = LightingEngine::new(2, 5);
        e.set_tile_map(vec![1u8; 25]); // one big room
        let idx = |x: usize, y: usize| -> usize { y * 5 + x };

        assert!(e.cast_ray(0, 1, 4, 1), "open room should have LOS");
        e.set_edge(idx(1, 1), idx(2, 1), 0);

        assert!(!e.cast_ray(0, 1, 4, 1), "sealed edge should block LOS");
        assert!(
            !e.neighbours(idx(1, 1), false).contains(&idx(2, 1)),
            "sealed edge should block the step"
        );
        // The rest of the room stays connected — pathfinding detours.
        let p = e.path(0, 1, 4, 1);
        assert!(!p.is_empty(), "expected a detour around the sealed edge");
    }

    #[test]
    fn closed_tile_door_seals_its_tile_for_light_and_movement() {
        // A tile-token door closes: all four edges of its tile arrive as
        // mask-0 overrides. The tile must go dark and unreachable, and
        // reopening (both bits) must restore normal same-room behaviour.
        let mut e = LightingEngine::new(4, 8);
        let tpr = e.tiles_per_row();
        let cpt = e.cells_per_tile();
        e.set_tile_map(vec![1u8; tpr * tpr]);
        let door = (4 * tpr + 4) as u32;
        let sides = [door - 1, door + 1, door - tpr as u32, door + tpr as u32];

        let flat: Vec<u32> = sides.iter().flat_map(|&s| [door, s, 0]).collect();
        e.replace_edges(&flat);
        assert!(
            e.neighbours(door as usize, false).is_empty(),
            "closed tile-door should be unreachable"
        );
        let west_cx = (3 * cpt + cpt / 2) as i16;
        let door_cx = (4 * cpt + cpt / 2) as i16;
        let cy = (4 * cpt + cpt / 2) as i16;
        assert!(
            e.is_blocked(west_cx, cy, door_cx, cy),
            "light must not enter a closed tile-door"
        );

        let flat: Vec<u32> = sides
            .iter()
            .flat_map(|&s| [door, s, (EDGE_LIGHT | EDGE_MOVE) as u32])
            .collect();
        e.replace_edges(&flat);
        assert_eq!(e.neighbours(door as usize, false).len(), 4);
        assert!(!e.is_blocked(west_cx, cy, door_cx, cy));
    }

    #[test]
    fn replace_edges_swaps_the_whole_set() {
        let mut e = LightingEngine::default();
        e.set_edge(5, 7, EDGE_LIGHT | EDGE_MOVE);

        e.replace_edges(&[1, 2, EDGE_LIGHT as u32]);
        assert_eq!(e.edge_mask(5, 7), None, "old edge should be gone");
        assert_eq!(e.edge_mask(1, 2), Some(EDGE_LIGHT));
    }

    #[test]
    fn closed_door_keeps_rooms_split_for_pathfinding() {
        let mut e = LightingEngine::new(2, 5);
        // Two rooms separated by a column of type-2 tiles at x=2.
        let mut tiles = vec![1u8; 25];
        for y in 0..5 {
            tiles[y * 5 + 2] = 2;
        }
        e.set_tile_map(tiles);
        // No door — different tile types ⇒ different rooms ⇒ no path.
        assert!(
            e.path(0, 1, 4, 1).is_empty(),
            "expected empty path across closed wall"
        );
    }

    #[test]
    fn open_door_joins_rooms_for_pathfinding() {
        let mut e = LightingEngine::new(2, 5);
        let mut tiles = vec![1u8; 25];
        for y in 0..5 {
            tiles[y * 5 + 2] = 2;
        }
        e.set_tile_map(tiles);
        // Open the door across the wall column at row 1.
        // Tile (1,1) and tile (3,1) are non-adjacent so the door is between
        // (1,1) and (2,1) and another between (2,1) and (3,1). But a single
        // door usually bridges directly between the two passable tiles, so
        // model it as: door between (1,1) and (3,1)? That isn't adjacent.
        // Use two doors that each step into the wall tile, then out.
        let idx = |x: usize, y: usize| -> usize { y * 5 + x };
        e.set_edge(idx(1, 1), idx(2, 1), EDGE_LIGHT | EDGE_MOVE);
        e.set_edge(idx(2, 1), idx(3, 1), EDGE_LIGHT | EDGE_MOVE);
        let p = e.path(0, 1, 4, 1);
        assert!(!p.is_empty(), "expected non-empty path through open door");
        assert_eq!(p[0], idx(0, 1));
        assert_eq!(p[p.len() - 1], idx(4, 1));
    }

    #[test]
    fn closed_door_blocks_light_into_adjacent_room() {
        // Two rooms separated by a column of wall tiles. Light placed inside
        // the west room must not leak into the east room when no door is open.
        let mut e = LightingEngine::new(4, 8);
        let tpr = e.tiles_per_row();
        let cpt = e.cells_per_tile();
        let mut tiles = vec![1u8; tpr * tpr];
        for y in 0..tpr {
            tiles[y * tpr + tpr / 2] = 0; // wall tiles
        }
        e.set_tile_map(tiles);

        let light_tx = tpr / 2 - 1;
        let light_ty = tpr / 2;
        let light_cx = (light_tx * cpt + cpt / 2) as i16;
        let light_cy = (light_ty * cpt + cpt / 2) as i16;
        e.update_or_add_light(1, (cpt * 4) as i16, light_cx, light_cy);

        // Sample a cell deep inside the east room — must be dark.
        let probe_cx = ((tpr / 2 + 1) * cpt + cpt / 2) as i16;
        let probe_cy = light_cy;
        let is_lit = !e.is_blocked(light_cx, light_cy, probe_cx, probe_cy);
        assert!(!is_lit, "closed wall should block ray to east room");
    }

    #[test]
    fn open_door_lets_light_cross_into_adjacent_room() {
        let mut e = LightingEngine::new(4, 8);
        let tpr = e.tiles_per_row();
        let cpt = e.cells_per_tile();
        let mut tiles = vec![1u8; tpr * tpr];
        for y in 0..tpr {
            tiles[y * tpr + tpr / 2] = 0;
        }
        e.set_tile_map(tiles);

        let light_ty = tpr / 2;
        let light_tx = tpr / 2 - 1;
        let wall_tx = tpr / 2;
        let east_tx = tpr / 2 + 1;
        e.set_edge(
            light_ty * tpr + light_tx,
            light_ty * tpr + wall_tx,
            EDGE_LIGHT | EDGE_MOVE,
        );
        e.set_edge(
            light_ty * tpr + wall_tx,
            light_ty * tpr + east_tx,
            EDGE_LIGHT | EDGE_MOVE,
        );

        let light_cx = (light_tx * cpt + cpt / 2) as i16;
        let light_cy = (light_ty * cpt + cpt / 2) as i16;
        // Probe a cell one cell east of the door's east boundary so it sits
        // squarely inside the east room.
        let probe_cx = (east_tx * cpt + cpt / 2) as i16;
        let probe_cy = light_cy;
        assert!(
            !e.is_blocked(light_cx, light_cy, probe_cx, probe_cy),
            "open door should join rooms — ray must not be blocked"
        );
    }

    #[test]
    fn window_edge_passes_los_but_blocks_pathfinding() {
        let mut e = LightingEngine::new(2, 5);
        // Two rooms separated by a column of type-2 tiles at x=2.
        let mut tiles = vec![1u8; 25];
        for y in 0..5 {
            tiles[y * 5 + 2] = 2;
        }
        e.set_tile_map(tiles);
        let idx = |x: usize, y: usize| -> usize { y * 5 + x };

        assert!(!e.cast_ray(0, 1, 4, 1), "boundary should block LOS pre-window");

        e.set_edge(idx(1, 1), idx(2, 1), EDGE_LIGHT);
        e.set_edge(idx(2, 1), idx(3, 1), EDGE_LIGHT);

        assert!(e.cast_ray(0, 1, 4, 1), "expected LOS through window");
        assert!(
            e.path(0, 1, 4, 1).is_empty(),
            "window must not join rooms for pathfinding"
        );
    }

    #[test]
    fn window_edge_lets_light_cross_into_adjacent_room() {
        // Same shape as the open-door lighting test, but with window edges.
        let mut e = LightingEngine::new(4, 8);
        let tpr = e.tiles_per_row();
        let cpt = e.cells_per_tile();
        let mut tiles = vec![1u8; tpr * tpr];
        for y in 0..tpr {
            tiles[y * tpr + tpr / 2] = 0;
        }
        e.set_tile_map(tiles);

        let light_ty = tpr / 2;
        let light_tx = tpr / 2 - 1;
        let wall_tx = tpr / 2;
        let east_tx = tpr / 2 + 1;
        e.set_edge(light_ty * tpr + light_tx, light_ty * tpr + wall_tx, EDGE_LIGHT);
        e.set_edge(light_ty * tpr + wall_tx, light_ty * tpr + east_tx, EDGE_LIGHT);

        let light_cx = (light_tx * cpt + cpt / 2) as i16;
        let light_cy = (light_ty * cpt + cpt / 2) as i16;
        let probe_cx = (east_tx * cpt + cpt / 2) as i16;
        assert!(
            !e.is_blocked(light_cx, light_cy, probe_cx, light_cy),
            "window should pass light — ray must not be blocked"
        );
    }

    #[test]
    fn open_door_does_not_dissolve_rest_of_wall_for_lighting() {
        // Two rooms of different tile types share a horizontal boundary. A
        // single door opens between (3,3) and (3,4). Rays crossing the
        // boundary elsewhere (e.g. through (1,3)→(1,4)) must remain blocked.
        let mut e = LightingEngine::new(4, 8);
        let tpr = e.tiles_per_row();
        let cpt = e.cells_per_tile();
        let mut tiles = vec![0u8; tpr * tpr];
        for y in 0..tpr {
            for x in 0..tpr {
                tiles[y * tpr + x] = if y < 4 { 1 } else { 2 };
            }
        }
        e.set_tile_map(tiles);
        // Door joins exactly tile (3,3) and (3,4).
        e.set_edge(3 * tpr + 3, 4 * tpr + 3, EDGE_LIGHT | EDGE_MOVE);

        // Probe across the boundary at a column far from the door (col 1).
        // Cell coords: x=1*cpt+cpt/2, y just above and just below the boundary.
        let cx = (1 * cpt + cpt / 2) as i16;
        let cy_upper = (3 * cpt + cpt / 2) as i16;
        let cy_lower = (4 * cpt + cpt / 2) as i16;
        assert!(
            e.is_blocked(cx, cy_upper, cx, cy_lower),
            "ray crossing the boundary far from the door must stay blocked"
        );
    }

    #[test]
    fn open_door_does_not_dissolve_rest_of_wall_for_pathfinding() {
        // Same shape as the lighting test: one door at column 3 must not let
        // pathfinding cross the boundary at column 1.
        let mut e = LightingEngine::new(2, 8);
        let tpr = e.tiles_per_row();
        let mut tiles = vec![0u8; tpr * tpr];
        for y in 0..tpr {
            for x in 0..tpr {
                tiles[y * tpr + x] = if y < 4 { 1 } else { 2 };
            }
        }
        e.set_tile_map(tiles);
        e.set_edge(3 * tpr + 3, 4 * tpr + 3, EDGE_LIGHT | EDGE_MOVE);

        // Path from (1, 3) to (1, 4) — different columns from the door —
        // would have to detour through (3, 3)→(3, 4) (the door) and back.
        // We don't assert path length, only that the path stays *off* the
        // boundary at non-door columns. Easier: assert that the boundary at
        // col 1 is NOT itself traversable as a direct step.
        let row3_col1 = 3 * tpr + 1;
        let row4_col1 = 4 * tpr + 1;
        assert!(
            !e.neighbours(row3_col1, false).contains(&row4_col1),
            "non-door boundary must not be a neighbour"
        );
    }

    // --- Ambient emitter ---------------------------------------------------

    /// Helper: is the cell at the centre of tile `(tx, ty)` opaque (in-room)?
    fn ambient_cell_opaque(e: &LightingEngine, id: u8, tx: usize, ty: usize) -> bool {
        let cpt = e.cells_per_tile();
        let cpr = e.cells_per_row();
        let cx = tx * cpt + cpt / 2;
        let cy = ty * cpt + cpt / 2;
        let canvas = e.ambient_canvas(id).expect("ambient exists");
        canvas[cy * cpr + cx].3 != 0
    }

    #[test]
    fn ambient_floods_only_its_room() {
        // Two rooms separated by a column of wall tiles (type 0) at x=2.
        let mut e = LightingEngine::new(2, 5);
        let mut tiles = vec![1u8; 25];
        for y in 0..5 {
            tiles[y * 5 + 2] = 0; // wall column
        }
        e.set_tile_map(tiles);

        // Emitter in the west room (tile (1,1)).
        e.update_or_add_ambient(0, 1, 1);

        // West-room cells are opaque white (mask; colour is renderer-side)...
        let cpr = e.cells_per_row();
        let cpt = e.cells_per_tile();
        let west = e.ambient_canvas(0).unwrap()[(1 * cpt + 1) * cpr + (1 * cpt + 1)];
        assert_eq!((west.0, west.1, west.2, west.3), (255, 255, 255, 255));
        assert!(ambient_cell_opaque(&e, 0, 0, 0), "same-room tile filled");

        // ...east-room cells (x>=3) stay transparent.
        assert!(!ambient_cell_opaque(&e, 0, 3, 1), "east room must be dark");
        assert!(!ambient_cell_opaque(&e, 0, 4, 0), "east room must be dark");
        // Wall column itself stays transparent.
        assert!(!ambient_cell_opaque(&e, 0, 2, 1), "wall tile must be dark");
    }

    #[test]
    fn ambient_does_not_cross_door_open_or_closed() {
        // Two same-... no: two rooms of different tile types share a vertical
        // boundary at column x=2 (west type 1, east type 2). A door joins them
        // for pathfinding/lighting, but ambient is `tile_uf`-bounded and must
        // not cross regardless of door state.
        let mut e = LightingEngine::new(2, 5);
        let mut tiles = vec![1u8; 25];
        for y in 0..5 {
            for x in 2..5 {
                tiles[y * 5 + x] = 2;
            }
        }
        e.set_tile_map(tiles);

        // Closed door: east room dark.
        e.update_or_add_ambient(0, 1, 1);
        assert!(ambient_cell_opaque(&e, 0, 1, 1), "west room filled");
        assert!(!ambient_cell_opaque(&e, 0, 3, 1), "closed door: east dark");

        // Open the door between (1,1) and (2,1); re-flood. Still east dark.
        e.set_edge(1 * 5 + 1, 1 * 5 + 2, EDGE_LIGHT | EDGE_MOVE);
        e.update_or_add_ambient(0, 1, 1);
        assert!(ambient_cell_opaque(&e, 0, 1, 1), "west room still filled");
        assert!(
            !ambient_cell_opaque(&e, 0, 3, 1),
            "open door must NOT let ambient cross (tile_uf is door-agnostic)"
        );
    }

    #[test]
    fn ambient_on_non_floor_tile_is_empty() {
        let mut e = LightingEngine::new(2, 5);
        // Whole map is floor except tile (1,1) which is a wall (type 0).
        let mut tiles = vec![1u8; 25];
        tiles[1 * 5 + 1] = 0;
        e.set_tile_map(tiles);

        e.update_or_add_ambient(0, 1, 1);
        let canvas = e.ambient_canvas(0).unwrap();
        assert!(
            canvas.iter().all(|c| c.3 == 0),
            "emitter on a non-floor tile emits a fully transparent canvas"
        );
    }

    #[test]
    fn ambient_out_of_range_is_empty() {
        let mut e = LightingEngine::new(2, 5);
        e.set_tile_map(vec![1u8; 25]);
        e.update_or_add_ambient(0, -1, 99);
        let canvas = e.ambient_canvas(0).unwrap();
        assert!(canvas.iter().all(|c| c.3 == 0), "out-of-range → empty canvas");
    }

    #[test]
    fn two_ambient_emitters_fill_their_respective_rooms() {
        // Two rooms split by a wall column at x=2; an emitter in each.
        let mut e = LightingEngine::new(2, 5);
        let mut tiles = vec![1u8; 25];
        for y in 0..5 {
            tiles[y * 5 + 2] = 0;
        }
        e.set_tile_map(tiles);

        e.update_or_add_ambient(0, 1, 1); // west
        e.update_or_add_ambient(1, 3, 1); // east

        assert!(ambient_cell_opaque(&e, 0, 1, 1) && !ambient_cell_opaque(&e, 0, 3, 1));
        assert!(ambient_cell_opaque(&e, 1, 3, 1) && !ambient_cell_opaque(&e, 1, 1, 1));
    }

    #[test]
    fn ambient_reflood_tracks_room_after_tile_edit() {
        // An emitter floods the whole one-room map; painting a wall that splits
        // the room (and re-flooding) leaves only the emitter's half lit.
        let mut e = LightingEngine::new(2, 5);
        e.set_tile_map(vec![1u8; 25]);
        e.update_or_add_ambient(0, 0, 1);
        assert!(ambient_cell_opaque(&e, 0, 4, 1), "single room: far tile lit");

        // Split with a wall column at x=2.
        for y in 0..5 {
            e.set_tile(2, y as u32, 0);
        }
        e.update_or_add_ambient(0, 0, 1);
        assert!(ambient_cell_opaque(&e, 0, 0, 1), "emitter half stays lit");
        assert!(!ambient_cell_opaque(&e, 0, 4, 1), "far half now dark after split");
    }

    // --- FOV canvas --------------------------------------------------------

    /// Helper: is cell `(cx, cy)` opaque (visible) in the last FOV canvas?
    fn fov_visible(e: &LightingEngine, cx: usize, cy: usize) -> bool {
        let cpr = e.cells_per_row();
        e.fov_canvas().expect("compute_fov was called")[cy * cpr + cx].3 != 0
    }

    /// Helper: count visible cells whose `cx` falls in `[cx_lo, cx_hi)`.
    /// The low-res test ray table (36 angles / dist 10) paints a holey disc, so
    /// region counts are robust where single-cell sampling is flaky.
    fn fov_count_in_columns(e: &LightingEngine, cx_lo: usize, cx_hi: usize) -> usize {
        let cpr = e.cells_per_row();
        let canvas = e.fov_canvas().expect("compute_fov was called");
        let mut n = 0;
        for cy in 0..cpr {
            for cx in cx_lo..cx_hi {
                if canvas[cy * cpr + cx].3 != 0 {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn fov_single_viewer_in_open_room() {
        // Empty world = one big room, no walls. A viewer sees its own cell and a
        // fan of nearby cells, but nothing past the ray table's max distance.
        let mut e = LightingEngine::new(2, 30);
        e.compute_fov(&[20, 20]);
        let cpr = e.cells_per_row();
        // The viewer's own cell is opaque white — binary alpha, no falloff.
        let here = e.fov_canvas().expect("computed")[20 * cpr + 20];
        assert_eq!((here.0, here.1, here.2, here.3), (255, 255, 255, 255));
        // Rays fan out, so many cells around the viewer are lit.
        assert!(
            fov_count_in_columns(&e, 0, cpr) > 20,
            "an open-room viewer lights a fan of cells"
        );
        // Sight is capped at the ray table's max distance (10 cells in test
        // builds): a cell well beyond it stays dark.
        assert!(!fov_visible(&e, 20, 35), "a far cell is out of sight");
    }

    #[test]
    fn fov_blocked_by_wall() {
        // Two rooms split by a wall column (type 0) at tile x=4; rest type 1.
        // Without the wall the viewer would see east-room cells; the wall must
        // occlude every one of them.
        let mut e = LightingEngine::new(4, 8);
        let tpr = e.tiles_per_row();
        let cpt = e.cells_per_tile();
        let wall_cx = (tpr / 2) * cpt; // first east-of-wall cell column
        let vx = (3 * cpt + cpt / 2) as i16;
        let vy = (4 * cpt + cpt / 2) as i16;
        let cpr = e.cells_per_row();

        // Baseline: open room, viewer sees across into the (future) east columns.
        e.set_tile_map(vec![1u8; tpr * tpr]);
        e.compute_fov(&[vx, vy]);
        assert!(
            fov_count_in_columns(&e, wall_cx, cpr) > 0,
            "without a wall the viewer sees east-of-boundary cells"
        );

        // Now raise the wall column and re-cast: the east room goes fully dark.
        let mut tiles = vec![1u8; tpr * tpr];
        for y in 0..tpr {
            tiles[y * tpr + tpr / 2] = 0;
        }
        e.set_tile_map(tiles);
        e.compute_fov(&[vx, vy]);
        assert!(fov_visible(&e, vx as usize, vy as usize), "viewer cell still lit");
        assert_eq!(
            fov_count_in_columns(&e, wall_cx, cpr),
            0,
            "wall must occlude all sight into the east room"
        );
    }

    #[test]
    fn fov_blocked_without_door_visible_with_open_door() {
        // West tiles type 1 (x<4), east tiles type 2 (x>=4): a room boundary at
        // the tile-3 / tile-4 edge. A viewer in the west reaches the east room
        // only when a door is open across that exact edge.
        let mut e = LightingEngine::new(4, 8);
        let tpr = e.tiles_per_row();
        let cpt = e.cells_per_tile();
        let mut tiles = vec![1u8; tpr * tpr];
        for y in 0..tpr {
            for x in (tpr / 2)..tpr {
                tiles[y * tpr + x] = 2;
            }
        }
        e.set_tile_map(tiles);

        let vx = (3 * cpt + cpt / 2) as i16;
        let vy = (4 * cpt + cpt / 2) as i16;
        let east_cx = (tpr / 2) * cpt; // first east-room cell column
        let cpr = e.cells_per_row();

        // No door: the boundary occludes sight into the east room.
        e.compute_fov(&[vx, vy]);
        assert_eq!(
            fov_count_in_columns(&e, east_cx, cpr),
            0,
            "closed boundary blocks all sight into the east room"
        );

        // Open the door between tile (3,4) and (4,4): sight crosses there.
        e.set_edge(4 * tpr + 3, 4 * tpr + 4, EDGE_LIGHT | EDGE_MOVE);
        e.compute_fov(&[vx, vy]);
        assert!(
            fov_count_in_columns(&e, east_cx, cpr) > 0,
            "open door lets sight cross into the east room"
        );
    }

    #[test]
    fn fov_multi_viewer_union() {
        // Two viewers far enough apart that neither alone covers the other. The
        // single FOV canvas is the union — both viewer cells are lit at once.
        let mut e = LightingEngine::new(2, 30);
        e.compute_fov(&[10, 10, 50, 50]);
        assert!(fov_visible(&e, 10, 10), "viewer A's cell visible");
        assert!(fov_visible(&e, 50, 50), "viewer B's cell visible");
        // A cell far from both viewers stays dark (out of range of either).
        assert!(!fov_visible(&e, 30, 30), "midpoint out of both viewers' range");
    }

    #[test]
    fn fov_empty_viewers_is_fully_transparent() {
        let mut e = LightingEngine::new(2, 30);
        e.compute_fov(&[]);
        let canvas = e.fov_canvas().expect("computed");
        assert!(
            canvas.iter().all(|c| c.3 == 0),
            "no viewers → fully transparent canvas"
        );
    }

    #[test]
    fn replacing_with_empty_set_restores_room_boundary() {
        let mut e = LightingEngine::new(2, 5);
        let mut tiles = vec![1u8; 25];
        for y in 0..5 {
            tiles[y * 5 + 2] = 2;
        }
        e.set_tile_map(tiles);
        let idx = |x: usize, y: usize| -> usize { y * 5 + x };
        e.set_edge(idx(1, 1), idx(2, 1), EDGE_LIGHT | EDGE_MOVE);
        e.set_edge(idx(2, 1), idx(3, 1), EDGE_LIGHT | EDGE_MOVE);
        assert!(!e.path(0, 1, 4, 1).is_empty());
        e.replace_edges(&[]);
        assert!(
            e.path(0, 1, 4, 1).is_empty(),
            "clearing edges must re-split the rooms"
        );
        assert_eq!(e.edge_mask(idx(1, 1), idx(2, 1)), None);
    }

    // --- Solid tiles (ADR-0016) ---------------------------------------------

    /// 8×8 tiles of 4 cells, one room; `(tile, opacity, collides)` solids.
    fn solid_world(solids: &[(usize, u32, u32)]) -> LightingEngine {
        let mut e = LightingEngine::new(4, 8);
        e.set_tile_map(vec![1u8; 64]);
        let flat: Vec<u32> = solids.iter().flat_map(|&(t, o, c)| [t as u32, o, c]).collect();
        e.replace_solids(&flat);
        e
    }

    fn fov_alpha(e: &LightingEngine, cx: usize, cy: usize) -> u8 {
        e.fov_canvas().expect("computed")[cy * e.cells_per_row() + cx].3
    }

    #[test]
    fn opaque_solid_is_seen_not_seen_through() {
        // Viewer at tile (2,4) centre, crate on tile (3,4) = cells x 12..15.
        let mut e = solid_world(&[(4 * 8 + 3, 255, 1)]);
        e.compute_fov(&[10, 18]);
        assert_eq!(fov_alpha(&e, 12, 18), 255, "crate face visible");
        assert_eq!(fov_alpha(&e, 13, 18), 0, "crate interior hidden");
        assert_eq!(fov_alpha(&e, 17, 18), 0, "beyond the crate hidden");
    }

    #[test]
    fn translucent_solid_attenuates_by_authored_value_per_crossing() {
        let mut e = solid_world(&[(4 * 8 + 3, 128, 0)]);
        e.compute_fov(&[10, 18]);
        let beyond = fov_alpha(&e, 17, 18) as i32;
        assert!((beyond - 127).abs() <= 3, "half transparency ≈ half alpha, got {}", beyond);
        assert_eq!(fov_alpha(&e, 12, 18), 255, "entry cell sees full weight");
    }

    #[test]
    fn origin_tile_never_occludes_its_own_viewer_or_light() {
        // Firepit: opaque tile with a light inside still lights its room.
        let mut e = solid_world(&[(4 * 8 + 3, 255, 1)]);
        e.compute_fov(&[14, 18]);
        assert_eq!(fov_alpha(&e, 19, 18), 255, "viewer inside sees out clearly");
        e.update_or_add_light(1, 8, 14, 18);
        let canvas = e.light_canvas(1).unwrap();
        let size = e.light_canvas_size(1).unwrap();
        assert!(canvas[8 * size + 13].3 > 0, "light inside lights beyond its tile");
    }

    #[test]
    fn light_is_dimmed_through_translucent_and_stopped_by_opaque() {
        let mut clear = solid_world(&[]);
        clear.update_or_add_light(1, 9, 10, 18);
        let mut smoke = solid_world(&[(4 * 8 + 3, 128, 0)]);
        smoke.update_or_add_light(1, 9, 10, 18);
        let mut wall = solid_world(&[(4 * 8 + 3, 255, 1)]);
        wall.update_or_add_light(1, 9, 10, 18);
        // Cell (17,18) = canvas (9+7, 9) in a radius-9 light.
        let at = |e: &LightingEngine| e.light_canvas(1).unwrap()[9 * 19 + 16].3;
        assert!(at(&clear) > 0);
        assert!(at(&smoke) > 0 && at(&smoke) < at(&clear), "smoke dims");
        assert_eq!(at(&wall), 0, "opaque blocks");
        let face = wall.light_canvas(1).unwrap()[9 * 19 + 11].3;
        assert!(face > 0, "lit crate face");
    }

    #[test]
    fn replace_solids_clears_removed_tiles() {
        let mut e = solid_world(&[(4 * 8 + 3, 255, 1)]);
        e.replace_solids(&[]);
        e.compute_fov(&[10, 18]);
        assert_eq!(fov_alpha(&e, 17, 18), 255);
        assert_eq!(e.solid_at(4 * 8 + 3), None);
    }

    #[test]
    fn cast_ray_passes_through_translucent_until_los_min() {
        let idx = |x: usize, y: usize| y * 8 + x;
        let e_ok = |solids: &[(usize, u32, u32)]| solid_world(solids).cast_ray(0, 4, 6, 4);
        assert!(!e_ok(&[(idx(3, 4), 255, 1)]), "opaque blocks LoS");
        assert!(e_ok(&[(idx(3, 4), 179, 0)]), "single smoke (t≈0.3) passes");
        assert!(!e_ok(&[(idx(3, 4), 179, 0), (idx(4, 4), 179, 0)]), "two smokes < LOS_MIN");
        let mut e = solid_world(&[(idx(6, 4), 255, 1), (idx(0, 4), 255, 1)]);
        assert!(e.cast_ray(0, 4, 6, 4), "endpoint solids never occlude");
    }

    #[test]
    fn colliding_solid_refuses_entry_but_allows_leaving() {
        let idx = |x: usize, y: usize| y * 8 + x;
        let mut e = solid_world(&[(idx(3, 4), 255, 1), (idx(5, 4), 77, 0)]);
        assert!(!e.neighbours(idx(2, 4), true).contains(&idx(3, 4)));
        assert!(e.neighbours(idx(4, 4), false).contains(&idx(5, 4)), "non-colliding stays passable");
        assert_eq!(e.neighbours(idx(3, 4), false).len(), 4, "leaving is fine");
        assert!(e.path(0, 4, 3, 4).is_empty(), "colliding goal");
        let p = e.path(0, 4, 6, 4);
        assert!(!p.is_empty() && !p.contains(&idx(3, 4)), "detours around");
    }
}
