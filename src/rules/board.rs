//! Board reads (path, LoS, rooms, light) in world-tile coords, and the per-layer grids derived from a model.
//! A grid is a memo of model data: `&mut` only because the engine caches union-find and light canvases.

use super::character::Ctx;
use super::edges::{derive_edges, derive_solids};
use super::model::{Layer, Model};
use super::tiles::LAYER_SIZE;
use super::types::Pt;
use crate::engine::LightingEngine;
use std::collections::BTreeMap;

/// One-tile wall ring around the world inside the engine grid.
pub const ENGINE_BUFFER_TILES: i32 = 1;
/// Rule light reads match the renderer's projection at this resolution.
pub const DEFAULT_CELLS_PER_TILE: usize = 9;
/// Light id rule reads `put` into; masks are read at once.
const SCRATCH_LIGHT: u8 = 255;

/// Transport-mask alpha of a light, side `2r+1`.
pub struct Mask {
    pub r: i32,
    pub alpha: Vec<u8>,
}

pub trait Board {
    fn in_world(&self, p: Pt) -> bool;
    /// Tile type; 0 = rock.
    fn tile_at(&self, p: Pt) -> i32;
    /// Room id; walls form rooms of their own.
    fn room_at(&mut self, p: Pt) -> usize;
    /// Route with both endpoints; empty = unreachable, wall endpoint or colliding goal.
    fn path(&mut self, from: Pt, to: Pt) -> Vec<Pt>;
    fn cast_ray(&mut self, from: Pt, to: Pt) -> bool;
    /// Tiles reachable in one step (room adjacency + door overlay), world only.
    fn neighbours(&mut self, p: Pt) -> Vec<Pt>;
    fn cells_per_tile(&self) -> usize;
    /// Mask of a light at engine cell (x, y); none = the board can't light.
    fn light_mask(&mut self, r: f64, x: i32, y: i32) -> Option<Mask>;
}

/// Engine-backed board of one layer; `sync` keeps it equal to the model.
pub struct RouteGrid {
    pub engine: LightingEngine,
    pub layer_size: i32,
    pub engine_size: i32,
    tiles: Vec<i32>,
    edges: Vec<u32>,
    solids: Vec<u32>,
}

impl RouteGrid {
    pub fn new(cells_per_tile: usize, layer_size: i32) -> Self {
        let engine_size = layer_size + 2 * ENGINE_BUFFER_TILES;
        RouteGrid {
            engine: LightingEngine::new(cells_per_tile, engine_size as usize),
            layer_size,
            engine_size,
            tiles: vec![],
            edges: vec![],
            solids: vec![],
        }
    }

    pub fn idx(&self, p: Pt) -> usize {
        ((p.x + ENGINE_BUFFER_TILES) + (p.y + ENGINE_BUFFER_TILES) * self.engine_size) as usize
    }

    fn pt_at(&self, i: usize) -> Pt {
        let i = i as i32;
        Pt {
            x: i % self.engine_size - ENGINE_BUFFER_TILES,
            y: i / self.engine_size - ENGINE_BUFFER_TILES,
        }
    }

