//! Rule light reads (ADR-0024): the renderer's lightmap merge, averaged over one tile.

use super::board::{Board, ENGINE_BUFFER_TILES};
use super::character::Ctx;
use super::tiles::{js_round, HALF_TILE, TILE_SIZE};
use super::types::*;

pub enum Emitter {
    Light {
        position: Px,
        radius: f64,
        intensity: f64,
    },
    Ambient {
        position: Px,
        intensity: f64,
    },
}

/// Mask alpha → shaped alpha; `sqrt` gives a fuller mid-radius.
fn shaped(falloff: Falloff, a: u8) -> u32 {
    match falloff {
        Falloff::Linear => a as u32,
        Falloff::Sqrt => js_round((a as f64 * 255.0).sqrt()) as u32,
    }
}

/// Token world px (tile corner) → engine cell at its tile centre; the renderer's projection.
pub fn engine_cell_of(pos: f64, cells_per_tile: usize) -> i32 {
    ((pos + HALF_TILE + TILE_SIZE) / TILE_SIZE * cells_per_tile as f64).floor() as i32
}

/// Every `<light>`/`<ambient>` on placed tokens.
pub fn emitters_of(tokens: &[Token], ctx: &Ctx) -> Vec<Emitter> {
    let mut out = vec![];
    for t in tokens {
        let Some(position) = t.position else { continue };
        let Some(def) = ctx.token_def(t.token_id.as_deref()) else {
            continue;
        };
        if let Some(l) = def.light {
            out.push(Emitter::Light {
                position,
                radius: l.radius,
                intensity: l.intensity,
            });
        }
        if let Some(a) = def.ambient {
            out.push(Emitter::Ambient {
                position,
                intensity: a.intensity,
            });
        }
    }
    out
}

const FULL: u32 = 255 * 255;

/// Light on a tile, 0 dark … 1 full; none = off the board or the board can't light.
pub fn light_level(
    board: &mut dyn Board,
    emitters: &[Emitter],
    tile: Pt,
    falloff: Falloff,
) -> Option<f64> {
    if !board.in_world(tile) {
        return None;
    }
    let cpt = board.cells_per_tile() as i32;
    let x0 = (tile.x + ENGINE_BUFFER_TILES) * cpt;
    let y0 = (tile.y + ENGINE_BUFFER_TILES) * cpt;
    let mut cells = vec![0u32; (cpt * cpt) as usize];
    for e in emitters {
        let intensity = match e {
            Emitter::Light { intensity, .. } | Emitter::Ambient { intensity, .. } => {
                js_round(intensity.clamp(0.0, 1.0) * 255.0) as u32
            }
        };
        if intensity == 0 {
            continue;
        }
        match e {
            Emitter::Ambient { position, .. } => {
                let at = Pt {
                    x: ((position.x + HALF_TILE) / TILE_SIZE).floor() as i32,
                    y: ((position.y + HALF_TILE) / TILE_SIZE).floor() as i32,
                };
                if board.in_world(at)
                    && board.tile_at(at) > 0
                    && board.room_at(at) == board.room_at(tile)
                {
                    cells.iter_mut().for_each(|c| *c += 255 * intensity);
                }
            }
            Emitter::Light {
                position, radius, ..
            } => {
                let lx = engine_cell_of(position.x, cpt as usize);
                let ly = engine_cell_of(position.y, cpt as usize);
                let reach = js_round(radius * cpt as f64) as i32;
                if lx + reach < x0
                    || ly + reach < y0
                    || lx - reach >= x0 + cpt
                    || ly - reach >= y0 + cpt
                {
                    continue;
                }
                let mask = board.light_mask(radius * cpt as f64, lx, ly)?;
                let side = mask.r * 2 + 1;
                for cy in 0..cpt {
                    let my = y0 + cy - ly + mask.r;
                    if my < 0 || my >= side {
                        continue;
                    }
                    for cx in 0..cpt {
                        let mx = x0 + cx - lx + mask.r;
                        if mx < 0 || mx >= side {
                            continue;
                        }
                        cells[(cx + cy * cpt) as usize] +=
                            shaped(falloff, mask.alpha[(mx + my * side) as usize]) * intensity;
                    }
                }
            }
        }
    }
    let sum: u64 = cells.iter().map(|v| (*v).min(FULL) as u64).sum();
    Some(sum as f64 / (cells.len() as u64 * FULL as u64) as f64)
}
