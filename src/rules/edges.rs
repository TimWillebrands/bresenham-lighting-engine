//! Token-derived board overlays: door/window edges, solids, footprints, doors (ADR-0003/0016).

use super::character::{is_dead, Ctx};
use super::tiles::{manhattan, tile_of, wall_edge_tiles};
use super::types::*;
use crate::engine::{EDGE_LIGHT, EDGE_MOVE};
use std::collections::BTreeMap;

/// Openable state; absent = closed.
pub fn is_open(token: &Token) -> bool {
    token.open.unwrap_or(false)
}

fn is_wall(def: Option<&TokenDef>) -> bool {
    def.and_then(|d| d.token_type.as_deref()) == Some("wall")
}

/// The two tiles an edge-bearing wall token (window or openable) separates.
fn edge_footprint(token: &Token, def: Option<&TokenDef>) -> Option<(Pt, Pt)> {
    let d = def?;
    if !is_wall(def) || !(d.is_window || d.is_openable) {
        return None;
    }
    wall_edge_tiles(token.position?, token.angle)
}

/// Door/window overlay as `(a, b, mask)` world-tile triples: window = light-only, closed door = 0.
pub fn derive_edges(tokens: &[Token], ctx: &Ctx) -> Vec<(Pt, Pt, u8)> {
    tokens
        .iter()
        .filter_map(|t| {
            let def = ctx.token_def(t.token_id.as_deref());
            let (a, b) = edge_footprint(t, def)?;
            let def = def?;
            let mask = if def.is_window && !def.is_openable {
                EDGE_LIGHT
            } else if is_open(t) {
                EDGE_LIGHT | EDGE_MOVE
            } else {
                0
            };
            Some((a, b, mask))
        })
        .collect()
}

/// Wall tokens are solid by their edge, never by `<solid>`; open or dead suspends it.
pub fn solid_of(token: &Token, def: Option<&TokenDef>, ctx: &Ctx) -> Option<Solid> {
    let solid = def?.solid?;
    (!is_wall(def) && !is_open(token) && !is_dead(token, ctx)).then_some(solid)
}

fn span(n: Option<f64>) -> i32 {
    n.filter(|n| n.is_finite() && *n >= 1.0)
        .map_or(1, |n| n.floor() as i32)
}

/// `width × height` tiles from the token's corner tile.
pub fn footprint(position: Px, def: Option<&TokenDef>) -> Vec<Pt> {
    let at = tile_of(position);
    let (w, h) = (
        span(def.and_then(|d| d.width)),
        span(def.and_then(|d| d.height)),
    );
    (0..h)
        .flat_map(|dy| {
            (0..w).map(move |dx| Pt {
                x: at.x + dx,
                y: at.y + dy,
            })
        })
        .collect()
}

/// A colliding solid covers `tile`; `except` = the token being placed.
pub fn collides_at(tokens: &[Token], ctx: &Ctx, tile: Pt, except: Option<&str>) -> bool {
    tokens.iter().any(|t| {
        let Some(pos) = t.position.filter(|_| Some(t.yid.as_str()) != except) else {
            return false;
        };
        let def = ctx.token_def(t.token_id.as_deref());
        solid_of(t, def, ctx).is_some_and(|s| s.collides) && footprint(pos, def).contains(&tile)
    })
}

/// Solid overlay per world tile: stacked solids multiply transparency, any collides.
pub fn derive_solids(
    tokens: &[Token],
    ctx: &Ctx,
    in_world: impl Fn(Pt) -> bool,
) -> BTreeMap<Pt, Solid> {
    let mut by_tile: BTreeMap<Pt, Solid> = BTreeMap::new();
    for t in tokens {
        let def = ctx.token_def(t.token_id.as_deref());
        let (Some(solid), Some(pos)) = (solid_of(t, def, ctx), t.position) else {
            continue;
        };
        for p in footprint(pos, def).into_iter().filter(|p| in_world(*p)) {
            let next = match by_tile.get(&p) {
                None => solid,
                Some(prev) => Solid {
                    collides: prev.collides || solid.collides,
                    transparency: prev.transparency * solid.transparency,
                },
            };
            by_tile.insert(p, next);
        }
    }
    by_tile
}

/// Door footprint without a board: edge doors = both edge tiles, tile doors = the tile itself.
pub fn door_footprint(token: &Token, def: Option<&TokenDef>) -> Option<Vec<Pt>> {
    if !def.is_some_and(|d| d.is_openable) {
        return None;
    }
    let pos = token.position?;
    if is_wall(def) {
        edge_footprint(token, def).map(|(a, b)| vec![a, b])
    } else {
        Some(vec![tile_of(pos)])
    }
}

/// Edge doors are worked from either side; tile doors from an orthogonal neighbour; `bonus` = interact reach.
pub fn next_to_door(at: Pt, tiles: &[Pt], bonus: f64) -> bool {
    let reach = if tiles.len() == 2 { 0.0 } else { 1.0 } + bonus;
    tiles.iter().any(|t| manhattan(*t, at) as f64 <= reach)
}
