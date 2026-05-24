# Unify collision detection around a single Wall + Object system

- Status: accepted
- Date: 2025-01-08 (rewritten 2026-05-24 for vocabulary alignment with [CONTEXT.md](../../CONTEXT.md); decision unchanged)

## Context and Problem Statement

After [ADR-0005](0005-hybrid-room-pixel-collision.md), the engine carried four collision modes (`Tile`, `Pixel`, `Auto`, `Hybrid`) selectable at runtime. Each handled a partial slice of the actual collision domain. They divided the engine's mental model along an implementation axis (which lookup structure runs?) rather than along the domain axis (what kind of thing is blocking the ray?).

The domain has exactly two kinds of blockers (see [CONTEXT.md](../../CONTEXT.md) for full definitions):

- **Walls** — edges between adjacent **Tiles** of different types. Authored by setting the tile map; derived statically into both cell edge-flags (`n/e/s/w_blocked` in `block_map`) and `UnionFind` **Room** partitions.
- **Objects** — coherent groups of blocked **Cells** in the runtime-mutable cell bitmap. Authored by setting individual cells.

Both can occlude a ray, but they live in different storage, are mutated through different APIs, and fail in different ways. A unified detector should consult both on every ray.

## Decision Drivers

- One mental model, anchored on domain primitives (Walls, Objects, Rooms) rather than on the implementation choice of broad-phase vs narrow-phase
- One configuration path (set the tile map; set object cells) rather than mode switching
- Eliminate dead/duplicate implementations: `TileCollisionMap` was never the best choice for any scenario once the unified detector existed

## Decision

Use a single collision detector, `HybridCollisionMap`, on every ray check. Remove the `CollisionMode` enum, the `Pixel` / `Tile` / `Auto` mode variants, and all switching logic. Mode is no longer a concept the API exposes.

The unified detector runs two phases on every `is_blocked(x0, y0, x1, y1)` call:

1. **Broad phase — Room boundary check.** `UnionFind::cast_ray` returns `true` iff the segment stays within a single Room. If it crosses a Room boundary, a **Wall** lies between, and the ray is blocked.
2. **Narrow phase — Cell bitmap walk.** If the broad phase didn't reject, Bresenham-walk the cell bitmap and return blocked on the first set cell encountered — the ray hit an **Object**.

When no tile map has been configured, all cells belong to one Room and broad phase always passes; the detector behaves as a pure Object bitmap. When a tile map is configured, the broad phase short-circuits most ray checks before they enter the narrow phase.

### Positive Consequences

- **One concept per blocker kind.** Wall semantics live in the tile map; Object semantics live in the cell bitmap; both are consulted by one detector. Code, tests, docs all use the same words.
- **No mode selection.** API callers configure inputs (tile map, object cells) and the engine does the right thing.
- **Performance gain at no cost.** The broad-phase Room check rejects most occluded rays in O(α(n)) UnionFind operations before the narrow-phase walk runs.
- **Smaller surface.** Removes one enum, three trait implementors, and a switching layer.

### Negative Consequences

- **Breaking change** for any caller that named a specific mode (internal only; ADR-0005 had just landed). External JS callers were not affected.
- **Always-allocated UnionFind**, even in scenes with no Walls. Memory cost is negligible at current map sizes (180×180 cells → 180×180 `Tile` UnionFind nodes).

## Implementation

1. Remove `CollisionMode` enum and `set_collision_mode` API.
2. `HybridCollisionMap` is the only `CollisionDetector` implementor; the trait remains for testability seams but has only one concrete impl in the live system.
3. WASM-exposed API gets two configuration paths: `update_map_data(tile_map, size)` (Walls + Rooms) and `set_pixel(cx, cy, blocked)` (Objects). No mode parameter.
4. Update documentation and rustdoc to use **Wall** / **Object** / **Room** vocabulary rather than mode names.

## Links

- Supersedes [ADR-0005](0005-hybrid-room-pixel-collision.md) by making the hybrid detector the only collision path.
- Related to [ADR-0004](0004-rust-native-collision-detection.md) performance goals.
- Followed by [ADR-0007](0007-extract-lighting-engine-type.md), which moves the unified collision system from a process-wide static into per-instance ownership on `LightingEngine`.
- Vocabulary aligned with [CONTEXT.md](../../CONTEXT.md).
