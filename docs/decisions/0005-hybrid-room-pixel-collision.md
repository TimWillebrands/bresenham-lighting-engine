# Hybrid Room-Based and Pixel-Based Collision Detection

- Status: accepted
- Date: 2025-08-03

Technical Story: We need to support lighting and collision in environments with distinct rooms. The existing pixel-based collision system is inefficient for this, and a more structured approach is required to enhance performance and enable new features.

## Context and Problem Statement

As outlined in [ADR-0004](0004-rust-native-collision-detection.md), we have a performant rust-native collision system. However, it is based on either a simple tile grid or a pixel bitmap. The pixel-based approach, while precise, is inefficient for large, room-based maps, as it requires checking every point along a ray. It also introduces an issue of "thickness" where walls between adjacent tiles are multiple pixels wide, making it difficult to model seamless room boundaries.

We have a `UnionFind` implementation in TypeScript (`MapGrid.ts`) that is perfectly suited for identifying contiguous areas (rooms) from a tilemap and calculating their exact edge loops. This logic is critical for performance and functionality but currently resides outside our core Rust engine.

How can we integrate this room-identification logic into the core engine to create a high-performance, hybrid collision system that supports both room boundaries and fine-grained pixel obstacles?

## Decision Drivers

- **Performance**: Must be significantly faster than a pure pixel-based approach for room-based maps.
- **Accuracy**: Must model room boundaries precisely without pixel "thickness".
- **Flexibility**: Must allow for a hybrid model where room-based collision (broad-phase) is combined with pixel-based collision (narrow-phase) for dynamic objects within rooms.
- **Code Cohesion**: Critical collision and map logic should be consolidated into the Rust core, not split between Rust and TypeScript.

## Considered Options

1.  **Pure Pixel-Based Collision**: Continue with the existing system. This is simple but fails to meet the performance and accuracy requirements for room-based environments.
2.  **Port `UnionFind` for Room Detection Only**: Port the `UnionFind` logic to Rust to define room boundaries, but keep it separate from the primary collision system. This would lead to a complex and disjointed architecture.
3.  **Hybrid `UnionFind` + Pixel Collision System**: Port the `UnionFind` logic to Rust and integrate it as a "broad-phase" collision layer. A ray is first checked against the room boundaries. If it does not cross a boundary, the existing pixel-based system is then used for "narrow-phase" checks against objects within that room.

## Decision Outcome

Chosen option: **"Hybrid `UnionFind` + Pixel Collision System"**, because it provides the best of both worlds.

This approach leverages the `UnionFind` data structure to efficiently determine room membership and boundaries from a simple tilemap. This acts as a highly optimized broad-phase check. For a ray cast, the engine first determines if the ray crosses a hard wall between rooms. If it doesn't, it can then perform the more expensive pixel-based checks for dynamic or detailed objects inside the room. This hierarchical strategy dramatically reduces the number of pixels that need to be checked, leading to a major performance improvement.

### Positive Consequences

- **Massive Performance Gain**: Avoids brute-force pixel checks for entire scenes, focusing computation where it is needed.
- **Architectural Soundness**: Creates a clean, hierarchical collision system (broad-phase and narrow-phase).
- **Enables New Features**: Allows for logic like "light up the entire room" or room-specific effects.
- **Consolidates Core Logic**: Moves the `UnionFind` implementation into the Rust engine, improving maintainability.

### Negative Consequences

- **Increased Complexity**: The collision system now has two layers to manage and maintain.
- **Data Dependency**: Requires a tilemap representation for the room layout in addition to the pixel map for obstacles.

## Links

- Supersedes parts of [ADR-0004](0004-rust-native-collision-detection.md) by introducing a more advanced, hybrid collision strategy.