    /// Push tiles, edges and solids that differ from the last sync.
    pub fn sync(&mut self, layer: &Layer, ctx: &Ctx) {
        if layer.tiles != self.tiles {
            self.tiles = layer.tiles.clone();
            let mut padded = vec![0u8; (self.engine_size * self.engine_size) as usize];
            for y in 0..self.layer_size {
                for x in 0..self.layer_size {
                    let v = self
                        .tiles
                        .get((x + y * self.layer_size) as usize)
                        .copied()
                        .unwrap_or(0);
                    padded[self.idx(Pt { x, y })] = v as u8;
                }
            }
            self.engine.set_tile_map(padded);
        }
        let edges: Vec<u32> = derive_edges(&layer.tokens, ctx)
            .into_iter()
            .flat_map(|(a, b, m)| [self.idx(a) as u32, self.idx(b) as u32, m as u32])
            .collect();
        if edges != self.edges {
            self.engine.replace_edges(&edges);
            self.edges = edges;
        }
        let size = self.layer_size;
        let in_world = |p: Pt| p.x >= 0 && p.y >= 0 && p.x < size && p.y < size;
        let solids: Vec<u32> = derive_solids(&layer.tokens, ctx, in_world)
            .into_iter()
            .flat_map(|(p, s)| {
                [
                    self.idx(p) as u32,
                    super::tiles::js_round((1.0 - s.transparency) * 255.0) as u32,
                    s.collides as u32,
                ]
            })
            .collect();
        if solids != self.solids {
            self.engine.replace_solids(&solids);
            self.solids = solids;
        }
    }
}

impl Board for RouteGrid {
    fn in_world(&self, p: Pt) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.layer_size && p.y < self.layer_size
    }

    fn tile_at(&self, p: Pt) -> i32 {
        self.engine.tile_at(self.idx(p))
    }

    fn room_at(&mut self, p: Pt) -> usize {
        let i = self.idx(p);
        self.engine.tile_find(i)
    }

    fn path(&mut self, from: Pt, to: Pt) -> Vec<Pt> {
        let b = ENGINE_BUFFER_TILES;
        let raw = self.engine.path(from.x + b, from.y + b, to.x + b, to.y + b);
        raw.into_iter().map(|i| self.pt_at(i)).collect()
    }

    fn cast_ray(&mut self, from: Pt, to: Pt) -> bool {
        let b = ENGINE_BUFFER_TILES;
        self.engine
            .cast_ray(from.x + b, from.y + b, to.x + b, to.y + b)
    }

    fn neighbours(&mut self, p: Pt) -> Vec<Pt> {
        if !self.in_world(p) {
            return vec![];
        }
        let i = self.idx(p);
        let raw = self.engine.neighbours(i, false);
        raw.into_iter()
            .map(|i| self.pt_at(i))
            .filter(|q| self.in_world(*q))
            .collect()
    }

    fn cells_per_tile(&self) -> usize {
        self.engine.cells_per_tile()
    }

    fn light_mask(&mut self, r: f64, x: i32, y: i32) -> Option<Mask> {
        let cr = (super::tiles::js_round(r) as i32).clamp(1, crate::lighting::max_dist() as i32);
        self.engine
            .update_or_add_light(SCRATCH_LIGHT, cr as i16, x as i16, y as i16);
        let canvas = self.engine.light_canvas(SCRATCH_LIGHT)?;
        Some(Mask {
            r: cr,
            alpha: canvas.iter().map(|c| c.3).collect(),
        })
    }
}

/// A grid per layer, synced from the model on demand.
pub struct Boards {
    pub cells_per_tile: usize,
    grids: BTreeMap<String, RouteGrid>,
}

impl Default for Boards {
    fn default() -> Self {
        Boards::new(DEFAULT_CELLS_PER_TILE)
    }
}

impl Boards {
    pub fn new(cells_per_tile: usize) -> Self {
        Boards {
            cells_per_tile,
            grids: BTreeMap::new(),
        }
    }

    /// The layer's grid, synced to `model`; none = no such layer.
    pub fn grid(&mut self, model: &Model, layer_yid: &str) -> Option<&mut RouteGrid> {
        let layer = model.layer(layer_yid)?;
        let cpt = self.cells_per_tile;
        let grid = self
            .grids
            .entry(layer_yid.to_string())
            .or_insert_with(|| RouteGrid::new(cpt, LAYER_SIZE));
        grid.sync(layer, &Ctx::new(model));
        Some(grid)
    }

    /// Drop grids of layers the model no longer has.
    pub fn retain(&mut self, model: &Model) {
        self.grids.retain(|yid, _| model.layer(yid).is_some());
    }
}
