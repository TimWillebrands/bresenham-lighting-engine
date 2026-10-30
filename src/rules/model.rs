//! The game as data: what a host reads out of its doc and hands to `update`/`apply`.

use super::ledger::GameAction;
use super::types::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Defs {
    #[serde(default)]
    pub tokens: BTreeMap<String, TokenDef>,
    #[serde(default)]
    pub items: BTreeMap<String, ItemDef>,
}

/// One map layer: world tiles row-major (`LAYER_SIZE²`, 0 = rock) and its tokens in doc order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub yid: String,
    #[serde(default)]
    pub tiles: Vec<i32>,
    #[serde(default)]
    pub tokens: Vec<Token>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerRole {
    #[default]
    Player,
    GameMaster,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub role: PlayerRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_character_yid: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    #[serde(default)]
    pub game: Game,
    #[serde(default)]
    pub turn: i64,
    #[serde(default)]
    pub defs: Defs,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub items: BTreeMap<String, Item>,
    #[serde(default)]
    pub inventories: BTreeMap<String, Inventory>,
    #[serde(default)]
    pub players: BTreeMap<String, Player>,
    /// This turn's ledger; hosts may omit it (only `apply` appends).
    #[serde(default)]
    pub ledger: Vec<GameAction>,
}

impl Model {
    /// A token and the layer holding it.
    pub fn token(&self, yid: &str) -> Option<(&Layer, &Token)> {
        self.layers
            .iter()
            .find_map(|l| l.tokens.iter().find(|t| t.yid == yid).map(|t| (l, t)))
    }

    pub fn token_mut(&mut self, yid: &str) -> Option<&mut Token> {
        self.layers
            .iter_mut()
            .find_map(|l| l.tokens.iter_mut().find(|t| t.yid == yid))
    }

    pub fn layer(&self, yid: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.yid == yid)
    }

    pub fn tokens(&self) -> impl Iterator<Item = (&Layer, &Token)> {
        self.layers
            .iter()
            .flat_map(|l| l.tokens.iter().map(move |t| (l, t)))
    }
}
