//! Game rules (ex-`dungeon-core`), pure and host-agnostic: no wasm, no Yjs, no clock, no ambient randomness.
//! Elm shape: [`game::update`] decides an [`game::Intent`] into [`game::Write`]s, [`game::apply`] lands them.

pub mod apply;
pub mod board;
pub mod character;
pub mod edges;
pub mod game;
pub mod ledger;
pub mod light;
pub mod model;
pub mod resolve;
pub mod rng;
pub mod stats;
pub mod tiles;
pub mod types;
pub mod wasm;

#[cfg(test)]
mod tests;
