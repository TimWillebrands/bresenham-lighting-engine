# Bresenham Lighting Engine — Context

A 2D lighting engine that raycasts light from point sources across a hierarchical world (coarse **tile** grid containing fine **cell** subdivisions), using Bresenham line walks for ray traversal and a unified pixel + room collision system to occlude rays.

## Language

### World structure

**Tile**:
A coarse-grid cell — the unit of the world's main layout (`TILES_PER_ROW × TILES_PER_ROW`, currently 30×30). A tile has a type; same-type adjacent tiles form a contiguous **Room**.
_Avoid_: "main-grid cell" (informal), "block" (overloaded).

**Cell**:
A fine-grid cell — the unit lighting operates on (`CELLS_PER_ROW × CELLS_PER_ROW`, currently 180×180; `CELLS_PER_TILE = 6` per tile edge). Light rays are traced cell-by-cell.
_Avoid_: "pixel" (in this codebase, "pixel" historically refers to a cell, which is misleading — see Flagged ambiguities), "subgrid cell" (informal alias OK in prose).

### Collision primitives

**Wall**:
A blocked edge between two adjacent tiles of different types. Materialises in two equivalent views: (a) as `n/e/s/w_blocked` flags on the cells that sit on the tile boundary, and (b) as a partition in the `UnionFind` room graph. Walls are derived from the tile map — they are not authored directly per edge.
_Avoid_: "obstacle" (ambiguous with **Object**), "edge collision".

**Object**:
A coherent group of blocked **Cells** in the runtime-mutable collision bitmap (`PixelCollisionMap`) that represents one in-world thing — a chair, a barrel, a character. The atomic write primitive (`set_pixel(cx, cy, true)`) marks a single cell as blocked; an Object is the higher-level concept built from many such writes.
_Avoid_: "obstacle" (ambiguous with **Wall**), "pixel obstacle" (confusing — see "Cell"), conflating "Object" with the atomic single-cell write.

**Room**:
A maximal set of tiles connected by walkable adjacency (same tile type, no wall between them). Computed by `UnionFind` from the tile map. The broad-phase collision check rejects a ray when its endpoints lie in different rooms.
_Avoid_: "region", "area".

### Lighting

**LightingEngine**:
An owned instance of the engine's mutable runtime state — tile map, room/wall data, object bitmap, and the registry of active Lights. Multiple instances can coexist (e.g. one per test scenario). Process-wide caches like the precomputed ray lookup table live outside any single LightingEngine.
_Avoid_: "World", "Scene", "Stage", "LightingWorld".

**Light**:
A point source with a position (in cell coords), an integer radius, and a unique id. Produces a square canvas of size `(2·radius + 1)²` of RGBA pixels.

**Canvas**:
The output buffer for a single light — RGBA values per cell within the light's bounding square. Composited externally for multi-light scenes.

**Ray**:
A precomputed Bresenham path from a light's centre to one of `ANGLES` directions at one of `MAX_DIST` distances. Stored in the `ALL_RAYS` lookup table.

## Relationships

- The world has exactly **one** Tile layout, which deterministically defines all **Walls** and all **Rooms**.
- A **Cell** belongs to exactly one **Tile** (and via that tile, exactly one **Room**).
- An **Object** occupies one **Cell** and is independent of Walls and Rooms.
- A ray from a **Light** is occluded if (a) its endpoints lie in different **Rooms** (broad-phase, UnionFind), OR (b) any **Cell** on its Bresenham path contains an **Object** (narrow-phase, `PixelCollisionMap`).
- Walls and Objects are authored through **different** APIs and should be tested by **different** scenarios.

## Example dialogue

> **Dev:** "If I want to test that a light is blocked by an inner wall of a room, do I add an Object or a Wall?"
> **Domain expert:** "A Wall — define the tile layout so the two tiles you care about have different types. The wall appears automatically as an edge between them. Objects are for things that aren't part of the architecture, like a chair sitting in the middle of a room."

> **Dev:** "So `PixelCollisionMap` stores walls?"
> **Domain expert:** "No — it stores Objects only. Walls live in the tile map and are read through the UnionFind room graph (broad-phase) and the cell edge-flags (narrow-phase rendering hints). The 'pixel' in the type name is historical and refers to cells, not screen pixels."

## Flagged ambiguities

- **"pixel"** in code (`PixelCollisionMap`, `set_pixel`, `get_pixel`) refers to a **Cell**, not a screen pixel. The public API name is preserved for back-compat (WASM/JS callers); treat the word as a synonym for **Cell** when reading the collision module.
- **"obstacle"** has been used in tests and docs (notably `VISUAL_TESTING.md`) to mean either a **Wall** or an **Object**. These are now distinct primitives with different storage, different authoring APIs, and different failure modes — do not use "obstacle" as a canonical term.
- **"grid" / "subgrid"** (informal) map to **Tile** / **Cell** (canonical). Use the canonical terms in code, comments, and ADRs.
- **"hybrid collision"** (from ADR-0006) names the combination but not its components. Prefer "**Room** + **Object** collision" when describing what the system does.
