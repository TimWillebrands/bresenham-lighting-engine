# Unify Collision Detection Around Hybrid Pixel + Room System

- Status: accepted
- Date: 2025-01-08

## Context and Problem Statement

After implementing the hybrid collision system (ADR-0005), we now have four collision modes: Tile, Pixel, Auto, and Hybrid. This creates unnecessary complexity and confusion:

- **Tile mode**: Uses the block_map system but provides no clear advantage over hybrid mode
- **Pixel mode**: Works well for freeform drawing but lacks room-based optimizations
- **Auto mode**: Adds complexity without clear benefits
- **Hybrid mode**: Provides the best of both worlds - room-based broad-phase + pixel-based narrow-phase

The hybrid system is flexible enough to handle all use cases:
- When no rooms are configured, it behaves like pure pixel collision
- When rooms are configured, it provides performance benefits through broad-phase collision

## Decision

Remove all collision modes except the hybrid system and make it the default and only collision detection method.

### Positive Consequences

- **Simplified API**: No more collision mode selection confusion
- **Unified Codebase**: Single collision detection path reduces maintenance burden
- **Better Performance**: All users benefit from the optimized hybrid approach
- **Clearer Intent**: Room-based collision is explicit via map configuration, not mode selection

### Negative Consequences

- **Breaking Change**: Existing code using specific modes will need updates
- **Slightly Higher Memory Usage**: UnionFind structures are always allocated (minimal impact)

## Implementation

1. Remove `CollisionMode` enum and related switching logic
2. Always use `HybridCollisionMap` as the collision detector
3. Remove collision mode configuration from WASM API
4. Update documentation to reflect unified approach
5. Simplify initialization code

## Links

- Supersedes [ADR-0005](0005-hybrid-room-pixel-collision.md) by making hybrid the only option
- Related to [ADR-0004](0004-rust-native-collision-detection.md) performance goals