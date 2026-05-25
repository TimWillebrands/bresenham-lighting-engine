//! World-dimension constants used to live here as `pub const`s; per
//! [ADR-0008](../../docs/decisions/0008-per-engine-all-rays-and-runtime-resolution.md),
//! resolution (`cells_per_tile`, `tiles_per_row`) is now a per-instance
//! [`crate::engine::LightingEngine`] constructor parameter. The default values
//! used by [`crate::engine::LightingEngine::default`] and
//! [`crate::engine::DEFAULT_ENGINE`] live in [`crate::engine`].
//!
//! This module is intentionally empty — kept as a stable module path so any
//! downstream `use crate::constants::*` import resolves without error.
