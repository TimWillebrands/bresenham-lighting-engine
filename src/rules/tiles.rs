//! Tile geometry, turn arithmetic and the JS rounding rules hosts expect.

use super::types::{Pt, Px};

pub const LAYER_SIZE: i32 = 30;
pub const TILE_SIZE: f64 = 96.0;
pub const HALF_TILE: f64 = TILE_SIZE / 2.0;

/// The turn counter alternates sides; a rule turn is one stage per side.
pub const SIDES: i64 = 2;

pub fn manhattan(a: Pt, b: Pt) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

/// World-px → tile. Token positions are tile corners.
pub fn tile_of(p: Px) -> Pt {
    Pt {
        x: (p.x / TILE_SIZE).floor() as i32,
        y: (p.y / TILE_SIZE).floor() as i32,
    }
}

/// Tile → world-px corner.
pub fn px_of(t: Pt) -> Px {
    Px {
        x: t.x as f64 * TILE_SIZE,
        y: t.y as f64 * TILE_SIZE,
    }
}

/// World-px → tile for centre/edge-anchored positions (wall tokens).
pub fn tile_of_center(p: Px) -> Pt {
    Pt {
        x: ((p.x + HALF_TILE) / TILE_SIZE).floor() as i32,
        y: ((p.y + HALF_TILE) / TILE_SIZE).floor() as i32,
    }
}

/// JS `Math.round`: halves go up.
pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// The two tiles a wall token's edge separates; off-axis angles = none.
pub fn wall_edge_tiles(pos: Px, angle: Option<f64>) -> Option<(Pt, Pt)> {
    let a = ((js_round(angle.unwrap_or(0.0)) % 360.0) + 360.0) % 360.0;
    if a % 90.0 != 0.0 {
        return None;
    }
    let (dx, dy) = if a == 90.0 || a == 270.0 {
        (HALF_TILE, 0.0)
    } else {
        (0.0, HALF_TILE)
    };
    // round, not floor: edge positions sit on tile boundaries/centres
    let tile = |x: f64, y: f64| Pt {
        x: js_round(x / TILE_SIZE) as i32,
        y: js_round(y / TILE_SIZE) as i32,
    };
    Some((tile(pos.x - dx, pos.y - dy), tile(pos.x + dx, pos.y + dy)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Players,
    Gamemaster,
}

/// Odd = Players, even = Gamemaster.
pub fn turn_type(turn: i64) -> Side {
    if turn.rem_euclid(SIDES) == 1 {
        Side::Players
    } else {
        Side::Gamemaster
    }
}

/// Expiry counter for `turns=N` applied on `turn`: every side gets N of its own turns (#172).
pub fn until_turn(turn: i64, turns: f64) -> i64 {
    turn + SIDES * turns as i64
}

/// Sprite heading from `from` to `to`, whole degrees; 0 = north, clockwise (screen y down).
pub fn heading_deg(from: Pt, to: Pt) -> f64 {
    let deg = ((to.y - from.y) as f64).atan2((to.x - from.x) as f64) * 180.0 / std::f64::consts::PI
        + 90.0;
    ((js_round(deg) % 360.0) + 360.0) % 360.0
}
